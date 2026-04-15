use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::common::{db::DbConn, errors::AppError};
use crate::groups::model::{
    GroupMember, GroupMessage, GroupMessageReceipt, GroupThread, NewGroupMember, NewGroupMessage,
    NewGroupMessageReceipt, NewGroupThread,
};
use crate::schema::{group_members, group_message_receipts, group_messages, group_threads};

pub fn create_thread(conn: &mut DbConn, thread: NewGroupThread) -> Result<GroupThread, AppError> {
    diesel::insert_into(group_threads::table)
        .values(&thread)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_thread(conn: &mut DbConn, thread_id: Uuid) -> Result<GroupThread, AppError> {
    group_threads::table
        .filter(group_threads::id.eq(thread_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Group {} not found", thread_id)))
}

pub fn list_threads_for_user(
    conn: &mut DbConn,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<GroupThread>, i64), AppError> {
    // Threads where user is an active member
    let member_thread_ids: Vec<Uuid> = group_members::table
        .filter(group_members::user_id.eq(user_id))
        .filter(group_members::removed_at.is_null())
        .select(group_members::thread_id)
        .load(conn)
        .map_err(AppError::from)?;

    let total: i64 = group_threads::table
        .filter(group_threads::id.eq_any(&member_thread_ids))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let records = group_threads::table
        .filter(group_threads::id.eq_any(&member_thread_ids))
        .order(group_threads::updated_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;

    Ok((records, total))
}

pub fn list_all_threads(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<GroupThread>, i64), AppError> {
    let total: i64 = group_threads::table.count().get_result(conn).map_err(AppError::from)?;
    let records = group_threads::table
        .order(group_threads::updated_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((records, total))
}

pub fn add_member(conn: &mut DbConn, member: NewGroupMember) -> Result<GroupMember, AppError> {
    // Check if already an active member
    let existing: Option<GroupMember> = group_members::table
        .filter(group_members::thread_id.eq(member.thread_id))
        .filter(group_members::user_id.eq(member.user_id))
        .filter(group_members::removed_at.is_null())
        .first(conn)
        .optional()
        .map_err(AppError::from)?;

    if existing.is_some() {
        return Err(AppError::Conflict("User is already a member of this group".into()));
    }

    diesel::insert_into(group_members::table)
        .values(&member)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn remove_member(
    conn: &mut DbConn,
    thread_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let rows = diesel::update(
        group_members::table
            .filter(group_members::thread_id.eq(thread_id))
            .filter(group_members::user_id.eq(user_id))
            .filter(group_members::removed_at.is_null()),
    )
    .set(group_members::removed_at.eq(Some(Utc::now())))
    .execute(conn)
    .map_err(AppError::from)?;

    if rows == 0 {
        return Err(AppError::NotFound("Member not found in group".into()));
    }

    Ok(())
}

pub fn is_active_member(conn: &mut DbConn, thread_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
    let count: i64 = group_members::table
        .filter(group_members::thread_id.eq(thread_id))
        .filter(group_members::user_id.eq(user_id))
        .filter(group_members::removed_at.is_null())
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;
    Ok(count > 0)
}

pub fn list_members(conn: &mut DbConn, thread_id: Uuid) -> Result<Vec<GroupMember>, AppError> {
    group_members::table
        .filter(group_members::thread_id.eq(thread_id))
        .filter(group_members::removed_at.is_null())
        .load(conn)
        .map_err(AppError::from)
}

pub fn create_message(
    conn: &mut DbConn,
    msg: NewGroupMessage,
) -> Result<GroupMessage, AppError> {
    let thread_id = msg.thread_id;
    let message = diesel::insert_into(group_messages::table)
        .values(&msg)
        .get_result(conn)
        .map_err(AppError::from)?;

    // Update thread updated_at
    diesel::update(group_threads::table.filter(group_threads::id.eq(thread_id)))
        .set(group_threads::updated_at.eq(Utc::now()))
        .execute(conn)
        .map_err(AppError::from)?;

    Ok(message)
}

pub fn list_messages(
    conn: &mut DbConn,
    thread_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<GroupMessage>, i64), AppError> {
    let total: i64 = group_messages::table
        .filter(group_messages::thread_id.eq(thread_id))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let records = group_messages::table
        .filter(group_messages::thread_id.eq(thread_id))
        .order(group_messages::created_at.asc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;

    Ok((records, total))
}

pub fn mark_message_read(
    conn: &mut DbConn,
    thread_id: Uuid,
    message_id: Uuid,
    user_id: Uuid,
) -> Result<GroupMessageReceipt, AppError> {
    // Enforce object-level binding: message must belong to the declared thread
    let msg_thread: Option<Uuid> = group_messages::table
        .filter(group_messages::id.eq(message_id))
        .select(group_messages::thread_id)
        .first(conn)
        .optional()
        .map_err(AppError::from)?;

    match msg_thread {
        None => return Err(AppError::NotFound("Message not found".into())),
        Some(tid) if tid != thread_id => {
            return Err(AppError::Forbidden("Message does not belong to this thread".into()))
        }
        Some(_) => {}
    }

    // Idempotent: if already read, return existing
    let existing: Option<GroupMessageReceipt> = group_message_receipts::table
        .filter(group_message_receipts::message_id.eq(message_id))
        .filter(group_message_receipts::user_id.eq(user_id))
        .first(conn)
        .optional()
        .map_err(AppError::from)?;

    if let Some(receipt) = existing {
        return Ok(receipt);
    }

    diesel::insert_into(group_message_receipts::table)
        .values(&NewGroupMessageReceipt {
            id: Uuid::new_v4(),
            message_id,
            user_id,
            read_at: Utc::now(),
        })
        .get_result(conn)
        .map_err(AppError::from)
}
