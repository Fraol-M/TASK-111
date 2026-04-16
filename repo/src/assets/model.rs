use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::AssetStatus"]
pub enum AssetStatus {
    #[db_rename = "active"]
    Active,
    #[db_rename = "maintenance"]
    Maintenance,
    #[db_rename = "retired"]
    Retired,
    #[db_rename = "disposed"]
    Disposed,
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::DepreciationMethod"]
pub enum DepreciationMethod {
    #[db_rename = "straight_line"]
    StraightLine,
    #[db_rename = "declining_balance"]
    DecliningBalance,
    #[db_rename = "none"]
    None,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::assets)]
pub struct Asset {
    pub id: Uuid,
    pub asset_code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: AssetStatus,
    pub procurement_cost: String, // AES-256-GCM encrypted cents
    pub depreciation_method: DepreciationMethod,
    pub useful_life_years: Option<i32>,
    pub purchase_date: Option<chrono::NaiveDate>,
    pub location: Option<String>,
    pub version: i32,
    pub classification: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub owner_unit: Option<String>,
    pub responsible_user_id: Option<Uuid>,
    pub useful_life_months: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::assets)]
pub struct NewAsset {
    pub id: Uuid,
    pub asset_code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: AssetStatus,
    pub procurement_cost: String,
    pub depreciation_method: DepreciationMethod,
    pub useful_life_years: Option<i32>,
    pub purchase_date: Option<chrono::NaiveDate>,
    pub location: Option<String>,
    pub version: i32,
    pub classification: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub owner_unit: Option<String>,
    pub responsible_user_id: Option<Uuid>,
    pub useful_life_months: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::asset_versions)]
pub struct AssetVersion {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub version_no: i32,
    pub snapshot_json: JsonValue,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::asset_versions)]
pub struct NewAssetVersion {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub version_no: i32,
    pub snapshot_json: JsonValue,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::asset_attachments)]
pub struct AssetAttachment {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub file_name: String,
    pub stored_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::asset_attachments)]
pub struct NewAssetAttachment {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub file_name: String,
    pub stored_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_by: Uuid,
    pub created_at: DateTime<Utc>,
}
