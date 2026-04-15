use actix_web::{web, HttpResponse};
use uuid::Uuid;

use crate::common::{
    db::DbPool,
    errors::AppError,
    extractors::{AdminUser, AuthUser, EvaluatorUser},
    pagination::{Page, PaginationParams},
    validation::validate_dto,
};
use crate::evaluations::{dto::*, repository, service};
use crate::users::model::UserRole;

fn is_admin(role: &str) -> bool {
    // JWT role is stored as lowercase db_rename value ("administrator")
    crate::users::model::UserRole::from_str(role)
        .map(|r| r == crate::users::model::UserRole::Administrator)
        .unwrap_or(false)
}

/// POST /evaluation-cycles  (Admin)
pub async fn create_cycle(
    auth: AdminUser,
    pool: web::Data<DbPool>,
    body: web::Json<CreateCycleRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    if body.starts_at >= body.ends_at {
        return Err(AppError::UnprocessableEntity("starts_at must be before ends_at".into()));
    }

    let cycle = service::create_cycle(
        &pool,
        body.name.clone(),
        body.description.clone(),
        body.starts_at,
        body.ends_at,
        auth.0.sub,
    )
    .await?;

    Ok(HttpResponse::Created().json(EvaluationCycleResponse::from(cycle)))
}

/// GET /evaluation-cycles  (Admin/Evaluator)
pub async fn list_cycles(
    _auth: EvaluatorUser,
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let limit = query.limit();
    let offset = query.offset();

    let pool_c = pool.clone();
    let (records, total) = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::list_cycles(&mut conn, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<EvaluationCycleResponse> = records.into_iter().map(EvaluationCycleResponse::from).collect();
    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

/// POST /evaluations  (Admin)
pub async fn create_evaluation(
    auth: AdminUser,
    pool: web::Data<DbPool>,
    body: web::Json<CreateEvaluationRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    let eval = service::create_evaluation(
        &pool,
        body.cycle_id,
        body.title.clone(),
        body.description.clone(),
        auth.0.sub,
        body.participant_scope.clone().unwrap_or(serde_json::json!([])),
    )
    .await?;

    Ok(HttpResponse::Created().json(EvaluationResponse::from(eval)))
}

/// GET /evaluations/{id}  (Admin=any; Evaluator=own assignments only)
pub async fn get_evaluation(
    auth: EvaluatorUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let eval_id = path.into_inner();
    let user_id = auth.0.sub;
    let role_str = auth.0.role.clone();

    let pool_c = pool.clone();
    let eval = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        if !is_admin(&role_str) {
            let has = repository::evaluator_has_assignment(&mut conn, eval_id, user_id)?;
            if !has {
                return Err(AppError::Forbidden("Not assigned to this evaluation".into()));
            }
        }
        repository::find_evaluation(&mut conn, eval_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(HttpResponse::Ok().json(EvaluationResponse::from(eval)))
}

/// PATCH /evaluations/{id}/state  (Admin)
pub async fn transition_evaluation(
    auth: AdminUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<TransitionEvaluationRequest>,
) -> Result<HttpResponse, AppError> {
    let eval_id = path.into_inner();
    let eval = service::transition_evaluation(&pool, eval_id, body.state.clone(), auth.0.sub).await?;
    Ok(HttpResponse::Ok().json(EvaluationResponse::from(eval)))
}

/// POST /evaluations/{id}/assignments  (Admin)
pub async fn create_assignment(
    _auth: AdminUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<CreateAssignmentRequest>,
) -> Result<HttpResponse, AppError> {
    let eval_id = path.into_inner();

    let assignment = service::create_assignment(
        &pool,
        eval_id,
        body.evaluator_id,
        body.subject_id,
        body.due_at,
    )
    .await?;

    Ok(HttpResponse::Created().json(EvaluationAssignmentResponse::from(assignment)))
}

/// GET /evaluations/{id}/assignments  (Admin=all; Evaluator=own only)
pub async fn list_assignments(
    auth: EvaluatorUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let eval_id = path.into_inner();
    let user_id = auth.0.sub;
    let role_str = auth.0.role.clone();

    let pool_c = pool.clone();
    let assignments = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        if is_admin(&role_str) {
            repository::list_assignments(&mut conn, eval_id)
        } else {
            repository::list_own_assignments(&mut conn, eval_id, user_id)
        }
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<EvaluationAssignmentResponse> = assignments.into_iter().map(EvaluationAssignmentResponse::from).collect();
    Ok(HttpResponse::Ok().json(data))
}

/// PATCH /evaluations/{id}/assignments/{aid}/state  (Evaluator=own; Admin=any)
pub async fn transition_assignment(
    auth: AuthUser,
    pool: web::Data<DbPool>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<TransitionAssignmentRequest>,
) -> Result<HttpResponse, AppError> {
    let (eval_id, assignment_id) = path.into_inner();
    let claims = auth.0;
    let admin = is_admin(&claims.role);

    // Require at least Evaluator role
    let role: UserRole = serde_json::from_value(serde_json::Value::String(claims.role.clone()))
        .unwrap_or(UserRole::Member);
    match role {
        UserRole::Administrator | UserRole::Evaluator => {}
        _ => return Err(AppError::Forbidden("Evaluator or Admin role required".into())),
    }

    let assignment = service::transition_assignment(
        &pool,
        eval_id,
        assignment_id,
        body.state.clone(),
        claims.sub,
        admin,
    )
    .await?;

    Ok(HttpResponse::Ok().json(EvaluationAssignmentResponse::from(assignment)))
}

/// POST /evaluations/{id}/assignments/{aid}/actions  (Evaluator=own)
pub async fn add_action(
    auth: EvaluatorUser,
    pool: web::Data<DbPool>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<AddActionRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    let (eval_id, assignment_id) = path.into_inner();
    let actor_id = auth.0.sub;

    let action = service::add_action(
        &pool,
        eval_id,
        assignment_id,
        actor_id,
        body.action_type.clone(),
        body.notes.clone(),
        body.payload.clone(),
    )
    .await?;

    Ok(HttpResponse::Created().json(EvaluationActionResponse::from(action)))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/evaluation-cycles")
            .route("", web::post().to(create_cycle))
            .route("", web::get().to(list_cycles)),
    )
    .service(
        web::scope("/evaluations")
            .route("", web::post().to(create_evaluation))
            .route("/{id}", web::get().to(get_evaluation))
            .route("/{id}/state", web::patch().to(transition_evaluation))
            .route("/{id}/assignments", web::post().to(create_assignment))
            .route("/{id}/assignments", web::get().to(list_assignments))
            .route("/{id}/assignments/{aid}/state", web::patch().to(transition_assignment))
            .route("/{id}/assignments/{aid}/actions", web::post().to(add_action)),
    );
}
