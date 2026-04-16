use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::BookingState"]
pub enum BookingState {
    #[db_rename = "draft"]
    Draft,
    #[db_rename = "held"]
    Held,
    #[db_rename = "confirmed"]
    Confirmed,
    #[db_rename = "changed"]
    Changed,
    #[db_rename = "cancelled"]
    Cancelled,
    #[db_rename = "completed"]
    Completed,
    #[db_rename = "exception_pending"]
    ExceptionPending,
    #[db_rename = "expired"]
    Expired,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::bookings)]
pub struct Booking {
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

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::bookings)]
pub struct NewBooking {
    pub id: Uuid,
    pub member_id: Uuid,
    pub state: BookingState,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub inventory_hold_expires_at: Option<DateTime<Utc>>,
    pub pickup_point_id: Option<Uuid>,
    pub zone_id: Option<Uuid>,
    pub total_cents: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::booking_items)]
pub struct BookingItem {
    pub id: Uuid,
    pub booking_id: Uuid,
    pub inventory_item_id: Uuid,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::booking_items)]
pub struct NewBookingItem {
    pub id: Uuid,
    pub booking_id: Uuid,
    pub inventory_item_id: Uuid,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::booking_status_history)]
pub struct BookingStatusHistory {
    pub id: Uuid,
    pub booking_id: Uuid,
    pub from_state: Option<BookingState>,
    pub to_state: BookingState,
    pub reason: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
