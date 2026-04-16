use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::PublishStatus"]
pub enum PublishStatus {
    #[db_rename = "published"]
    Published,
    #[db_rename = "unpublished"]
    Unpublished,
    #[db_rename = "archived"]
    Archived,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::inventory_items)]
pub struct InventoryItem {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub available_qty: i32,
    pub safety_stock: i32,
    pub publish_status: PublishStatus,
    pub pickup_point_id: Option<Uuid>,
    pub zone_id: Option<Uuid>,
    pub cutoff_hours: i32,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::inventory_items)]
pub struct NewInventoryItem {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub available_qty: i32,
    pub safety_stock: i32,
    pub publish_status: PublishStatus,
    pub pickup_point_id: Option<Uuid>,
    pub zone_id: Option<Uuid>,
    pub cutoff_hours: i32,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::inventory_holds)]
pub struct InventoryHold {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub booking_id: Option<Uuid>,
    pub quantity: i32,
    pub expires_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::inventory_holds)]
pub struct NewInventoryHold {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub booking_id: Option<Uuid>,
    pub quantity: i32,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::restock_alerts)]
pub struct RestockAlert {
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub triggered_qty: i32,
    pub triggered_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<Uuid>,
}

// ───────────────────────────── Pickup points ─────────────────────────────

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::pickup_points)]
pub struct PickupPoint {
    pub id: Uuid,
    pub name: String,
    pub address: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    /// Precedence-ordered fulfilment cutoff (hours before booking start).
    /// When set on a pickup point, it overrides the item-level cutoff.
    pub cutoff_hours: Option<i32>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::pickup_points)]
pub struct NewPickupPoint {
    pub id: Uuid,
    pub name: String,
    pub address: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub cutoff_hours: Option<i32>,
}

// ──────────────────────────── Delivery zones ─────────────────────────────

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::delivery_zones)]
pub struct DeliveryZone {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    /// Precedence-ordered fulfilment cutoff (hours before booking start).
    /// Highest precedence — overrides both pickup-point and item cutoffs.
    pub cutoff_hours: Option<i32>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::delivery_zones)]
pub struct NewDeliveryZone {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub cutoff_hours: Option<i32>,
}
