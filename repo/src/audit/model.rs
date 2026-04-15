use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Queryable, Selectable, Serialize, Clone)]
#[diesel(table_name = crate::schema::audit_logs)]
pub struct AuditLog {
    pub id: Uuid,
    pub correlation_id: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub row_hash: String,
    pub previous_hash: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::audit_logs)]
pub struct NewAuditLog {
    pub id: Uuid,
    pub correlation_id: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub row_hash: String,
    pub previous_hash: Option<String>,
}

/// Idempotency key record for common::idempotency module.
#[derive(Debug, Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::idempotency_keys)]
pub struct IdempotencyKey {
    pub id: Uuid,
    pub key_value: String,
    pub request_hash: String,
    pub response_status: Option<i16>,
    pub response_body: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Job run record.
#[derive(Debug, Queryable, Selectable, Serialize, Clone)]
#[diesel(table_name = crate::schema::job_runs)]
pub struct JobRun {
    pub id: Uuid,
    pub job_name: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub items_processed: Option<i32>,
    pub error_detail: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::job_runs)]
pub struct NewJobRun {
    pub id: Uuid,
    pub job_name: String,
    pub started_at: DateTime<Utc>,
    pub status: String,
}
