use chrono::Utc;
use uuid::Uuid;

use crate::common::{db::DbPool, errors::AppError};
use crate::evaluations::{
    model::{
        AssignmentState, Evaluation, EvaluationAction, EvaluationAssignment, EvaluationCycle,
        EvaluationState, NewEvaluationAction, NewEvaluationAssignment, NewEvaluationCycle,
        NewEvaluation,
    },
    repository,
    state_machine::{AssignmentStateMachine, EvaluationStateMachine},
};

pub async fn create_cycle(
    pool: &DbPool,
    name: String,
    description: Option<String>,
    starts_at: chrono::DateTime<Utc>,
    ends_at: chrono::DateTime<Utc>,
    created_by: Uuid,
) -> Result<EvaluationCycle, AppError> {
    let now = Utc::now();
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<EvaluationCycle, AppError> {
        let mut conn = pool_c.get()?;
        repository::create_cycle(
            &mut conn,
            NewEvaluationCycle {
                id: Uuid::new_v4(),
                name,
                description,
                starts_at,
                ends_at,
                created_by,
                created_at: now,
                updated_at: now,
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn create_evaluation(
    pool: &DbPool,
    cycle_id: Option<Uuid>,
    title: String,
    description: Option<String>,
    created_by: Uuid,
    participant_scope: serde_json::Value,
) -> Result<Evaluation, AppError> {
    let now = Utc::now();
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<Evaluation, AppError> {
        let mut conn = pool_c.get()?;
        repository::create_evaluation(
            &mut conn,
            NewEvaluation {
                id: Uuid::new_v4(),
                cycle_id,
                title,
                description,
                state: EvaluationState::Draft,
                version: 0,
                created_by,
                participant_scope,
                created_at: now,
                updated_at: now,
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn transition_evaluation(
    pool: &DbPool,
    eval_id: Uuid,
    to_state: EvaluationState,
    actor_id: Uuid,
) -> Result<Evaluation, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<Evaluation, AppError> {
        let mut conn = pool_c.get()?;
        let eval = repository::find_evaluation(&mut conn, eval_id)?;
        EvaluationStateMachine::transition(&eval.state, &to_state)?;
        repository::transition_evaluation_state(&mut conn, eval_id, eval.state, to_state, eval.version)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn create_assignment(
    pool: &DbPool,
    eval_id: Uuid,
    evaluator_id: Uuid,
    subject_id: Option<Uuid>,
    due_at: Option<chrono::DateTime<Utc>>,
) -> Result<EvaluationAssignment, AppError> {
    let now = Utc::now();
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<EvaluationAssignment, AppError> {
        let mut conn = pool_c.get()?;
        // Verify evaluation exists and is in Open state
        let eval = repository::find_evaluation(&mut conn, eval_id)?;
        if eval.state != EvaluationState::Open {
            return Err(AppError::PreconditionFailed(
                "Can only assign evaluators to Open evaluations".into(),
            ));
        }

        // Enforce participant_scope: if the scope array is non-empty, both the
        // subject and evaluator must appear in it. An empty scope means unrestricted.
        if let Some(scope_arr) = eval.participant_scope.as_array() {
            if !scope_arr.is_empty() {
                let scope_ids: Vec<&str> = scope_arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect();

                if let Some(sid) = subject_id {
                    let sid_str = sid.to_string();
                    if !scope_ids.iter().any(|s| *s == sid_str) {
                        return Err(AppError::UnprocessableEntity(format!(
                            "Subject {} is outside the evaluation's participant_scope",
                            sid
                        )));
                    }
                }

                let eid_str = evaluator_id.to_string();
                if !scope_ids.iter().any(|s| *s == eid_str) {
                    return Err(AppError::UnprocessableEntity(format!(
                        "Evaluator {} is outside the evaluation's participant_scope",
                        evaluator_id
                    )));
                }
            }
        }

        repository::create_assignment(
            &mut conn,
            NewEvaluationAssignment {
                id: Uuid::new_v4(),
                evaluation_id: eval_id,
                evaluator_id,
                subject_id,
                state: AssignmentState::Pending,
                due_at,
                created_at: now,
                updated_at: now,
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn transition_assignment(
    pool: &DbPool,
    eval_id: Uuid,
    assignment_id: Uuid,
    to_state: AssignmentState,
    actor_id: Uuid,
    is_admin: bool,
) -> Result<EvaluationAssignment, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<EvaluationAssignment, AppError> {
        let mut conn = pool_c.get()?;
        let assignment = repository::find_assignment(&mut conn, assignment_id)?;

        // Verify assignment belongs to this evaluation
        if assignment.evaluation_id != eval_id {
            return Err(AppError::NotFound("Assignment not found in this evaluation".into()));
        }

        // Node-level permission: only the assigned evaluator or admin can transition
        if !is_admin && assignment.evaluator_id != actor_id {
            return Err(AppError::Forbidden("Can only transition your own assignment".into()));
        }

        AssignmentStateMachine::transition(&assignment.state, &to_state)?;
        repository::transition_assignment_state(&mut conn, assignment_id, to_state)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn add_action(
    pool: &DbPool,
    eval_id: Uuid,
    assignment_id: Uuid,
    actor_id: Uuid,
    action_type: String,
    notes: Option<String>,
    payload: Option<serde_json::Value>,
) -> Result<EvaluationAction, AppError> {
    let now = Utc::now();
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<EvaluationAction, AppError> {
        let mut conn = pool_c.get()?;
        // Verify assignment exists and belongs to the evaluation in the URL path
        let assignment = repository::find_assignment(&mut conn, assignment_id)?;
        if assignment.evaluation_id != eval_id {
            return Err(AppError::NotFound(format!(
                "Assignment {} does not belong to evaluation {}",
                assignment_id, eval_id
            )));
        }
        // Evaluator can only act on their own assignment
        if assignment.evaluator_id != actor_id {
            return Err(AppError::Forbidden("Can only add actions to your own assignment".into()));
        }
        repository::create_action(
            &mut conn,
            NewEvaluationAction {
                id: Uuid::new_v4(),
                assignment_id,
                actor_id,
                action_type,
                notes,
                payload,
                created_at: now,
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
