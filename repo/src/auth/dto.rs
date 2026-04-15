use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 1, message = "username is required"))]
    pub username: String,
    #[validate(length(min = 1, message = "password is required"))]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user_id: Uuid,
    pub username: String,
    pub role: String,
}
