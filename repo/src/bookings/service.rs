use chrono::Utc;
use tracing::error;
use uuid::Uuid;

use crate::bookings::{
    model::{Booking, BookingState, NewBooking, NewBookingItem},
    repository,
    state_machine::BookingStateMachine,
};
use crate::common::{db::DbPool, errors::AppError};
use crate::config::AppConfig;
use crate::notifications::model::TemplateTrigger;
use crate::notifications::service as notif_service;

/// Send a booking lifecycle notification using the member's preferred channel.
/// Logs errors instead of swallowing them silently.
///
/// The variable payload is derived from the booking row so templates with
/// strict `variable_schema` requirements (e.g. `start_at`, `end_at`, `state`,
/// `pickup_point_id`, `zone_id`) can render without failing. Templates that
/// only reference `booking_id` still work because unused variables are ignored
/// by the renderer.
async fn send_booking_notification(
    pool: &DbPool,
    cfg: &AppConfig,
    member_id: Uuid,
    trigger: TemplateTrigger,
    booking: &Booking,
) {
    let channel = notif_service::resolve_user_channel(pool, cfg, member_id).await;
    let vars = booking_template_vars(booking);
    let booking_id = booking.id;
    if let Err(e) = notif_service::send_notification(
        pool, cfg, member_id, trigger, channel, vars, Some(booking_id),
    ).await {
        error!(
            booking_id = %booking_id,
            member_id = %member_id,
            error = %e,
            "Failed to send booking lifecycle notification"
        );
    }
}

/// Build the standard booking-trigger template variable map.
/// Kept centralised so every lifecycle trigger (create, confirm, cancel, change,
/// complete, exception) agrees on variable names and formats.
fn booking_template_vars(b: &Booking) -> std::collections::HashMap<String, serde_json::Value> {
    let mut vars = std::collections::HashMap::new();
    vars.insert("booking_id".to_string(), serde_json::Value::String(b.id.to_string()));
    vars.insert("start_at".to_string(), serde_json::Value::String(b.start_at.to_rfc3339()));
    vars.insert("end_at".to_string(), serde_json::Value::String(b.end_at.to_rfc3339()));
    vars.insert(
        "state".to_string(),
        serde_json::to_value(&b.state).unwrap_or(serde_json::Value::Null),
    );
    vars.insert(
        "total_cents".to_string(),
        serde_json::Value::Number(serde_json::Number::from(b.total_cents)),
    );
    if let Some(pid) = b.pickup_point_id {
        vars.insert("pickup_point_id".to_string(), serde_json::Value::String(pid.to_string()));
    }
    if let Some(zid) = b.zone_id {
        vars.insert("zone_id".to_string(), serde_json::Value::String(zid.to_string()));
    }
    vars
}

pub struct BookingItemInput {
    pub inventory_item_id: Uuid,
    pub quantity: i32,
    pub unit_price_cents: i64,
}

