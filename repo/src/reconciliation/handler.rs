use actix_web::{web, HttpResponse};
use uuid::Uuid;

use crate::common::{
    db::DbPool,
    errors::AppError,
    extractors::FinanceUser,
    pagination::{Page, PaginationParams},
};
use crate::config::AppConfig;
use crate::reconciliation::{dto::*, repository, service};

/// POST /reconciliation/import  (Finance — multipart upload)
pub async fn import_file(
    auth: FinanceUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    mut payload: actix_multipart::Multipart,
) -> Result<HttpResponse, AppError> {
    use futures_util::StreamExt;

    let actor_id = auth.0.sub;
    let max_bytes = cfg.storage.max_upload_bytes;
    let mut file_name = String::from("upload.csv");
    let mut file_bytes: Vec<u8> = Vec::new();

    while let Some(field_result) = payload.next().await {
        let mut field = field_result
            .map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;

        let cd = field.content_disposition().clone();
        if let Some(name) = cd.get_filename() {
            file_name = name.to_string();
        }

        // Validate filename extension
        let lower_name = file_name.to_lowercase();
        if !lower_name.ends_with(".csv") {
            return Err(AppError::UnprocessableEntity("Only CSV files are accepted".into()));
        }

        // Validate Content-Type of the multipart field
        let content_type = field.content_type().map(|m| m.to_string()).unwrap_or_default();
        if !content_type.is_empty()
            && !content_type.starts_with("text/csv")
            && !content_type.starts_with("text/plain")
            && !content_type.starts_with("application/csv")
            && !content_type.starts_with("application/octet-stream")
        {
            return Err(AppError::UnprocessableEntity(format!(
                "Unsupported Content-Type '{}'; expected text/csv",
                content_type
            )));
        }

        // Streaming size cap: refuse early once accumulated bytes exceed the
        // configured max. Without this guard a single oversized upload could
        // hold the request handler, an actix worker, and pool memory equal to
        // the full attacker-chosen size before the post-buffer service-layer
        // size check fires. Aborting at the chunk boundary bounds memory use
        // to roughly `max_upload_bytes + last_chunk_size`.
        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|e| AppError::UnprocessableEntity(e.to_string()))?;
            if file_bytes.len() + data.len() > max_bytes {
                return Err(AppError::UnprocessableEntity(format!(
                    "Reconciliation upload exceeds {} byte limit",
                    max_bytes
                )));
            }
            file_bytes.extend_from_slice(&data);
        }
    }

    if file_bytes.is_empty() {
        return Err(AppError::UnprocessableEntity("No file data received".into()));
    }

    let import = service::import_file(&pool, &cfg, actor_id, file_name, file_bytes).await?;
    Ok(HttpResponse::Created().json(ImportResponse::from(import)))
}

/// GET /reconciliation/imports  (Finance/Admin)
pub async fn list_imports(
    _auth: FinanceUser,
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let limit = query.limit();
    let offset = query.offset();

    let pool_c = pool.clone();
    let (records, total) = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::list_imports(&mut conn, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<ImportResponse> = records.into_iter().map(ImportResponse::from).collect();
    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

/// GET /reconciliation/imports/{id}  (Finance/Admin)
pub async fn get_import(
    _auth: FinanceUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let import_id = path.into_inner();

    let pool_c = pool.clone();
    let import = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_import(&mut conn, import_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(HttpResponse::Ok().json(ImportResponse::from(import)))
}

/// GET /reconciliation/imports/{id}/rows  (Finance/Admin)
pub async fn list_rows(
    _auth: FinanceUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let import_id = path.into_inner();
    let limit = query.limit();
    let offset = query.offset();

    let pool_c = pool.clone();
    let (records, total) = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::list_rows(&mut conn, import_id, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<ReconciliationRowResponse> = records.into_iter().map(ReconciliationRowResponse::from).collect();
    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/reconciliation")
            .route("/import", web::post().to(import_file))
            .route("/imports", web::get().to(list_imports))
            .route("/imports/{id}", web::get().to(get_import))
            .route("/imports/{id}/rows", web::get().to(list_rows)),
    );
}
