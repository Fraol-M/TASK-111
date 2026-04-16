use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::EvaluationState"]
pub enum EvaluationState {
    #[db_rename = "draft"]
    Draft,
    #[db_rename = "open"]
    Open,
    #[db_rename = "in_review"]
    InReview,
    #[db_rename = "completed"]
    Completed,
    #[db_rename = "cancelled"]
    Cancelled,
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::AssignmentState"]
pub enum AssignmentState {
    #[db_rename = "pending"]
    Pending,
    #[db_rename = "in_progress"]
    InProgress,
    #[db_rename = "submitted"]
    Submitted,
    #[db_rename = "approved"]
    Approved,
    #[db_rename = "rejected"]
    Rejected,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::evaluation_cycles)]
pub struct EvaluationCycle {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::evaluation_cycles)]
pub struct NewEvaluationCycle {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::evaluations)]
pub struct Evaluation {
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

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::evaluations)]
pub struct NewEvaluation {
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

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::evaluation_assignments)]
pub struct EvaluationAssignment {
    pub id: Uuid,
    pub evaluation_id: Uuid,
    pub evaluator_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub state: AssignmentState,
    pub due_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::evaluation_assignments)]
pub struct NewEvaluationAssignment {
    pub id: Uuid,
    pub evaluation_id: Uuid,
    pub evaluator_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub state: AssignmentState,
    pub due_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::evaluation_actions)]
pub struct EvaluationAction {
    pub id: Uuid,
    pub assignment_id: Uuid,
    pub actor_id: Uuid,
    pub action_type: String,
    pub notes: Option<String>,
    pub payload: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::evaluation_actions)]
pub struct NewEvaluationAction {
    pub id: Uuid,
    pub assignment_id: Uuid,
    pub actor_id: Uuid,
    pub action_type: String,
    pub notes: Option<String>,
    pub payload: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
}