pub async fn create_booking(
    pool: &DbPool,
    cfg: &AppConfig,
    member_id: Uuid,
    start_at: chrono::DateTime<Utc>,
    end_at: chrono::DateTime<Utc>,
    pickup_point_id: Option<Uuid>,
    zone_id: Option<Uuid>,
    items: Vec<BookingItemInput>,
) -> Result<Booking, AppError> {
    if items.is_empty() {
        return Err(AppError::UnprocessableEntity("Booking must have at least one item".into()));
    }

    let booking_id = Uuid::new_v4();
    let total_cents: i64 = items.iter().map(|i| i.quantity as i64 * i.unit_price_cents).sum();
    let hold_timeout = cfg.booking.hold_timeout_minutes;
    let strategy = cfg.booking.inventory_strategy.clone();
    let pool_c = pool.clone();
    let now = Utc::now();
    let items_data: Vec<(Uuid, i32, i64)> = items
        .iter()
        .map(|i| (i.inventory_item_id, i.quantity, i.unit_price_cents))
        .collect();

    // Single transaction: cutoff check + booking row + inventory deduction + items + state transition.
    // All mutations are atomic — no orphaned bookings or partial hold states on failure.
    let booking = actix_web::web::block(move || -> Result<Booking, AppError> {
        let mut conn = pool_c.get()?;
        use diesel::prelude::*;

        conn.transaction::<Booking, AppError, _>(|conn| {
            // 1. Cutoff enforcement: resolve effective cutoff with precedence
            //    zone.cutoff_hours > pickup_point.cutoff_hours > item.cutoff_hours
            for (item_id, _, _) in &items_data {
                let item = crate::inventory::repository::find_item(conn, *item_id)?;

                let zone_cutoff: Option<i32> = item.zone_id.and_then(|zid| {
                    crate::schema::delivery_zones::table
                        .filter(crate::schema::delivery_zones::id.eq(zid))
                        .select(crate::schema::delivery_zones::cutoff_hours)
                        .first::<Option<i32>>(conn)
                        .ok()
                        .flatten()
                });
                let pickup_cutoff: Option<i32> = item.pickup_point_id.and_then(|pid| {
                    crate::schema::pickup_points::table
                        .filter(crate::schema::pickup_points::id.eq(pid))
                        .select(crate::schema::pickup_points::cutoff_hours)
                        .first::<Option<i32>>(conn)
                        .ok()
                        .flatten()
                });

                let effective_cutoff = zone_cutoff
                    .or(pickup_cutoff)
                    .unwrap_or(item.cutoff_hours);

                let cutoff_deadline = now + chrono::Duration::hours(effective_cutoff as i64);
                if start_at < cutoff_deadline {
                    return Err(AppError::UnprocessableEntity(format!(
                        "Booking start_at is within the {}h fulfillment cutoff window for item {}",
                        effective_cutoff, item_id
                    )));
                }
            }

            // 2. Create booking row in Draft state
            repository::create_booking(conn, NewBooking {
                id: booking_id,
                member_id,
                state: BookingState::Draft,
                start_at,
                end_at,
                inventory_hold_expires_at: None,
                pickup_point_id,
                zone_id,
                total_cents,
                version: 0,
                created_at: now,
                updated_at: now,
            })?;

            if strategy == "immediate" {
                // Immediate strategy: deduct inventory directly without hold reservation.
                // Booking transitions Draft → Confirmed in a single step.
                for (item_id, qty, price) in &items_data {
                    // Lock and deduct directly within the same transaction
                    let item: crate::inventory::model::InventoryItem = crate::schema::inventory_items::table
                        .filter(crate::schema::inventory_items::id.eq(*item_id))
                        .for_update()
                        .first(conn)
                        .map_err(|_| AppError::NotFound(format!("Inventory item {} not found", item_id)))?;

                    if item.available_qty < *qty {
                        return Err(AppError::Conflict(format!(
                            "Insufficient inventory: available={}, requested={}",
                            item.available_qty, qty
                        )));
                    }

                    let new_qty = item.available_qty - *qty;
                    diesel::update(
                        crate::schema::inventory_items::table
                            .filter(crate::schema::inventory_items::id.eq(*item_id))
                            .filter(crate::schema::inventory_items::version.eq(item.version)),
                    )
                    .set((
                        crate::schema::inventory_items::available_qty.eq(new_qty),
                        crate::schema::inventory_items::version.eq(item.version + 1),
                        crate::schema::inventory_items::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .map_err(AppError::from)?;

                    // Ledger entry for the immediate deduction
                    diesel::insert_into(crate::schema::inventory_ledger::table)
                        .values((
                            crate::schema::inventory_ledger::id.eq(Uuid::new_v4()),
                            crate::schema::inventory_ledger::inventory_item_id.eq(*item_id),
                            crate::schema::inventory_ledger::delta.eq(-*qty),
                            crate::schema::inventory_ledger::qty_after.eq(new_qty),
                            crate::schema::inventory_ledger::reason.eq("booking_immediate"),
                            crate::schema::inventory_ledger::correlation_id.eq(Some(format!("booking:{}:{}", booking_id, item_id))),
                            crate::schema::inventory_ledger::actor_user_id.eq(Some(member_id)),
                            crate::schema::inventory_ledger::created_at.eq(now),
                        ))
                        .on_conflict(crate::schema::inventory_ledger::correlation_id)
                        .do_nothing()
                        .execute(conn)
                        .map_err(AppError::from)?;

                    // Centralized: auto-unpublish + restock alert
                    crate::inventory::service::check_qty_alerts(
                        conn, *item_id, new_qty, item.available_qty, item.safety_stock,
                    )?;

                    repository::create_booking_item(conn, NewBookingItem {
                        id: Uuid::new_v4(),
                        booking_id,
                        inventory_item_id: *item_id,
                        quantity: *qty,
                        unit_price_cents: *price,
                        created_at: now,
                    })?;
                }

                // Transition Draft → Confirmed directly (no hold phase)
                repository::transition_state(
                    conn,
                    booking_id,
                    BookingState::Draft,
                    BookingState::Confirmed,
                    None,
                    Some(member_id),
                    0,
                )
            } else {
                // Hold strategy (default): pre-decrement inventory via hold reservation.
                // Booking transitions Draft → Held; quantity is restored on cancel/expiry.
                let mut hold_expiry = None;
                for (item_id, qty, price) in &items_data {
                    let hold = crate::inventory::service::create_hold_in_tx(
                        conn,
                        *item_id,
                        Some(booking_id),
                        *qty,
                        hold_timeout,
                        Some(format!("booking:{}:{}", booking_id, item_id)),
                        Some(member_id),
                    )?;
                    hold_expiry = Some(hold.expires_at);
                    repository::create_booking_item(conn, NewBookingItem {
                        id: Uuid::new_v4(),
                        booking_id,
                        inventory_item_id: *item_id,
                        quantity: *qty,
                        unit_price_cents: *price,
                        created_at: now,
                    })?;
                }

                // 4. Set hold expiry and transition Draft → Held
                if let Some(expiry) = hold_expiry {
                    repository::update_hold_expiry(conn, booking_id, Some(expiry))?;
                }

                repository::transition_state(
                    conn,
                    booking_id,
                    BookingState::Draft,
                    BookingState::Held,
                    None,
                    Some(member_id),
                    0,
                )
            }
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    if booking.state == BookingState::Confirmed {
        send_booking_notification(pool, cfg, member_id, TemplateTrigger::BookingConfirmed, &booking).await;
    }

    Ok(booking)
}

pub async fn confirm_booking(
    pool: &DbPool,
    cfg: &AppConfig,
    booking_id: Uuid,
    actor_id: Uuid,
) -> Result<Booking, AppError> {
    let pool_c = pool.clone();
    let booking = actix_web::web::block(move || -> Result<Booking, AppError> {
        let mut conn = pool_c.get()?;
        let booking = repository::find_booking(&mut conn, booking_id)?;

        // Only member-owned booking or privileged
        BookingStateMachine::transition(&booking.state, &BookingState::Confirmed)?;

        repository::transition_state(
            &mut conn,
            booking_id,
            booking.state,
            BookingState::Confirmed,
            None,
            Some(actor_id),
            booking.version,
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    send_booking_notification(pool, cfg, booking.member_id, TemplateTrigger::BookingConfirmed, &booking).await;

    Ok(booking)
}

pub async fn cancel_booking(
    pool: &DbPool,
    cfg: &AppConfig,
    booking_id: Uuid,
    reason: Option<String>,
    actor_id: Uuid,
) -> Result<Booking, AppError> {
    // Strategy-aware cancellation:
    //   * "hold"      — inventory is reserved via inventory_holds; cancel releases
    //                   those holds (the release path restores qty + ledger row).
    //   * "immediate" — inventory was deducted directly at creation, so cancel MUST
    //                   restore the quantity itself from booking_items and emit a
    //                   compensating ledger entry. Without this, immediate-strategy
    //                   cancellations permanently shrink availability.
    //
    // All work (state transition + hold release / qty restore + ledger) runs in a
    // single DB transaction so a crash mid-cancel cannot leave the booking in a
    // Cancelled state with inventory still deducted.
    let strategy = cfg.booking.inventory_strategy.clone();
    let pool_clone = pool.clone();
    let booking = actix_web::web::block(move || -> Result<Booking, AppError> {
        use crate::schema::{booking_items, inventory_holds, inventory_items, inventory_ledger};
        use diesel::prelude::*;

        let mut conn = pool_clone.get()?;
        conn.transaction::<Booking, AppError, _>(|conn| {
            let booking = repository::find_booking(conn, booking_id)?;
            BookingStateMachine::transition(&booking.state, &BookingState::Cancelled)?;
            let updated = repository::transition_state(
                conn,
                booking_id,
                booking.state.clone(),
                BookingState::Cancelled,
                reason,
                Some(actor_id),
                booking.version,
            )?;

            if strategy == "immediate" {
                // Restore inventory deducted at booking creation.
                let items: Vec<(Uuid, i32)> = booking_items::table
                    .filter(booking_items::booking_id.eq(booking_id))
                    .select((booking_items::inventory_item_id, booking_items::quantity))
                    .load(conn)
                    .map_err(AppError::from)?;

                let now = Utc::now();
                for (item_id, qty) in items {
                    let item: crate::inventory::model::InventoryItem = inventory_items::table
                        .filter(inventory_items::id.eq(item_id))
                        .for_update()
                        .first(conn)
                        .map_err(AppError::from)?;
                    let restored = item.available_qty + qty;
                    diesel::update(
                        inventory_items::table
                            .filter(inventory_items::id.eq(item_id))
                            .filter(inventory_items::version.eq(item.version)),
                    )
                    .set((
                        inventory_items::available_qty.eq(restored),
                        inventory_items::version.eq(item.version + 1),
                        inventory_items::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .map_err(AppError::from)?;

                    // Compensating ledger entry — idempotent on correlation_id so a
                    // retry of cancel does not double-restore.
                    diesel::insert_into(inventory_ledger::table)
                        .values((
                            inventory_ledger::id.eq(Uuid::new_v4()),
                            inventory_ledger::inventory_item_id.eq(item_id),
                            inventory_ledger::delta.eq(qty),
                            inventory_ledger::qty_after.eq(restored),
                            inventory_ledger::reason.eq("booking_cancel_restore"),
                            inventory_ledger::correlation_id.eq(Some(format!(
                                "cancel:booking:{}:{}",
                                booking_id, item_id
                            ))),
                            inventory_ledger::actor_user_id.eq(Some(actor_id)),
                            inventory_ledger::created_at.eq(now),
                        ))
                        .on_conflict(inventory_ledger::correlation_id)
                        .do_nothing()
                        .execute(conn)
                        .map_err(AppError::from)?;

                    // Publication state / restock alert consistency.
                    crate::inventory::service::check_qty_alerts(
                        conn,
                        item_id,
                        restored,
                        item.available_qty,
                        item.safety_stock,
                    )?;
                }
            } else {
                // Hold strategy: release all active inventory holds for this booking.
                let hold_ids: Vec<Uuid> = inventory_holds::table
                    .filter(inventory_holds::booking_id.eq(booking_id))
                    .filter(inventory_holds::released_at.is_null())
                    .select(inventory_holds::id)
                    .load(conn)
                    .map_err(AppError::from)?;
                for hold_id in hold_ids {
                    crate::inventory::service::release_hold_in_tx(
                        conn,
                        hold_id,
                        Some(actor_id),
                    )?;
                }
            }

            Ok(updated)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    send_booking_notification(pool, cfg, booking.member_id, TemplateTrigger::BookingCancelled, &booking).await;

    Ok(booking)
}

pub async fn change_booking(
    pool: &DbPool,
    cfg: &AppConfig,
    booking_id: Uuid,
    actor_id: Uuid,
    items: Vec<BookingItemInput>,
    start_at: Option<chrono::DateTime<Utc>>,
    end_at: Option<chrono::DateTime<Utc>>,
    pickup_point_id: Option<Uuid>,
    zone_id: Option<Uuid>,
    reason: Option<String>,
) -> Result<Booking, AppError> {
    if items.is_empty() {
        return Err(AppError::UnprocessableEntity("Booking must have at least one item".into()));
    }

    let hold_timeout = cfg.booking.hold_timeout_minutes;
    let strategy = cfg.booking.inventory_strategy.clone();
    let pool_c = pool.clone();
    let now = Utc::now();
    let items_data: Vec<(Uuid, i32, i64)> = items
        .iter()
        .map(|i| (i.inventory_item_id, i.quantity, i.unit_price_cents))
        .collect();

    // Single transaction: lock booking, validate, adjust inventory, rewrite items, transition.
    let (booking, member_id) = actix_web::web::block(move || -> Result<(Booking, Uuid), AppError> {
        let mut conn = pool_c.get()?;
        use crate::schema::{booking_items, bookings, inventory_holds};
        use diesel::prelude::*;

        conn.transaction::<(Booking, Uuid), AppError, _>(|conn| {
            // 1. Lock current booking row to prevent concurrent changes
            let current: crate::bookings::model::Booking = bookings::table
                .filter(bookings::id.eq(booking_id))
                .for_update()
                .first(conn)
                .map_err(|_| AppError::NotFound(format!("Booking {} not found", booking_id)))?;

            let effective_start = start_at.unwrap_or(current.start_at);
            let effective_end = end_at.unwrap_or(current.end_at);

            // 1b. Time window validation
            if effective_start >= effective_end {
                return Err(AppError::UnprocessableEntity(
                    "Booking start_at must be before end_at".into(),
                ));
            }

            // 2. Cutoff check for new items (zone > pickup > item precedence)
            for (item_id, _, _) in &items_data {
                let item = crate::inventory::repository::find_item(conn, *item_id)?;

                let zone_cutoff: Option<i32> = item.zone_id.and_then(|zid| {
                    crate::schema::delivery_zones::table
                        .filter(crate::schema::delivery_zones::id.eq(zid))
                        .select(crate::schema::delivery_zones::cutoff_hours)
                        .first::<Option<i32>>(conn)
                        .ok()
                        .flatten()
                });
                let pickup_cutoff: Option<i32> = item.pickup_point_id.and_then(|pid| {
                    crate::schema::pickup_points::table
                        .filter(crate::schema::pickup_points::id.eq(pid))
                        .select(crate::schema::pickup_points::cutoff_hours)
                        .first::<Option<i32>>(conn)
                        .ok()
                        .flatten()
                });

                let effective_cutoff = zone_cutoff
                    .or(pickup_cutoff)
                    .unwrap_or(item.cutoff_hours);

                let cutoff_deadline = now + chrono::Duration::hours(effective_cutoff as i64);
                if effective_start < cutoff_deadline {
                    return Err(AppError::UnprocessableEntity(format!(
                        "Booking start_at is within the {}h fulfillment cutoff window for item {}",
                        effective_cutoff, item_id
                    )));
                }
            }

            if strategy == "immediate" {
                // Immediate strategy: restore old item qtys, deduct new item qtys directly.
                // No hold creation — compute net deltas atomically.
                let old_items: Vec<(Uuid, i32)> = booking_items::table
                    .filter(booking_items::booking_id.eq(booking_id))
                    .select((booking_items::inventory_item_id, booking_items::quantity))
                    .load(conn)
                    .map_err(AppError::from)?;

                // Restore old items
                for (item_id, old_qty) in &old_items {
                    let item: crate::inventory::model::InventoryItem = crate::schema::inventory_items::table
                        .filter(crate::schema::inventory_items::id.eq(*item_id))
                        .for_update()
                        .first(conn)
                        .map_err(AppError::from)?;
                    let restored_qty = item.available_qty + *old_qty;
                    diesel::update(
                        crate::schema::inventory_items::table
                            .filter(crate::schema::inventory_items::id.eq(*item_id))
                            .filter(crate::schema::inventory_items::version.eq(item.version)),
                    )
                    .set((
                        crate::schema::inventory_items::available_qty.eq(restored_qty),
                        crate::schema::inventory_items::version.eq(item.version + 1),
                        crate::schema::inventory_items::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .map_err(AppError::from)?;
                    diesel::insert_into(crate::schema::inventory_ledger::table)
                        .values((
                            crate::schema::inventory_ledger::id.eq(Uuid::new_v4()),
                            crate::schema::inventory_ledger::inventory_item_id.eq(*item_id),
                            crate::schema::inventory_ledger::delta.eq(*old_qty),
                            crate::schema::inventory_ledger::qty_after.eq(restored_qty),
                            crate::schema::inventory_ledger::reason.eq("change_restore"),
                            crate::schema::inventory_ledger::actor_user_id.eq(Some(actor_id)),
                            crate::schema::inventory_ledger::created_at.eq(now),
                        ))
                        .execute(conn)
                        .map_err(AppError::from)?;
                    crate::inventory::service::check_qty_alerts(conn, *item_id, restored_qty, item.available_qty, item.safety_stock)?;
                }

                // Deduct new items
                for (item_id, qty, _) in &items_data {
                    let item: crate::inventory::model::InventoryItem = crate::schema::inventory_items::table
                        .filter(crate::schema::inventory_items::id.eq(*item_id))
                        .for_update()
                        .first(conn)
                        .map_err(AppError::from)?;
                    if item.available_qty < *qty {
                        return Err(AppError::Conflict(format!(
                            "Insufficient inventory: available={}, requested={}", item.available_qty, qty
                        )));
                    }
                    let new_qty = item.available_qty - *qty;
                    diesel::update(
                        crate::schema::inventory_items::table
                            .filter(crate::schema::inventory_items::id.eq(*item_id))
                            .filter(crate::schema::inventory_items::version.eq(item.version)),
                    )
                    .set((
                        crate::schema::inventory_items::available_qty.eq(new_qty),
                        crate::schema::inventory_items::version.eq(item.version + 1),
                        crate::schema::inventory_items::updated_at.eq(now),
                    ))
                    .execute(conn)
                    .map_err(AppError::from)?;
                    diesel::insert_into(crate::schema::inventory_ledger::table)
                        .values((
                            crate::schema::inventory_ledger::id.eq(Uuid::new_v4()),
                            crate::schema::inventory_ledger::inventory_item_id.eq(*item_id),
                            crate::schema::inventory_ledger::delta.eq(-*qty),
                            crate::schema::inventory_ledger::qty_after.eq(new_qty),
                            crate::schema::inventory_ledger::reason.eq("change_deduct"),
                            crate::schema::inventory_ledger::correlation_id.eq(Some(format!("change:booking:{}:{}", booking_id, item_id))),
                            crate::schema::inventory_ledger::actor_user_id.eq(Some(actor_id)),
                            crate::schema::inventory_ledger::created_at.eq(now),
                        ))
                        .on_conflict(crate::schema::inventory_ledger::correlation_id)
                        .do_nothing()
                        .execute(conn)
                        .map_err(AppError::from)?;
                    crate::inventory::service::check_qty_alerts(conn, *item_id, new_qty, item.available_qty, item.safety_stock)?;
                }
            } else {
                // Hold strategy: create new holds, release old holds
                let old_hold_ids: Vec<Uuid> = inventory_holds::table
                    .filter(inventory_holds::booking_id.eq(booking_id))
                    .filter(inventory_holds::released_at.is_null())
                    .select(inventory_holds::id)
                    .load(conn)
                    .map_err(AppError::from)?;

                let mut hold_expiry = None;
                let mut new_hold_ids: Vec<Uuid> = Vec::new();
                for (item_id, qty, _) in &items_data {
                    let hold = crate::inventory::service::create_hold_in_tx(
                        conn,
                        *item_id,
                        Some(booking_id),
                        *qty,
                        hold_timeout,
                        Some(format!("change:booking:{}:{}", booking_id, item_id)),
                        Some(actor_id),
                    )?;
                    hold_expiry = Some(hold.expires_at);
                    new_hold_ids.push(hold.id);
                }

                for hold_id in old_hold_ids.into_iter().filter(|id| !new_hold_ids.contains(id)) {
                    crate::inventory::service::release_hold_in_tx(conn, hold_id, Some(actor_id))?;
                }

                if let Some(expiry) = hold_expiry {
                    diesel::update(bookings::table.filter(bookings::id.eq(booking_id)))
                        .set(bookings::inventory_hold_expires_at.eq(Some(expiry)))
                        .execute(conn)
                        .map_err(AppError::from)?;
                }
            }

            // 6. Rewrite booking items
            diesel::delete(booking_items::table.filter(booking_items::booking_id.eq(booking_id)))
                .execute(conn)
                .map_err(AppError::from)?;

            let new_total_cents: i64 = items_data.iter().map(|(_, qty, price)| *qty as i64 * price).sum();
            for (item_id, qty, price) in &items_data {
                repository::create_booking_item(conn, NewBookingItem {
                    id: Uuid::new_v4(),
                    booking_id,
                    inventory_item_id: *item_id,
                    quantity: *qty,
                    unit_price_cents: *price,
                    created_at: now,
                })?;
            }

            // 7. Update booking fields (immediate strategy has no holds, clear expiry)
            diesel::update(bookings::table.filter(bookings::id.eq(booking_id)))
                .set((
                    bookings::start_at.eq(effective_start),
                    bookings::end_at.eq(effective_end),
                    bookings::pickup_point_id.eq(pickup_point_id),
                    bookings::zone_id.eq(zone_id),
                    bookings::total_cents.eq(new_total_cents),
                    bookings::updated_at.eq(now),
                ))
                .execute(conn)
                .map_err(AppError::from)?;

            // 8. Transition state
            let member_id = current.member_id;
            let booking = repository::transition_state(
                conn,
                booking_id,
                current.state,
                BookingState::Changed,
                reason,
                Some(actor_id),
                current.version,
            )?;

            Ok((booking, member_id))
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    send_booking_notification(pool, cfg, member_id, TemplateTrigger::BookingChanged, &booking).await;

    Ok(booking)
}

pub async fn complete_booking(
    pool: &DbPool,
    cfg: &AppConfig,
    booking_id: Uuid,
    reason: Option<String>,
    actor_id: Uuid,
) -> Result<Booking, AppError> {
    let pool_c = pool.clone();
    let booking = actix_web::web::block(move || -> Result<Booking, AppError> {
        let mut conn = pool_c.get()?;
        let booking = repository::find_booking(&mut conn, booking_id)?;
        BookingStateMachine::transition(&booking.state, &BookingState::Completed)?;
        repository::transition_state(
            &mut conn,
            booking_id,
            booking.state,
            BookingState::Completed,
            reason,
            Some(actor_id),
            booking.version,
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    send_booking_notification(pool, cfg, booking.member_id, TemplateTrigger::BookingCompleted, &booking).await;

    Ok(booking)
}

pub async fn flag_exception(
    pool: &DbPool,
    cfg: &AppConfig,
    booking_id: Uuid,
    reason: Option<String>,
    actor_id: Uuid,
) -> Result<Booking, AppError> {
    let pool_c = pool.clone();
    let booking = actix_web::web::block(move || -> Result<Booking, AppError> {
        let mut conn = pool_c.get()?;
        let booking = repository::find_booking(&mut conn, booking_id)?;
        BookingStateMachine::transition(&booking.state, &BookingState::ExceptionPending)?;
        repository::transition_state(
            &mut conn,
            booking_id,
            booking.state,
            BookingState::ExceptionPending,
            reason,
            Some(actor_id),
            booking.version,
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    send_booking_notification(pool, cfg, booking.member_id, TemplateTrigger::BookingException, &booking).await;

    Ok(booking)
}

pub async fn expire_held_bookings(pool: &DbPool) -> Result<usize, AppError> {
    let pool = pool.clone();
    actix_web::web::block(move || -> Result<usize, AppError> {
        let mut conn = pool.get()?;
        let expired = repository::find_expired_held_bookings(&mut conn)?;
        let count = expired.len();
        for booking in expired {
            repository::transition_state(
                &mut conn,
                booking.id,
                BookingState::Held,
                BookingState::Expired,
                Some("Hold timeout".into()),
                None,
                booking.version,
            )?;
        }
        Ok(count)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
