use actix_web::{web, HttpResponse};

use crate::auth::{dto::LoginRequest, service};
use crate::common::{db::DbPool, errors::AppError, extractors::AuthUser, validation::validate_dto};
use crate::config::AppConfig;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout))
            .route("/me", web::get().to(me)),
    );
}

async fn login(
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&body.0)?;
    let response = service::login(&pool, &cfg, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(response))
}

async fn logout(
    auth: AuthUser,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, AppError> {
    service::logout(&pool, auth.0.jti).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Logged out successfully" })))
}

async fn me(
    auth: AuthUser,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, AppError> {
    let response = service::me(&pool, auth.0.sub).await?;
    Ok(HttpResponse::Ok().json(response))
}
