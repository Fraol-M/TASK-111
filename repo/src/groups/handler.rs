use actix_web::{web, HttpResponse};
use uuid::Uuid;

use crate::common::{
    db::DbPool,
    errors::AppError,
    extractors::{AuthUser, OperationsUser},
    pagination::{Page, PaginationParams},
    validation::validate_dto,
};
use crate::groups::{dto::*, repository, service};
use crate::users::model::UserRole;

/// POST /groups  (Ops/Admin)
pub async fn create_group(
    auth: OperationsUser,
    pool: web::Data<DbPool>,
    body: web::Json<CreateGroupRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    let group = service::create_group(
        &pool,
        body.name.clone(),
        body.description.clone(),
        auth.0.sub,
    )
    .await?;

    Ok(HttpResponse::Created().json(GroupThreadResponse::from(group)))
}

/// GET /groups  (Member=enrolled only; Admin/Ops=all)
pub async fn list_groups(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let claims = auth.0;
    let role: UserRole = serde_json::from_value(serde_json::Value::String(claims.role.clone()))
        .unwrap_or(UserRole::Member);

    let limit = query.limit();
    let offset = query.offset();

    let user_id = claims.sub;
    let pool_c = pool.clone();
    let (records, total) = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        match role {
            UserRole::Administrator | UserRole::OperationsManager => {
                repository::list_all_threads(&mut conn, limit, offset)
            }
            _ => repository::list_threads_for_user(&mut conn, user_id, limit, offset),
        }
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<GroupThreadResponse> = records.into_iter().map(GroupThreadResponse::from).collect();
    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

/// GET /groups/{id}
pub async fn get_group(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let thread_id = path.into_inner();
    let user_id = auth.0.sub;
    let role: UserRole = serde_json::from_value(serde_json::Value::String(auth.0.role.clone()))
        .unwrap_or(UserRole::Member);

    let pool_c = pool.clone();
    let thread = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        let thread = repository::find_thread(&mut conn, thread_id)?;

        let is_privileged = matches!(role, UserRole::Administrator | UserRole::OperationsManager);
        if !is_privileged && !repository::is_active_member(&mut conn, thread_id, user_id)? {
            return Err(AppError::Forbidden("Not a member of this group".into()));
        }

        Ok(thread)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(HttpResponse::Ok().json(GroupThreadResponse::from(thread)))
}

/// POST /groups/{id}/members  (Ops/Admin)
pub async fn add_member(
    _auth: OperationsUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<AddMemberRequest>,
) -> Result<HttpResponse, AppError> {
    let thread_id = path.into_inner();
    let member = service::add_member(&pool, thread_id, body.user_id).await?;
    Ok(HttpResponse::Created().json(GroupMemberResponse::from(member)))
}

/// DELETE /groups/{id}/members/{user_id}  (Ops/Admin)
pub async fn remove_member(
    _auth: OperationsUser,
    pool: web::Data<DbPool>,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse, AppError> {
    let (thread_id, user_id) = path.into_inner();
    service::remove_member(&pool, thread_id, user_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// GET /groups/{id}/members  (Ops/Admin or member)
pub async fn list_members(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let thread_id = path.into_inner();
    let user_id = auth.0.sub;
    let role: UserRole = serde_json::from_value(serde_json::Value::String(auth.0.role.clone()))
        .unwrap_or(UserRole::Member);

    let pool_c = pool.clone();
    let members = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        let is_privileged = matches!(role, UserRole::Administrator | UserRole::OperationsManager);
        if !is_privileged && !repository::is_active_member(&mut conn, thread_id, user_id)? {
            return Err(AppError::Forbidden("Not a member of this group".into()));
        }
        repository::list_members(&mut conn, thread_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<GroupMemberResponse> = members.into_iter().map(GroupMemberResponse::from).collect();
    Ok(HttpResponse::Ok().json(data))
}

/// POST /groups/{id}/messages  (thread member)
pub async fn post_message(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<PostMessageRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    let thread_id = path.into_inner();
    let sender_id = auth.0.sub;

    let msg = service::post_message(&pool, thread_id, sender_id, body.body.clone()).await?;
    Ok(HttpResponse::Created().json(GroupMessageResponse::from(msg)))
}

/// GET /groups/{id}/messages  (thread member, paginated)
pub async fn list_messages(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let thread_id = path.into_inner();
    let user_id = auth.0.sub;
    let role: UserRole = serde_json::from_value(serde_json::Value::String(auth.0.role.clone()))
        .unwrap_or(UserRole::Member);

    let limit = query.limit();
    let offset = query.offset();

    let pool_c = pool.clone();
    let (records, total) = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        let is_privileged = matches!(role, UserRole::Administrator | UserRole::OperationsManager);
        if !is_privileged && !repository::is_active_member(&mut conn, thread_id, user_id)? {
            return Err(AppError::Forbidden("Not a member of this group".into()));
        }
        repository::list_messages(&mut conn, thread_id, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<GroupMessageResponse> = records.into_iter().map(GroupMessageResponse::from).collect();
    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

/// PATCH /groups/{id}/messages/{msg_id}/read  (self)
pub async fn mark_message_read(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse, AppError> {
    let (thread_id, message_id) = path.into_inner();
    let user_id = auth.0.sub;

    let pool_c = pool.clone();
    let receipt = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        // Must be a member
        if !repository::is_active_member(&mut conn, thread_id, user_id)? {
            return Err(AppError::Forbidden("Not a member of this group".into()));
        }
        repository::mark_message_read(&mut conn, thread_id, message_id, user_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(HttpResponse::Ok().json(GroupMessageReceiptResponse::from(receipt)))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/groups")
            .route("", web::post().to(create_group))
            .route("", web::get().to(list_groups))
            .route("/{id}", web::get().to(get_group))
            .route("/{id}/members", web::post().to(add_member))
            .route("/{id}/members", web::get().to(list_members))
            .route("/{id}/members/{user_id}", web::delete().to(remove_member))
            .route("/{id}/messages", web::post().to(post_message))
            .route("/{id}/messages", web::get().to(list_messages))
            .route("/{id}/messages/{msg_id}/read", web::patch().to(mark_message_read)),
    );
}
