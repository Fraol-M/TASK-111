use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::common::{db::DbConn, errors::AppError};
use crate::evaluations::model::{
    AssignmentState, EvaluationAction, EvaluationAssignment, EvaluationCycle, EvaluationState,
    NewEvaluationAction, NewEvaluationAssignment, NewEvaluationCycle, NewEvaluation, Evaluation,
};
use crate::schema::{evaluation_actions, evaluation_assignments, evaluation_cycles, evaluations};

pub fn create_cycle(conn: &mut DbConn, cycle: NewEvaluationCycle) -> Result<EvaluationCycle, AppError> {
    diesel::insert_into(evaluation_cycles::table)
        .values(&cycle)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn list_cycles(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<EvaluationCycle>, i64), AppError> {
    let total: i64 = evaluation_cycles::table.count().get_result(conn).map_err(AppError::from)?;
    let records = evaluation_cycles::table
        .order(evaluation_cycles::starts_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((records, total))
}

pub fn create_evaluation(conn: &mut DbConn, eval: NewEvaluation) -> Result<Evaluation, AppError> {
    diesel::insert_into(evaluations::table)
        .values(&eval)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_evaluation(conn: &mut DbConn, eval_id: Uuid) -> Result<Evaluation, AppError> {
    evaluations::table
        .filter(evaluations::id.eq(eval_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Evaluation {} not found", eval_id)))
}

pub fn transition_evaluation_state(
    conn: &mut DbConn,
    eval_id: Uuid,
    _from_state: EvaluationState,
    to_state: EvaluationState,
    expected_version: i32,
) -> Result<Evaluation, AppError> {
    let rows = diesel::update(
        evaluations::table
            .filter(evaluations::id.eq(eval_id))
            .filter(evaluations::version.eq(expected_version)),
    )
    .set((
        evaluations::state.eq(&to_state),
        evaluations::version.eq(expected_version + 1),
        evaluations::updated_at.eq(Utc::now()),
    ))
    .execute(conn)
    .map_err(AppError::from)?;

    if rows == 0 {
        return Err(AppError::PreconditionFailed("Concurrent modification on evaluation".into()));
    }

    find_evaluation(conn, eval_id)
}

pub fn create_assignment(
    conn: &mut DbConn,
    assignment: NewEvaluationAssignment,
) -> Result<EvaluationAssignment, AppError> {
    diesel::insert_into(evaluation_assignments::table)
        .values(&assignment)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_assignment(
    conn: &mut DbConn,
    assignment_id: Uuid,
) -> Result<EvaluationAssignment, AppError> {
    evaluation_assignments::table
        .filter(evaluation_assignments::id.eq(assignment_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Assignment {} not found", assignment_id)))
}

pub fn list_assignments(
    conn: &mut DbConn,
    eval_id: Uuid,
) -> Result<Vec<EvaluationAssignment>, AppError> {
    evaluation_assignments::table
        .filter(evaluation_assignments::evaluation_id.eq(eval_id))
        .order(evaluation_assignments::created_at.asc())
        .load(conn)
        .map_err(AppError::from)
}

/// Returns true if the given evaluator has at least one assignment for the evaluation.
pub fn evaluator_has_assignment(
    conn: &mut DbConn,
    eval_id: Uuid,
    evaluator_id: Uuid,
) -> Result<bool, AppError> {
    let count: i64 = evaluation_assignments::table
        .filter(evaluation_assignments::evaluation_id.eq(eval_id))
        .filter(evaluation_assignments::evaluator_id.eq(evaluator_id))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;
    Ok(count > 0)
}

/// Returns only the assignments owned by the given evaluator within an evaluation.
pub fn list_own_assignments(
    conn: &mut DbConn,
    eval_id: Uuid,
    evaluator_id: Uuid,
) -> Result<Vec<EvaluationAssignment>, AppError> {
    evaluation_assignments::table
        .filter(evaluation_assignments::evaluation_id.eq(eval_id))
        .filter(evaluation_assignments::evaluator_id.eq(evaluator_id))
        .order(evaluation_assignments::created_at.asc())
        .load(conn)
        .map_err(AppError::from)
}

pub fn transition_assignment_state(
    conn: &mut DbConn,
    assignment_id: Uuid,
    to_state: AssignmentState,
) -> Result<EvaluationAssignment, AppError> {
    diesel::update(evaluation_assignments::table.filter(evaluation_assignments::id.eq(assignment_id)))
        .set((
            evaluation_assignments::state.eq(&to_state),
            evaluation_assignments::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(AppError::from)?;

    find_assignment(conn, assignment_id)
}

pub fn create_action(
    conn: &mut DbConn,
    action: NewEvaluationAction,
) -> Result<EvaluationAction, AppError> {
    diesel::insert_into(evaluation_actions::table)
        .values(&action)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn list_actions(
    conn: &mut DbConn,
    assignment_id: Uuid,
) -> Result<Vec<EvaluationAction>, AppError> {
    evaluation_actions::table
        .filter(evaluation_actions::assignment_id.eq(assignment_id))
        .order(evaluation_actions::created_at.asc())
        .load(conn)
        .map_err(AppError::from)
}
