use sha2::{Digest, Sha256};
use uuid::Uuid;
use chrono::Utc;

use crate::common::{db::DbPool, errors::AppError};
use crate::config::AppConfig;
use crate::reconciliation::{
    model::{NewReconciliationImport, NewReconciliationRow, ReconciliationImport},
    repository,
};

/// A parsed CSV row from the reconciliation file.
/// CSV columns: external_reference, external_amount_cents, transaction_date
#[derive(Debug)]
struct CsvRow {
    external_reference: String,
    external_amount_cents: i64,
    /// Validated presence, not stored (no DB column yet).
    #[allow(dead_code)]
    transaction_date: String,
}

/// Compute SHA-256 checksum of file bytes.
pub fn compute_checksum(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Parse CSV bytes into reconciliation rows.
/// Expected header (any order): external_reference, external_amount_cents, transaction_date
fn parse_csv(bytes: &[u8]) -> Result<Vec<CsvRow>, AppError> {
    let mut reader = csv::Reader::from_reader(bytes);
    let mut rows = Vec::new();

    // Build column-name → index map from actual headers, then validate required columns.
    let col_index: std::collections::HashMap<String, usize> = {
        let headers = reader
            .headers()
            .map_err(|e| AppError::UnprocessableEntity(format!("CSV header error: {}", e)))?;

        let expected = ["external_reference", "external_amount_cents", "transaction_date"];
        for col in &expected {
            if !headers.iter().any(|h| h == *col) {
                return Err(AppError::UnprocessableEntity(format!(
                    "CSV missing required column: {}",
                    col
                )));
            }
        }

        headers
            .iter()
            .enumerate()
            .map(|(i, h)| (h.to_string(), i))
            .collect()
    };

    let ref_idx = *col_index.get("external_reference").unwrap();
    let amt_idx = *col_index.get("external_amount_cents").unwrap();
    let date_idx = *col_index.get("transaction_date").unwrap();

    for result in reader.records() {
        let record = result.map_err(|e| AppError::UnprocessableEntity(format!("CSV parse error: {}", e)))?;

        let external_reference = record
            .get(ref_idx)
            .ok_or_else(|| AppError::UnprocessableEntity("Missing external_reference column".into()))?
            .to_string();

        let amount_str = record
            .get(amt_idx)
            .ok_or_else(|| AppError::UnprocessableEntity("Missing external_amount_cents column".into()))?;

        let external_amount_cents: i64 = amount_str
            .trim()
            .parse()
            .map_err(|_| AppError::UnprocessableEntity(format!("Invalid external_amount_cents: '{}'", amount_str)))?;

        let transaction_date = record
            .get(date_idx)
            .ok_or_else(|| AppError::UnprocessableEntity("Missing transaction_date column".into()))?
            .to_string();

        // Validate strict YYYY-MM-DD format
        chrono::NaiveDate::parse_from_str(transaction_date.trim(), "%Y-%m-%d")
            .map_err(|_| AppError::UnprocessableEntity(format!(
                "Invalid transaction_date '{}': must be YYYY-MM-DD",
                transaction_date.trim()
            )))?;

        rows.push(CsvRow {
            external_reference,
            external_amount_cents,
            transaction_date,
        });
    }

    Ok(rows)
}

/// Import a reconciliation CSV file. Validates checksum for deduplication.
pub async fn import_file(
    pool: &DbPool,
    cfg: &AppConfig,
    actor_id: Uuid,
    file_name: String,
    file_bytes: Vec<u8>,
) -> Result<ReconciliationImport, AppError> {
    if file_bytes.is_empty() {
        return Err(AppError::UnprocessableEntity("File is empty".into()));
    }

    // Config-driven size limit
    if file_bytes.len() > cfg.storage.max_upload_bytes {
        return Err(AppError::UnprocessableEntity(format!(
            "File exceeds {} byte limit",
            cfg.storage.max_upload_bytes
        )));
    }

    let checksum = compute_checksum(&file_bytes);

    // Parse CSV
    let csv_rows = parse_csv(&file_bytes)?;

    let pool_c = pool.clone();
    let checksum_c = checksum.clone();

    // Check for duplicate import
    let existing = actix_web::web::block(move || -> Result<Option<ReconciliationImport>, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_import_by_checksum(&mut conn, &checksum_c)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    if existing.is_some() {
        return Err(AppError::Conflict("A file with this checksum has already been imported".into()));
    }

    // Persist the original CSV to the configured staging directory under a
    // checksum-derived name BEFORE inserting the import row. This way:
    //   * Finance/audit can re-open the source bytes for any imported row.
    //   * The on-disk artifact is keyed by content (sha256), so a second
    //     attempt with the same file is a no-op write.
    //   * If the DB transaction below rolls back, the file may exist on disk
    //     without a corresponding row — that is acceptable (orphan files are
    //     re-deduped by checksum on the next attempt and can be cleaned by
    //     a background sweep).
    let storage_dir = std::path::PathBuf::from(&cfg.storage.reconciliation_dir);
    let storage_path = storage_dir.join(format!("{}.csv", checksum));
    let storage_path_str = storage_path
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Internal("Reconciliation storage path is not valid UTF-8".into()))?;
    {
        let bytes_for_write = file_bytes.clone();
        let dir_for_write = storage_dir.clone();
        let path_for_write = storage_path.clone();
        actix_web::web::block(move || -> Result<(), AppError> {
            std::fs::create_dir_all(&dir_for_write)
                .map_err(|e| AppError::Internal(format!("Failed to create reconciliation dir: {}", e)))?;
            std::fs::write(&path_for_write, &bytes_for_write)
                .map_err(|e| AppError::Internal(format!("Failed to write reconciliation file: {}", e)))?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    let now = Utc::now();
    let import_id = Uuid::new_v4();
    let total_count = csv_rows.len() as i32;
    let rows_to_process: Vec<(String, i64)> = csv_rows
        .into_iter()
        .map(|r| (r.external_reference, r.external_amount_cents))
        .collect();

    // Single transaction: create import record + insert all rows + compute counts + finalize.
    // If any step fails, the entire import is rolled back — no partial/stuck state.
    let pool_c2 = pool.clone();
    let file_name_c = file_name.clone();
    let checksum_c2 = checksum.clone();
    let storage_path_for_insert = storage_path_str.clone();

    let final_import = actix_web::web::block(move || -> Result<ReconciliationImport, AppError> {
        use crate::payments::model::PaymentState;
        use crate::schema::{payments, reconciliation_rows};
        use diesel::prelude::*;

        let mut conn = pool_c2.get()?;
        conn.transaction::<ReconciliationImport, AppError, _>(|conn| {
            // 1. Create import record in processing state
            repository::create_import(
                conn,
                NewReconciliationImport {
                    id: import_id,
                    file_name: file_name_c,
                    file_checksum: checksum_c2,
                    status: "processing".into(),
                    total_rows: 0,
                    matched_rows: 0,
                    unmatched_rows: 0,
                    imported_by: actor_id,
                    created_at: now,
                    updated_at: now,
                    storage_path: Some(storage_path_for_insert),
                },
            )?;

            // 2. Match and insert rows
            for (ext_ref, ext_amount) in &rows_to_process {
                let matching_payment: Option<crate::payments::model::Payment> = payments::table
                    .filter(payments::external_reference.eq(Some(ext_ref)))
                    .filter(payments::state.eq(PaymentState::Completed))
                    .first(conn)
                    .optional()
                    .map_err(AppError::from)?;

                let (payment_id, internal_amount, discrepancy, status) = match matching_payment {
                    Some(p) => {
                        let discrepancy = ext_amount - p.amount_cents;
                        let status = if discrepancy == 0 { "matched" } else { "discrepancy" };
                        (Some(p.id), Some(p.amount_cents), Some(discrepancy), status.to_string())
                    }
                    None => (None, None, None, "unmatched".to_string()),
                };

                repository::create_row(
                    conn,
                    NewReconciliationRow {
                        id: Uuid::new_v4(),
                        import_id,
                        external_reference: ext_ref.clone(),
                        external_amount_cents: *ext_amount,
                        payment_id,
                        internal_amount_cents: internal_amount,
                        discrepancy_cents: discrepancy,
                        status,
                        created_at: Utc::now(),
                    },
                )?;
            }

            // 3. Compute final counts and finalize import
            let matched_count: i64 = reconciliation_rows::table
                .filter(reconciliation_rows::import_id.eq(import_id))
                .filter(reconciliation_rows::status.ne("unmatched"))
                .count()
                .get_result(conn)
                .map_err(AppError::from)?;

            let unmatched_count: i64 = reconciliation_rows::table
                .filter(reconciliation_rows::import_id.eq(import_id))
                .filter(reconciliation_rows::status.eq("unmatched"))
                .count()
                .get_result(conn)
                .map_err(AppError::from)?;

            repository::update_import(
                conn,
                import_id,
                "completed",
                total_count,
                matched_count as i32,
                unmatched_count as i32,
            )
        }) // end transaction
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(final_import)
}

// ───────────────────────────────────────────────────────────────────────────
// Unit tests for the pure CSV/checksum primitives. These do not touch the DB
// or the multipart layer; full end-to-end import behavior is exercised by
// `tests/reconciliation.rs`.
// ───────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_is_deterministic_and_hex() {
        let a = compute_checksum(b"hello,world\n1,2");
        let b = compute_checksum(b"hello,world\n1,2");
        assert_eq!(a, b, "checksum must be deterministic over identical bytes");
        assert_eq!(a.len(), 64, "sha256 hex output is 64 chars");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()), "checksum must be hex");
    }

    #[test]
    fn checksum_distinguishes_byte_changes() {
        let a = compute_checksum(b"alpha");
        let b = compute_checksum(b"alphA");
        assert_ne!(a, b, "single-byte change must change the checksum");
    }

    #[test]
    fn parse_csv_accepts_canonical_header_order() {
        let csv = b"external_reference,external_amount_cents,transaction_date\nREF1,1000,2026-01-15\n";
        let rows = parse_csv(csv).expect("canonical-order CSV must parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].external_reference, "REF1");
        assert_eq!(rows[0].external_amount_cents, 1000);
    }

    #[test]
    fn parse_csv_is_column_order_agnostic() {
        // Different header order should still yield the same logical rows —
        // this is the property the reviewer-checklist asserts.
        let csv = b"transaction_date,external_amount_cents,external_reference\n2026-01-15,250,REF42\n";
        let rows = parse_csv(csv).expect("reordered headers must still parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].external_reference, "REF42");
        assert_eq!(rows[0].external_amount_cents, 250);
    }

    #[test]
    fn parse_csv_rejects_missing_required_column() {
        // No `transaction_date` column.
        let csv = b"external_reference,external_amount_cents\nREF1,100\n";
        let err = parse_csv(csv).expect_err("missing required column must fail");
        match err {
            AppError::UnprocessableEntity(msg) => {
                assert!(msg.contains("transaction_date"), "error must name the missing column, got: {}", msg);
            }
            other => panic!("expected UnprocessableEntity, got {:?}", other),
        }
    }

    #[test]
    fn parse_csv_rejects_invalid_amount() {
        let csv = b"external_reference,external_amount_cents,transaction_date\nREF1,not_a_number,2026-01-15\n";
        let err = parse_csv(csv).expect_err("non-integer amount must fail");
        assert!(matches!(err, AppError::UnprocessableEntity(_)));
    }

    #[test]
    fn parse_csv_rejects_invalid_date_format() {
        // YYYY-MM-DD is the only accepted format.
        let csv = b"external_reference,external_amount_cents,transaction_date\nREF1,100,01/15/2026\n";
        let err = parse_csv(csv).expect_err("non-ISO date must fail");
        match err {
            AppError::UnprocessableEntity(msg) => {
                assert!(msg.contains("transaction_date") || msg.contains("YYYY-MM-DD"),
                    "error must explain the date format requirement, got: {}", msg);
            }
            other => panic!("expected UnprocessableEntity, got {:?}", other),
        }
    }

    #[test]
    fn parse_csv_handles_multiple_rows() {
        let csv = b"external_reference,external_amount_cents,transaction_date\nA,1,2026-01-01\nB,2,2026-01-02\nC,3,2026-01-03\n";
        let rows = parse_csv(csv).expect("multi-row CSV must parse");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].external_reference, "A");
        assert_eq!(rows[2].external_amount_cents, 3);
    }
}
