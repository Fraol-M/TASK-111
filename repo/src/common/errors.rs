use actix_web::HttpResponse;
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("Precondition failed: {0}")]
    PreconditionFailed(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Idempotency replay")]
    #[allow(dead_code)]
    IdempotencyReplay { status: u16, body: String },
}

impl actix_web::ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::NotFound(msg) => HttpResponse::NotFound().json(json!({
                "error": "not_found",
                "message": msg
            })),
            AppError::Unauthorized => HttpResponse::Unauthorized().json(json!({
                "error": "unauthorized",
                "message": "Authentication required"
            })),
            AppError::Forbidden(msg) => HttpResponse::Forbidden().json(json!({
                "error": "forbidden",
                "message": msg
            })),
            AppError::Conflict(msg) => HttpResponse::Conflict().json(json!({
                "error": "conflict",
                "message": msg
            })),
            AppError::UnprocessableEntity(msg) => {
                HttpResponse::UnprocessableEntity().json(json!({
                    "error": "unprocessable_entity",
                    "message": msg
                }))
            }
            AppError::PreconditionFailed(msg) => {
                HttpResponse::build(actix_web::http::StatusCode::PRECONDITION_FAILED)
                    .json(json!({
                        "error": "precondition_failed",
                        "message": msg
                    }))
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                HttpResponse::InternalServerError().json(json!({
                    "error": "internal_server_error",
                    "message": "An internal error occurred"
                }))
            }
            AppError::IdempotencyReplay { status, body } => {
                HttpResponse::build(
                    actix_web::http::StatusCode::from_u16(*status)
                        .unwrap_or(actix_web::http::StatusCode::OK),
                )
                .content_type("application/json")
                .body(body.clone())
            }
        }
    }
}

impl From<diesel::result::Error> for AppError {
    fn from(e: diesel::result::Error) -> Self {
        use diesel::result::DatabaseErrorKind;
        match e {
            diesel::result::Error::NotFound => AppError::NotFound("Record not found".into()),
            diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, info) => {
                AppError::Conflict(info.message().to_string())
            }
            // Map foreign-key / NOT NULL / CHECK violations to 422 so controlled
            // domain validation failures (e.g. a negative delta that hits a
            // `CHECK (points_balance >= 0)`) surface as user-facing validation
            // errors rather than opaque 500s. Service-layer guards are the
            // primary line of defense; this is defense-in-depth for any path
            // that slips through.
            diesel::result::Error::DatabaseError(DatabaseErrorKind::CheckViolation, info) => {
                AppError::UnprocessableEntity(info.message().to_string())
            }
            diesel::result::Error::DatabaseError(DatabaseErrorKind::NotNullViolation, info) => {
                AppError::UnprocessableEntity(info.message().to_string())
            }
            diesel::result::Error::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, info) => {
                AppError::UnprocessableEntity(info.message().to_string())
            }
            _ => AppError::Internal(e.to_string()),
        }
    }
}

impl From<r2d2::Error> for AppError {
    fn from(e: r2d2::Error) -> Self {
        AppError::Internal(format!("DB pool error: {}", e))
    }
}
