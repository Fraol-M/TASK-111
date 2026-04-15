use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateInventoryItemRequest {
    #[validate(length(min = 1, message = "sku is required"))]
    pub sku: String,
    #[validate(length(min = 1, message = "name is required"))]
    pub name: String,
    pub description: Option<String>,
    #[validate(range(min = 0, message = "available_qty must be >= 0"))]
    pub available_qty: i32,
    #[validate(range(min = 0, message = "safety_stock must be >= 0"))]
    pub safety_stock: i32,
    pub pickup_point_id: Option<Uuid>,
    pub zone_id: Option<Uuid>,
    #[validate(range(min = 0, message = "cutoff_hours must be >= 0"))]
    pub cutoff_hours: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateInventoryItemRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    #[validate(range(min = 0, message = "available_qty must be >= 0"))]
    pub available_qty: Option<i32>,
    #[validate(range(min = 0, message = "safety_stock must be >= 0"))]
    pub safety_stock: Option<i32>,
    #[validate(range(min = 0, message = "cutoff_hours must be >= 0"))]
    pub cutoff_hours: Option<i32>,
    /// Optimistic concurrency version
    pub version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RestockRequest {
    #[validate(range(min = 1, message = "quantity must be positive"))]
    pub quantity: i32,
    pub note: Option<String>,
}

// ────────── Pickup point / Delivery zone management DTOs ──────────────
//
// These support the admin API that exposes cutoff_hours configuration.
// Cutoff precedence at booking time: zone > pickup > item.cutoff_hours.

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePickupPointRequest {
    #[validate(length(min = 1, message = "name is required"))]
    pub name: String,
    pub address: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
    #[validate(range(min = 0, message = "cutoff_hours must be >= 0"))]
    pub cutoff_hours: Option<i32>,
}

#[derive(Debug, Deserialize, Validate, Serialize, Default)]
pub struct UpdatePickupPointRequest {
    #[validate(length(min = 1, message = "name must not be empty"))]
    pub name: Option<String>,
    pub address: Option<String>,
    pub active: Option<bool>,
    #[validate(range(min = 0, message = "cutoff_hours must be >= 0"))]
    pub cutoff_hours: Option<i32>,
    /// Explicit reset for cutoff_hours. When `true`, cutoff_hours is set to
    /// NULL regardless of the `cutoff_hours` field value (useful because
    /// `Option<i32>` alone cannot distinguish "unset" from "clear").
    #[serde(default)]
    pub clear_cutoff: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateDeliveryZoneRequest {
    #[validate(length(min = 1, message = "name is required"))]
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
    #[validate(range(min = 0, message = "cutoff_hours must be >= 0"))]
    pub cutoff_hours: Option<i32>,
}

#[derive(Debug, Deserialize, Validate, Serialize, Default)]
pub struct UpdateDeliveryZoneRequest {
    #[validate(length(min = 1, message = "name must not be empty"))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub active: Option<bool>,
    #[validate(range(min = 0, message = "cutoff_hours must be >= 0"))]
    pub cutoff_hours: Option<i32>,
    #[serde(default)]
    pub clear_cutoff: bool,
}

fn default_true() -> bool {
    true
}
