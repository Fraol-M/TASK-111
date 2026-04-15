use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::group_threads)]
pub struct GroupThread {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::group_threads)]
pub struct NewGroupThread {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::group_members)]
pub struct GroupMember {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
    pub removed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::group_members)]
pub struct NewGroupMember {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::group_messages)]
pub struct GroupMessage {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub sender_id: Uuid,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::group_messages)]
pub struct NewGroupMessage {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub sender_id: Uuid,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::group_message_receipts)]
pub struct GroupMessageReceipt {
    pub id: Uuid,
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub read_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::group_message_receipts)]
pub struct NewGroupMessageReceipt {
    pub id: Uuid,
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub read_at: DateTime<Utc>,
}
