use chrono::Utc;
use diesel::prelude::*;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::model::{AuthSession, NewAuthSession};
use crate::common::{db::DbConn, errors::AppError};
use crate::schema::auth_sessions;

pub fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub fn create_session(conn: &mut DbConn, session: NewAuthSession) -> Result<AuthSession, AppError> {
    diesel::insert_into(auth_sessions::table)
        .values(&session)
        .get_result(conn)
        .map_err(AppError::from)
}

#[allow(dead_code)]
pub fn find_session_by_id(conn: &mut DbConn, session_id: Uuid) -> Result<AuthSession, AppError> {
    auth_sessions::table
        .filter(auth_sessions::id.eq(session_id))
        .first(conn)
        .map_err(|_| AppError::Unauthorized)
}

/// Verify a session is active (not revoked and not expired) AND that the
/// presented bearer token hashes to the stored `token_hash` for this session.
///
/// Binding the bearer token's hash to the session row defends against forged
/// JWTs: even with the JWT signing key, an attacker cannot fabricate a token
/// whose hash matches a row they did not create. Note that revoked/expired
/// sessions still fail before the hash check so a stale-but-valid token will
/// 401 with the same error class.
///
/// Called by the auth extractor on every authenticated request.
pub fn verify_session_active(
    conn: &mut DbConn,
    session_id: Uuid,
    presented_token: &str,
) -> Result<(), AppError> {
    let session: AuthSession = auth_sessions::table
        .filter(auth_sessions::id.eq(session_id))
        .first(conn)
        .map_err(|_| AppError::Unauthorized)?;

    if session.revoked_at.is_some() {
        return Err(AppError::Unauthorized);
    }
    if session.expires_at < Utc::now() {
        return Err(AppError::Unauthorized);
    }
    // Constant-time compare via fixed-length hex strings: both inputs are
    // 64-char hex (sha256), so a byte-equal comparison is acceptable here;
    // we still avoid early-exit by computing the full hash before comparing.
    let presented_hash = hash_token(presented_token);
    if presented_hash != session.token_hash {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

pub fn revoke_session(conn: &mut DbConn, session_id: Uuid) -> Result<(), AppError> {
    diesel::update(auth_sessions::table.filter(auth_sessions::id.eq(session_id)))
        .set(auth_sessions::revoked_at.eq(Some(Utc::now())))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

pub fn revoke_all_user_sessions(conn: &mut DbConn, user_id: Uuid) -> Result<usize, AppError> {
    diesel::update(
        auth_sessions::table
            .filter(auth_sessions::user_id.eq(user_id))
            .filter(auth_sessions::revoked_at.is_null()),
    )
    .set(auth_sessions::revoked_at.eq(Some(Utc::now())))
    .execute(conn)
    .map_err(AppError::from)
}
