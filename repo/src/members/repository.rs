use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::common::{db::DbConn, errors::AppError};
use crate::members::model::{
    Member, MemberPreferences, MemberTier, NewMember, NewPointsLedger, NewWalletLedger,
    PointsLedger, PointsTxnType, WalletLedger, WalletTxnType,
};
use crate::schema::{member_preferences, members, points_ledger, wallet_ledger};

pub fn find_member(conn: &mut DbConn, user_id: Uuid) -> Result<Member, AppError> {
    members::table
        .filter(members::user_id.eq(user_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Member {} not found", user_id)))
}

pub fn create_member(conn: &mut DbConn, new_member: NewMember) -> Result<Member, AppError> {
    diesel::insert_into(members::table)
        .values(&new_member)
        .get_result(conn)
        .map_err(AppError::from)
}

/// Optimistic concurrency update: only updates if version matches.
pub fn update_points_balance(
    conn: &mut DbConn,
    user_id: Uuid,
    new_balance: i32,
    expected_version: i32,
) -> Result<(), AppError> {
    let rows = diesel::update(
        members::table
            .filter(members::user_id.eq(user_id))
            .filter(members::version.eq(expected_version)),
    )
    .set((
        members::points_balance.eq(new_balance),
        members::version.eq(expected_version + 1),
        members::updated_at.eq(Utc::now()),
    ))
    .execute(conn)
    .map_err(AppError::from)?;

    if rows == 0 {
        Err(AppError::PreconditionFailed(
            "Concurrent modification detected on member record".into(),
        ))
    } else {
        Ok(())
    }
}

pub fn update_wallet_balance(
    conn: &mut DbConn,
    user_id: Uuid,
    encrypted_balance: &str,
    expected_version: i32,
) -> Result<(), AppError> {
    let rows = diesel::update(
        members::table
            .filter(members::user_id.eq(user_id))
            .filter(members::version.eq(expected_version)),
    )
    .set((
        members::wallet_balance.eq(encrypted_balance),
        members::version.eq(expected_version + 1),
        members::updated_at.eq(Utc::now()),
    ))
    .execute(conn)
    .map_err(AppError::from)?;

    if rows == 0 {
        Err(AppError::PreconditionFailed(
            "Concurrent modification detected on member record".into(),
        ))
    } else {
        Ok(())
    }
}

pub fn update_tier(
    conn: &mut DbConn,
    user_id: Uuid,
    new_tier: MemberTier,
) -> Result<(), AppError> {
    diesel::update(members::table.filter(members::user_id.eq(user_id)))
        .set((members::tier.eq(new_tier), members::updated_at.eq(Utc::now())))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

pub fn update_rolling_spend(
    conn: &mut DbConn,
    user_id: Uuid,
    new_spend: i64,
) -> Result<(), AppError> {
    diesel::update(members::table.filter(members::user_id.eq(user_id)))
        .set((
            members::rolling_12m_spend.eq(new_spend),
            members::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

pub fn set_blacklist(
    conn: &mut DbConn,
    user_id: Uuid,
    flag: bool,
) -> Result<(), AppError> {
    let now = if flag { Some(Utc::now()) } else { None };
    diesel::update(members::table.filter(members::user_id.eq(user_id)))
        .set((
            members::blacklist_flag.eq(flag),
            members::blacklisted_at.eq(now),
            members::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

pub fn set_redemption_frozen_until(
    conn: &mut DbConn,
    user_id: Uuid,
    until: Option<chrono::DateTime<Utc>>,
) -> Result<(), AppError> {
    diesel::update(members::table.filter(members::user_id.eq(user_id)))
        .set((
            members::redemption_frozen_until.eq(until),
            members::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}

pub fn append_points_ledger(
    conn: &mut DbConn,
    entry: NewPointsLedger,
) -> Result<PointsLedger, AppError> {
    diesel::insert_into(points_ledger::table)
        .values(&entry)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn list_points_ledger(
    conn: &mut DbConn,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<PointsLedger>, i64), AppError> {
    let total: i64 = points_ledger::table
        .filter(points_ledger::user_id.eq(user_id))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let records = points_ledger::table
        .filter(points_ledger::user_id.eq(user_id))
        .order(points_ledger::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;

    Ok((records, total))
}

pub fn append_wallet_ledger(
    conn: &mut DbConn,
    entry: NewWalletLedger,
) -> Result<WalletLedger, AppError> {
    diesel::insert_into(wallet_ledger::table)
        .values(&entry)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn list_wallet_ledger(
    conn: &mut DbConn,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<WalletLedger>, i64), AppError> {
    let total: i64 = wallet_ledger::table
        .filter(wallet_ledger::user_id.eq(user_id))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let records = wallet_ledger::table
        .filter(wallet_ledger::user_id.eq(user_id))
        .order(wallet_ledger::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;

    Ok((records, total))
}

pub fn get_preferences(conn: &mut DbConn, user_id: Uuid) -> Result<MemberPreferences, AppError> {
    member_preferences::table
        .filter(member_preferences::user_id.eq(user_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Preferences for {} not found", user_id)))
}

pub fn upsert_preferences(
    conn: &mut DbConn,
    user_id: Uuid,
    opt_out: serde_json::Value,
    channel: &str,
    tz_offset_minutes: i32,
) -> Result<MemberPreferences, AppError> {
    diesel::insert_into(member_preferences::table)
        .values((
            member_preferences::user_id.eq(user_id),
            member_preferences::notification_opt_out.eq(&opt_out),
            member_preferences::preferred_channel.eq(channel),
            member_preferences::timezone_offset_minutes.eq(tz_offset_minutes),
            member_preferences::updated_at.eq(Utc::now()),
        ))
        .on_conflict(member_preferences::user_id)
        .do_update()
        .set((
            member_preferences::notification_opt_out.eq(&opt_out),
            member_preferences::preferred_channel.eq(channel),
            member_preferences::timezone_offset_minutes.eq(tz_offset_minutes),
            member_preferences::updated_at.eq(Utc::now()),
        ))
        .get_result(conn)
        .map_err(AppError::from)
}

/// Get all members for tier recalculation job.
pub fn list_all_member_ids(conn: &mut DbConn) -> Result<Vec<Uuid>, AppError> {
    members::table
        .select(members::user_id)
        .load(conn)
        .map_err(AppError::from)
}
