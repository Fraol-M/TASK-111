use actix_web::{web, HttpResponse};
use chrono::Utc;
use uuid::Uuid;

use crate::common::{
    db::DbPool, errors::AppError, extractors::AuthUser,
    pagination::{Page, PaginationParams}, validation::validate_dto,
};
use crate::inventory::{
    dto::*,
    model::{NewDeliveryZone, NewInventoryItem, NewPickupPoint, PublishStatus},
    repository, service,
};
use crate::users::policy;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/inventory")
            // Pickup-point management (admin/operations). Registered BEFORE the
            // `/{id}` catch-all so "/pickup-points" is not matched as an item id.
            .route("/pickup-points", web::post().to(create_pickup_point))
            .route("/pickup-points", web::get().to(list_pickup_points))
            .route("/pickup-points/{id}", web::get().to(get_pickup_point))
            .route("/pickup-points/{id}", web::patch().to(update_pickup_point))
            .route("/pickup-points/{id}", web::delete().to(delete_pickup_point))
            // Delivery-zone management (admin/operations)
            .route("/zones", web::post().to(create_zone))
            .route("/zones", web::get().to(list_zones))
            .route("/zones/{id}", web::get().to(get_zone))
            .route("/zones/{id}", web::patch().to(update_zone))
            .route("/zones/{id}", web::delete().to(delete_zone))
            // Alerts — also before `/{id}` to avoid "alerts" being parsed as Uuid
            .route("/alerts", web::get().to(list_alerts))
            .route("/alerts/{id}/ack", web::patch().to(ack_alert))
            // Item CRUD
            .route("", web::post().to(create_item))
            .route("", web::get().to(list_items))
            .route("/{id}", web::get().to(get_item))
            .route("/{id}", web::patch().to(update_item))
            .route("/{id}/restock", web::post().to(restock_item)),
    );
}

async fn create_item(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    body: web::Json<CreateInventoryItemRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    validate_dto(&body.0)?;
    let item = NewInventoryItem {
        id: Uuid::new_v4(),
        sku: body.sku.clone(),
        name: body.name.clone(),
        description: body.description.clone(),
        available_qty: body.available_qty,
        safety_stock: body.safety_stock,
        publish_status: if body.available_qty > 0 { PublishStatus::Published } else { PublishStatus::Unpublished },
        pickup_point_id: body.pickup_point_id,
        zone_id: body.zone_id,
        cutoff_hours: body.cutoff_hours.unwrap_or(2),
        version: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let mut conn = pool.get()?;
    let created = actix_web::web::block(move || repository::create_item(&mut conn, item))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Created().json(created))
}

