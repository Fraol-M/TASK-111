use chrono::{DateTime, Timelike, Utc};
use diesel::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::audit::model::{AuditLog, NewAuditLog};
use crate::common::{db::DbConn, errors::AppError, pagination::PaginationParams};
use crate::schema::audit_logs;

/// Compute a SHA-256 hash over the full set of security-relevant audit fields
/// plus the previous row's hash to form a tamper-evident chain. Any modification
/// to any hashed column or deletion of an intermediate row will break the chain
/// when verified.
///
/// Fields covered (order-sensitive - do not reorder without a re-hash migration):
///   id, correlation_id, actor_user_id, action, entity_type, entity_id,
///   old_value (canonical JSON), new_value (canonical JSON), metadata (canonical
///   JSON), created_at, previous_hash.
///
/// Each field is framed with a length-prefix (`<name>:<len>:<bytes>\n`) so two
/// adjacent fields cannot collude to produce the same hash as a different
/// split; null/absent fields are framed with length 0 and a distinct marker so
/// `old_value = null` and `old_value` absent hash differently.
#[allow(clippy::too_many_arguments)]
fn compute_row_hash_fields(
    id: &Uuid,
    correlation_id: Option<&str>,
    actor_user_id: Option<&Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: &str,
    old_value: Option<&serde_json::Value>,
    new_value: Option<&serde_json::Value>,
    metadata: Option<&serde_json::Value>,
    created_at: &DateTime<Utc>,
    previous_hash: Option<&str>,
) -> String {
    fn feed_str(hasher: &mut Sha256, name: &str, value: &str) {
        let prefix = format!("{}:{}:", name, value.len());
        hasher.update(prefix.as_bytes());
        hasher.update(value.as_bytes());
        hasher.update(b"\n");
    }

    fn feed_opt_str(hasher: &mut Sha256, name: &str, value: Option<&str>) {
        match value {
            Some(s) => feed_str(hasher, name, s),
            None => {
                let marker = format!("{}:-\n", name);
                hasher.update(marker.as_bytes());
            }
        }
    }

    fn feed_opt_json(hasher: &mut Sha256, name: &str, value: Option<&serde_json::Value>) {
        // Canonicalize JSON to a stable byte string: serde_json::to_string
        // writes object keys in insertion order, not sorted. Re-serialize via
        // BTreeMap so any object ordering produces the same hash.
        fn canonicalize(v: &serde_json::Value) -> String {
            match v {
                serde_json::Value::Object(map) => {
                    let sorted: std::collections::BTreeMap<_, _> = map.iter().collect();
                    let parts: Vec<String> = sorted
                        .iter()
                        .map(|(k, val)| {
                            format!(
                                "{}:{}",
                                serde_json::to_string(k).unwrap_or_default(),
                                canonicalize(val)
                            )
                        })
                        .collect();
                    format!("{{{}}}", parts.join(","))
                }
                serde_json::Value::Array(arr) => {
                    let parts: Vec<String> = arr.iter().map(canonicalize).collect();
                    format!("[{}]", parts.join(","))
                }
                _ => serde_json::to_string(v).unwrap_or_default(),
            }
        }

        match value {
            Some(v) => feed_str(hasher, name, &canonicalize(v)),
            None => {
                let marker = format!("{}:-\n", name);
                hasher.update(marker.as_bytes());
            }
        }
    }

    let mut hasher = Sha256::new();
    feed_str(&mut hasher, "id", &id.to_string());
    feed_opt_str(&mut hasher, "correlation_id", correlation_id);
    feed_opt_str(
        &mut hasher,
        "actor_user_id",
        actor_user_id.map(|u| u.to_string()).as_deref(),
    );
    feed_str(&mut hasher, "action", action);
    feed_str(&mut hasher, "entity_type", entity_type);
    feed_str(&mut hasher, "entity_id", entity_id);
    feed_opt_json(&mut hasher, "old_value", old_value);
    feed_opt_json(&mut hasher, "new_value", new_value);
    feed_opt_json(&mut hasher, "metadata", metadata);
    feed_str(&mut hasher, "created_at", &created_at.to_rfc3339());
    feed_opt_str(&mut hasher, "previous_hash", previous_hash);
    hex::encode(hasher.finalize())
}

fn compute_row_hash(log: &NewAuditLog, previous_hash: Option<&str>) -> String {
    compute_row_hash_fields(
        &log.id,
        log.correlation_id.as_deref(),
        log.actor_user_id.as_ref(),
        &log.action,
        &log.entity_type,
        &log.entity_id,
        log.old_value.as_ref(),
        log.new_value.as_ref(),
        log.metadata.as_ref(),
        &log.created_at,
        previous_hash,
    )
}

