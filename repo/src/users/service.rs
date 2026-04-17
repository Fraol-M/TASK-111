use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, PasswordHash, PasswordVerifier,
};
use chrono::Utc;
use uuid::Uuid;

use crate::common::{db::DbPool, errors::AppError};
use crate::users::{
    dto::{ChangePasswordRequest, ChangeStatusRequest, CreateUserRequest, UpdateUserRequest, UserResponse},
    model::{NewUser, UserRole, UserStatus},
    repository,
};

fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Password hashing failed: {}", e)))
}

fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(format!("Invalid hash format: {}", e)))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub async fn create_user(
    pool: &DbPool,
    req: CreateUserRequest,
) -> Result<UserResponse, AppError> {
    let hash = hash_password(&req.password)?;
    let mut conn = pool.get()?;

    let new_user = NewUser {
        id: Uuid::new_v4(),
        username: req.username.clone(),
        password_hash: hash.clone(),
        role: req.role.clone(),
        status: UserStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let new_user_id = new_user.id;
    let new_role = req.role.clone();

    let user = actix_web::web::block(move || {
        use diesel::prelude::*;
        // Single connection, single transaction: user + password history + member profile
        conn.transaction::<_, AppError, _>(|conn| {
            repository::create_user(conn, new_user)?;
            repository::insert_password_history(conn, new_user_id, &hash)?;

            // Auto-provision member profile when creating a Member-role user.
            // Member APIs (points, wallet, preferences, payments) require this row.
            if new_role == UserRole::Member {
                use crate::schema::members;
                diesel::insert_into(members::table)
                    .values((
                        members::user_id.eq(new_user_id),
                        members::tier.eq(crate::members::model::MemberTier::Silver),
                        members::points_balance.eq(0),
                        members::rolling_12m_spend.eq(0_i64),
                        members::updated_at.eq(Utc::now()),
                    ))
                    .execute(conn)
                    .map_err(AppError::from)?;
            }

            repository::find_by_id(conn, new_user_id)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(user.into())
}

pub async fn get_user(pool: &DbPool, user_id: Uuid) -> Result<UserResponse, AppError> {
    let mut conn = pool.get()?;
    let user = actix_web::web::block(move || repository::find_by_id(&mut conn, user_id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(user.into())
}

pub async fn list_users(
    pool: &DbPool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<UserResponse>, i64), AppError> {
    let mut conn = pool.get()?;
    let (users, total) =
        actix_web::web::block(move || repository::list_users(&mut conn, limit, offset))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok((users.into_iter().map(UserResponse::from).collect(), total))
}

pub async fn update_user(
    pool: &DbPool,
    user_id: Uuid,
    dto: UpdateUserRequest,
) -> Result<UserResponse, AppError> {
    let pool = pool.clone();
    let role_changed = dto.role.is_some();
    let new_role_opt = dto.role.clone();
    actix_web::web::block(move || -> Result<UserResponse, AppError> {
        use diesel::prelude::*;
        let mut conn = pool.get()?;

        // Whole update is transactional: role change + session revoke + member
        // provisioning either all commit or all roll back. This prevents a
        // user from landing in a state where they authenticate as Member but
        // member-domain endpoints 404 because the profile row is absent.
        let user = conn.transaction::<_, AppError, _>(|conn| {
            let user = repository::update_user(conn, user_id, dto.username, dto.role)?;

            // Revoke all active sessions when the role changes so any in-flight JWT
            // with the old role is invalidated immediately (prevents privilege persistence
            // after a downgrade: e.g. Administrator → Member).
            if role_changed {
                crate::auth::repository::revoke_all_user_sessions(conn, user_id)?;
            }

            // Provision (or resurrect) the member profile when the new role is Member.
            // Admins can re-role users in any direction, so we do an idempotent upsert
            // on the members row — ON CONFLICT keeps any historical wallet/points balance
            // intact rather than resetting an existing profile.
            if matches!(new_role_opt, Some(UserRole::Member)) {
                use crate::schema::{member_preferences, members};
                let now = Utc::now();
                diesel::insert_into(members::table)
                    .values((
                        members::user_id.eq(user_id),
                        members::tier.eq(crate::members::model::MemberTier::Silver),
                        members::points_balance.eq(0),
                        members::rolling_12m_spend.eq(0_i64),
                        members::updated_at.eq(now),
                    ))
                    .on_conflict(members::user_id)
                    .do_nothing()
                    .execute(conn)
                    .map_err(AppError::from)?;

                // Seed default preferences so notification channel resolution does
                // not log a NotFound on first lifecycle trigger.
                diesel::insert_into(member_preferences::table)
                    .values((
                        member_preferences::user_id.eq(user_id),
                        member_preferences::notification_opt_out.eq(serde_json::Value::Array(vec![])),
                        member_preferences::preferred_channel.eq("in_app"),
                        member_preferences::timezone_offset_minutes.eq(0),
                        member_preferences::updated_at.eq(now),
                    ))
                    .on_conflict(member_preferences::user_id)
                    .do_nothing()
                    .execute(conn)
                    .map_err(AppError::from)?;
            }

            Ok(user)
        })?;

        Ok(UserResponse::from(user))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn change_status(
    pool: &DbPool,
    user_id: Uuid,
    req: ChangeStatusRequest,
) -> Result<UserResponse, AppError> {
    let pool_c = pool.clone();
    let user = actix_web::web::block(move || -> Result<crate::users::model::User, AppError> {
        let mut conn = pool_c.get()?;
        let user = repository::update_user_status(&mut conn, user_id, req.status)?;
        // Revoke all active sessions immediately so token checks pick up the status change
        crate::auth::repository::revoke_all_user_sessions(&mut conn, user_id)?;
        Ok(user)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(user.into())
}

pub async fn change_password(
    pool: &DbPool,
    user_id: Uuid,
    req: ChangePasswordRequest,
    actor_role: &str,
) -> Result<(), AppError> {
    let pool_clone = pool.clone();
    let new_password = req.new_password.clone();
    let actor_role = actor_role.to_string();

    actix_web::web::block(move || -> Result<(), AppError> {
        let mut conn = pool_clone.get()?;

        let user = repository::find_by_id(&mut conn, user_id)?;

        // Non-admins must provide current password
        if actor_role.as_str() != "administrator" {
            let current = req.current_password.as_deref().unwrap_or("");
            if !verify_password(current, &user.password_hash)? {
                return Err(AppError::Forbidden("Current password is incorrect".into()));
            }
        }

        // Check against last 5 passwords
        let recent_hashes = repository::get_recent_password_hashes(&mut conn, user_id, 5)?;
        for old_hash in &recent_hashes {
            if verify_password(&new_password, old_hash)? {
                return Err(AppError::UnprocessableEntity(
                    "New password cannot be the same as any of the last 5 passwords".into(),
                ));
            }
        }

        let new_hash = hash_password(&new_password)?;
        repository::update_password_hash(&mut conn, user_id, &new_hash)?;
        repository::insert_password_history(&mut conn, user_id, &new_hash)?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
