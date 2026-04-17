use chrono::{Duration, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::common::{db::{DbConn, DbPool}, errors::AppError};
use crate::inventory::model::{InventoryHold, InventoryItem, NewInventoryHold, PublishStatus};
use crate::schema::{inventory_holds, inventory_items, inventory_ledger, restock_alerts};

/// Centralized post-mutation check: auto-unpublish when qty reaches zero,
/// auto-republish when qty recovers from zero, and generate a restock alert
/// when qty falls at or below safety_stock. Called from all inventory mutation
/// paths to ensure consistent behavior.
pub fn check_qty_alerts(
    conn: &mut DbConn,
    item_id: Uuid,
    new_qty: i32,
    old_qty: i32,
    safety_stock: i32,
) -> Result<(), AppError> {
    // Auto-unpublish if qty reached zero
    if new_qty == 0 && old_qty > 0 {
        diesel::update(inventory_items::table.filter(inventory_items::id.eq(item_id)))
            .set(inventory_items::publish_status.eq(PublishStatus::Unpublished))
            .execute(conn)
            .map_err(AppError::from)?;
    } else if new_qty > 0 && old_qty == 0 {
        diesel::update(inventory_items::table.filter(inventory_items::id.eq(item_id)))
            .set(inventory_items::publish_status.eq(PublishStatus::Published))
            .execute(conn)
            .map_err(AppError::from)?;
    }

    // Restock alert if at or below safety stock (and not already open)
    if new_qty <= safety_stock {
        let existing_alert: Option<crate::inventory::model::RestockAlert> =
            restock_alerts::table
                .filter(restock_alerts::inventory_item_id.eq(item_id))
                .filter(restock_alerts::acknowledged_at.is_null())
                .first(conn)
                .optional()
                .map_err(AppError::from)?;

        if existing_alert.is_none() {
            diesel::insert_into(restock_alerts::table)
                .values((
                    restock_alerts::id.eq(Uuid::new_v4()),
                    restock_alerts::inventory_item_id.eq(item_id),
                    restock_alerts::triggered_qty.eq(new_qty),
                    restock_alerts::triggered_at.eq(Utc::now()),
                ))
                .execute(conn)
                .map_err(AppError::from)?;
        }
    }

    Ok(())
}

/// Inner hold-creation logic that can be called within an existing transaction.
/// Used by `create_hold` (standalone) and by booking service for atomic orchestration.
pub fn create_hold_in_tx(
    conn: &mut DbConn,
    item_id: Uuid,
    booking_id: Option<Uuid>,
    quantity: i32,
    hold_timeout_minutes: i64,
    correlation_id: Option<String>,
    actor_id: Option<Uuid>,
) -> Result<InventoryHold, AppError> {
            // 0. Idempotency guard: if this correlation_id was already processed, return the
            //    existing hold rather than re-running the quantity decrement.
            if let Some(ref cid) = correlation_id {
                let already: Option<Uuid> = inventory_ledger::table
                    .filter(inventory_ledger::correlation_id.eq(cid.as_str()))
                    .select(inventory_ledger::id)
                    .first(conn)
                    .optional()
                    .map_err(AppError::from)?;
                if already.is_some() {
                    return if let Some(bid) = booking_id {
                        inventory_holds::table
                            .filter(inventory_holds::booking_id.eq(bid))
                            .filter(inventory_holds::inventory_item_id.eq(item_id))
                            .order(inventory_holds::created_at.desc())
                            .first(conn)
                            .map_err(|_| AppError::Conflict(
                                "Duplicate correlation_id but original hold not found".into(),
                            ))
                    } else {
                        Err(AppError::Conflict(format!(
                            "Duplicate correlation_id '{}': already processed",
                            cid
                        )))
                    };
                }
            }

            // 1. Lock the inventory row
            let item: crate::inventory::model::InventoryItem = inventory_items::table
                .filter(inventory_items::id.eq(item_id))
                .for_update()
                .first(conn)
                .map_err(|_| AppError::NotFound(format!("Inventory item {} not found", item_id)))?;

            // 2. Oversell protection
            if item.available_qty < quantity {
                return Err(AppError::Conflict(format!(
                    "Insufficient inventory: available={}, requested={}",
                    item.available_qty, quantity
                )));
            }

            // 3. Create hold
            let expires_at = Utc::now() + Duration::minutes(hold_timeout_minutes);
            let hold = NewInventoryHold {
                id: Uuid::new_v4(),
                inventory_item_id: item_id,
                booking_id,
                quantity,
                expires_at,
                created_at: Utc::now(),
            };
            let created_hold: InventoryHold = diesel::insert_into(inventory_holds::table)
                .values(&hold)
                .get_result(conn)
                .map_err(AppError::from)?;

            // 4. Decrement inventory (optimistic concurrency on version)
            let new_qty = item.available_qty - quantity;
            let new_version = item.version + 1;
            let rows = diesel::update(
                inventory_items::table
                    .filter(inventory_items::id.eq(item_id))
                    .filter(inventory_items::version.eq(item.version)),
            )
            .set((
                inventory_items::available_qty.eq(new_qty),
                inventory_items::version.eq(new_version),
                inventory_items::updated_at.eq(Utc::now()),
            ))
            .execute(conn)
            .map_err(AppError::from)?;

            if rows == 0 {
                return Err(AppError::Conflict(
                    "Concurrent inventory modification — please retry".into(),
                ));
            }

            // 5. Append ledger (idempotent)
            let cid = correlation_id.as_deref().map(|s| s.to_string());
            diesel::insert_into(inventory_ledger::table)
                .values((
                    inventory_ledger::id.eq(Uuid::new_v4()),
                    inventory_ledger::inventory_item_id.eq(item_id),
                    inventory_ledger::delta.eq(-quantity),
                    inventory_ledger::qty_after.eq(new_qty),
                    inventory_ledger::reason.eq("hold_created"),
                    inventory_ledger::correlation_id.eq(cid),
                    inventory_ledger::actor_user_id.eq(actor_id),
                    inventory_ledger::created_at.eq(Utc::now()),
                ))
                .on_conflict(inventory_ledger::correlation_id)
                .do_nothing()
                .execute(conn)
                .map_err(AppError::from)?;

            // 6–7. Centralized: auto-unpublish + restock alert
            check_qty_alerts(conn, item_id, new_qty, item.available_qty, item.safety_stock)?;

            Ok(created_hold)
}

/// Inner hold-release logic callable within an existing transaction.
pub fn release_hold_in_tx(
    conn: &mut DbConn,
    hold_id: Uuid,
    actor_id: Option<Uuid>,
) -> Result<(), AppError> {
            let hold: InventoryHold = inventory_holds::table
                .filter(inventory_holds::id.eq(hold_id))
                .for_update()
                .first(conn)
                .map_err(|_| AppError::NotFound(format!("Hold {} not found", hold_id)))?;

            if hold.released_at.is_some() {
                return Ok(()); // Already released — idempotent
            }

            diesel::update(inventory_holds::table.filter(inventory_holds::id.eq(hold_id)))
                .set(inventory_holds::released_at.eq(Some(Utc::now())))
                .execute(conn)
                .map_err(AppError::from)?;

            let item: crate::inventory::model::InventoryItem = inventory_items::table
                .filter(inventory_items::id.eq(hold.inventory_item_id))
                .for_update()
                .first(conn)
                .map_err(AppError::from)?;

            let new_qty = item.available_qty + hold.quantity;
            diesel::update(
                inventory_items::table
                    .filter(inventory_items::id.eq(hold.inventory_item_id))
                    .filter(inventory_items::version.eq(item.version)),
            )
            .set((
                inventory_items::available_qty.eq(new_qty),
                inventory_items::version.eq(item.version + 1),
                inventory_items::updated_at.eq(Utc::now()),
            ))
            .execute(conn)
            .map_err(AppError::from)?;

            if item.available_qty == 0 && new_qty > 0 {
                diesel::update(
                    inventory_items::table.filter(inventory_items::id.eq(hold.inventory_item_id)),
                )
                .set(inventory_items::publish_status.eq(PublishStatus::Published))
                .execute(conn)
                .map_err(AppError::from)?;
            }

            diesel::insert_into(inventory_ledger::table)
                .values((
                    inventory_ledger::id.eq(Uuid::new_v4()),
                    inventory_ledger::inventory_item_id.eq(hold.inventory_item_id),
                    inventory_ledger::delta.eq(hold.quantity),
                    inventory_ledger::qty_after.eq(new_qty),
                    inventory_ledger::reason.eq("hold_released"),
                    inventory_ledger::actor_user_id.eq(actor_id),
                    inventory_ledger::created_at.eq(Utc::now()),
                ))
                .execute(conn)
                .map_err(AppError::from)?;

            Ok(())
}

/// Atomically reserve inventory. This is the critical section.
///
/// Transaction:
/// 1. SELECT ... FOR UPDATE on inventory_items (row-level lock)
/// 2. Check available_qty >= quantity (oversell protection)
/// 3. INSERT inventory_holds
/// 4. UPDATE inventory_items: available_qty -= quantity, version += 1
/// 5. INSERT inventory_ledger (idempotent via correlation_id)
/// 6. If new qty == 0: unpublish item
/// 7. If new qty <= safety_stock: insert restock alert
#[allow(dead_code)]
pub async fn create_hold(
    pool: &DbPool,
    item_id: Uuid,
    booking_id: Option<Uuid>,
    quantity: i32,
    hold_timeout_minutes: i64,
    correlation_id: Option<String>,
    actor_id: Option<Uuid>,
) -> Result<InventoryHold, AppError> {
    let pool = pool.clone();

    actix_web::web::block(move || -> Result<InventoryHold, AppError> {
        let mut conn = pool.get()?;

        conn.transaction::<InventoryHold, AppError, _>(|conn| {
            create_hold_in_tx(conn, item_id, booking_id, quantity, hold_timeout_minutes, correlation_id, actor_id)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Release an inventory hold and restore the quantity.
pub async fn release_hold(pool: &DbPool, hold_id: Uuid, actor_id: Option<Uuid>) -> Result<(), AppError> {
    let pool = pool.clone();

    actix_web::web::block(move || -> Result<(), AppError> {
        let mut conn = pool.get()?;
        conn.transaction::<(), AppError, _>(|conn| {
            release_hold_in_tx(conn, hold_id, actor_id)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Job: release all expired holds. Returns count released.
/// Delegates to release_hold() so each expiry gets the same transaction,
/// FOR UPDATE lock, and ledger append as a normal release.
/// Job: release all expired holds whose bookings are NOT already confirmed/completed/changed.
/// Confirmed bookings' holds are consumed — they must not be released back to stock.
pub async fn expire_stale_holds(pool: &DbPool) -> Result<usize, AppError> {
    let pool_c = pool.clone();

    let expired_ids: Vec<Uuid> = actix_web::web::block(move || -> Result<Vec<Uuid>, AppError> {
        use crate::bookings::model::BookingState;
        use crate::schema::bookings;
        let mut conn = pool_c.get()?;

        // Only release holds whose associated booking is still in a pre-confirmation
        // state (Draft, Held) or has no booking. Confirmed/Completed/Changed bookings
        // have consumed their inventory — releasing would cause oversell.
        let consumed_states = vec![
            BookingState::Confirmed,
            BookingState::Completed,
            BookingState::Changed,
        ];

        let all_expired: Vec<(Uuid, Option<Uuid>)> = inventory_holds::table
            .filter(inventory_holds::expires_at.lt(Utc::now()))
            .filter(inventory_holds::released_at.is_null())
            .select((inventory_holds::id, inventory_holds::booking_id))
            .load(&mut conn)
            .map_err(AppError::from)?;

        let mut releasable = Vec::new();
        for (hold_id, booking_id_opt) in all_expired {
            let should_release = match booking_id_opt {
                None => true, // standalone hold, safe to release
                Some(bid) => {
                    let booking_state: BookingState = bookings::table
                        .filter(bookings::id.eq(bid))
                        .select(bookings::state)
                        .first(&mut conn)
                        .map_err(AppError::from)?;
                    !consumed_states.contains(&booking_state)
                }
            };
            if should_release {
                releasable.push(hold_id);
            }
        }
        Ok(releasable)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let count = expired_ids.len();
    for hold_id in expired_ids {
        release_hold(pool, hold_id, None).await?;
    }
    Ok(count)
}

/// Atomically update all patchable fields of an inventory item with optimistic concurrency.
/// Quantity changes are recorded in the ledger. Other field changes (name, description,
/// safety_stock, cutoff_hours) are applied without a ledger entry.
pub async fn update_item(
    pool: &DbPool,
    item_id: Uuid,
    name: Option<String>,
    description: Option<String>,
    available_qty: Option<i32>,
    safety_stock: Option<i32>,
    cutoff_hours: Option<i32>,
    expected_version: i32,
    actor_id: Option<Uuid>,
) -> Result<InventoryItem, AppError> {
    let pool = pool.clone();
    actix_web::web::block(move || -> Result<InventoryItem, AppError> {
        let mut conn = pool.get()?;
        conn.transaction::<InventoryItem, AppError, _>(|conn| {
            let current: InventoryItem = inventory_items::table
                .filter(inventory_items::id.eq(item_id))
                .for_update()
                .first(conn)
                .map_err(AppError::from)?;

            let new_qty = available_qty.unwrap_or(current.available_qty);
            let new_name = name.unwrap_or_else(|| current.name.clone());
            let new_description = description.map(Some).unwrap_or_else(|| current.description.clone());
            let new_safety_stock = safety_stock.unwrap_or(current.safety_stock);
            let new_cutoff_hours = cutoff_hours.unwrap_or(current.cutoff_hours);

            let rows = diesel::update(
                inventory_items::table
                    .filter(inventory_items::id.eq(item_id))
                    .filter(inventory_items::version.eq(expected_version)),
            )
            .set((
                inventory_items::name.eq(&new_name),
                inventory_items::description.eq(&new_description),
                inventory_items::available_qty.eq(new_qty),
                inventory_items::safety_stock.eq(new_safety_stock),
                inventory_items::cutoff_hours.eq(new_cutoff_hours),
                inventory_items::version.eq(expected_version + 1),
                inventory_items::updated_at.eq(Utc::now()),
            ))
            .execute(conn)
            .map_err(AppError::from)?;

            if rows == 0 {
                return Err(AppError::PreconditionFailed(
                    "Concurrent modification on inventory item".into(),
                ));
            }

            // Record ledger entry only when quantity actually changed.
            if new_qty != current.available_qty {
                let delta = new_qty - current.available_qty;
                diesel::insert_into(inventory_ledger::table)
                    .values((
                        inventory_ledger::id.eq(Uuid::new_v4()),
                        inventory_ledger::inventory_item_id.eq(item_id),
                        inventory_ledger::delta.eq(delta),
                        inventory_ledger::qty_after.eq(new_qty),
                        inventory_ledger::reason.eq("manual_update"),
                        inventory_ledger::actor_user_id.eq(actor_id),
                        inventory_ledger::created_at.eq(Utc::now()),
                    ))
                    .execute(conn)
                    .map_err(AppError::from)?;
            }

            // Centralized: auto-publish/unpublish + restock alert
            check_qty_alerts(conn, item_id, new_qty, current.available_qty, new_safety_stock)?;

            crate::inventory::repository::find_item(conn, item_id)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Atomically update an inventory item's quantity with an audit ledger entry.
/// Uses FOR UPDATE to prevent concurrent modification.
#[allow(dead_code)]
pub async fn update_item_qty(
    pool: &DbPool,
    item_id: Uuid,
    new_qty: i32,
    expected_version: i32,
    actor_id: Option<Uuid>,
) -> Result<InventoryItem, AppError> {
    let pool = pool.clone();
    actix_web::web::block(move || -> Result<InventoryItem, AppError> {
        let mut conn = pool.get()?;
        conn.transaction::<InventoryItem, AppError, _>(|conn| {
            let current: InventoryItem = inventory_items::table
                .filter(inventory_items::id.eq(item_id))
                .for_update()
                .first(conn)
                .map_err(AppError::from)?;

            let delta = new_qty - current.available_qty;
            let updated = crate::inventory::repository::update_item_qty(conn, item_id, new_qty, expected_version)?;

            diesel::insert_into(inventory_ledger::table)
                .values((
                    inventory_ledger::id.eq(Uuid::new_v4()),
                    inventory_ledger::inventory_item_id.eq(item_id),
                    inventory_ledger::delta.eq(delta),
                    inventory_ledger::qty_after.eq(new_qty),
                    inventory_ledger::reason.eq("manual_update"),
                    inventory_ledger::actor_user_id.eq(actor_id),
                    inventory_ledger::created_at.eq(Utc::now()),
                ))
                .execute(conn)
                .map_err(AppError::from)?;

            // Keep publication state and restock alert consistent after any
            // quantity mutation. Without this call, a manual qty update that
            // drops to zero leaves the item marked published (and vice versa).
            check_qty_alerts(conn, item_id, new_qty, current.available_qty, current.safety_stock)?;

            Ok(updated)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Atomically restock inventory items with a full ledger entry.
/// Uses FOR UPDATE to prevent concurrent modification.
pub async fn restock_item(
    pool: &DbPool,
    item_id: Uuid,
    quantity: i32,
    actor_id: Uuid,
) -> Result<InventoryItem, AppError> {
    let pool = pool.clone();
    actix_web::web::block(move || -> Result<InventoryItem, AppError> {
        let mut conn = pool.get()?;
        conn.transaction::<InventoryItem, AppError, _>(|conn| {
            let current: InventoryItem = inventory_items::table
                .filter(inventory_items::id.eq(item_id))
                .for_update()
                .first(conn)
                .map_err(AppError::from)?;

            let new_qty = current.available_qty + quantity;
            let updated = crate::inventory::repository::update_item_qty(conn, item_id, new_qty, current.version)?;

            diesel::insert_into(inventory_ledger::table)
                .values((
                    inventory_ledger::id.eq(Uuid::new_v4()),
                    inventory_ledger::inventory_item_id.eq(item_id),
                    inventory_ledger::delta.eq(quantity),
                    inventory_ledger::qty_after.eq(new_qty),
                    inventory_ledger::reason.eq("restock"),
                    inventory_ledger::actor_user_id.eq(Some(actor_id)),
                    inventory_ledger::created_at.eq(Utc::now()),
                ))
                .execute(conn)
                .map_err(AppError::from)?;

            // A restock that moves the item from zero to positive republishes it,
            // and a restock that keeps the qty at/below safety_stock should still
            // produce an alert (no alert if already open — handled inside the helper).
            check_qty_alerts(conn, item_id, new_qty, current.available_qty, current.safety_stock)?;

            Ok(updated)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
