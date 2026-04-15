use actix_web::{dev::Payload, web, FromRequest, HttpRequest};
use futures::future::{ready, Ready};
use uuid::Uuid;

use crate::common::{claims::Claims, db::DbPool, errors::AppError};
use crate::config::AppConfig;

/// Extracts and validates the Bearer token from Authorization header.
/// Verifies JWT signature + expiry, then checks session is not revoked in DB.
pub struct AuthUser(pub Claims);

impl FromRequest for AuthUser {
    type Error = AppError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let result = extract_claims(req);
        ready(result.map(AuthUser))
    }
}

fn extract_claims(req: &HttpRequest) -> Result<Claims, AppError> {
    let cfg = req
        .app_data::<web::Data<AppConfig>>()
        .ok_or_else(|| AppError::Internal("AppConfig not registered".into()))?;

    let token = extract_bearer_token(req)?;
    let claims = crate::common::claims::decode_token(&token, &cfg.jwt.secret)?;

    // Verify session not revoked AND that the bearer token's hash matches the
    // session's stored token_hash (forged-JWT defense in depth).
    let pool = req
        .app_data::<web::Data<DbPool>>()
        .ok_or_else(|| AppError::Internal("DbPool not registered".into()))?;

    let mut conn = pool.get().map_err(|e| AppError::Internal(e.to_string()))?;
    crate::auth::repository::verify_session_active(&mut conn, claims.jti, &token)?;

    // Reject requests from suspended or deleted accounts even with valid tokens
    let user = crate::users::repository::find_by_id(&mut conn, claims.sub)
        .map_err(|_| AppError::Unauthorized)?;
    if user.status != crate::users::model::UserStatus::Active {
        return Err(AppError::Unauthorized);
    }

    Ok(claims)
}

fn extract_bearer_token(req: &HttpRequest) -> Result<String, AppError> {
    let header = req
        .headers()
        .get("Authorization")
        .ok_or(AppError::Unauthorized)?;

    let value = header.to_str().map_err(|_| AppError::Unauthorized)?;
    if let Some(token) = value.strip_prefix("Bearer ") {
        Ok(token.to_string())
    } else {
        Err(AppError::Unauthorized)
    }
}

/// Role-checking extractor helpers.
pub struct AdminUser(pub Claims);
pub struct OperationsUser(pub Claims);
pub struct FinanceUser(pub Claims);
pub struct AssetManagerUser(pub Claims);
pub struct EvaluatorUser(pub Claims);
/// Reviewer authority (admin or reviewer). Distinct from `EvaluatorUser`: a
/// reviewer can approve/reject completed evaluations but does not perform
/// the assessment work. `AdminUser` is accepted as a superset.
pub struct ReviewerUser(pub Claims);

macro_rules! role_extractor {
    ($type:ident, $($role:literal),+) => {
        impl FromRequest for $type {
            type Error = AppError;
            type Future = Ready<Result<Self, Self::Error>>;

            fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
                let result = extract_claims(req).and_then(|claims| {
                    let allowed = &[$($role),+];
                    if allowed.contains(&claims.role.as_str()) {
                        Ok($type(claims))
                    } else {
                        Err(AppError::Forbidden(format!(
                            "Role '{}' is not permitted for this operation",
                            claims.role
                        )))
                    }
                });
                ready(result)
            }
        }
    };
}

role_extractor!(AdminUser, "administrator");
role_extractor!(OperationsUser, "administrator", "operations_manager");
role_extractor!(FinanceUser, "administrator", "finance");
role_extractor!(AssetManagerUser, "administrator", "asset_manager");
role_extractor!(EvaluatorUser, "administrator", "evaluator");
role_extractor!(ReviewerUser, "administrator", "reviewer");

/// Helper: require the authenticated user to be a specific user_id OR one of the allowed roles.
pub fn require_self_or_roles(
    claims: &Claims,
    target_id: Uuid,
    roles: &[&str],
) -> Result<(), AppError> {
    if claims.sub == target_id || roles.contains(&claims.role.as_str()) {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Access restricted to owner or privileged role".into(),
        ))
    }
}
