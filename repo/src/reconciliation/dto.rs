use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::reconciliation::model::{ReconciliationImport, ReconciliationRow};
use crate::common::pagination::Page;

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub id: Uuid,
    pub file_name: String,
    pub file_checksum: String,
    pub status: String,
    pub total_rows: i32,
    pub matched_rows: i32,
    pub unmatched_rows: i32,
    pub imported_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub storage_path: Option<String>,
}

impl From<ReconciliationImport> for ImportResponse {
    fn from(i: ReconciliationImport) -> Self {
        Self {
            id: i.id,
            file_name: i.file_name,
            file_checksum: i.file_checksum,
            status: i.status,
            total_rows: i.total_rows,
            matched_rows: i.matched_rows,
            unmatched_rows: i.unmatched_rows,
            imported_by: i.imported_by,
            created_at: i.created_at,
            updated_at: i.updated_at,
            storage_path: i.storage_path,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ReconciliationRowResponse {
    pub id: Uuid,
    pub import_id: Uuid,
    pub external_reference: String,
    pub external_amount_cents: i64,
    pub payment_id: Option<Uuid>,
    pub internal_amount_cents: Option<i64>,
    pub discrepancy_cents: Option<i64>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

impl From<ReconciliationRow> for ReconciliationRowResponse {
    fn from(r: ReconciliationRow) -> Self {
        Self {
            id: r.id,
            import_id: r.import_id,
            external_reference: r.external_reference,
            external_amount_cents: r.external_amount_cents,
            payment_id: r.payment_id,
            internal_amount_cents: r.internal_amount_cents,
            discrepancy_cents: r.discrepancy_cents,
            status: r.status,
            created_at: r.created_at,
        }
    }
}

pub type ImportListResponse = Page<ImportResponse>;
pub type RowListResponse = Page<ReconciliationRowResponse>;
