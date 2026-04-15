use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::IntentState"]
pub enum IntentState {
    #[db_rename = "open"]
    Open,
    #[db_rename = "captured"]
    Captured,
    #[db_rename = "timed_out"]
    TimedOut,
    #[db_rename = "cancelled"]
    Cancelled,
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::PaymentState"]
pub enum PaymentState {
    #[db_rename = "pending"]
    Pending,
    #[db_rename = "completed"]
    Completed,
    #[db_rename = "refunded"]
    Refunded,
    #[db_rename = "partially_refunded"]
    PartiallyRefunded,
    #[db_rename = "failed"]
    Failed,
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::RefundState"]
pub enum RefundState {
    #[db_rename = "pending"]
    Pending,
    #[db_rename = "approved"]
    Approved,
    #[db_rename = "rejected"]
    Rejected,
    #[db_rename = "processed"]
    Processed,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::payment_intents)]
pub struct PaymentIntent {
    pub id: Uuid,
    pub booking_id: Option<Uuid>,
    pub member_id: Uuid,
    pub amount_cents: i64,
    pub state: IntentState,
    pub idempotency_key: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Tax portion of amount_cents (0 if untaxed). net = amount_cents - tax_cents.
    pub tax_cents: i64,
    /// Optimistic concurrency token — incremented on every state transition.
    pub version: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::payment_intents)]
pub struct NewPaymentIntent {
    pub id: Uuid,
    pub booking_id: Option<Uuid>,
    pub member_id: Uuid,
    pub amount_cents: i64,
    pub state: IntentState,
    pub idempotency_key: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tax_cents: i64,
    pub version: i32,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::payments)]
pub struct Payment {
    pub id: Uuid,
    pub intent_id: Uuid,
    pub member_id: Uuid,
    pub booking_id: Option<Uuid>,
    pub amount_cents: i64,
    pub payment_method: String,
    pub state: PaymentState,
    pub idempotency_key: String,
    pub external_reference: Option<String>,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Tax portion of amount_cents (0 if untaxed). net = amount_cents - tax_cents.
    pub tax_cents: i64,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::payments)]
pub struct NewPayment {
    pub id: Uuid,
    pub intent_id: Uuid,
    pub member_id: Uuid,
    pub booking_id: Option<Uuid>,
    pub amount_cents: i64,
    pub payment_method: String,
    pub state: PaymentState,
    pub idempotency_key: String,
    pub external_reference: Option<String>,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tax_cents: i64,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::refunds)]
pub struct Refund {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub amount_cents: i64,
    pub reason: Option<String>,
    pub state: RefundState,
    pub idempotency_key: String,
    pub requested_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Optimistic concurrency token — incremented on every state transition.
    pub version: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::refunds)]
pub struct NewRefund {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub amount_cents: i64,
    pub reason: Option<String>,
    pub state: RefundState,
    pub idempotency_key: String,
    pub requested_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i32,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::payment_adjustments)]
pub struct PaymentAdjustment {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub amount_cents: i64,
    pub reason: String,
    pub created_by: Uuid,
    pub state: String,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::payment_adjustments)]
pub struct NewPaymentAdjustment {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub amount_cents: i64,
    pub reason: String,
    pub created_by: Uuid,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
