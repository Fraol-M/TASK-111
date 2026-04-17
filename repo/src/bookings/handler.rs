use actix_web::{web, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::bookings::{dto::*, repository, service};
use crate::bookings::model::BookingState;
use crate::bookings::service::BookingItemInput;
use crate::common::{
    db::DbPool,
    errors::AppError,
    extractors::{AuthUser, OperationsUser},
    idempotency,
    pagination::{Page, PaginationParams},
    validation::validate_dto,
};
use crate::config::AppConfig;

/// Extract, validate, and check an Idempotency-Key header.
/// Returns `Ok(Some(response))` for a replay, `Ok(None)` to proceed, or `Err` on invalid key.
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

    let req_hash = idempotency::hash_request(req.method().as_str(), req.path(), body_bytes);

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
        let _ = actix_web::web::block(move || {
            let mut conn = pool_c.get()?;
            idempotency::store_response(&mut conn, &key, status, &body_str)
        })
        .await;
    }
}

/// POST /bookings  (Member — creates booking)
pub async fn create_booking(
    req: HttpRequest,
    auth: AuthUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    body: web::Json<CreateBookingRequest>,
) -> Result<HttpResponse, AppError> {
    use crate::users::model::UserRole;

    // Only Members may create bookings
    let role: UserRole = serde_json::from_value(serde_json::Value::String(auth.0.role.clone()))
        .unwrap_or(UserRole::Member);
    if !matches!(role, UserRole::Member) {
        return Err(AppError::Forbidden("Only members can create bookings".into()));
    }

    validate_dto(&*body)?;

    if body.start_at >= body.end_at {
        return Err(AppError::UnprocessableEntity("start_at must be before end_at".into()));
    }

    let body_bytes = serde_json::to_vec(&body.0).unwrap_or_default();
    if let Some(replay) = check_idempotency(&req, &pool, &body_bytes).await? {
        return Ok(replay);
    }

    let member_id = auth.0.sub;
    let items: Vec<BookingItemInput> = body
        .items
        .iter()
        .map(|i| BookingItemInput {
            inventory_item_id: i.inventory_item_id,
            quantity: i.quantity,
            unit_price_cents: i.unit_price_cents,
        })
        .collect();

    let booking = service::create_booking(
        &pool,
        &cfg,
        member_id,
        body.start_at,
        body.end_at,
        body.pickup_point_id,
        body.zone_id,
        items,
    )
    .await?;

    let resp = BookingResponse::from(booking);
    let resp_str = serde_json::to_string(&resp).unwrap_or_default();
    store_idempotency_response(&pool, &req, 201, &resp_str).await;
    Ok(HttpResponse::Created().json(resp))
}

