use argon2::{Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::rngs::OsRng;
use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::auth::{
    dto::{LoginRequest, LoginResponse, MeResponse},
    model::NewAuthSession,
    repository,
};
use crate::common::{claims::make_claims, claims::encode_token, db::DbPool, errors::AppError};
use crate::config::AppConfig;
use crate::users::{model::UserStatus, repository as user_repo};

/// Hash a password with Argon2id — used during user creation and tests.
pub fn hash_password(password: &str) -> Result<String, crate::common::errors::AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let params = Params::new(65536, 3, 4, None)
        .map_err(|e| crate::common::errors::AppError::Internal(e.to_string()))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| crate::common::errors::AppError::Internal(e.to_string()))
}

pub async fn login(
    pool: &DbPool,
    cfg: &AppConfig,
    req: LoginRequest,
) -> Result<LoginResponse, AppError> {
    let pool = pool.clone();
    let secret = cfg.jwt.secret.clone();
    let expiry = cfg.jwt.expiry_seconds;

    actix_web::web::block(move || -> Result<LoginResponse, AppError> {
        let mut conn = pool.get()?;

        // Find user by username
        let user = user_repo::find_by_username(&mut conn, &req.username)
            .map_err(|_| AppError::Unauthorized)?;

        // Check account status
        if user.status != UserStatus::Active {
            return Err(AppError::Unauthorized);
        }

        // Verify password
        let hash = PasswordHash::new(&user.password_hash)
            .map_err(|_| AppError::Internal("Invalid password hash stored".into()))?;
        Argon2::default()
            .verify_password(req.password.as_bytes(), &hash)
            .map_err(|_| AppError::Unauthorized)?;

        // Create session.
        //
        // The session row stores the SHA-256 hash of the JWT we hand back to
        // the client. The auth extractor recomputes this hash from the
        // incoming bearer token on every request and rejects mismatches.
        // This binds JWT validity to a specific session row at login time:
        // even if an attacker obtains the JWT signing key and forges a token
        // for `(sub, role, jti=existing_session_id)`, the forged token's
        // hash will not match the stored `token_hash` and authentication
        // will fail. Logout (revoke_session) still works because the
        // extractor's revoked_at check happens before the hash comparison.
        let session_id = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::seconds(expiry);

        // Encode JWT first so we can bind the session to its hash.
        let claims = make_claims(user.id, user.role.as_str(), session_id, expiry);
        let jwt = encode_token(&claims, &secret)?;
        let token_hash = repository::hash_token(&jwt);

        let session = NewAuthSession {
            id: session_id,
            user_id: user.id,
            token_hash,
            expires_at,
            created_at: Utc::now(),
        };
        repository::create_session(&mut conn, session)?;

        Ok(LoginResponse {
            token: jwt,
            expires_at,
            user_id: user.id,
            role: user.role.as_str().to_string(),
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn logout(pool: &DbPool, session_id: Uuid) -> Result<(), AppError> {
    let pool = pool.clone();
    actix_web::web::block(move || {
        let mut conn = pool.get()?;
        repository::revoke_session(&mut conn, session_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn me(pool: &DbPool, user_id: Uuid) -> Result<MeResponse, AppError> {
    let pool = pool.clone();
    actix_web::web::block(move || {
        let mut conn = pool.get()?;
        let user = user_repo::find_by_id(&mut conn, user_id)?;
        Ok::<MeResponse, AppError>(MeResponse {
            user_id: user.id,
            username: user.username,
            role: user.role.as_str().to_string(),
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
