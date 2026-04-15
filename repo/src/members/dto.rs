use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::members::model::{BlacklistReason, MemberTier};

#[derive(Debug, Deserialize, Validate)]
pub struct RedeemPointsRequest {
    #[validate(range(min = 100, message = "minimum redemption is 100 points"))]
    pub amount_pts: i32,
    pub reference_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AdjustPointsRequest {
    pub delta: i32,
    pub reference_id: Option<Uuid>,
    #[validate(length(min = 1, message = "note is required for manual adjustment"))]
    pub note: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct WalletTopUpRequest {
    #[validate(range(min = 1, message = "amount must be positive"))]
    pub amount_cents: i64,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct FreezeRedemptionRequest {
    /// Structured reason code — required to match the prompt's strict reason-code policy.
    pub reason: BlacklistReason,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePreferencesRequest {
    pub notification_opt_out: Option<Vec<String>>,
    pub preferred_channel: Option<String>,
    /// UTC offset in minutes for DND window evaluation (e.g. +180 = UTC+3, -300 = UTC-5).
    #[validate(range(min = -720, max = 840, message = "timezone_offset_minutes must be in range -720..=840"))]
    pub timezone_offset_minutes: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct BlacklistMemberRequest {
    pub reason: BlacklistReason,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ForceTierRequest {
    pub tier: MemberTier,
}
