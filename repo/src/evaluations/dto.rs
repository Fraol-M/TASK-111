use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;
use validator::Validate;

use crate::evaluations::model::{
    AssignmentState, EvaluationAction, EvaluationAssignment, EvaluationCycle, EvaluationState,
    Evaluation,
};
use crate::common::pagination::Page;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCycleRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateEvaluationRequest {
    pub cycle_id: Option<Uuid>,
    #[validate(length(min = 1, max = 500))]
    pub title: String,
    pub description: Option<String>,
    /// JSON array of user/group IDs this evaluation covers. Defaults to [] if omitted.
    pub participant_scope: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct TransitionEvaluationRequest {
    pub state: EvaluationState,
}

#[derive(Debug, Deserialize)]
pub struct CreateAssignmentRequest {
    pub evaluator_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct TransitionAssignmentRequest {
    pub state: AssignmentState,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddActionRequest {
    #[validate(length(min = 1, max = 100))]
    pub action_type: String,
    pub notes: Option<String>,
    pub payload: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
pub struct EvaluationCycleResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<EvaluationCycle> for EvaluationCycleResponse {
    fn from(c: EvaluationCycle) -> Self {
        Self {
            id: c.id,
            name: c.name,
            description: c.description,
            starts_at: c.starts_at,
            ends_at: c.ends_at,
            created_by: c.created_by,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EvaluationResponse {
    pub id: Uuid,
    pub cycle_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub state: EvaluationState,
    pub version: i32,
    pub created_by: Uuid,
    pub participant_scope: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Evaluation> for EvaluationResponse {
    fn from(e: Evaluation) -> Self {
        Self {
            id: e.id,
            cycle_id: e.cycle_id,
            title: e.title,
            description: e.description,
            state: e.state,
            version: e.version,
            created_by: e.created_by,
            participant_scope: e.participant_scope,
            created_at: e.created_at,
            updated_at: e.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EvaluationAssignmentResponse {
    pub id: Uuid,
    pub evaluation_id: Uuid,
    pub evaluator_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub state: AssignmentState,
    pub due_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<EvaluationAssignment> for EvaluationAssignmentResponse {
    fn from(a: EvaluationAssignment) -> Self {
        Self {
            id: a.id,
            evaluation_id: a.evaluation_id,
            evaluator_id: a.evaluator_id,
            subject_id: a.subject_id,
            state: a.state,
            due_at: a.due_at,
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EvaluationActionResponse {
    pub id: Uuid,
    pub assignment_id: Uuid,
    pub actor_id: Uuid,
    pub action_type: String,
    pub notes: Option<String>,
    pub payload: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
}

impl From<EvaluationAction> for EvaluationActionResponse {
    fn from(a: EvaluationAction) -> Self {
        Self {
            id: a.id,
            assignment_id: a.assignment_id,
            actor_id: a.actor_id,
            action_type: a.action_type,
            notes: a.notes,
            payload: a.payload,
            created_at: a.created_at,
        }
    }
}

pub type CycleListResponse = Page<EvaluationCycleResponse>;
pub type EvaluationListResponse = Page<EvaluationResponse>;
