use actix_web::{dev::Service, web, App, HttpResponse};
use tracing_actix_web::TracingLogger;

use crate::common::{crypto::EncryptionKey, db::DbPool};
use crate::config::AppConfig;

/// Build the Actix-web application with all routes and middleware registered.
/// Used by both the production server and integration tests.
pub fn build_app(
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    enc_key: web::Data<EncryptionKey>,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    App::new()
        .app_data(pool)
        .app_data(cfg)
        .app_data(enc_key)
        // Structured request tracing (correlation IDs injected via TracingLogger)
        .wrap(TracingLogger::default())
        // Correlation ID middleware
        .wrap(crate::common::correlation::CorrelationIdMiddleware)
        // Box response body so the return type stays ServiceResponse<BoxBody>
        .wrap_fn(|req, srv| {
            let fut = srv.call(req);
            async move { fut.await.map(|res| res.map_into_boxed_body()) }
        })
        // JSON extractor config — return 422 on bad JSON
        .app_data(
            web::JsonConfig::default()
                .error_handler(|err, _req| {
                    let msg = format!("{}", err);
                    actix_web::error::InternalError::from_response(
                        err,
                        HttpResponse::UnprocessableEntity().json(serde_json::json!({
                            "error": "invalid_json",
                            "message": msg
                        })),
                    )
                    .into()
                }),
        )
        // Health check (unauthenticated)
        .route("/health", web::get().to(health_handler))
        // API v1 routes
        .service(
            web::scope("/api/v1")
                .configure(crate::auth::handler::configure)
                .configure(crate::users::handler::configure)
                .configure(crate::members::handler::configure)
                .configure(crate::bookings::handler::configure)
                .configure(crate::inventory::handler::configure)
                .configure(crate::notifications::handler::configure)
                .configure(crate::groups::handler::configure)
                .configure(crate::assets::handler::configure)
                .configure(crate::evaluations::handler::configure)
                .configure(crate::payments::handler::configure)
                .configure(crate::reconciliation::handler::configure)
                .configure(crate::audit::handler::configure),
        )
}

async fn health_handler(pool: web::Data<DbPool>) -> HttpResponse {
    // Check DB connectivity
    match pool.get() {
        Ok(mut conn) => {
            use diesel::prelude::*;
            match diesel::sql_query("SELECT 1").execute(&mut conn) {
                Ok(_) => HttpResponse::Ok().json(serde_json::json!({
                    "status": "ok",
                    "db": "ok"
                })),
                Err(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
                    "status": "degraded",
                    "db": "error"
                })),
            }
        }
        Err(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "degraded",
            "db": "unavailable"
        })),
    }
}
