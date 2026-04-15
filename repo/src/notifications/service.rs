use chrono::{Timelike, Utc};
use sha2::{Digest, Sha256};
use tracing::info;
use uuid::Uuid;

use crate::common::{db::DbPool, errors::AppError};
use crate::config::{AppConfig, DndConfig};
use crate::notifications::{
    model::{
        DeliveryState, NewNotification, NewNotificationAttempt,
        NewNotificationTemplate, NotificationChannel, NotificationTemplate, TemplateTrigger,
    },
    repository,
};

/// Resolve the delivery channel for a user from their stored preferences.
/// Falls back to `InApp` when the preference row is missing, the DB lookup fails,
/// the preferred_channel value does not map to a known channel, OR the channel
/// is not enabled in the current deployment profile (`cfg.notifications`).
/// Business flows should call this to honour configurable channel preferences
/// instead of hardcoding a channel.
pub async fn resolve_user_channel(
    pool: &DbPool,
    cfg: &AppConfig,
    user_id: Uuid,
) -> NotificationChannel {
    let pool_c = pool.clone();
    let preferred: String = actix_web::web::block(move || -> Result<String, AppError> {
        let mut conn = pool_c.get()?;
        Ok(crate::members::repository::get_preferences(&mut conn, user_id)
            .map(|p| p.preferred_channel)
            .unwrap_or_else(|_| "in_app".into()))
    })
    .await
    .ok()
    .and_then(|r| r.ok())
    .unwrap_or_else(|| "in_app".into());

    let candidate = match preferred.as_str() {
        "email" => NotificationChannel::Email,
        "sms" => NotificationChannel::Sms,
        "push" => NotificationChannel::Push,
        _ => NotificationChannel::InApp,
    };

    // Belt-and-braces: even if a stored preference points at a disabled channel
    // (e.g. an operator disabled email after preferences were saved), do not
    // attempt that channel — fall back to InApp which is always available.
    if cfg.notifications.channel_is_enabled(candidate.as_db_str()) {
        candidate
    } else {
        NotificationChannel::InApp
    }
}

/// Render a template body by substituting {{variable}} placeholders.
/// Validates against variable_schema if present.
pub fn render_template(
    template: &str,
    variables: &std::collections::HashMap<String, serde_json::Value>,
    schema: &Option<serde_json::Value>,
) -> Result<String, AppError> {
    // Validate required variables against schema
    if let Some(schema_obj) = schema {
        if let Some(schema_map) = schema_obj.as_object() {
            for (key, type_val) in schema_map {
                let expected_type = type_val.as_str().unwrap_or("string");
                match variables.get(key) {
                    None => {
                        return Err(AppError::UnprocessableEntity(format!(
                            "Missing required template variable: {}",
                            key
                        )));
                    }
                    Some(val) => {
                        // Type validation
                        let type_ok = match expected_type {
                            "string" => val.is_string(),
                            "integer" => val.is_i64() || val.is_u64(),
                            "uuid" => val.as_str().map(|s| Uuid::parse_str(s).is_ok()).unwrap_or(false),
                            "timestamp" => val.as_str()
                                .map(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok())
                                .unwrap_or(false),
                            _ => {
                                return Err(AppError::UnprocessableEntity(format!(
                                    "Unknown variable type '{}' for key '{}'; allowed: string, integer, uuid, timestamp",
                                    expected_type, key
                                )));
                            }
                        };
                        if !type_ok {
                            return Err(AppError::UnprocessableEntity(format!(
                                "Variable '{}' has wrong type, expected {}",
                                key, expected_type
                            )));
                        }
                    }
                }
            }
        }
    }

    // Substitute placeholders
    let mut result = template.to_string();
    for (key, val) in variables {
        let placeholder = format!("{{{{{}}}}}", key);
        let replacement = match val {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        result = result.replace(&placeholder, &replacement);
    }

    Ok(result)
}

/// Compute SHA-256 hash of the notification body for deduplication.
pub fn compute_payload_hash(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    hex::encode(hasher.finalize())
}

/// Check if the current hour is within the DND window for the given UTC offset.
/// `utc_offset_minutes` converts UTC now to the user's local time.
/// DND is active if hour >= start_hour or hour < end_hour (handles midnight wrap).
pub fn is_dnd_active(cfg: &DndConfig, utc_offset_minutes: i32) -> bool {
    let offset_secs = utc_offset_minutes * 60;
    let offset = chrono::FixedOffset::east_opt(offset_secs)
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
    let hour = Utc::now().with_timezone(&offset).hour();
    if cfg.start_hour < cfg.end_hour {
        hour >= cfg.start_hour && hour < cfg.end_hour
    } else {
        hour >= cfg.start_hour || hour < cfg.end_hour
    }
}

/// Compute the next delivery time after the DND window in the user's local timezone.
pub fn next_dnd_deliver_at(cfg: &DndConfig, utc_offset_minutes: i32) -> chrono::DateTime<Utc> {
    let offset_secs = utc_offset_minutes * 60;
    let offset = chrono::FixedOffset::east_opt(offset_secs)
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
    let local_now = Utc::now().with_timezone(&offset);
    let today = local_now.date_naive();

    // Schedule at end_hour:01 in the user's local timezone, then convert to UTC
    let end_h = cfg.end_hour as u32;
    let deliver_time = chrono::NaiveTime::from_hms_opt(end_h, 1, 0).unwrap_or_default();
    let deliver_naive = today.and_time(deliver_time);
    let deliver_utc = deliver_naive
        .and_local_timezone(offset)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1));

    if deliver_utc <= Utc::now() {
        deliver_utc + chrono::Duration::days(1)
    } else {
        deliver_utc
    }
}

