use actix_web::{web, HttpResponse};
use uuid::Uuid;

use crate::common::{
    db::DbPool,
    errors::AppError,
    extractors::AuthUser,
    pagination::{Page, PaginationParams},
    validation::validate_dto,
};
use crate::users::{
    dto::{ChangePasswordRequest, ChangeStatusRequest, CreateUserRequest, UpdateUserRequest},
    policy,
    service,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/users")
            .route("", web::post().to(create_user))
            .route("", web::get().to(list_users))
            .route("/{id}", web::get().to(get_user))
            .route("/{id}", web::patch().to(update_user))
            .route("/{id}/status", web::patch().to(change_status))
            .route("/{id}/password", web::post().to(change_password)),
    );
}

async fn create_user(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    body: web::Json<CreateUserRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::ADMIN)?;
    validate_dto(&body.0)?;
    let user = service::create_user(&pool, body.into_inner()).await?;
    Ok(HttpResponse::Created().json(user))
}

async fn list_users(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::ADMIN)?;
    let (users, total) = service::list_users(&pool, pagination.limit(), pagination.offset()).await?;
    Ok(HttpResponse::Ok().json(Page::new(users, total, &pagination)))
}

async fn get_user(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    policy::require_self_or_role(&auth.0, target_id, policy::ADMIN)?;
    let user = service::get_user(&pool, target_id).await?;
    Ok(HttpResponse::Ok().json(user))
}

async fn update_user(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateUserRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::ADMIN)?;
    validate_dto(&body.0)?;
    let user = service::update_user(&pool, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(user))
}

async fn change_status(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<ChangeStatusRequest>,
) -> Result<HttpResponse, AppError> {
    policy::require_role(&auth.0, policy::ADMIN)?;
    let user = service::change_status(&pool, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(user))
}

async fn change_password(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<ChangePasswordRequest>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    policy::require_self_or_role(&auth.0, target_id, policy::ADMIN)?;
    validate_dto(&body.0)?;
    service::change_password(&pool, target_id, body.into_inner(), &auth.0.role).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Password changed successfully" })))
}
