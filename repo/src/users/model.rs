use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::UserRole"]
pub enum UserRole {
    #[db_rename = "administrator"]
    Administrator,
    #[db_rename = "operations_manager"]
    OperationsManager,
    #[db_rename = "finance"]
    Finance,
    #[db_rename = "asset_manager"]
    AssetManager,
    #[db_rename = "evaluator"]
    Evaluator,
    /// Reviewer: distinct from `Evaluator`. Reviewers can read completed
    /// evaluations and action approval/rejection. They do NOT perform the
    /// assessment work itself (that is the evaluator's role). Kept as a
    /// separate role so policy can grant narrower rights than evaluator.
    #[db_rename = "reviewer"]
    Reviewer,
    #[db_rename = "member"]
    Member,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Administrator => "administrator",
            UserRole::OperationsManager => "operations_manager",
            UserRole::Finance => "finance",
            UserRole::AssetManager => "asset_manager",
            UserRole::Evaluator => "evaluator",
            UserRole::Reviewer => "reviewer",
            UserRole::Member => "member",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "administrator" => Some(UserRole::Administrator),
            "operations_manager" => Some(UserRole::OperationsManager),
            "finance" => Some(UserRole::Finance),
            "asset_manager" => Some(UserRole::AssetManager),
            "evaluator" => Some(UserRole::Evaluator),
            "reviewer" => Some(UserRole::Reviewer),
            "member" => Some(UserRole::Member),
            _ => None,
        }
    }
}

#[derive(DbEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[ExistingTypePath = "crate::schema::sql_types::UserStatus"]
pub enum UserStatus {
    #[db_rename = "active"]
    Active,
    #[db_rename = "suspended"]
    Suspended,
    #[db_rename = "deleted"]
    Deleted,
}

#[derive(Debug, Queryable, Selectable, Clone, Serialize)]
#[diesel(table_name = crate::schema::users)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::users)]
pub struct NewUser {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Queryable, Selectable, Clone)]
#[diesel(table_name = crate::schema::password_history)]
pub struct PasswordHistory {
    pub id: Uuid,
    pub user_id: Uuid,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::password_history)]
pub struct NewPasswordHistory {
    pub id: Uuid,
    pub user_id: Uuid,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}