fn find_chain_tail_hash(conn: &mut DbConn) -> Result<Option<String>, AppError> {
    let rows: Vec<(String, Option<String>, DateTime<Utc>)> = audit_logs::table
        .select((audit_logs::row_hash, audit_logs::previous_hash, audit_logs::created_at))
        .load(conn)
        .map_err(AppError::from)?;

    if rows.is_empty() {
        return Ok(None);
    }

    let referenced_hashes: HashSet<String> = rows
        .iter()
        .filter_map(|(_, previous_hash, _)| previous_hash.clone())
        .collect();

    let mut tails: Vec<(String, DateTime<Utc>)> = rows
        .into_iter()
        .map(|(row_hash, _, created_at)| (row_hash, created_at))
        .filter(|(row_hash, _)| !referenced_hashes.contains(row_hash))
        .collect();

    match tails.len() {
        0 => Err(AppError::Internal(
            "Audit chain has no tail; the linkage is cyclic or corrupted".into(),
        )),
        1 => Ok(Some(tails.pop().expect("single tail").0)),
        _ => {
            // Older test runs could leave a historical fork. Continue the most
            // recent validated tail so new inserts remain append-only instead of
            // failing forever on legacy state.
            let latest_tail = tails
                .into_iter()
                .max_by(|left, right| {
                    left.1
                        .cmp(&right.1)
                        .then_with(|| left.0.cmp(&right.0))
                })
                .expect("tail exists");
            Ok(Some(latest_tail.0))
        }
    }
}

pub fn insert_audit_log(conn: &mut DbConn, mut log: NewAuditLog) -> Result<AuditLog, AppError> {
    // PostgreSQL stores timestamptz at microsecond precision. Normalize before
    // hashing so later verification over the persisted row reproduces the same
    // byte sequence instead of drifting on truncated nanoseconds.
    let micros = log.created_at.timestamp_subsec_micros();
    if let Some(normalized) = log.created_at.with_nanosecond(micros * 1_000) {
        log.created_at = normalized;
    }

    conn.transaction::<AuditLog, AppError, _>(|conn| {
        // A linear hash chain needs serialized appends so two writers cannot
        // both read the same tail and fork the chain.
        diesel::sql_query("LOCK TABLE audit_logs IN SHARE ROW EXCLUSIVE MODE")
            .execute(conn)
            .map_err(AppError::from)?;

        let prev_hash = find_chain_tail_hash(conn)?;
        log.previous_hash = prev_hash.clone();
        log.row_hash = compute_row_hash(&log, prev_hash.as_deref());

        diesel::insert_into(audit_logs::table)
            .values(&log)
            .get_result(conn)
            .map_err(AppError::from)
    })
}

