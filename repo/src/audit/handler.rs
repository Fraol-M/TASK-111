use actix_web::{web, HttpResponse};
use uuid::Uuid;

use crate::audit::repository::{get_audit_log, query_audit_logs, AuditLogFilter};
use crate::common::{db::DbPool, errors::AppError, extractors::AdminUser, pagination::{Page, PaginationParams}};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/audit")
            .route("/logs", web::get().to(list_logs))
            .route("/logs/{id}", web::get().to(get_log)),
    );
}

async fn list_logs(
    _admin: AdminUser,
    pool: web::Data<DbPool>,
    pagination: web::Query<PaginationParams>,
    filter: web::Query<LogFilterParams>,
) -> Result<HttpResponse, AppError> {
    let mut conn = pool.get()?;
    let audit_filter = AuditLogFilter {
        entity_type: filter.entity_type.clone(),
        entity_id: filter.entity_id.clone(),
        actor_user_id: filter.actor_user_id,
        from: None,
        to: None,
    };
    let (logs, total) = query_audit_logs(&mut conn, audit_filter, &pagination)?;
    Ok(HttpResponse::Ok().json(Page::new(logs, total, &pagination)))
}

async fn get_log(
    _admin: AdminUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let mut conn = pool.get()?;
    let log = get_audit_log(&mut conn, path.into_inner())?;
    Ok(HttpResponse::Ok().json(log))
}

#[derive(serde::Deserialize)]
struct LogFilterParams {
    entity_type: Option<String>,
    entity_id: Option<String>,
    actor_user_id: Option<Uuid>,
}
