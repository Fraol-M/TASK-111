use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::reconciliation_imports)]
pub struct ReconciliationImport {
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
    /// Path on disk under `cfg.storage.reconciliation_dir` where the original
    /// uploaded CSV is retained for audit/replay. Nullable for historical rows.
    pub storage_path: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::reconciliation_imports)]
pub struct NewReconciliationImport {
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

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::reconciliation_rows)]
pub struct ReconciliationRow {
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

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::reconciliation_rows)]
pub struct NewReconciliationRow {
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
