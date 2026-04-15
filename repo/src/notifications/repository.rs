use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::common::{db::DbConn, errors::AppError};
use crate::notifications::model::{
    DeliveryState, DndQueueEntry, NewDndQueueEntry, NewNotification, NewNotificationAttempt,
    NewNotificationTemplate, Notification, NotificationAttempt, NotificationChannel,
    NotificationTemplate, TemplateTrigger,
};
use crate::schema::{dnd_queue, notification_attempts, notification_templates, notifications};

pub fn create_template(
    conn: &mut DbConn,
    tmpl: NewNotificationTemplate,
) -> Result<NotificationTemplate, AppError> {
    diesel::insert_into(notification_templates::table)
        .values(&tmpl)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_template_by_id(
    conn: &mut DbConn,
    template_id: Uuid,
) -> Result<NotificationTemplate, AppError> {
    notification_templates::table
        .filter(notification_templates::id.eq(template_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Template {} not found", template_id)))
}

pub fn find_template_for_trigger(
    conn: &mut DbConn,
    trigger: &TemplateTrigger,
    channel: &NotificationChannel,
) -> Result<Option<NotificationTemplate>, AppError> {
    notification_templates::table
        .filter(notification_templates::trigger_type.eq(trigger))
        .filter(notification_templates::channel.eq(channel))
        .first(conn)
        .optional()
        .map_err(AppError::from)
}

pub fn list_templates(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<NotificationTemplate>, i64), AppError> {
    let total: i64 = notification_templates::table
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;
    let records = notification_templates::table
        .order(notification_templates::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((records, total))
}

pub fn update_template(
    conn: &mut DbConn,
    template_id: Uuid,
    name: Option<String>,
    subject_template: Option<String>,
    body_template: Option<String>,
    variable_schema: Option<serde_json::Value>,
    is_critical: Option<bool>,
) -> Result<NotificationTemplate, AppError> {
    // Fetch existing, then update fields
    let existing = find_template_by_id(conn, template_id)?;

    diesel::update(notification_templates::table.filter(notification_templates::id.eq(template_id)))
        .set((
            notification_templates::name.eq(name.unwrap_or(existing.name)),
            notification_templates::subject_template.eq(subject_template.or(existing.subject_template)),
            notification_templates::body_template.eq(body_template.unwrap_or(existing.body_template)),
            notification_templates::variable_schema.eq(variable_schema.or(existing.variable_schema)),
            notification_templates::is_critical.eq(is_critical.unwrap_or(existing.is_critical)),
            notification_templates::updated_at.eq(Utc::now()),
        ))
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn create_notification(
    conn: &mut DbConn,
    notif: NewNotification,
) -> Result<Notification, AppError> {
    diesel::insert_into(notifications::table)
        .values(&notif)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_notification(
    conn: &mut DbConn,
    notification_id: Uuid,
) -> Result<Notification, AppError> {
    notifications::table
        .filter(notifications::id.eq(notification_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Notification {} not found", notification_id)))
}

pub fn list_notifications_for_user(
    conn: &mut DbConn,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Notification>, i64), AppError> {
    let total: i64 = notifications::table
        .filter(notifications::user_id.eq(user_id))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let records = notifications::table
        .filter(notifications::user_id.eq(user_id))
        .order(notifications::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;

    Ok((records, total))
}

pub fn list_all_notifications(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Notification>, i64), AppError> {
    let total: i64 = notifications::table
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let records = notifications::table
        .order(notifications::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;

    Ok((records, total))
}

pub fn mark_notification_read(
    conn: &mut DbConn,
    notification_id: Uuid,
    user_id: Uuid,
) -> Result<Notification, AppError> {
    let notif = find_notification(conn, notification_id)?;
    if notif.user_id != user_id {
        return Err(AppError::Forbidden("Cannot mark another user's notification as read".into()));
    }

    diesel::update(notifications::table.filter(notifications::id.eq(notification_id)))
        .set((
            notifications::read_at.eq(Some(Utc::now())),
            notifications::updated_at.eq(Utc::now()),
        ))
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn update_notification_state(
    conn: &mut DbConn,
    notification_id: Uuid,
    state: &DeliveryState,
    dnd_suppressed: bool,
) -> Result<(), AppError> {
    diesel::update(notifications::table.filter(notifications::id.eq(notification_id)))
        .set((
            notifications::delivery_state.eq(state),
            notifications::dnd_suppressed.eq(dnd_suppressed),
            notifications::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

pub fn create_attempt(
    conn: &mut DbConn,
    attempt: NewNotificationAttempt,
) -> Result<NotificationAttempt, AppError> {
    diesel::insert_into(notification_attempts::table)
        .values(&attempt)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn create_dnd_entry(
    conn: &mut DbConn,
    entry: NewDndQueueEntry,
) -> Result<DndQueueEntry, AppError> {
    diesel::insert_into(dnd_queue::table)
        .values(&entry)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_pending_dnd_entries(
    conn: &mut DbConn,
    before: DateTime<Utc>,
) -> Result<Vec<DndQueueEntry>, AppError> {
    dnd_queue::table
        .filter(dnd_queue::scheduled_deliver_at.le(before))
        .filter(dnd_queue::processed_at.is_null())
        .load(conn)
        .map_err(AppError::from)
}

pub fn mark_dnd_entry_processed(
    conn: &mut DbConn,
    entry_id: Uuid,
) -> Result<(), AppError> {
    diesel::update(dnd_queue::table.filter(dnd_queue::id.eq(entry_id)))
        .set(dnd_queue::processed_at.eq(Some(Utc::now())))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}
