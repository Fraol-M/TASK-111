use chrono::{Duration, Utc};
use diesel::prelude::*;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::common::{db::DbConn, errors::AppError};

/// Check or register an idempotency key before processing a write.
///
/// Race semantics (Postgres, UNIQUE on `key_value`):
///   1. INSERT ... ON CONFLICT DO NOTHING RETURNING id — atomic. Exactly one
///      concurrent caller receives a row back; losers receive an empty result.
///   2. If we inserted: return Ok(None) — caller proceeds with the write and
///      later calls `store_response` to persist the outcome.
///   3. If we lost the race (empty result): re-read the row that now exists.
///      - If `request_hash` differs, the caller is misusing the key → 422.
///      - If a cached response exists, return it as a replay → (status, body).
///      - Otherwise an in-flight request with the same key is still
///        processing → 409 Conflict, not a silent pass-through.
///
/// The previous implementation did a plain read → insert-or-nothing and
/// returned Ok(None) in both the "I inserted" and "someone else inserted
/// between my read and my write" cases, which meant two concurrent requests
/// could both proceed and cause duplicate writes. This version closes that
/// window by branching on the atomic INSERT's RETURNING result.
pub fn check_or_register(
    conn: &mut DbConn,
    key: &str,
    req_hash: &str,
) -> Result<Option<(u16, String)>, AppError> {
    use crate::schema::idempotency_keys::dsl::*;

    let now = Utc::now();
    let new_id = Uuid::new_v4();

    // Atomic insert-or-nothing with RETURNING id. Only the caller whose insert
    // actually took effect sees a non-empty vec — this is the race winner.
    let inserted: Vec<Uuid> = diesel::insert_into(idempotency_keys)
        .values((
            id.eq(new_id),
            key_value.eq(key),
            request_hash.eq(req_hash),
            created_at.eq(now),
            expires_at.eq(now + Duration::hours(24)),
        ))
        .on_conflict_do_nothing()
        .returning(id)
        .get_results(conn)
        .map_err(AppError::from)?;

    if !inserted.is_empty() {
        // We won the race: this is a brand-new request, proceed normally.
        return Ok(None);
    }

    // We lost the race (or this is a genuine replay). The row is guaranteed to
    // exist now — re-read it to decide replay vs in-flight vs payload mismatch.
    let record: crate::audit::model::IdempotencyKey = idempotency_keys
        .filter(key_value.eq(key))
        .first(conn)
        .map_err(AppError::from)?;

    if record.request_hash != req_hash {
        return Err(AppError::UnprocessableEntity(
            "Idempotency key reused with different request payload".into(),
        ));
    }
    if let (Some(status), Some(body)) = (record.response_status, record.response_body) {
        return Ok(Some((status as u16, body)));
    }
    Err(AppError::Conflict(
        "Request with this idempotency key is already being processed".into(),
    ))
}

/// Store the response for a previously registered idempotency key.
pub fn store_response(
    conn: &mut DbConn,
    key: &str,
    status: u16,
    body: &str,
) -> Result<(), AppError> {
    use crate::schema::idempotency_keys::dsl::*;

    diesel::update(idempotency_keys.filter(key_value.eq(key)))
        .set((
            response_status.eq(status as i16),
            response_body.eq(body),
        ))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

/// Compute SHA-256 hash of the request payload for idempotency comparison.
pub fn hash_request(method: &str, path: &str, body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(method.as_bytes());
    hasher.update(b":");
    hasher.update(path.as_bytes());
    hasher.update(b":");
    hasher.update(body);
    format!("{:x}", hasher.finalize())
}
