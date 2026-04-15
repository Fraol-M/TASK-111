use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::assets::model::{
    Asset, AssetAttachment, AssetVersion, NewAsset, NewAssetAttachment, NewAssetVersion,
};
use crate::common::{db::DbConn, errors::AppError};
use crate::schema::{asset_attachments, asset_versions, assets};

pub fn create_asset(conn: &mut DbConn, asset: NewAsset) -> Result<Asset, AppError> {
    diesel::insert_into(assets::table)
        .values(&asset)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_asset(conn: &mut DbConn, asset_id: Uuid) -> Result<Asset, AppError> {
    assets::table
        .filter(assets::id.eq(asset_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Asset {} not found", asset_id)))
}

pub fn find_asset_for_update(conn: &mut DbConn, asset_id: Uuid, expected_version: i32) -> Result<Asset, AppError> {
    assets::table
        .filter(assets::id.eq(asset_id))
        .filter(assets::version.eq(expected_version))
        .for_update()
        .first(conn)
        .map_err(|_| AppError::PreconditionFailed("Asset not found at expected version (concurrent modification?)".into()))
}

pub fn list_assets(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Asset>, i64), AppError> {
    let total: i64 = assets::table.count().get_result(conn).map_err(AppError::from)?;
    let records = assets::table
        .order(assets::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((records, total))
}

pub fn update_asset(
    conn: &mut DbConn,
    asset_id: Uuid,
    name: Option<String>,
    description: Option<String>,
    status: Option<crate::assets::model::AssetStatus>,
    procurement_cost: Option<String>,
    location: Option<String>,
    useful_life_years: Option<i32>,
    useful_life_months: Option<i32>,
    purchase_date: Option<chrono::NaiveDate>,
    classification: Option<String>,
    brand: Option<String>,
    model: Option<String>,
    owner_unit: Option<String>,
    responsible_user_id: Option<Uuid>,
    expected_version: i32,
) -> Result<Asset, AppError> {
    // Take snapshot BEFORE update for tamper-evident versioning
    let existing = find_asset_for_update(conn, asset_id, expected_version)?;
    let snapshot = serde_json::to_value(&existing).unwrap_or(serde_json::Value::Null);

    // Store version snapshot
    diesel::insert_into(asset_versions::table)
        .values(&NewAssetVersion {
            id: Uuid::new_v4(),
            asset_id,
            version_no: expected_version + 1,
            snapshot_json: snapshot,
            created_by: None,
            created_at: Utc::now(),
        })
        .execute(conn)
        .map_err(AppError::from)?;

    // Update the asset
    let rows = diesel::update(
        assets::table
            .filter(assets::id.eq(asset_id))
            .filter(assets::version.eq(expected_version)),
    )
    .set((
        assets::name.eq(name.unwrap_or(existing.name)),
        assets::description.eq(description.or(existing.description)),
        assets::status.eq(status.unwrap_or(existing.status)),
        assets::procurement_cost.eq(procurement_cost.unwrap_or(existing.procurement_cost)),
        assets::location.eq(location.or(existing.location)),
        assets::useful_life_years.eq(useful_life_years.or(existing.useful_life_years)),
        assets::useful_life_months.eq(useful_life_months.or(existing.useful_life_months)),
        assets::purchase_date.eq(purchase_date.or(existing.purchase_date)),
        assets::classification.eq(classification.or(existing.classification)),
        assets::brand.eq(brand.or(existing.brand)),
        assets::model.eq(model.or(existing.model)),
        assets::owner_unit.eq(owner_unit.or(existing.owner_unit)),
        assets::responsible_user_id.eq(responsible_user_id.or(existing.responsible_user_id)),
        assets::version.eq(expected_version + 1),
        assets::updated_at.eq(Utc::now()),
    ))
    .execute(conn)
    .map_err(AppError::from)?;

    if rows == 0 {
        return Err(AppError::PreconditionFailed("Concurrent modification on asset".into()));
    }

    find_asset(conn, asset_id)
}

pub fn list_versions(conn: &mut DbConn, asset_id: Uuid) -> Result<Vec<AssetVersion>, AppError> {
    asset_versions::table
        .filter(asset_versions::asset_id.eq(asset_id))
        .order(asset_versions::version_no.asc())
        .load(conn)
        .map_err(AppError::from)
}

pub fn get_version(
    conn: &mut DbConn,
    asset_id: Uuid,
    version_no: i32,
) -> Result<AssetVersion, AppError> {
    asset_versions::table
        .filter(asset_versions::asset_id.eq(asset_id))
        .filter(asset_versions::version_no.eq(version_no))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Version {} for asset {} not found", version_no, asset_id)))
}

pub fn create_attachment(
    conn: &mut DbConn,
    attachment: NewAssetAttachment,
) -> Result<AssetAttachment, AppError> {
    diesel::insert_into(asset_attachments::table)
        .values(&attachment)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn list_attachments(
    conn: &mut DbConn,
    asset_id: Uuid,
) -> Result<Vec<AssetAttachment>, AppError> {
    asset_attachments::table
        .filter(asset_attachments::asset_id.eq(asset_id))
        .order(asset_attachments::created_at.desc())
        .load(conn)
        .map_err(AppError::from)
}
