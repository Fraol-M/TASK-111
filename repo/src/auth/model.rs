use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

#[derive(Debug, Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::auth_sessions)]
pub struct AuthSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::auth_sessions)]
pub struct NewAuthSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
