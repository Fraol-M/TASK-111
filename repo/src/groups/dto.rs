use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::groups::model::{GroupMember, GroupMessage, GroupMessageReceipt, GroupThread};
use crate::common::pagination::Page;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateGroupRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PostMessageRequest {
    #[validate(length(min = 1, max = 10000))]
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct GroupThreadResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<GroupThread> for GroupThreadResponse {
    fn from(g: GroupThread) -> Self {
        Self {
            id: g.id,
            name: g.name,
            description: g.description,
            created_by: g.created_by,
            created_at: g.created_at,
            updated_at: g.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GroupMemberResponse {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
    pub removed_at: Option<DateTime<Utc>>,
}

impl From<GroupMember> for GroupMemberResponse {
    fn from(m: GroupMember) -> Self {
        Self {
            id: m.id,
            thread_id: m.thread_id,
            user_id: m.user_id,
            joined_at: m.joined_at,
            removed_at: m.removed_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GroupMessageResponse {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub sender_id: Uuid,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

impl From<GroupMessage> for GroupMessageResponse {
    fn from(m: GroupMessage) -> Self {
        Self {
            id: m.id,
            thread_id: m.thread_id,
            sender_id: m.sender_id,
            body: m.body,
            created_at: m.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GroupMessageReceiptResponse {
    pub id: Uuid,
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub read_at: DateTime<Utc>,
}

impl From<GroupMessageReceipt> for GroupMessageReceiptResponse {
    fn from(r: GroupMessageReceipt) -> Self {
        Self {
            id: r.id,
            message_id: r.message_id,
            user_id: r.user_id,
            read_at: r.read_at,
        }
    }
}

#[allow(dead_code)]
pub type GroupListResponse = Page<GroupThreadResponse>;
#[allow(dead_code)]
pub type MessageListResponse = Page<GroupMessageResponse>;
