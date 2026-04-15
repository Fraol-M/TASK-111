use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::payments::model::{
    IntentState, Payment, PaymentAdjustment, PaymentIntent, PaymentState, Refund, RefundState,
};
use crate::common::pagination::Page;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateIntentRequest {
    pub booking_id: Option<Uuid>,
    pub member_id: Uuid,
    #[validate(range(min = 1))]
    pub amount_cents: i64,
    /// Tax portion of amount_cents. Defaults to 0 (untaxed). Must satisfy
    /// 0 <= tax_cents <= amount_cents. Points accrue on (amount - tax).
    #[serde(default)]
    #[validate(range(min = 0))]
    pub tax_cents: i64,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CapturePaymentRequest {
    #[validate(length(min = 1, max = 100))]
    pub payment_method: String,
    pub idempotency_key: String,
    pub external_reference: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRefundRequest {
    #[validate(range(min = 1))]
    pub amount_cents: i64,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAdjustmentRequest {
    pub payment_id: Uuid,
    pub amount_cents: i64,
    #[validate(length(min = 1))]
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct PaymentIntentResponse {
    pub id: Uuid,
    pub booking_id: Option<Uuid>,
    pub member_id: Uuid,
    pub amount_cents: i64,
    pub tax_cents: i64,
    pub state: IntentState,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<PaymentIntent> for PaymentIntentResponse {
    fn from(i: PaymentIntent) -> Self {
        Self {
            id: i.id,
            booking_id: i.booking_id,
            member_id: i.member_id,
            amount_cents: i.amount_cents,
            tax_cents: i.tax_cents,
            state: i.state,
            expires_at: i.expires_at,
            created_at: i.created_at,
            updated_at: i.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaymentResponse {
    pub id: Uuid,
    pub intent_id: Uuid,
    pub member_id: Uuid,
    pub booking_id: Option<Uuid>,
    pub amount_cents: i64,
    pub tax_cents: i64,
    pub payment_method: String,
    pub state: PaymentState,
    pub external_reference: Option<String>,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Payment> for PaymentResponse {
    fn from(p: Payment) -> Self {
        Self {
            id: p.id,
            intent_id: p.intent_id,
            member_id: p.member_id,
            booking_id: p.booking_id,
            amount_cents: p.amount_cents,
            tax_cents: p.tax_cents,
            payment_method: p.payment_method,
            state: p.state,
            external_reference: p.external_reference,
            version: p.version,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RefundResponse {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub amount_cents: i64,
    pub reason: Option<String>,
    pub state: RefundState,
    pub requested_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Refund> for RefundResponse {
    fn from(r: Refund) -> Self {
        Self {
            id: r.id,
            payment_id: r.payment_id,
            amount_cents: r.amount_cents,
            reason: r.reason,
            state: r.state,
            requested_by: r.requested_by,
            approved_by: r.approved_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AdjustmentResponse {
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

impl From<PaymentAdjustment> for AdjustmentResponse {
    fn from(a: PaymentAdjustment) -> Self {
        Self {
            id: a.id,
            payment_id: a.payment_id,
            amount_cents: a.amount_cents,
            reason: a.reason,
            created_by: a.created_by,
            state: a.state,
            approved_by: a.approved_by,
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }
}

pub type PaymentListResponse = Page<PaymentResponse>;
