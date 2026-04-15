use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::assets::{
    model::{Asset, AssetAttachment, AssetStatus, AssetVersion, DepreciationMethod, NewAsset, NewAssetAttachment},
    repository,
};
use crate::audit::model::NewAuditLog;
use crate::audit::repository::insert_audit_log;
use crate::common::{crypto::EncryptionKey, db::DbPool, errors::AppError};

pub async fn create_asset(
    pool: &DbPool,
    enc: &EncryptionKey,
    actor_id: Uuid,
    asset_code: String,
    name: String,
    description: Option<String>,
    status: AssetStatus,
    procurement_cost_cents: i64,
    depreciation_method: DepreciationMethod,
    useful_life_years: Option<i32>,
    useful_life_months: Option<i32>,
    purchase_date: Option<chrono::NaiveDate>,
    location: Option<String>,
    classification: Option<String>,
    brand: Option<String>,
    model: Option<String>,
    owner_unit: Option<String>,
    responsible_user_id: Option<uuid::Uuid>,
    correlation_id: Option<String>,
) -> Result<Asset, AppError> {
    let encrypted_cost = enc.encrypt(&procurement_cost_cents.to_string())?;
    let now = Utc::now();
    let pool_c = pool.clone();

    actix_web::web::block(move || -> Result<Asset, AppError> {
        let mut conn = pool_c.get()?;
        let asset = repository::create_asset(
            &mut conn,
            NewAsset {
                id: Uuid::new_v4(),
                asset_code,
                name,
                description,
                status,
                procurement_cost: encrypted_cost,
                depreciation_method,
                useful_life_years,
                useful_life_months,
                purchase_date,
                location,
                classification,
                brand,
                model,
                owner_unit,
                responsible_user_id,
                version: 0,
                created_at: now,
                updated_at: now,
            },
        )?;

        insert_audit_log(&mut conn, NewAuditLog {
            id: Uuid::new_v4(),
            correlation_id,
            actor_user_id: Some(actor_id),
            action: "asset_created".to_string(),
            entity_type: "asset".to_string(),
            entity_id: asset.id.to_string(),
            old_value: None,
            new_value: Some(serde_json::json!({
                "asset_code": asset.asset_code,
                "name": asset.name,
            })),
            metadata: None,
            created_at: now,
            row_hash: String::new(),
            previous_hash: None,
        })?;

        Ok(asset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn update_asset(
    pool: &DbPool,
    enc: &EncryptionKey,
    asset_id: Uuid,
    actor_id: Uuid,
    name: Option<String>,
    description: Option<String>,
    status: Option<AssetStatus>,
    procurement_cost_cents: Option<i64>,
    location: Option<String>,
    useful_life_years: Option<i32>,
    useful_life_months: Option<i32>,
    purchase_date: Option<chrono::NaiveDate>,
    classification: Option<String>,
    brand: Option<String>,
    model: Option<String>,
    owner_unit: Option<String>,
    responsible_user_id: Option<uuid::Uuid>,
    expected_version: i32,
    correlation_id: Option<String>,
) -> Result<Asset, AppError> {
    // Encrypt cost if provided
    let encrypted_cost = procurement_cost_cents
        .map(|c| enc.encrypt(&c.to_string()))
        .transpose()?;

    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<Asset, AppError> {
        let mut conn = pool_c.get()?;
        let version_entry = repository::update_asset(
            &mut conn,
            asset_id,
            name,
            description,
            status,
            encrypted_cost,
            location,
            useful_life_years,
            useful_life_months,
            purchase_date,
            classification,
            brand,
            model,
            owner_unit,
            responsible_user_id,
            expected_version,
        )?;

        // Set the created_by on the version snapshot we just stored
        // (The snapshot is stored inside update_asset, actor_id is applied separately)
        diesel::update(
            crate::schema::asset_versions::table
                .filter(crate::schema::asset_versions::asset_id.eq(asset_id))
                .filter(crate::schema::asset_versions::version_no.eq(expected_version + 1)),
        )
        .set(crate::schema::asset_versions::created_by.eq(Some(actor_id)))
        .execute(&mut conn)
        .map_err(AppError::from)?;

        insert_audit_log(&mut conn, NewAuditLog {
            id: Uuid::new_v4(),
            correlation_id,
            actor_user_id: Some(actor_id),
            action: "asset_updated".to_string(),
            entity_type: "asset".to_string(),
            entity_id: asset_id.to_string(),
            old_value: None, // pre-edit snapshot is preserved in asset_versions
            new_value: Some(serde_json::json!({ "version": expected_version + 1 })),
            metadata: None,
            created_at: Utc::now(),
            row_hash: String::new(),
            previous_hash: None,
        })?;

        Ok(version_entry)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Decrypt cost for authorized roles, mask for others.
pub fn mask_or_decrypt_cost(asset: &Asset, enc: &EncryptionKey, show_full: bool) -> String {
    if show_full {
        enc.decrypt(&asset.procurement_cost)
            .unwrap_or_else(|_| "***".into())
    } else {
        EncryptionKey::mask(&asset.procurement_cost, 0)
    }
}

pub async fn get_asset(pool: &DbPool, asset_id: Uuid) -> Result<Asset, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<Asset, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_asset(&mut conn, asset_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn list_assets(
    pool: &DbPool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Asset>, i64), AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::list_assets(&mut conn, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn list_versions(pool: &DbPool, asset_id: Uuid) -> Result<Vec<AssetVersion>, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<Vec<AssetVersion>, AppError> {
        let mut conn = pool_c.get()?;
        repository::list_versions(&mut conn, asset_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn get_version(
    pool: &DbPool,
    asset_id: Uuid,
    version_no: i32,
) -> Result<AssetVersion, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<AssetVersion, AppError> {
        let mut conn = pool_c.get()?;
        repository::get_version(&mut conn, asset_id, version_no)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn add_attachment(
    pool: &DbPool,
    attachments_dir: String,
    max_upload_bytes: usize,
    asset_id: Uuid,
    actor_id: Uuid,
    file_name: String,
    mime_type: String,
    file_bytes: Vec<u8>,
) -> Result<AssetAttachment, AppError> {
    // Validate MIME type (images and PDF only)
    let allowed_mimes = [
        "image/jpeg", "image/png", "image/gif", "image/webp",
        "application/pdf",
    ];
    if !allowed_mimes.contains(&mime_type.as_str()) {
        return Err(AppError::UnprocessableEntity(format!(
            "MIME type '{}' not allowed for attachments",
            mime_type
        )));
    }

    // Upload size limit: driven by `cfg.storage.max_upload_bytes` so all upload
    // surfaces (reconciliation imports, asset attachments) share one operator
    // knob instead of drifting via per-module hardcoded constants.
    if file_bytes.len() > max_upload_bytes {
        return Err(AppError::UnprocessableEntity(format!(
            "File exceeds {} byte upload limit",
            max_upload_bytes
        )));
    }

    // Sanitize: use UUID-based stored name, never use user-provided name as path
    let extension = file_name.rsplit('.').next().unwrap_or("bin");
    let stored_name = format!("{}.{}", Uuid::new_v4(), extension);

    let pool_c = pool.clone();
    let size_bytes = file_bytes.len() as i64;
    let now = Utc::now();
    let stored_name_c = stored_name.clone();

    actix_web::web::block(move || -> Result<AssetAttachment, AppError> {
        // Ensure the attachments directory exists and write the file
        std::fs::create_dir_all(&attachments_dir)
            .map_err(|e| AppError::Internal(format!("Failed to create attachments dir: {}", e)))?;
        let dest = std::path::Path::new(&attachments_dir).join(&stored_name_c);
        std::fs::write(&dest, &file_bytes)
            .map_err(|e| AppError::Internal(format!("Failed to write attachment file: {}", e)))?;

        let mut conn = pool_c.get()?;
        repository::create_attachment(
            &mut conn,
            NewAssetAttachment {
                id: Uuid::new_v4(),
                asset_id,
                file_name,
                stored_name: stored_name_c,
                mime_type,
                size_bytes,
                uploaded_by: actor_id,
                created_at: now,
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
