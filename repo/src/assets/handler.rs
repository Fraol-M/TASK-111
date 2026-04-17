use actix_web::{web, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::assets::{dto::*, service};
use crate::common::{
    crypto::EncryptionKey,
    db::DbPool,
    errors::AppError,
    extractors::{AssetManagerUser, AuthUser},
    pagination::{Page, PaginationParams},
    validation::validate_dto,
};
use crate::config::AppConfig;
use crate::users::model::UserRole;

fn can_see_cost(role: &UserRole) -> bool {
    matches!(role, UserRole::Administrator | UserRole::Finance | UserRole::AssetManager)
}

/// POST /assets  (AssetManager/Admin)
pub async fn create_asset(
    req: HttpRequest,
    auth: AssetManagerUser,
    pool: web::Data<DbPool>,
    enc: web::Data<EncryptionKey>,
    body: web::Json<CreateAssetRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    let asset = service::create_asset(
        &pool,
        &enc,
        auth.0.sub,
        body.asset_code.clone(),
        body.name.clone(),
        body.description.clone(),
        body.status.clone(),
        body.procurement_cost_cents,
        body.depreciation_method.clone(),
        body.useful_life_years,
        body.useful_life_months,
        body.purchase_date,
        body.location.clone(),
        body.classification.clone(),
        body.brand.clone(),
        body.model.clone(),
        body.owner_unit.clone(),
        body.responsible_user_id,
        correlation_id,
    )
    .await?;

    let role: UserRole = serde_json::from_value(serde_json::Value::String(auth.0.role.clone()))
        .unwrap_or(UserRole::AssetManager);
    let cost_display = service::mask_or_decrypt_cost(&asset, &enc, can_see_cost(&role));
    Ok(HttpResponse::Created().json(AssetResponse::from_asset(asset, cost_display)))
}

/// GET /assets  (all authenticated — cost masked per role)
pub async fn list_assets(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    enc: web::Data<EncryptionKey>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let role: UserRole = serde_json::from_value(serde_json::Value::String(auth.0.role.clone()))
        .unwrap_or(UserRole::Member);
    let show_cost = can_see_cost(&role);

    let (records, total) = service::list_assets(&pool, query.limit(), query.offset()).await?;

    let data: Vec<AssetResponse> = records
        .into_iter()
        .map(|a| {
            let cost = service::mask_or_decrypt_cost(&a, &enc, show_cost);
            AssetResponse::from_asset(a, cost)
        })
        .collect();

    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

/// GET /assets/{id}  (all authenticated — cost masked per role)
pub async fn get_asset(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    enc: web::Data<EncryptionKey>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let asset_id = path.into_inner();
    let role: UserRole = serde_json::from_value(serde_json::Value::String(auth.0.role.clone()))
        .unwrap_or(UserRole::Member);

    let asset = service::get_asset(&pool, asset_id).await?;
    let cost = service::mask_or_decrypt_cost(&asset, &enc, can_see_cost(&role));
    Ok(HttpResponse::Ok().json(AssetResponse::from_asset(asset, cost)))
}

/// PATCH /assets/{id}  (AssetManager/Admin — optimistic concurrency)
pub async fn update_asset(
    req: HttpRequest,
    auth: AssetManagerUser,
    pool: web::Data<DbPool>,
    enc: web::Data<EncryptionKey>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAssetRequest>,
) -> Result<HttpResponse, AppError> {
    let asset_id = path.into_inner();
    let actor_id = auth.0.sub;
    let correlation_id = crate::common::correlation::get_correlation_id(&req);

    let asset = service::update_asset(
        &pool,
        &enc,
        asset_id,
        actor_id,
        body.name.clone(),
        body.description.clone(),
        body.status.clone(),
        body.procurement_cost_cents,
        body.location.clone(),
        body.useful_life_years,
        body.useful_life_months,
        body.purchase_date,
        body.classification.clone(),
        body.brand.clone(),
        body.model.clone(),
        body.owner_unit.clone(),
        body.responsible_user_id,
        body.expected_version,
        correlation_id,
    )
    .await?;

    let role: UserRole = serde_json::from_value(serde_json::Value::String(auth.0.role.clone()))
        .unwrap_or(UserRole::AssetManager);
    let cost = service::mask_or_decrypt_cost(&asset, &enc, can_see_cost(&role));
    Ok(HttpResponse::Ok().json(AssetResponse::from_asset(asset, cost)))
}

/// GET /assets/{id}/versions  (AssetManager/Admin)
pub async fn list_versions(
    _auth: AssetManagerUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let asset_id = path.into_inner();
    let versions = service::list_versions(&pool, asset_id).await?;
    let data: Vec<AssetVersionResponse> = versions.into_iter().map(AssetVersionResponse::from).collect();
    Ok(HttpResponse::Ok().json(data))
}

/// GET /assets/{id}/versions/{v}  (AssetManager/Admin)
pub async fn get_version(
    _auth: AssetManagerUser,
    pool: web::Data<DbPool>,
    path: web::Path<(Uuid, i32)>,
) -> Result<HttpResponse, AppError> {
    let (asset_id, version_no) = path.into_inner();
    let version = service::get_version(&pool, asset_id, version_no).await?;
    Ok(HttpResponse::Ok().json(AssetVersionResponse::from(version)))
}

/// POST /assets/{id}/attachments  (AssetManager/Admin — multipart)
pub async fn upload_attachment(
    auth: AssetManagerUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    _enc: web::Data<EncryptionKey>,
    path: web::Path<Uuid>,
    mut payload: actix_multipart::Multipart,
) -> Result<HttpResponse, AppError> {
    use actix_multipart::Field;
    use futures_util::StreamExt;

    let asset_id = path.into_inner();
    let actor_id = auth.0.sub;

    // Verify asset exists
    service::get_asset(&pool, asset_id).await?;

    let mut file_name = String::from("attachment");
    let mut mime_type = String::from("application/octet-stream");
    let mut file_bytes: Vec<u8> = Vec::new();

    while let Some(field_result) = payload.next().await {
        let mut field: Field = field_result.map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

        let content_disposition = field.content_disposition().clone();
        if let Some(name) = content_disposition.get_filename() {
            file_name = name.to_string();
        }
        mime_type = field.content_type().map(|m| m.to_string()).unwrap_or_else(|| "application/octet-stream".to_string());

        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;
            file_bytes.extend_from_slice(&data);
        }
    }

    let attachment = service::add_attachment(
        &pool,
        cfg.storage.attachments_dir.clone(),
        cfg.storage.max_upload_bytes,
        asset_id,
        actor_id,
        file_name,
        mime_type,
        file_bytes,
    ).await?;
    Ok(HttpResponse::Created().json(AssetAttachmentResponse::from(attachment)))
}

/// GET /assets/{id}/attachments  (AssetManager/Admin)
pub async fn list_attachments(
    _auth: AssetManagerUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let asset_id = path.into_inner();

    let pool_c = pool.clone();
    let attachments = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        crate::assets::repository::list_attachments(&mut conn, asset_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<AssetAttachmentResponse> = attachments.into_iter().map(AssetAttachmentResponse::from).collect();
    Ok(HttpResponse::Ok().json(data))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/assets")
            .route("", web::post().to(create_asset))
            .route("", web::get().to(list_assets))
            .route("/{id}", web::get().to(get_asset))
            .route("/{id}", web::patch().to(update_asset))
            .route("/{id}/versions", web::get().to(list_versions))
            .route("/{id}/versions/{v}", web::get().to(get_version))
            .route("/{id}/attachments", web::post().to(upload_attachment))
            .route("/{id}/attachments", web::get().to(list_attachments)),
    );
}
