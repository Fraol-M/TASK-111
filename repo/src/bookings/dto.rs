use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::bookings::model::{Booking, BookingItem, BookingState, BookingStatusHistory};
use crate::common::pagination::Page;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CreateBookingRequest {
    #[validate(length(min = 1, message = "items must not be empty"))]
    pub items: Vec<BookingItemInput>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub pickup_point_id: Option<Uuid>,
    pub zone_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct BookingItemInput {
    pub inventory_item_id: Uuid,
    #[validate(range(min = 1, message = "quantity must be at least 1"))]
    pub quantity: i32,
    #[validate(range(min = 1, message = "unit_price_cents must be positive"))]
    pub unit_price_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct CancelBookingRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ChangeBookingRequest {
    #[validate(length(min = 1, message = "items must not be empty"))]
    pub items: Vec<BookingItemInput>,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub pickup_point_id: Option<Uuid>,
    pub zone_id: Option<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteBookingRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExceptionRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BookingResponse {
    pub id: Uuid,
    pub member_id: Uuid,
    pub state: BookingState,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub inventory_hold_expires_at: Option<DateTime<Utc>>,
    pub change_reason: Option<String>,
    pub pickup_point_id: Option<Uuid>,
    pub zone_id: Option<Uuid>,
    pub total_cents: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Booking> for BookingResponse {
    fn from(b: Booking) -> Self {
        Self {
            id: b.id,
            member_id: b.member_id,
            state: b.state,
            start_at: b.start_at,
            end_at: b.end_at,
            inventory_hold_expires_at: b.inventory_hold_expires_at,
            change_reason: b.change_reason,
            pickup_point_id: b.pickup_point_id,
            zone_id: b.zone_id,
            total_cents: b.total_cents,
            version: b.version,
            created_at: b.created_at,
            updated_at: b.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BookingItemResponse {
    pub id: Uuid,
    pub booking_id: Uuid,
    pub inventory_item_id: Uuid,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub created_at: DateTime<Utc>,
}

impl From<BookingItem> for BookingItemResponse {
    fn from(i: BookingItem) -> Self {
        Self {
            id: i.id,
            booking_id: i.booking_id,
            inventory_item_id: i.inventory_item_id,
            quantity: i.quantity,
            unit_price_cents: i.unit_price_cents,
            created_at: i.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BookingHistoryResponse {
    pub id: Uuid,
    pub booking_id: Uuid,
    pub from_state: Option<BookingState>,
    pub to_state: BookingState,
    pub reason: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl From<BookingStatusHistory> for BookingHistoryResponse {
    fn from(h: BookingStatusHistory) -> Self {
        Self {
            id: h.id,
            booking_id: h.booking_id,
            from_state: h.from_state,
            to_state: h.to_state,
            reason: h.reason,
            actor_user_id: h.actor_user_id,
            created_at: h.created_at,
        }
    }
}

#[allow(dead_code)]
pub type BookingListResponse = Page<BookingResponse>;
