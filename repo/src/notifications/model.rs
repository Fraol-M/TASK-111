use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Delivery channel for notifications.
///
/// All channels are modeled in the DB enum. At runtime, `dispatch_to_channel`
/// gates delivery: `InApp` always succeeds (DB-persisted, client-polled).
/// `Email`, `Sms`, and `Push` return an error until their provider is wired,
/// which triggers the service to create an automatic `InApp` fallback
/// notification so the user always receives the message.
#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::NotificationChannel"]
pub enum NotificationChannel {
    #[db_rename = "in_app"]
    InApp,
    #[db_rename = "email"]
    Email,
    #[db_rename = "sms"]
    Sms,
    #[db_rename = "push"]
    Push,
}

impl NotificationChannel {
    /// Stable wire/string form matching the DB enum label and API contract.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            NotificationChannel::InApp => "in_app",
            NotificationChannel::Email => "email",
            NotificationChannel::Sms => "sms",
            NotificationChannel::Push => "push",
        }
    }
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::DeliveryState"]
pub enum DeliveryState {
    #[db_rename = "pending"]
    Pending,
    #[db_rename = "delivered"]
    Delivered,
    #[db_rename = "failed"]
    Failed,
    #[db_rename = "suppressed_dnd"]
    SuppressedDnd,
    #[db_rename = "opted_out"]
    OptedOut,
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::TemplateTrigger"]
pub enum TemplateTrigger {
    #[db_rename = "booking_confirmed"]
    BookingConfirmed,
    #[db_rename = "booking_cancelled"]
    BookingCancelled,
    #[db_rename = "booking_changed"]
    BookingChanged,
    #[db_rename = "booking_reminder_24h"]
    BookingReminder24h,
    #[db_rename = "booking_completed"]
    BookingCompleted,
    #[db_rename = "booking_exception"]
    BookingException,
    #[db_rename = "payment_captured"]
    PaymentCaptured,
    #[db_rename = "refund_approved"]
    RefundApproved,
    #[db_rename = "points_earned"]
    PointsEarned,
    #[db_rename = "tier_upgraded"]
    TierUpgraded,
    #[db_rename = "tier_downgraded"]
    TierDowngraded,
    #[db_rename = "wallet_topup"]
    WalletTopup,
    #[db_rename = "custom"]
    Custom,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::notification_templates)]
pub struct NotificationTemplate {
    pub id: Uuid,
    pub name: String,
    pub trigger_type: TemplateTrigger,
    pub channel: NotificationChannel,
    pub subject_template: Option<String>,
    pub body_template: String,
    pub variable_schema: Option<JsonValue>,
    pub is_critical: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::notification_templates)]
pub struct NewNotificationTemplate {
    pub id: Uuid,
    pub name: String,
    pub trigger_type: TemplateTrigger,
    pub channel: NotificationChannel,
    pub subject_template: Option<String>,
    pub body_template: String,
    pub variable_schema: Option<JsonValue>,
    pub is_critical: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::notifications)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    pub trigger_type: TemplateTrigger,
    pub channel: NotificationChannel,
    pub subject: Option<String>,
    pub body: String,
    pub payload_hash: String,
    pub delivery_state: DeliveryState,
    pub dnd_suppressed: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub reference_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::notifications)]
pub struct NewNotification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    pub trigger_type: TemplateTrigger,
    pub channel: NotificationChannel,
    pub subject: Option<String>,
    pub body: String,
    pub payload_hash: String,
    pub delivery_state: DeliveryState,
    pub dnd_suppressed: bool,
    pub reference_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::notification_attempts)]
pub struct NotificationAttempt {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub attempted_at: DateTime<Utc>,
    pub succeeded: bool,
    pub error_detail: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::notification_attempts)]
pub struct NewNotificationAttempt {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub attempted_at: DateTime<Utc>,
    pub succeeded: bool,
    pub error_detail: Option<String>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::dnd_queue)]
pub struct DndQueueEntry {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub user_id: Uuid,
    pub scheduled_deliver_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::dnd_queue)]
pub struct NewDndQueueEntry {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub user_id: Uuid,
    pub scheduled_deliver_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