async fn list_items(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let _ = auth; // Any authenticated user
    let limit = pagination.limit();
    let offset = pagination.offset();
    let mut conn = pool.get()?;
    let (items, total) = actix_web::web::block(move || {
        repository::list_items(&mut conn, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(Page::new(items, total, &pagination)))
}

async fn get_item(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let _ = auth;
    let mut conn = pool.get()?;
    let item = actix_web::web::block(move || repository::find_item(&mut conn, path.into_inner()))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(item))
}

async fn update_item(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateInventoryItemRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    validate_dto(&body.0)?;
    let item_id = path.into_inner();
    let actor_id = auth.0.sub;

    let updated = service::update_item(
        &pool,
        item_id,
        body.name.clone(),
        body.description.clone(),
        body.available_qty,
        body.safety_stock,
        body.cutoff_hours,
        body.version,
        Some(actor_id),
    )
    .await?;
    Ok(HttpResponse::Ok().json(updated))
}

async fn restock_item(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<RestockRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    validate_dto(&body.0)?;
    let item_id = path.into_inner();
    let actor_id = auth.0.sub;
    let updated = service::restock_item(&pool, item_id, body.quantity, actor_id).await?;
    Ok(HttpResponse::Ok().json(updated))
}

async fn list_alerts(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    let limit = pagination.limit();
    let offset = pagination.offset();
    let mut conn = pool.get()?;
    let (alerts, total) = actix_web::web::block(move || {
        repository::list_open_alerts(&mut conn, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(Page::new(alerts, total, &pagination)))
}

async fn ack_alert(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    let alert_id = path.into_inner();
    let actor_id = auth.0.sub;
    let mut conn = pool.get()?;
    actix_web::web::block(move || repository::acknowledge_alert(&mut conn, alert_id, actor_id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Alert acknowledged" })))
}

// ─────────────────── Pickup-point management handlers ───────────────────

async fn create_pickup_point(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    body: web::Json<CreatePickupPointRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    validate_dto(&body.0)?;
    let new = NewPickupPoint {
        id: Uuid::new_v4(),
        name: body.name.clone(),
        address: body.address.clone(),
        active: body.active,
        created_at: Utc::now(),
        cutoff_hours: body.cutoff_hours,
    };
    let mut conn = pool.get()?;
    let created = actix_web::web::block(move || repository::create_pickup_point(&mut conn, new))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Created().json(created))
}

async fn list_pickup_points(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    // Readable by any authenticated caller — the list is metadata that any
    // member needs to see when choosing a pickup point at booking time.
    let _ = auth;
    let limit = pagination.limit();
    let offset = pagination.offset();
    let mut conn = pool.get()?;
    let (items, total) = actix_web::web::block(move || {
        repository::list_pickup_points(&mut conn, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(Page::new(items, total, &pagination)))
}

async fn get_pickup_point(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let _ = auth;
    let mut conn = pool.get()?;
    let item = actix_web::web::block(move || {
        repository::find_pickup_point(&mut conn, path.into_inner())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(item))
}

async fn update_pickup_point(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdatePickupPointRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    validate_dto(&body.0)?;
    let id = path.into_inner();
    let name = body.name.clone();
    let address = body.address.clone();
    let active = body.active;
    let cutoff_hours = body.cutoff_hours;
    let clear_cutoff = body.clear_cutoff;

    let mut conn = pool.get()?;
    let updated = actix_web::web::block(move || {
        repository::update_pickup_point(
            &mut conn,
            id,
            name,
            address,
            active,
            cutoff_hours,
            clear_cutoff,
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(updated))
}

async fn delete_pickup_point(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    let id = path.into_inner();
    let mut conn = pool.get()?;
    actix_web::web::block(move || repository::delete_pickup_point(&mut conn, id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::NoContent().finish())
}

// ─────────────────── Delivery-zone management handlers ───────────────────

async fn create_zone(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    body: web::Json<CreateDeliveryZoneRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    validate_dto(&body.0)?;
    let new = NewDeliveryZone {
        id: Uuid::new_v4(),
        name: body.name.clone(),
        description: body.description.clone(),
        active: body.active,
        created_at: Utc::now(),
        cutoff_hours: body.cutoff_hours,
    };
    let mut conn = pool.get()?;
    let created = actix_web::web::block(move || repository::create_zone(&mut conn, new))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Created().json(created))
}

async fn list_zones(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let _ = auth;
    let limit = pagination.limit();
    let offset = pagination.offset();
    let mut conn = pool.get()?;
    let (items, total) = actix_web::web::block(move || {
        repository::list_zones(&mut conn, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(Page::new(items, total, &pagination)))
}

async fn get_zone(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let _ = auth;
    let mut conn = pool.get()?;
    let item = actix_web::web::block(move || {
        repository::find_zone(&mut conn, path.into_inner())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(item))
}

async fn update_zone(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateDeliveryZoneRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    validate_dto(&body.0)?;
    let id = path.into_inner();
    let name = body.name.clone();
    let description = body.description.clone();
    let active = body.active;
    let cutoff_hours = body.cutoff_hours;
    let clear_cutoff = body.clear_cutoff;

    let mut conn = pool.get()?;
    let updated = actix_web::web::block(move || {
        repository::update_zone(
            &mut conn,
            id,
            name,
            description,
            active,
            cutoff_hours,
            clear_cutoff,
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::Ok().json(updated))
}

async fn delete_zone(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::OPS)?;
    let id = path.into_inner();
    let mut conn = pool.get()?;
    actix_web::web::block(move || repository::delete_zone(&mut conn, id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(HttpResponse::NoContent().finish())
}
