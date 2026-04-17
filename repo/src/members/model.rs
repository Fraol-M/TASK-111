use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::MemberTier"]
pub enum MemberTier {
    #[db_rename = "silver"]
    Silver,
    #[db_rename = "gold"]
    Gold,
    #[db_rename = "platinum"]
    Platinum,
}

impl MemberTier {
    /// Rolling 12-month spend thresholds in cents
    pub fn from_spend_cents(spend: i64) -> Self {
        if spend >= 1_500_000 {
            MemberTier::Platinum
        } else if spend >= 500_000 {
            MemberTier::Gold
        } else {
            MemberTier::Silver
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            MemberTier::Silver => "silver",
            MemberTier::Gold => "gold",
            MemberTier::Platinum => "platinum",
        }
    }
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::BlacklistReason"]
pub enum BlacklistReason {
    #[db_rename = "fraud"]
    Fraud,
    #[db_rename = "payment_default"]
    PaymentDefault,
    #[db_rename = "policy_violation"]
    PolicyViolation,
    #[db_rename = "manual"]
    Manual,
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::PointsTxnType"]
pub enum PointsTxnType {
    #[db_rename = "earn"]
    Earn,
    #[db_rename = "redeem"]
    Redeem,
    #[db_rename = "adjust"]
    Adjust,
    #[db_rename = "expire"]
    Expire,
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::WalletTxnType"]
pub enum WalletTxnType {
    #[db_rename = "top_up"]
    TopUp,
    #[db_rename = "debit"]
    Debit,
    #[db_rename = "refund"]
    Refund,
    #[db_rename = "adjustment"]
    Adjustment,
}

#[allow(dead_code)]
#[derive(Debug, Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::members)]
pub struct Member {
    pub user_id: Uuid,
    pub tier: MemberTier,
    pub points_balance: i32,
    pub wallet_balance: String, // AES-GCM encrypted
    pub blacklist_flag: bool,
    pub blacklist_reason: Option<BlacklistReason>,
    pub blacklisted_at: Option<DateTime<Utc>>,
    pub redemption_frozen_until: Option<DateTime<Utc>>,
    pub rolling_12m_spend: i64,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::members)]
pub struct NewMember {
    pub user_id: Uuid,
    pub tier: MemberTier,
    pub points_balance: i32,
    pub wallet_balance: String,
    pub blacklist_flag: bool,
    pub rolling_12m_spend: i64,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::points_ledger)]
pub struct PointsLedger {
    pub id: Uuid,
    pub user_id: Uuid,
    pub txn_type: PointsTxnType,
    pub delta: i32,
    pub balance_after: i32,
    pub reference_id: Option<Uuid>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::points_ledger)]
pub struct NewPointsLedger {
    pub id: Uuid,
    pub user_id: Uuid,
    pub txn_type: PointsTxnType,
    pub delta: i32,
    pub balance_after: i32,
    pub reference_id: Option<Uuid>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::wallet_ledger)]
pub struct WalletLedger {
    pub id: Uuid,
    pub user_id: Uuid,
    pub txn_type: WalletTxnType,
    pub delta_cents: i64,
    pub balance_after_cents: i64,
    pub reference_id: Option<Uuid>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::wallet_ledger)]
pub struct NewWalletLedger {
    pub id: Uuid,
    pub user_id: Uuid,
    pub txn_type: WalletTxnType,
    pub delta_cents: i64,
    pub balance_after_cents: i64,
    pub reference_id: Option<Uuid>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::member_preferences)]
pub struct MemberPreferences {
    pub user_id: Uuid,
    pub notification_opt_out: serde_json::Value,
    pub preferred_channel: String,
    /// UTC offset in minutes (e.g. +180 = UTC+3, -300 = UTC-5). Default 0 = UTC.
    pub timezone_offset_minutes: i32,
    pub updated_at: DateTime<Utc>,
}
