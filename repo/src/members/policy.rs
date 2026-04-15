use uuid::Uuid;

use crate::common::{claims::Claims, errors::AppError};
use crate::users::policy;

/// Member profile access: self, admin, finance, or ops only.
/// Asset managers and evaluators have no business need for member data.
const MEMBER_VIEWER: &[&str] = &["administrator", "operations_manager", "finance"];

pub fn can_view_member(claims: &Claims, target_id: Uuid) -> Result<(), AppError> {
    policy::require_self_or_role(claims, target_id, MEMBER_VIEWER)
}

pub fn can_view_wallet(claims: &Claims, target_id: Uuid) -> Result<(), AppError> {
    // Finance and Admin can view full wallet; Member can only view own (masked by service)
    policy::require_self_or_role(claims, target_id, policy::FINANCE)
}

pub fn can_manage_points(claims: &Claims) -> Result<(), AppError> {
    policy::require_role(claims, policy::OPS)
}

pub fn can_manage_wallet(claims: &Claims) -> Result<(), AppError> {
    policy::require_role(claims, policy::FINANCE)
}

pub fn can_blacklist(claims: &Claims) -> Result<(), AppError> {
    policy::require_role(claims, policy::ADMIN)
}

pub fn can_view_preferences(claims: &Claims, target_id: Uuid) -> Result<(), AppError> {
    policy::require_self_or_role(claims, target_id, policy::ADMIN)
}

pub fn can_edit_preferences(claims: &Claims, target_id: Uuid) -> Result<(), AppError> {
    // Only the member can edit their own preferences
    if claims.sub == target_id {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Only the member can edit their own preferences".into(),
        ))
    }
}

pub fn is_finance_or_admin(role: &str) -> bool {
    matches!(role, "administrator" | "finance")
}
