use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::users::model::{UserRole, UserStatus};

#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(min = 3, max = 100, message = "username must be 3-100 characters"))]
    pub username: String,
    #[validate(length(min = 8, message = "password must be at least 8 characters"))]
    pub password: String,
    pub role: UserRole,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[validate(length(min = 3, max = 100))]
    pub username: Option<String>,
    pub role: Option<UserRole>,
}

#[derive(Debug, Deserialize)]
pub struct ChangeStatusRequest {
    pub status: UserStatus,
    #[allow(dead_code)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 8, message = "password must be at least 8 characters"))]
    pub current_password: Option<String>,
    #[validate(length(min = 8, message = "new password must be at least 8 characters"))]
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub role: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<crate::users::model::User> for UserResponse {
    fn from(u: crate::users::model::User) -> Self {
        UserResponse {
            id: u.id,
            username: u.username,
            role: u.role.as_str().to_string(),
            status: format!("{:?}", u.status).to_lowercase(),
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}
