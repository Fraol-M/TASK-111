use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::common::{db::DbConn, errors::AppError};
use crate::reconciliation::model::{
    NewReconciliationImport, NewReconciliationRow, ReconciliationImport, ReconciliationRow,
};
use crate::schema::{reconciliation_imports, reconciliation_rows};

pub fn find_import_by_checksum(
    conn: &mut DbConn,
    checksum: &str,
) -> Result<Option<ReconciliationImport>, AppError> {
    reconciliation_imports::table
        .filter(reconciliation_imports::file_checksum.eq(checksum))
        .first(conn)
        .optional()
        .map_err(AppError::from)
}

pub fn create_import(
    conn: &mut DbConn,
    import: NewReconciliationImport,
) -> Result<ReconciliationImport, AppError> {
    diesel::insert_into(reconciliation_imports::table)
        .values(&import)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn update_import(
    conn: &mut DbConn,
    import_id: Uuid,
    status: &str,
    total_rows: i32,
    matched_rows: i32,
    unmatched_rows: i32,
) -> Result<ReconciliationImport, AppError> {
    diesel::update(reconciliation_imports::table.filter(reconciliation_imports::id.eq(import_id)))
        .set((
            reconciliation_imports::status.eq(status),
            reconciliation_imports::total_rows.eq(total_rows),
            reconciliation_imports::matched_rows.eq(matched_rows),
            reconciliation_imports::unmatched_rows.eq(unmatched_rows),
            reconciliation_imports::updated_at.eq(Utc::now()),
        ))
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn list_imports(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<ReconciliationImport>, i64), AppError> {
    let total: i64 = reconciliation_imports::table.count().get_result(conn).map_err(AppError::from)?;
    let records = reconciliation_imports::table
        .order(reconciliation_imports::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((records, total))
}

pub fn find_import(conn: &mut DbConn, import_id: Uuid) -> Result<ReconciliationImport, AppError> {
    reconciliation_imports::table
        .filter(reconciliation_imports::id.eq(import_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Import {} not found", import_id)))
}

pub fn create_row(conn: &mut DbConn, row: NewReconciliationRow) -> Result<ReconciliationRow, AppError> {
    diesel::insert_into(reconciliation_rows::table)
        .values(&row)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn list_rows(
    conn: &mut DbConn,
    import_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<ReconciliationRow>, i64), AppError> {
    let total: i64 = reconciliation_rows::table
        .filter(reconciliation_rows::import_id.eq(import_id))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let records = reconciliation_rows::table
        .filter(reconciliation_rows::import_id.eq(import_id))
        .order(reconciliation_rows::created_at.asc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;

    Ok((records, total))
}
