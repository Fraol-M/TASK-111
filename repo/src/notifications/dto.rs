use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;
use validator::Validate;

use crate::notifications::model::{
    DeliveryState, Notification, NotificationChannel, NotificationTemplate, TemplateTrigger,
};
use crate::common::pagination::Page;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateTemplateRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub trigger_type: TemplateTrigger,
    pub channel: NotificationChannel,
    pub subject_template: Option<String>,
    #[validate(length(min = 1))]
    pub body_template: String,
    pub variable_schema: Option<JsonValue>,
    #[serde(default)]
    pub is_critical: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTemplateRequest {
    pub name: Option<String>,
    pub subject_template: Option<String>,
    pub body_template: Option<String>,
    pub variable_schema: Option<JsonValue>,
    pub is_critical: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PreviewRequest {
    pub template_id: Uuid,
    #[serde(default)]
    pub variables: std::collections::HashMap<String, JsonValue>,
}

#[derive(Debug, Serialize)]
pub struct PreviewResponse {
    pub subject: Option<String>,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub template_id: Option<Uuid>,
    pub trigger_type: TemplateTrigger,
    pub channel: NotificationChannel,
    pub subject: Option<String>,
    pub body: String,
    pub delivery_state: DeliveryState,
    pub dnd_suppressed: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Notification> for NotificationResponse {
    fn from(n: Notification) -> Self {
        Self {
            id: n.id,
            user_id: n.user_id,
            template_id: n.template_id,
            trigger_type: n.trigger_type,
            channel: n.channel,
            subject: n.subject,
            body: n.body,
            delivery_state: n.delivery_state,
            dnd_suppressed: n.dnd_suppressed,
            read_at: n.read_at,
            created_at: n.created_at,
            updated_at: n.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TemplateResponse {
    pub id: Uuid,
    pub name: String,
    pub trigger_type: TemplateTrigger,
    pub channel: NotificationChannel,
    pub subject_template: Option<String>,
    pub body_template: String,
    pub variable_schema: Option<JsonValue>,
    pub is_critical: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<NotificationTemplate> for TemplateResponse {
    fn from(t: NotificationTemplate) -> Self {
        Self {
            id: t.id,
            name: t.name,
            trigger_type: t.trigger_type,
            channel: t.channel,
            subject_template: t.subject_template,
            body_template: t.body_template,
            variable_schema: t.variable_schema,
            is_critical: t.is_critical,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

pub type NotificationListResponse = Page<NotificationResponse>;
pub type TemplateListResponse = Page<TemplateResponse>;