/// GET /bookings  (Member=own; Admin/Ops=all)
pub async fn list_bookings(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    use crate::users::model::UserRole;

    let role: UserRole = serde_json::from_value(serde_json::Value::String(auth.0.role.clone()))
        .unwrap_or(UserRole::Member);
    let user_id = auth.0.sub;

    let limit = query.limit();
    let offset = query.offset();

    let pool_c = pool.clone();
    let (records, total) = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        match role {
            UserRole::Administrator | UserRole::OperationsManager => {
                repository::list_all_bookings(&mut conn, limit, offset)
            }
            _ => repository::list_bookings_for_member(&mut conn, user_id, limit, offset),
        }
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<BookingResponse> = records.into_iter().map(BookingResponse::from).collect();
    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

/// GET /bookings/{id}
pub async fn get_booking(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    use crate::users::model::UserRole;

    let booking_id = path.into_inner();
    let claims = auth.0;

    let pool_c = pool.clone();
    let booking = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_booking(&mut conn, booking_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let role: UserRole = serde_json::from_value(serde_json::Value::String(claims.role.clone()))
        .unwrap_or(UserRole::Member);

    let is_privileged = matches!(
        role,
        UserRole::Administrator | UserRole::OperationsManager | UserRole::Finance
    );

    if !is_privileged && booking.member_id != claims.sub {
        return Err(AppError::Forbidden("Access denied".into()));
    }

    Ok(HttpResponse::Ok().json(BookingResponse::from(booking)))
}

/// PATCH /bookings/{id}/confirm  (Member or Ops)
pub async fn confirm_booking(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    use crate::users::model::UserRole;

    let booking_id = path.into_inner();
    let claims = auth.0;

    // Verify ownership or privilege
    let pool_c = pool.clone();
    let booking_check = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_booking(&mut conn, booking_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let role: UserRole = serde_json::from_value(serde_json::Value::String(claims.role.clone()))
        .unwrap_or(UserRole::Member);

    let is_privileged = matches!(role, UserRole::Administrator | UserRole::OperationsManager);
    if !is_privileged && booking_check.member_id != claims.sub {
        return Err(AppError::Forbidden("Access denied".into()));
    }

    let booking = service::confirm_booking(&pool, &cfg, booking_id, claims.sub).await?;
    Ok(HttpResponse::Ok().json(BookingResponse::from(booking)))
}

/// PATCH /bookings/{id}/cancel
pub async fn cancel_booking(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    path: web::Path<Uuid>,
    body: web::Json<CancelBookingRequest>,
) -> Result<HttpResponse, AppError> {
    use crate::users::model::UserRole;

    let booking_id = path.into_inner();
    let claims = auth.0;

    // Verify ownership or privilege
    let pool_c = pool.clone();
    let booking_check = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_booking(&mut conn, booking_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let role: UserRole = serde_json::from_value(serde_json::Value::String(claims.role.clone()))
        .unwrap_or(UserRole::Member);

    let is_privileged = matches!(role, UserRole::Administrator | UserRole::OperationsManager);
    if !is_privileged && booking_check.member_id != claims.sub {
        return Err(AppError::Forbidden("Access denied".into()));
    }

    let booking = service::cancel_booking(&pool, &cfg, booking_id, body.reason.clone(), claims.sub).await?;
    Ok(HttpResponse::Ok().json(BookingResponse::from(booking)))
}

/// PATCH /bookings/{id}/change  (Member or Ops)
pub async fn change_booking(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    path: web::Path<Uuid>,
    body: web::Json<ChangeBookingRequest>,
) -> Result<HttpResponse, AppError> {
    use crate::users::model::UserRole;

    validate_dto(&*body)?;

    let booking_id = path.into_inner();
    let claims = auth.0;

    // Verify ownership or privilege and check current state
    let pool_c = pool.clone();
    let booking_check = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_booking(&mut conn, booking_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let role: UserRole = serde_json::from_value(serde_json::Value::String(claims.role.clone()))
        .unwrap_or(UserRole::Member);

    let is_privileged = matches!(role, UserRole::Administrator | UserRole::OperationsManager);
    if !is_privileged && booking_check.member_id != claims.sub {
        return Err(AppError::Forbidden("Access denied".into()));
    }

    // Only Confirmed or Changed states can be changed
    if !matches!(booking_check.state, BookingState::Confirmed | BookingState::Changed) {
        return Err(AppError::PreconditionFailed(format!(
            "Cannot change booking in {:?} state",
            booking_check.state
        )));
    }

    let items: Vec<BookingItemInput> = body
        .items
        .iter()
        .map(|i| BookingItemInput {
            inventory_item_id: i.inventory_item_id,
            quantity: i.quantity,
            unit_price_cents: i.unit_price_cents,
        })
        .collect();

    let booking = service::change_booking(
        &pool,
        &cfg,
        booking_id,
        claims.sub,
        items,
        body.start_at,
        body.end_at,
        body.pickup_point_id,
        body.zone_id,
        body.reason.clone(),
    )
    .await?;

    Ok(HttpResponse::Ok().json(BookingResponse::from(booking)))
}

/// PATCH /bookings/{id}/complete  (Ops/Admin)
pub async fn complete_booking(
    _auth: OperationsUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    path: web::Path<Uuid>,
    body: Option<web::Json<CompleteBookingRequest>>,
) -> Result<HttpResponse, AppError> {
    let booking_id = path.into_inner();
    let actor_id = _auth.0.sub;
    let reason = body.as_ref().and_then(|b| b.reason.clone());

    let booking = service::complete_booking(&pool, &cfg, booking_id, reason, actor_id).await?;
    Ok(HttpResponse::Ok().json(BookingResponse::from(booking)))
}

/// PATCH /bookings/{id}/exception  (Ops/Admin)
pub async fn flag_exception(
    _auth: OperationsUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    path: web::Path<Uuid>,
    body: web::Json<ExceptionRequest>,
) -> Result<HttpResponse, AppError> {
    let booking_id = path.into_inner();
    let actor_id = _auth.0.sub;

    let booking = service::flag_exception(&pool, &cfg, booking_id, body.reason.clone(), actor_id).await?;
    Ok(HttpResponse::Ok().json(BookingResponse::from(booking)))
}

/// GET /bookings/{id}/items
pub async fn list_booking_items(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    use crate::users::model::UserRole;

    let booking_id = path.into_inner();
    let claims = auth.0;

    let pool_c = pool.clone();
    let booking = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_booking(&mut conn, booking_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let role: UserRole = serde_json::from_value(serde_json::Value::String(claims.role.clone()))
        .unwrap_or(UserRole::Member);

    let is_privileged = matches!(
        role,
        UserRole::Administrator | UserRole::OperationsManager | UserRole::Finance
    );

    if !is_privileged && booking.member_id != claims.sub {
        return Err(AppError::Forbidden("Access denied".into()));
    }

    let pool_c2 = pool.clone();
    let items = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c2.get()?;
        repository::list_booking_items(&mut conn, booking_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<BookingItemResponse> = items.into_iter().map(BookingItemResponse::from).collect();
    Ok(HttpResponse::Ok().json(data))
}

/// GET /bookings/{id}/history  (Admin/Ops)
pub async fn get_booking_history(
    _auth: OperationsUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let booking_id = path.into_inner();

    let pool_c = pool.clone();
    let history = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::get_status_history(&mut conn, booking_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<BookingHistoryResponse> = history.into_iter().map(BookingHistoryResponse::from).collect();
    Ok(HttpResponse::Ok().json(data))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/bookings")
            .route("", web::post().to(create_booking))
            .route("", web::get().to(list_bookings))
            .route("/{id}", web::get().to(get_booking))
            .route("/{id}/confirm", web::patch().to(confirm_booking))
            .route("/{id}/cancel", web::patch().to(cancel_booking))
            .route("/{id}/change", web::patch().to(change_booking))
            .route("/{id}/complete", web::patch().to(complete_booking))
            .route("/{id}/exception", web::patch().to(flag_exception))
            .route("/{id}/items", web::get().to(list_booking_items))
            .route("/{id}/history", web::get().to(get_booking_history)),
    );
}