/// Verify the integrity of the audit log hash chain.
/// Reconstructs append order from previous_hash links and recomputes each
/// row_hash, rather than trusting created_at ordering, which may differ from
/// commit order.
#[allow(dead_code)]
pub fn verify_audit_chain(conn: &mut DbConn) -> Result<usize, AppError> {
    let rows: Vec<AuditLog> = audit_logs::table.load(conn).map_err(AppError::from)?;

    if rows.is_empty() {
        return Ok(0);
    }

    let mut index_by_hash: HashMap<String, usize> = HashMap::with_capacity(rows.len());
    let mut children_by_prev: HashMap<String, Vec<usize>> = HashMap::new();
    let mut roots: Vec<usize> = Vec::new();

    for (idx, row) in rows.iter().enumerate() {
        if index_by_hash.insert(row.row_hash.clone(), idx).is_some() {
            return Err(AppError::Internal(format!(
                "Audit chain broken at row {}: duplicate row_hash detected",
                row.id
            )));
        }
    }

    for (idx, row) in rows.iter().enumerate() {
        match row.previous_hash.as_ref() {
            Some(previous_hash) => {
                if !index_by_hash.contains_key(previous_hash) {
                    return Err(AppError::Internal(format!(
                        "Audit chain broken at row {}: previous_hash points to a missing predecessor",
                        row.id
                    )));
                }
                children_by_prev
                    .entry(previous_hash.clone())
                    .or_default()
                    .push(idx);
            }
            None => roots.push(idx),
        }
    }

    let root_idx = match roots.as_slice() {
        [root_idx] => *root_idx,
        [] => {
            return Err(AppError::Internal(
                "Audit chain broken: no root row with NULL previous_hash".into(),
            ))
        }
        _ => {
            return Err(AppError::Internal(format!(
                "Audit chain broken: expected 1 root row, found {}",
                roots.len()
            )))
        }
    };

    let mut visited: HashSet<usize> = HashSet::with_capacity(rows.len());
    let mut stack: Vec<(usize, Option<String>)> = vec![(root_idx, None)];

    while let Some((current_idx, expected_prev)) = stack.pop() {
        let row = &rows[current_idx];

        if row.previous_hash != expected_prev {
            return Err(AppError::Internal(format!(
                "Audit chain broken at row {}: previous_hash mismatch",
                row.id
            )));
        }

        if !visited.insert(current_idx) {
            return Err(AppError::Internal(format!(
                "Audit chain broken at row {}: cycle detected",
                row.id
            )));
        }

        let recomputed = compute_row_hash_fields(
            &row.id,
            row.correlation_id.as_deref(),
            row.actor_user_id.as_ref(),
            &row.action,
            &row.entity_type,
            &row.entity_id,
            row.old_value.as_ref(),
            row.new_value.as_ref(),
            row.metadata.as_ref(),
            &row.created_at,
            row.previous_hash.as_deref(),
        );
        if recomputed != row.row_hash {
            return Err(AppError::Internal(format!(
                "Audit chain broken at row {}: row_hash mismatch (expected {}, found {})",
                row.id, recomputed, row.row_hash
            )));
        }

        if let Some(children) = children_by_prev.get(&row.row_hash) {
            for child_idx in children.iter().rev() {
                stack.push((*child_idx, Some(row.row_hash.clone())));
            }
        }
    }

    if visited.len() != rows.len() {
        let orphan = rows
            .iter()
            .enumerate()
            .find(|(idx, _)| !visited.contains(idx))
            .map(|(_, row)| row.id)
            .expect("unvisited row exists");
        return Err(AppError::Internal(format!(
            "Audit chain broken: row {} is disconnected from the main chain",
            orphan
        )));
    }

    Ok(rows.len())
}

pub struct AuditLogFilter {
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

pub fn query_audit_logs(
    conn: &mut DbConn,
    filter: AuditLogFilter,
    pagination: &PaginationParams,
) -> Result<(Vec<AuditLog>, i64), AppError> {
    use crate::schema::audit_logs::dsl::*;

    // Macro expands inline at each call site, avoiding closure lifetime constraints
    // on the borrowed string fields of AuditLogFilter.
    macro_rules! filtered_query {
        () => {{
            let mut q = audit_logs.into_boxed();
            if let Some(ref et) = filter.entity_type {
                q = q.filter(entity_type.eq(et));
            }
            if let Some(ref eid) = filter.entity_id {
                q = q.filter(entity_id.eq(eid));
            }
            if let Some(actor) = filter.actor_user_id {
                q = q.filter(actor_user_id.eq(actor));
            }
            if let Some(from_ts) = filter.from {
                q = q.filter(created_at.ge(from_ts));
            }
            if let Some(to_ts) = filter.to {
                q = q.filter(created_at.le(to_ts));
            }
            q
        }};
    }

    let total: i64 = filtered_query!()
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let records = filtered_query!()
        .order(created_at.desc())
        .limit(pagination.limit())
        .offset(pagination.offset())
        .load::<AuditLog>(conn)
        .map_err(AppError::from)?;

    Ok((records, total))
}

pub fn get_audit_log(conn: &mut DbConn, log_id: Uuid) -> Result<AuditLog, AppError> {
    use crate::schema::audit_logs::dsl::*;
    audit_logs
        .filter(id.eq(log_id))
        .first(conn)
        .map_err(AppError::from)
}

/// Insert a job_run record with status 'running'. Returns the new run ID.
pub fn start_job_run(conn: &mut DbConn, job_name: &str) -> Result<Uuid, AppError> {
    use crate::audit::model::NewJobRun;
    use crate::schema::job_runs;

    let id = Uuid::new_v4();
    diesel::insert_into(job_runs::table)
        .values(NewJobRun {
            id,
            job_name: job_name.to_string(),
            started_at: Utc::now(),
            status: "running".to_string(),
        })
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(id)
}

/// Update a job_run record to completed or failed.
pub fn finish_job_run(
    conn: &mut DbConn,
    run_id: Uuid,
    status: &str,
    items_processed: Option<i32>,
    error_detail: Option<String>,
) -> Result<(), AppError> {
    use crate::schema::job_runs;

    diesel::update(job_runs::table.filter(job_runs::id.eq(run_id)))
        .set((
            job_runs::finished_at.eq(Utc::now()),
            job_runs::status.eq(status),
            job_runs::items_processed.eq(items_processed),
            job_runs::error_detail.eq(error_detail),
        ))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}
