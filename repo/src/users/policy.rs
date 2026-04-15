use uuid::Uuid;

use crate::common::{claims::Claims, errors::AppError};

pub fn require_role(claims: &Claims, allowed: &[&str]) -> Result<(), AppError> {
    if allowed.contains(&claims.role.as_str()) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Role '{}' is not permitted for this operation",
            claims.role
        )))
    }
}

pub fn require_self_or_role(
    claims: &Claims,
    target_id: Uuid,
    roles: &[&str],
) -> Result<(), AppError> {
    if claims.sub == target_id || roles.contains(&claims.role.as_str()) {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Access restricted to the resource owner or a privileged role".into(),
        ))
    }
}

pub const ADMIN: &[&str] = &["administrator"];
pub const OPS: &[&str] = &["administrator", "operations_manager"];
pub const FINANCE: &[&str] = &["administrator", "finance"];
pub const ASSET: &[&str] = &["administrator", "asset_manager"];
pub const EVALUATOR: &[&str] = &["administrator", "evaluator"];
/// Reviewer authority: admin always fits, reviewer explicitly allowed. Kept
/// separate from EVALUATOR because a reviewer is not authorized to perform
/// the assessment itself, only to approve/reject completed evaluations.
pub const REVIEWER: &[&str] = &["administrator", "reviewer"];
/// Combined evaluator + reviewer view of an evaluation (read access for
/// endpoints that both roles must see, e.g. reading a completed evaluation).
pub const EVALUATOR_OR_REVIEWER: &[&str] = &["administrator", "evaluator", "reviewer"];
pub const PRIVILEGED: &[&str] = &[
    "administrator",
    "operations_manager",
    "finance",
    "asset_manager",
    "evaluator",
    "reviewer",
];