/// Dispatch a notification to its delivery channel.
/// InApp: persisted in DB and polled by the client — returns Ok(()) immediately.
/// Email/Sms/Push: returns Err until real provider credentials are configured.
///   When dispatch fails, the caller creates an in_app fallback notification
///   so the user always receives the message via their inbox.
fn dispatch_to_channel(channel: &NotificationChannel, user_id: Uuid, subject: Option<&str>, body: &str) -> Result<(), String> {
    match channel {
        NotificationChannel::InApp => Ok(()),
        NotificationChannel::Email => {
            info!(user_id = %user_id, subject = ?subject, body_len = body.len(), "email dispatch attempted — no provider configured");
            Err("email channel not configured: wire SMTP provider credentials to enable".into())
        }
        NotificationChannel::Sms => {
            info!(user_id = %user_id, body_len = body.len(), "sms dispatch attempted — no provider configured");
            Err("sms channel not configured: wire SMS gateway credentials to enable".into())
        }
        NotificationChannel::Push => {
            info!(user_id = %user_id, subject = ?subject, body_len = body.len(), "push dispatch attempted — no provider configured");
            Err("push channel not configured: wire push provider credentials to enable".into())
        }
    }
}

/// Core notification dispatch. Handles opt-out, DND, template rendering, and persisting.
///
/// Per-category opt-out semantics:
///   * `is_critical=true` templates always deliver (opt-out cannot suppress them).
///   * Otherwise, if the user has opted out of this trigger's category, the
///     dispatch is recorded as `delivery_state=opted_out` and no body is
///     persisted/sent — REGARDLESS of channel. The previous behavior bypassed
///     opt-out for `in_app`, which made the prompt's "users may opt out per
///     category" requirement ineffective in the default deployment profile
///     (where `in_app` is the only enabled channel).
///
/// DND semantics are independent of opt-out and are applied below.
pub fn send_notification<'a>(
    pool: &'a DbPool,
    cfg: &'a AppConfig,
    user_id: Uuid,
    trigger: TemplateTrigger,
    channel: NotificationChannel,
    variables: std::collections::HashMap<String, serde_json::Value>,
    reference_id: Option<Uuid>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::notifications::model::Notification, AppError>> + Send + 'a>> {
    let dnd_cfg = cfg.dnd.clone();
    Box::pin(async move {
    let pool_c = pool.clone();
    let trigger_c = trigger.clone();
    let channel_c = channel.clone();

    // Load template and check member opt-out preference in one DB round-trip
    let (template_opt, opted_out, tz_offset_minutes): (Option<NotificationTemplate>, bool, i32) =
        actix_web::web::block(move || -> Result<_, AppError> {
            let mut conn = pool_c.get()?;
            let tmpl = repository::find_template_for_trigger(&mut conn, &trigger_c, &channel_c)?;

            // Load preferences once for both opt-out check and per-user DND timezone
            let prefs_result = crate::members::repository::get_preferences(&mut conn, user_id);
            let tz_offset = prefs_result.as_ref().map(|p| p.timezone_offset_minutes).unwrap_or(0);

            // Opt-out applies to ALL channels (including in_app) for non-critical
            // templates. Critical templates always deliver.
            let is_critical = tmpl.as_ref().map(|t| t.is_critical).unwrap_or(false);
            let opted_out = if is_critical {
                false
            } else {
                // Opt-out keys are trigger category strings (e.g. "booking_reminder_24h"),
                // not channel strings. Serialize the trigger to get its db_rename value.
                let trigger_key = serde_json::to_value(&trigger_c)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                match prefs_result {
                    Ok(prefs) => prefs
                        .notification_opt_out
                        .as_array()
                        .map(|arr| arr.iter().any(|item| item.as_str() == Some(trigger_key.as_str())))
                        .unwrap_or(false),
                    Err(_) => false, // no preferences record → not opted out
                }
            };

            Ok((tmpl, opted_out, tz_offset))
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;

    let template = template_opt.ok_or_else(|| {
        AppError::NotFound(format!(
            "No template for trigger {:?} and channel {:?}",
            trigger, channel
        ))
    })?;

    // If opted out, record the suppression and return early
    if opted_out {
        let now = Utc::now();
        let notification_id = Uuid::new_v4();
        let template_id = template.id;
        let trigger_clone = trigger.clone();
        let channel_clone = channel.clone();
        let pool_c2 = pool.clone();

        let notification = actix_web::web::block(move || -> Result<_, AppError> {
            let mut conn = pool_c2.get()?;
            repository::create_notification(
                &mut conn,
                NewNotification {
                    id: notification_id,
                    user_id,
                    template_id: Some(template_id),
                    trigger_type: trigger_clone,
                    channel: channel_clone,
                    subject: None,
                    body: String::new(),
                    payload_hash: String::new(),
                    delivery_state: DeliveryState::OptedOut,
                    dnd_suppressed: false,
                    reference_id,
                    created_at: now,
                    updated_at: now,
                },
            )
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;

        return Ok(notification);
    }

    // Render body
    let rendered_body = render_template(&template.body_template, &variables, &template.variable_schema)?;
    let rendered_subject = template
        .subject_template
        .as_deref()
        .map(|s| render_template(s, &variables, &None))
        .transpose()?;

    let payload_hash = compute_payload_hash(&rendered_body);

    // DND suppression: non-critical notifications are queued for delivery after
    // the user's quiet-hours window ends. Critical templates always deliver
    // immediately, regardless of DND. DND applies uniformly across channels —
    // including `in_app` — so recipients see non-critical notifications only
    // during their active hours.
    let is_critical = template.is_critical;
    let dnd_active = !is_critical && is_dnd_active(&dnd_cfg, tz_offset_minutes);
    let scheduled_deliver_at = if dnd_active {
        Some(next_dnd_deliver_at(&dnd_cfg, tz_offset_minutes))
    } else {
        None
    };

    let now = Utc::now();
    let notification_id = Uuid::new_v4();
    let template_id = template.id;
    let trigger_clone = trigger.clone();
    let channel_clone = channel.clone();
    let channel_for_dispatch = channel.clone();
    let body_clone = rendered_body.clone();
    let body_for_dispatch = rendered_body.clone();
    let hash_clone = payload_hash.clone();
    let subject_clone = rendered_subject.clone();
    let subject_for_dispatch = rendered_subject.clone();
    let pool_c2 = pool.clone();

    let notification = actix_web::web::block(move || -> Result<_, AppError> {
        use crate::notifications::model::NewDndQueueEntry;
        let mut conn = pool_c2.get()?;
        // Non-DND notifications start as Pending; state is updated after dispatch attempt.
        // DND-suppressed notifications are created directly in SuppressedDnd state.
        let initial_state = if dnd_active { DeliveryState::SuppressedDnd } else { DeliveryState::Pending };
        let mut notif = repository::create_notification(
            &mut conn,
            NewNotification {
                id: notification_id,
                user_id,
                template_id: Some(template_id),
                trigger_type: trigger_clone,
                channel: channel_clone,
                subject: subject_clone,
                body: body_clone,
                payload_hash: hash_clone,
                delivery_state: initial_state,
                dnd_suppressed: dnd_active,
                reference_id,
                created_at: now,
                updated_at: now,
            },
        )?;

        if dnd_active {
            repository::create_dnd_entry(
                &mut conn,
                NewDndQueueEntry {
                    id: Uuid::new_v4(),
                    notification_id: notif.id,
                    user_id,
                    scheduled_deliver_at: scheduled_deliver_at.unwrap_or(now),
                    created_at: now,
                },
            )?;
        } else {
            // Attempt dispatch; result drives the attempt record and final delivery state.
            let dispatch_result = dispatch_to_channel(
                &channel_for_dispatch,
                user_id,
                subject_for_dispatch.as_deref(),
                &body_for_dispatch,
            );
            let (final_state, succeeded, error_detail) = match dispatch_result {
                Ok(()) => (DeliveryState::Delivered, true, None),
                Err(ref e) => (DeliveryState::Failed, false, Some(e.clone())),
            };
            repository::create_attempt(
                &mut conn,
                NewNotificationAttempt {
                    id: Uuid::new_v4(),
                    notification_id: notif.id,
                    attempted_at: now,
                    succeeded,
                    error_detail,
                },
            )?;
            repository::update_notification_state(&mut conn, notif.id, &final_state, false)?;
            notif.delivery_state = final_state.clone();

            // Graceful fallback: when a non-in_app channel dispatch fails, create an
            // additional in_app notification so the user still receives the message.
            if !succeeded && channel_for_dispatch != NotificationChannel::InApp {
                let fallback_id = Uuid::new_v4();
                repository::create_notification(
                    &mut conn,
                    NewNotification {
                        id: fallback_id,
                        user_id,
                        template_id: Some(template_id),
                        trigger_type: notif.trigger_type.clone(),
                        channel: NotificationChannel::InApp,
                        subject: notif.subject.clone(),
                        body: notif.body.clone(),
                        payload_hash: notif.payload_hash.clone(),
                        delivery_state: DeliveryState::Delivered,
                        dnd_suppressed: false,
                        reference_id,
                        created_at: now,
                        updated_at: now,
                    },
                )?;
                repository::create_attempt(
                    &mut conn,
                    NewNotificationAttempt {
                        id: Uuid::new_v4(),
                        notification_id: fallback_id,
                        attempted_at: now,
                        succeeded: true,
                        error_detail: None,
                    },
                )?;
                info!(
                    original_id = %notif.id,
                    fallback_id = %fallback_id,
                    "non-in_app dispatch failed; created in_app fallback notification"
                );
            }
        }

        Ok(notif)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(notification)
    }) // end Box::pin
}

/// Preview a template with given variables — renders without persisting.
pub fn preview_template(
    template: &NotificationTemplate,
    variables: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(Option<String>, String), AppError> {
    let body = render_template(&template.body_template, &variables, &template.variable_schema)?;
    let subject = template
        .subject_template
        .as_deref()
        .map(|s| render_template(s, &variables, &None))
        .transpose()?;
    Ok((subject, body))
}

/// Create a new notification template.
pub async fn create_template(
    pool: &DbPool,
    trigger: TemplateTrigger,
    channel: NotificationChannel,
    name: String,
    subject_template: Option<String>,
    body_template: String,
    variable_schema: Option<serde_json::Value>,
    is_critical: bool,
) -> Result<NotificationTemplate, AppError> {
    let now = Utc::now();
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::create_template(
            &mut conn,
            NewNotificationTemplate {
                id: Uuid::new_v4(),
                name,
                trigger_type: trigger,
                channel,
                subject_template,
                body_template,
                variable_schema,
                is_critical,
                created_at: now,
                updated_at: now,
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Process all DND queue entries whose scheduled time has passed.
/// For each entry: load the notification, attempt channel dispatch, record the
/// attempt result, update delivery state (Delivered or Failed), then mark processed.
pub async fn deliver_dnd_queue(pool: &DbPool) -> Result<usize, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<usize, AppError> {
        let mut conn = pool_c.get()?;
        let entries = repository::find_pending_dnd_entries(&mut conn, Utc::now())?;
        let count = entries.len();
        for entry in entries {
            // Load notification to obtain channel, user, body, and subject for dispatch.
            let notif = repository::find_notification(&mut conn, entry.notification_id)?;

            let dispatch_result = dispatch_to_channel(
                &notif.channel,
                notif.user_id,
                notif.subject.as_deref(),
                &notif.body,
            );
            let (final_state, succeeded, error_detail) = match dispatch_result {
                Ok(()) => (DeliveryState::Delivered, true, None),
                Err(e) => (DeliveryState::Failed, false, Some(e)),
            };

            repository::create_attempt(
                &mut conn,
                NewNotificationAttempt {
                    id: Uuid::new_v4(),
                    notification_id: notif.id,
                    attempted_at: Utc::now(),
                    succeeded,
                    error_detail,
                },
            )?;

            repository::update_notification_state(
                &mut conn,
                entry.notification_id,
                &final_state,
                true, // was DND suppressed
            )?;

            // Graceful fallback on DND redelivery failure for non-in_app channels
            if !succeeded && notif.channel != NotificationChannel::InApp {
                let fallback_id = Uuid::new_v4();
                repository::create_notification(
                    &mut conn,
                    NewNotification {
                        id: fallback_id,
                        user_id: notif.user_id,
                        template_id: notif.template_id,
                        trigger_type: notif.trigger_type.clone(),
                        channel: NotificationChannel::InApp,
                        subject: notif.subject.clone(),
                        body: notif.body.clone(),
                        payload_hash: notif.payload_hash.clone(),
                        delivery_state: DeliveryState::Delivered,
                        dnd_suppressed: false,
                        reference_id: notif.reference_id,
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    },
                )?;
                repository::create_attempt(
                    &mut conn,
                    NewNotificationAttempt {
                        id: Uuid::new_v4(),
                        notification_id: fallback_id,
                        attempted_at: Utc::now(),
                        succeeded: true,
                        error_detail: None,
                    },
                )?;
            }

            repository::mark_dnd_entry_processed(&mut conn, entry.id)?;
        }
        Ok(count)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
