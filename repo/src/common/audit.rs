use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::common::db::DbPool;
use crate::common::errors::AppError;

/// A single audit event to be persisted.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub correlation_id: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub action: &'static str,
    pub entity_type: &'static str,
    pub entity_id: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub metadata: Option<Value>,
}

#[async_trait]
#[allow(dead_code)]
pub trait AuditSink: Send + Sync + 'static {
    async fn record(&self, event: AuditEvent) -> Result<(), AppError>;
}

/// Production implementation — writes to the audit_logs table.
pub struct DbAuditSink(pub DbPool);

#[async_trait]
impl AuditSink for DbAuditSink {
    async fn record(&self, event: AuditEvent) -> Result<(), AppError> {
        let pool = self.0.clone();
        actix_web::web::block(move || -> Result<(), AppError> {
            let mut conn = pool.get()?;

            let new_log = crate::audit::model::NewAuditLog {
                id: Uuid::new_v4(),
                correlation_id: event.correlation_id,
                actor_user_id: event.actor_user_id,
                action: event.action.to_string(),
                entity_type: event.entity_type.to_string(),
                entity_id: event.entity_id,
                old_value: event.old_value,
                new_value: event.new_value,
                metadata: event.metadata,
                created_at: chrono::Utc::now(),
                row_hash: String::new(),
                previous_hash: None,
            };

            crate::audit::repository::insert_audit_log(&mut conn, new_log)?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::Internal(format!("Audit sink block error: {}", e)))?
    }
}

/// No-op sink for testing.
pub struct NoopAuditSink;

#[async_trait]
impl AuditSink for NoopAuditSink {
    async fn record(&self, _event: AuditEvent) -> Result<(), AppError> {
        Ok(())
    }
}
