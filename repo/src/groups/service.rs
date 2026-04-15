use chrono::Utc;
use uuid::Uuid;

use crate::common::{db::DbPool, errors::AppError};
use crate::groups::{
    model::{GroupMessage, GroupMember, GroupThread, NewGroupMember, NewGroupMessage, NewGroupThread},
    repository,
};

pub async fn create_group(
    pool: &DbPool,
    name: String,
    description: Option<String>,
    created_by: Uuid,
) -> Result<GroupThread, AppError> {
    let now = Utc::now();
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<GroupThread, AppError> {
        let mut conn = pool_c.get()?;
        repository::create_thread(
            &mut conn,
            NewGroupThread {
                id: Uuid::new_v4(),
                name,
                description,
                created_by,
                created_at: now,
                updated_at: now,
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn add_member(
    pool: &DbPool,
    thread_id: Uuid,
    user_id: Uuid,
) -> Result<GroupMember, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<GroupMember, AppError> {
        let mut conn = pool_c.get()?;
        // Verify thread exists
        repository::find_thread(&mut conn, thread_id)?;
        repository::add_member(
            &mut conn,
            NewGroupMember {
                id: Uuid::new_v4(),
                thread_id,
                user_id,
                joined_at: Utc::now(),
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn remove_member(
    pool: &DbPool,
    thread_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<(), AppError> {
        let mut conn = pool_c.get()?;
        repository::remove_member(&mut conn, thread_id, user_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn post_message(
    pool: &DbPool,
    thread_id: Uuid,
    sender_id: Uuid,
    body: String,
) -> Result<GroupMessage, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<GroupMessage, AppError> {
        let mut conn = pool_c.get()?;
        // Verify sender is an active member
        if !repository::is_active_member(&mut conn, thread_id, sender_id)? {
            return Err(AppError::Forbidden("Not a member of this group".into()));
        }
        repository::create_message(
            &mut conn,
            NewGroupMessage {
                id: Uuid::new_v4(),
                thread_id,
                sender_id,
                body,
                created_at: Utc::now(),
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
