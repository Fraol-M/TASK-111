use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::common::{db::DbConn, errors::AppError};
use crate::inventory::model::{
    DeliveryZone, InventoryItem, NewDeliveryZone, NewInventoryItem, NewPickupPoint, PickupPoint,
    RestockAlert,
};
use crate::schema::{delivery_zones, inventory_items, pickup_points, restock_alerts};

pub fn create_item(conn: &mut DbConn, item: NewInventoryItem) -> Result<InventoryItem, AppError> {
    diesel::insert_into(inventory_items::table)
        .values(&item)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_item(conn: &mut DbConn, item_id: Uuid) -> Result<InventoryItem, AppError> {
    inventory_items::table
        .filter(inventory_items::id.eq(item_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Inventory item {} not found", item_id)))
}

pub fn list_items(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<InventoryItem>, i64), AppError> {
    let total: i64 = inventory_items::table.count().get_result(conn).map_err(AppError::from)?;
    let items = inventory_items::table
        .order(inventory_items::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((items, total))
}

pub fn update_item_qty(
    conn: &mut DbConn,
    item_id: Uuid,
    new_qty: i32,
    expected_version: i32,
) -> Result<InventoryItem, AppError> {
    let rows = diesel::update(
        inventory_items::table
            .filter(inventory_items::id.eq(item_id))
            .filter(inventory_items::version.eq(expected_version)),
    )
    .set((
        inventory_items::available_qty.eq(new_qty),
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
    find_item(conn, item_id)
}

pub fn list_open_alerts(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<RestockAlert>, i64), AppError> {
    let total: i64 = restock_alerts::table
        .filter(restock_alerts::acknowledged_at.is_null())
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let alerts = restock_alerts::table
        .filter(restock_alerts::acknowledged_at.is_null())
        .order(restock_alerts::triggered_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;

    Ok((alerts, total))
}

pub fn acknowledge_alert(
    conn: &mut DbConn,
    alert_id: Uuid,
    acknowledged_by: Uuid,
) -> Result<(), AppError> {
    diesel::update(restock_alerts::table.filter(restock_alerts::id.eq(alert_id)))
        .set((
            restock_alerts::acknowledged_at.eq(Some(Utc::now())),
            restock_alerts::acknowledged_by.eq(Some(acknowledged_by)),
        ))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

// ───────────────────────────── Pickup points ─────────────────────────────

pub fn create_pickup_point(
    conn: &mut DbConn,
    new: NewPickupPoint,
) -> Result<PickupPoint, AppError> {
    diesel::insert_into(pickup_points::table)
        .values(&new)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_pickup_point(conn: &mut DbConn, id: Uuid) -> Result<PickupPoint, AppError> {
    pickup_points::table
        .filter(pickup_points::id.eq(id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Pickup point {} not found", id)))
}

pub fn list_pickup_points(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<PickupPoint>, i64), AppError> {
    let total: i64 = pickup_points::table
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;
    let items = pickup_points::table
        .order(pickup_points::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((items, total))
}

/// Patch update. `name`/`address`/`active` apply when Some. For `cutoff_hours`
/// the caller must supply either `Some(n)` to set a new value or pass
/// `clear=true` to explicitly unset (NULL) — this matches the `UpdateRequest`
/// DTO which exposes both a value and an explicit `clear_cutoff` flag.
pub fn update_pickup_point(
    conn: &mut DbConn,
    id: Uuid,
    name: Option<String>,
    address: Option<String>,
    active: Option<bool>,
    cutoff_hours: Option<i32>,
    clear_cutoff: bool,
) -> Result<PickupPoint, AppError> {
    // Diesel's ergonomics for partial updates with mixed column types are
    // awkward inside a single query, so we fetch + conditionally write each
    // column. The path is admin-only and low-frequency so the extra round-trips
    // are acceptable in exchange for clarity.
    let existing = find_pickup_point(conn, id)?;

    let next_name = name.unwrap_or(existing.name);
    let next_address = address.or(existing.address);
    let next_active = active.unwrap_or(existing.active);
    let next_cutoff: Option<i32> = if clear_cutoff {
        None
    } else {
        cutoff_hours.or(existing.cutoff_hours)
    };

    diesel::update(pickup_points::table.filter(pickup_points::id.eq(id)))
        .set((
            pickup_points::name.eq(next_name),
            pickup_points::address.eq(next_address),
            pickup_points::active.eq(next_active),
            pickup_points::cutoff_hours.eq(next_cutoff),
        ))
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn delete_pickup_point(conn: &mut DbConn, id: Uuid) -> Result<(), AppError> {
    // Hard delete is rejected if any inventory_items reference this pickup
    // point — surface as 409 Conflict rather than a raw FK violation. We do a
    // defensive pre-check so the error message is clear instead of relying
    // solely on the DB constraint.
    let refs: i64 = inventory_items::table
        .filter(inventory_items::pickup_point_id.eq(Some(id)))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;
    if refs > 0 {
        return Err(AppError::Conflict(format!(
            "Pickup point {} is referenced by {} inventory item(s); deactivate instead",
            id, refs
        )));
    }

    let rows = diesel::delete(pickup_points::table.filter(pickup_points::id.eq(id)))
        .execute(conn)
        .map_err(AppError::from)?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("Pickup point {} not found", id)));
    }
    Ok(())
}

// ──────────────────────────── Delivery zones ─────────────────────────────

pub fn create_zone(conn: &mut DbConn, new: NewDeliveryZone) -> Result<DeliveryZone, AppError> {
    diesel::insert_into(delivery_zones::table)
        .values(&new)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_zone(conn: &mut DbConn, id: Uuid) -> Result<DeliveryZone, AppError> {
    delivery_zones::table
        .filter(delivery_zones::id.eq(id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Delivery zone {} not found", id)))
}

pub fn list_zones(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DeliveryZone>, i64), AppError> {
    let total: i64 = delivery_zones::table
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;
    let items = delivery_zones::table
        .order(delivery_zones::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((items, total))
}

pub fn update_zone(
    conn: &mut DbConn,
    id: Uuid,
    name: Option<String>,
    description: Option<String>,
    active: Option<bool>,
    cutoff_hours: Option<i32>,
    clear_cutoff: bool,
) -> Result<DeliveryZone, AppError> {
    let existing = find_zone(conn, id)?;

    let next_name = name.unwrap_or(existing.name);
    let next_description = description.or(existing.description);
    let next_active = active.unwrap_or(existing.active);
    let next_cutoff: Option<i32> = if clear_cutoff {
        None
    } else {
        cutoff_hours.or(existing.cutoff_hours)
    };

    diesel::update(delivery_zones::table.filter(delivery_zones::id.eq(id)))
        .set((
            delivery_zones::name.eq(next_name),
            delivery_zones::description.eq(next_description),
            delivery_zones::active.eq(next_active),
            delivery_zones::cutoff_hours.eq(next_cutoff),
        ))
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn delete_zone(conn: &mut DbConn, id: Uuid) -> Result<(), AppError> {
    let refs: i64 = inventory_items::table
        .filter(inventory_items::zone_id.eq(Some(id)))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;
    if refs > 0 {
        return Err(AppError::Conflict(format!(
            "Delivery zone {} is referenced by {} inventory item(s); deactivate instead",
            id, refs
        )));
    }
    let rows = diesel::delete(delivery_zones::table.filter(delivery_zones::id.eq(id)))
        .execute(conn)
        .map_err(AppError::from)?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("Delivery zone {} not found", id)));
    }
    Ok(())
}
