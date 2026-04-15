use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::errors::AppError;

/// JWT claims embedded in every access token.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject — the authenticated user_id.
    pub sub: Uuid,
    /// Role of the authenticated user (loaded from DB, never trusted from client).
    pub role: String,
    /// JWT expiry (unix timestamp).
    pub exp: i64,
    /// Issued-at (unix timestamp).
    pub iat: i64,
    /// Session ID — used to verify session has not been revoked.
    pub jti: Uuid,
}

pub fn encode_token(claims: &Claims, secret: &str) -> Result<String, AppError> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Token encode error: {}", e)))
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let mut validation = Validation::default();
    validation.validate_exp = true;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| {
        tracing::debug!("Token decode failed: {}", e);
        AppError::Unauthorized
    })
}

pub fn make_claims(user_id: Uuid, role: &str, session_id: Uuid, expiry_seconds: i64) -> Claims {
    let now = Utc::now().timestamp();
    Claims {
        sub: user_id,
        role: role.to_string(),
        exp: now + expiry_seconds,
        iat: now,
        jti: session_id,
    }
}
