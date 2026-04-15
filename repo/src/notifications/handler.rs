use actix_web::{web, HttpResponse};
use uuid::Uuid;

use crate::common::{
    db::DbPool,
    errors::AppError,
    extractors::{AuthUser, OperationsUser},
    pagination::{Page, PaginationParams},
    validation::validate_dto,
};
use crate::config::AppConfig;
use crate::notifications::{dto::*, repository, service};
use crate::users::model::UserRole;

/// GET /notifications  (Member=own; Admin=all)
pub async fn list_notifications(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let claims = auth.0;
    let role: UserRole = serde_json::from_value(serde_json::Value::String(claims.role.clone()))
        .unwrap_or(UserRole::Member);

    let limit = query.limit();
    let offset = query.offset();

    let pool_c = pool.clone();
    let user_id = claims.sub;
    let (records, total) = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        match role {
            UserRole::Administrator => repository::list_all_notifications(&mut conn, limit, offset),
            _ => repository::list_notifications_for_user(&mut conn, user_id, limit, offset),
        }
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<NotificationResponse> = records.into_iter().map(NotificationResponse::from).collect();
    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

/// PATCH /notifications/{id}/read  (self)
pub async fn mark_read(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let notification_id = path.into_inner();
    let user_id = auth.0.sub;

    let pool_c = pool.clone();
    let notif = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::mark_notification_read(&mut conn, notification_id, user_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(HttpResponse::Ok().json(NotificationResponse::from(notif)))
}

/// GET /notifications/templates  (Ops/Admin)
pub async fn list_templates(
    _auth: OperationsUser,
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let limit = query.limit();
    let offset = query.offset();

    let pool_c = pool.clone();
    let (records, total) = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::list_templates(&mut conn, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<TemplateResponse> = records.into_iter().map(TemplateResponse::from).collect();
    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

/// POST /notifications/templates  (Ops/Admin)
pub async fn create_template(
    _auth: OperationsUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    body: web::Json<CreateTemplateRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    // Gate on operator-configured channels: templates for channels that are
    // not wired to a real provider in this deployment profile must not be
    // creatable. This prevents operational confusion where a Email/SMS/Push
    // template exists but every send logs a failed dispatch + fallback.
    let channel_str = body.channel.as_db_str();
    if !cfg.notifications.channel_is_enabled(channel_str) {
        return Err(AppError::UnprocessableEntity(format!(
            "Channel '{}' is not enabled in this deployment (enabled: {})",
            channel_str,
            cfg.notifications.enabled_channels().join(", ")
        )));
    }

    let tmpl = service::create_template(
        &pool,
        body.trigger_type.clone(),
        body.channel.clone(),
        body.name.clone(),
        body.subject_template.clone(),
        body.body_template.clone(),
        body.variable_schema.clone(),
        body.is_critical,
    )
    .await?;

    Ok(HttpResponse::Created().json(TemplateResponse::from(tmpl)))
}

/// PATCH /notifications/templates/{id}  (Ops/Admin)
pub async fn update_template(
    _auth: OperationsUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateTemplateRequest>,
) -> Result<HttpResponse, AppError> {
    let template_id = path.into_inner();

    let pool_c = pool.clone();
    let tmpl = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::update_template(
            &mut conn,
            template_id,
            body.name.clone(),
            body.subject_template.clone(),
            body.body_template.clone(),
            body.variable_schema.clone(),
            body.is_critical,
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(HttpResponse::Ok().json(TemplateResponse::from(tmpl)))
}

/// POST /notifications/preview  (Ops/Admin)
pub async fn preview_template(
    _auth: OperationsUser,
    pool: web::Data<DbPool>,
    body: web::Json<PreviewRequest>,
) -> Result<HttpResponse, AppError> {
    let template_id = body.template_id;
    let variables = body.variables.clone();

    let pool_c = pool.clone();
    let tmpl = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_template_by_id(&mut conn, template_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let (subject, body_rendered) = service::preview_template(&tmpl, variables)?;

    Ok(HttpResponse::Ok().json(PreviewResponse {
        subject,
        body: body_rendered,
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/notifications")
            .route("", web::get().to(list_notifications))
            .route("/{id}/read", web::patch().to(mark_read))
            .route("/templates", web::get().to(list_templates))
            .route("/templates", web::post().to(create_template))
            .route("/templates/{id}", web::patch().to(update_template))
            .route("/preview", web::post().to(preview_template)),
    );
}
