use actix_web::{web, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::common::{
    crypto::EncryptionKey,
    db::DbPool,
    errors::AppError,
    extractors::AuthUser,
    idempotency,
    pagination::{Page, PaginationParams},
    validation::validate_dto,
};
use crate::members::{dto::*, policy, repository, service};

/// Extract, validate, and check an Idempotency-Key header.
/// Returns `Ok(Some((status, body)))` for a replay, `Ok(None)` to proceed, or `Err` on failure.
async fn check_idempotency(
    req: &HttpRequest,
    pool: &web::Data<DbPool>,
    body_bytes: &[u8],
) -> Result<Option<HttpResponse>, AppError> {
    let key = req
        .headers()
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::UnprocessableEntity("Idempotency-Key header required".into()))?
        .to_string();

    let req_hash = idempotency::hash_request(
        req.method().as_str(),
        req.path(),
        body_bytes,
    );

    let pool_c = pool.clone();
    let key_c = key.clone();
    let hash_c = req_hash.clone();

    let cached = actix_web::web::block(move || {
        let mut conn = pool_c.get()?;
        idempotency::check_or_register(&mut conn, &key_c, &hash_c)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    if let Some((status, body_str)) = cached {
        let status_code = actix_web::http::StatusCode::from_u16(status)
            .unwrap_or(actix_web::http::StatusCode::OK);
        return Ok(Some(
            HttpResponse::build(status_code)
                .content_type("application/json")
                .body(body_str),
        ));
    }

    Ok(None)
}

async fn store_idempotency_response(
    pool: &web::Data<DbPool>,
    req: &HttpRequest,
    _body_bytes: &[u8],
    status: u16,
    response_body: &str,
) {
    let key = req
        .headers()
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(key) = key {
        let pool_c = pool.clone();
        let body_str = response_body.to_string();
        // Best-effort: ignore errors — key was already registered by check_idempotency
        let _ = actix_web::web::block(move || {
            let mut conn = pool_c.get()?;
            idempotency::store_response(&mut conn, &key, status, &body_str)
        })
        .await;
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/members")
            .route("/{id}", web::get().to(get_member))
            .route("/{id}/tier", web::patch().to(force_tier))
            .route("/{id}/blacklist", web::post().to(blacklist_member))
            .route("/{id}/points", web::post().to(adjust_points))
            .route("/{id}/redeem", web::post().to(redeem_points))
            .route("/{id}/points/ledger", web::get().to(points_ledger))
            .route("/{id}/wallet/topup", web::post().to(wallet_topup))
            .route("/{id}/wallet/ledger", web::get().to(wallet_ledger))
            .route("/{id}/freeze", web::post().to(freeze_redemption))
            .route("/{id}/preferences", web::get().to(get_preferences))
            .route("/{id}/preferences", web::patch().to(update_preferences)),
    );
}

async fn get_member(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    enc: web::Data<EncryptionKey>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    policy::can_view_member(&auth.0, target_id)?;
    let info = service::get_member_info(&pool, &enc, target_id, &auth.0.role).await?;
    Ok(HttpResponse::Ok().json(info))
}

async fn adjust_points(
    req: HttpRequest,
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<AdjustPointsRequest>,
) -> Result<HttpResponse, AppError> {
    policy::can_manage_points(&auth.0)?;
    validate_dto(&body.0)?;

    let body_bytes = serde_json::to_vec(&body.0).unwrap_or_default();
    if let Some(replay) = check_idempotency(&req, &pool, &body_bytes).await? {
        return Ok(replay);
    }

    let target_id = path.into_inner();
    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    service::earn_points(&pool, target_id, body.delta, body.reference_id, Some(body.note.clone()), Some(auth.0.sub), correlation_id).await?;

    let resp_body = serde_json::json!({ "message": "Points adjusted" });
    let resp_str = serde_json::to_string(&resp_body).unwrap_or_default();
    store_idempotency_response(&pool, &req, &body_bytes, 200, &resp_str).await;
    Ok(HttpResponse::Ok().json(resp_body))
}

async fn redeem_points(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<RedeemPointsRequest>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    // Only the member can redeem their own points
    if auth.0.sub != target_id {
        return Err(AppError::Forbidden("Only the member can redeem their own points".into()));
    }
    validate_dto(&body.0)?;
    service::redeem_points(&pool, target_id, body.amount_pts, body.reference_id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Points redeemed" })))
}

async fn points_ledger(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    policy::can_view_member(&auth.0, target_id)?;
    let limit = pagination.limit();
    let offset = pagination.offset();
    let mut conn = pool.get()?;
    let (records, total) = actix_web::web::block(move || {
        repository::list_points_ledger(&mut conn, target_id, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(Page::new(records, total, &pagination)))
}

async fn wallet_topup(
    req: HttpRequest,
    auth: AuthUser,
    pool: web::Data<DbPool>,
    enc: web::Data<EncryptionKey>,
    path: web::Path<Uuid>,
    body: web::Json<WalletTopUpRequest>,
) -> Result<HttpResponse, AppError> {
    policy::can_manage_wallet(&auth.0)?;
    validate_dto(&body.0)?;

    let body_bytes = serde_json::to_vec(&body.0).unwrap_or_default();
    if let Some(replay) = check_idempotency(&req, &pool, &body_bytes).await? {
        return Ok(replay);
    }

    let target_id = path.into_inner();
    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    service::top_up_wallet(&pool, &enc, target_id, body.amount_cents, body.note.clone(), Some(auth.0.sub), correlation_id).await?;

    let resp_body = serde_json::json!({ "message": "Wallet topped up" });
    let resp_str = serde_json::to_string(&resp_body).unwrap_or_default();
    store_idempotency_response(&pool, &req, &body_bytes, 200, &resp_str).await;
    Ok(HttpResponse::Ok().json(resp_body))
}

async fn wallet_ledger(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    policy::can_view_wallet(&auth.0, target_id)?;
    let limit = pagination.limit();
    let offset = pagination.offset();
    let mut conn = pool.get()?;
    let (records, total) = actix_web::web::block(move || {
        repository::list_wallet_ledger(&mut conn, target_id, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(Page::new(records, total, &pagination)))
}

async fn freeze_redemption(
    req: HttpRequest,
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<FreezeRedemptionRequest>,
) -> Result<HttpResponse, AppError> {
    policy::can_blacklist(&auth.0)?;
    validate_dto(&body.0)?;
    let target_id = path.into_inner();
    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    service::freeze_redemption(&pool, target_id, body.reason.clone(), body.note.clone(), auth.0.sub, correlation_id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Redemption frozen" })))
}

async fn blacklist_member(
    req: HttpRequest,
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<BlacklistMemberRequest>,
) -> Result<HttpResponse, AppError> {
    policy::can_blacklist(&auth.0)?;
    let target_id = path.into_inner();
    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    service::blacklist_member(&pool, target_id, body.reason.clone(), body.note.clone(), auth.0.sub, correlation_id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Member blacklisted" })))
}

async fn force_tier(
    req: HttpRequest,
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<ForceTierRequest>,
) -> Result<HttpResponse, AppError> {
    policy::can_blacklist(&auth.0)?; // Admin-only — reuse the admin gate
    let target_id = path.into_inner();
    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    service::force_tier(&pool, target_id, body.tier.clone(), auth.0.sub, correlation_id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Tier updated" })))
}

async fn get_preferences(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    policy::can_view_preferences(&auth.0, target_id)?;
    let mut conn = pool.get()?;
    let prefs = actix_web::web::block(move || repository::get_preferences(&mut conn, target_id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(prefs))
}

async fn update_preferences(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<crate::config::AppConfig>,
    path: web::Path<Uuid>,
    body: web::Json<UpdatePreferencesRequest>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    policy::can_edit_preferences(&auth.0, target_id)?;
    validate_dto(&body.0)?;

    let opt_out = body
        .notification_opt_out
        .as_ref()
        .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Array(vec![])))
        .unwrap_or(serde_json::Value::Array(vec![]));
    let channel = body.preferred_channel.as_deref().unwrap_or("in_app").to_string();

    // Reject channel preferences for channels not enabled in this deployment
    // (e.g. picking 'email' on a profile where no SMTP provider is wired).
    // This avoids silently routing every notification through the in-app
    // fallback path and surfacing as "delivered" when the user expected email.
    if !cfg.notifications.channel_is_enabled(&channel) {
        return Err(AppError::UnprocessableEntity(format!(
            "Channel '{}' is not enabled in this deployment (enabled: {})",
            channel,
            cfg.notifications.enabled_channels().join(", ")
        )));
    }

    let tz_offset = body.timezone_offset_minutes.unwrap_or(0);

    let mut conn = pool.get()?;
    let prefs = actix_web::web::block(move || {
        repository::upsert_preferences(&mut conn, target_id, opt_out, &channel, tz_offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(prefs))
}
