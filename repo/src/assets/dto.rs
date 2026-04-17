use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;
use validator::Validate;

use crate::assets::model::{Asset, AssetAttachment, AssetStatus, AssetVersion, DepreciationMethod};
use crate::common::pagination::Page;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAssetRequest {
    #[validate(length(min = 1, max = 100))]
    pub asset_code: String,
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub description: Option<String>,
    pub status: AssetStatus,
    #[validate(range(min = 0))]
    pub procurement_cost_cents: i64,
    pub depreciation_method: DepreciationMethod,
    pub useful_life_years: Option<i32>,
    pub useful_life_months: Option<i32>,
    pub purchase_date: Option<NaiveDate>,
    pub location: Option<String>,
    pub classification: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub owner_unit: Option<String>,
    pub responsible_user_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAssetRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<AssetStatus>,
    pub procurement_cost_cents: Option<i64>,
    pub location: Option<String>,
    pub useful_life_years: Option<i32>,
    pub useful_life_months: Option<i32>,
    pub purchase_date: Option<NaiveDate>,
    pub classification: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub owner_unit: Option<String>,
    pub responsible_user_id: Option<Uuid>,
    pub expected_version: i32,
}

#[derive(Debug, Serialize)]
pub struct AssetResponse {
    pub id: Uuid,
    pub asset_code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: AssetStatus,
    /// Either the decrypted cost string (Finance/Admin) or masked value
    pub procurement_cost: String,
    pub depreciation_method: DepreciationMethod,
    pub useful_life_years: Option<i32>,
    pub useful_life_months: Option<i32>,
    pub purchase_date: Option<NaiveDate>,
    pub location: Option<String>,
    pub classification: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub owner_unit: Option<String>,
    pub responsible_user_id: Option<Uuid>,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AssetResponse {
    pub fn from_asset(a: Asset, cost_display: String) -> Self {
        Self {
            id: a.id,
            asset_code: a.asset_code,
            name: a.name,
            description: a.description,
            status: a.status,
            procurement_cost: cost_display,
            depreciation_method: a.depreciation_method,
            useful_life_years: a.useful_life_years,
            useful_life_months: a.useful_life_months,
            purchase_date: a.purchase_date,
            location: a.location,
            classification: a.classification,
            brand: a.brand,
            model: a.model,
            owner_unit: a.owner_unit,
            responsible_user_id: a.responsible_user_id,
            version: a.version,
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AssetVersionResponse {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub version_no: i32,
    pub snapshot_json: JsonValue,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl From<AssetVersion> for AssetVersionResponse {
    fn from(v: AssetVersion) -> Self {
        Self {
            id: v.id,
            asset_id: v.asset_id,
            version_no: v.version_no,
            snapshot_json: v.snapshot_json,
            created_by: v.created_by,
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AssetAttachmentResponse {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_by: Uuid,
    pub created_at: DateTime<Utc>,
}

impl From<AssetAttachment> for AssetAttachmentResponse {
    fn from(a: AssetAttachment) -> Self {
        Self {
            id: a.id,
            asset_id: a.asset_id,
            file_name: a.file_name,
            mime_type: a.mime_type,
            size_bytes: a.size_bytes,
            uploaded_by: a.uploaded_by,
            created_at: a.created_at,
        }
    }
}

#[allow(dead_code)]
pub type AssetListResponse = Page<AssetResponse>;
