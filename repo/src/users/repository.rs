use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::common::{db::DbConn, errors::AppError};
use crate::users::model::{NewPasswordHistory, NewUser, PasswordHistory, User, UserRole, UserStatus};
use crate::schema::{password_history, users};

pub fn create_user(conn: &mut DbConn, new_user: NewUser) -> Result<User, AppError> {
    diesel::insert_into(users::table)
        .values(&new_user)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_by_id(conn: &mut DbConn, user_id: Uuid) -> Result<User, AppError> {
    users::table
        .filter(users::id.eq(user_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("User {} not found", user_id)))
}

pub fn find_by_username(conn: &mut DbConn, uname: &str) -> Result<User, AppError> {
    users::table
        .filter(users::username.eq(uname))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("User '{}' not found", uname)))
}

pub fn list_users(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<User>, i64), AppError> {
    let total: i64 = users::table.count().get_result(conn).map_err(AppError::from)?;
    let records = users::table
        .order(users::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load::<User>(conn)
        .map_err(AppError::from)?;
    Ok((records, total))
}

pub fn update_user(
    conn: &mut DbConn,
    user_id: Uuid,
    username: Option<String>,
    role: Option<UserRole>,
) -> Result<User, AppError> {
    let existing = find_by_id(conn, user_id)?;
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set((
            users::username.eq(username.unwrap_or(existing.username)),
            users::role.eq(role.unwrap_or(existing.role)),
            users::updated_at.eq(Utc::now()),
        ))
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn update_user_status(
    conn: &mut DbConn,
    user_id: Uuid,
    new_status: UserStatus,
) -> Result<User, AppError> {
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set((users::status.eq(new_status), users::updated_at.eq(Utc::now())))
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn update_password_hash(
    conn: &mut DbConn,
    user_id: Uuid,
    new_hash: &str,
) -> Result<(), AppError> {
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set((
            users::password_hash.eq(new_hash),
            users::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

pub fn insert_password_history(
    conn: &mut DbConn,
    user_id: Uuid,
    hash: &str,
) -> Result<(), AppError> {
    let record = NewPasswordHistory {
        id: Uuid::new_v4(),
        user_id,
        password_hash: hash.to_string(),
        created_at: Utc::now(),
    };
    diesel::insert_into(password_history::table)
        .values(&record)
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

/// Returns the last N password hashes for the user.
pub fn get_recent_password_hashes(
    conn: &mut DbConn,
    user_id: Uuid,
    n: i64,
) -> Result<Vec<String>, AppError> {
    password_history::table
        .filter(password_history::user_id.eq(user_id))
        .order(password_history::created_at.desc())
        .limit(n)
        .load::<PasswordHistory>(conn)
        .map(|records| records.into_iter().map(|r| r.password_hash).collect())
        .map_err(AppError::from)
}
