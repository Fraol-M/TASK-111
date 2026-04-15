use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::audit::model::NewAuditLog;
use crate::audit::repository::insert_audit_log;
use crate::common::{crypto::EncryptionKey, db::DbPool, errors::AppError};
use crate::members::{
    model::{BlacklistReason, MemberTier, NewPointsLedger, NewWalletLedger, PointsTxnType, WalletTxnType},
    repository,
};
use crate::schema::blacklist_events;
use diesel::prelude::*;

pub const POINTS_PER_DOLLAR_CENTS: i32 = 100; // 1 pt per $1 = 1 pt per 100c
pub const REDEMPTION_INCREMENT: i32 = 100;

pub async fn get_member_info(
    pool: &DbPool,
    enc: &EncryptionKey,
    user_id: Uuid,
    requester_role: &str,
) -> Result<serde_json::Value, AppError> {
    let pool = pool.clone();
    let enc = enc.clone();
    let role = requester_role.to_string();

    actix_web::web::block(move || -> Result<serde_json::Value, AppError> {
        let mut conn = pool.get()?;
        let member = repository::find_member(&mut conn, user_id)?;

        // Decrypt wallet balance
        let raw_balance_cents: i64 = if member.wallet_balance.is_empty() {
            0
        } else {
            enc.decrypt(&member.wallet_balance)?
                .parse::<i64>()
                .unwrap_or(0)
        };

        let wallet_display = if crate::members::policy::is_finance_or_admin(&role) {
            format!("{:.2}", raw_balance_cents as f64 / 100.0)
        } else {
            EncryptionKey::mask(&format!("{}", raw_balance_cents), 4)
        };

        Ok(serde_json::json!({
            "user_id": member.user_id,
            "tier": member.tier.as_str(),
            "points_balance": member.points_balance,
            "wallet_balance_display": wallet_display,
            "blacklist_flag": member.blacklist_flag,
            "redemption_frozen_until": member.redemption_frozen_until,
            "rolling_12m_spend_cents": member.rolling_12m_spend,
            "updated_at": member.updated_at,
        }))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn earn_points(
    pool: &DbPool,
    user_id: Uuid,
    delta: i32,
    reference_id: Option<Uuid>,
    note: Option<String>,
    actor_id: Option<Uuid>,
    correlation_id: Option<String>,
) -> Result<(), AppError> {
    let pool = pool.clone();
    actix_web::web::block(move || -> Result<(), AppError> {
        let mut conn = pool.get()?;
        let member = repository::find_member(&mut conn, user_id)?;
        let old_balance = member.points_balance;
        // Compute with saturation to surface overflow deterministically rather
        // than panicking on an i32 overflow from a pathological delta.
        let new_balance = old_balance.saturating_add(delta);

        // Validate domain invariant BEFORE writing so clients receive a clean
        // 422 instead of a 500 from the DB CHECK (points_balance >= 0).
        // The CHECK is retained as defense-in-depth; this guard ensures
        // controlled error shape for the common case of a negative adjustment
        // exceeding the current balance.
        if new_balance < 0 {
            return Err(AppError::UnprocessableEntity(format!(
                "Points adjustment would drive balance negative: current={}, delta={}, result={}",
                old_balance, delta, new_balance
            )));
        }

        repository::update_points_balance(&mut conn, user_id, new_balance, member.version)?;
        repository::append_points_ledger(
            &mut conn,
            NewPointsLedger {
                id: Uuid::new_v4(),
                user_id,
                txn_type: PointsTxnType::Earn,
                delta,
                balance_after: new_balance,
                reference_id,
                note: note.clone(),
                created_at: Utc::now(),
            },
        )?;

        insert_audit_log(&mut conn, NewAuditLog {
            id: Uuid::new_v4(),
            correlation_id,
            actor_user_id: actor_id,
            action: "points_adjusted".to_string(),
            entity_type: "member".to_string(),
            entity_id: user_id.to_string(),
            old_value: Some(serde_json::json!({ "points_balance": old_balance })),
            new_value: Some(serde_json::json!({ "points_balance": new_balance, "delta": delta, "note": note })),
            metadata: None,
            created_at: Utc::now(),
            row_hash: String::new(),
            previous_hash: None,
        })?;

        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn redeem_points(
    pool: &DbPool,
    user_id: Uuid,
    amount_pts: i32,
    reference_id: Option<Uuid>,
) -> Result<(), AppError> {
    if amount_pts <= 0 || amount_pts % REDEMPTION_INCREMENT != 0 {
        return Err(AppError::UnprocessableEntity(format!(
            "Points redemption must be in multiples of {}",
            REDEMPTION_INCREMENT
        )));
    }

    let pool = pool.clone();
    actix_web::web::block(move || -> Result<(), AppError> {
        let mut conn = pool.get()?;
        let member = repository::find_member(&mut conn, user_id)?;

        // Check redemption freeze
        if let Some(frozen_until) = member.redemption_frozen_until {
            if frozen_until > Utc::now() {
                return Err(AppError::Forbidden(format!(
                    "Points redemption is frozen until {}",
                    frozen_until
                )));
            }
        }

        if member.points_balance < amount_pts {
            return Err(AppError::UnprocessableEntity(format!(
                "Insufficient points balance: have {}, need {}",
                member.points_balance, amount_pts
            )));
        }

        let new_balance = member.points_balance - amount_pts;
        repository::update_points_balance(&mut conn, user_id, new_balance, member.version)?;
        repository::append_points_ledger(
            &mut conn,
            NewPointsLedger {
                id: Uuid::new_v4(),
                user_id,
                txn_type: PointsTxnType::Redeem,
                delta: -amount_pts,
                balance_after: new_balance,
                reference_id,
                note: None,
                created_at: Utc::now(),
            },
        )?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn top_up_wallet(
    pool: &DbPool,
    enc: &EncryptionKey,
    user_id: Uuid,
    amount_cents: i64,
    note: Option<String>,
    actor_id: Option<Uuid>,
    correlation_id: Option<String>,
) -> Result<(), AppError> {
    if amount_cents <= 0 {
        return Err(AppError::UnprocessableEntity(
            "Top-up amount must be positive".into(),
        ));
    }

    let pool = pool.clone();
    let enc = enc.clone();

    actix_web::web::block(move || -> Result<(), AppError> {
        let mut conn = pool.get()?;
        let member = repository::find_member(&mut conn, user_id)?;

        let current_cents: i64 = if member.wallet_balance.is_empty() {
            0
        } else {
            enc.decrypt(&member.wallet_balance)?.parse::<i64>().unwrap_or(0)
        };

        let new_cents = current_cents + amount_cents;
        let encrypted = enc.encrypt(&new_cents.to_string())?;
        repository::update_wallet_balance(&mut conn, user_id, &encrypted, member.version)?;
        repository::append_wallet_ledger(
            &mut conn,
            NewWalletLedger {
                id: Uuid::new_v4(),
                user_id,
                txn_type: WalletTxnType::TopUp,
                delta_cents: amount_cents,
                balance_after_cents: new_cents,
                reference_id: None,
                note: note.clone(),
                created_at: Utc::now(),
            },
        )?;

        insert_audit_log(&mut conn, NewAuditLog {
            id: Uuid::new_v4(),
            correlation_id,
            actor_user_id: actor_id,
            action: "wallet_topup".to_string(),
            entity_type: "member".to_string(),
            entity_id: user_id.to_string(),
            old_value: None,
            new_value: Some(serde_json::json!({ "delta_cents": amount_cents, "note": note })),
            metadata: None,
            created_at: Utc::now(),
            row_hash: String::new(),
            previous_hash: None,
        })?;

        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn freeze_redemption(
    pool: &DbPool,
    user_id: Uuid,
    reason: BlacklistReason,
    note: Option<String>,
    actor_id: Uuid,
    correlation_id: Option<String>,
) -> Result<(), AppError> {
    // Prompt specifies a fixed 30-day redemption freeze; duration is not caller-controlled.
    const FREEZE_DAYS: i64 = 30;
    let pool = pool.clone();
    actix_web::web::block(move || -> Result<(), AppError> {
        let mut conn = pool.get()?;
        let until = Utc::now() + Duration::days(FREEZE_DAYS);
        repository::set_redemption_frozen_until(&mut conn, user_id, Some(until))?;

        // Log blacklist event
        diesel::insert_into(blacklist_events::table)
            .values((
                crate::schema::blacklist_events::id.eq(Uuid::new_v4()),
                crate::schema::blacklist_events::user_id.eq(user_id),
                crate::schema::blacklist_events::action.eq("freeze"),
                crate::schema::blacklist_events::reason.eq(Some(reason)),
                crate::schema::blacklist_events::duration_days.eq(FREEZE_DAYS as i32),
                crate::schema::blacklist_events::note.eq(note.clone()),
                crate::schema::blacklist_events::actor_user_id.eq(actor_id),
                crate::schema::blacklist_events::created_at.eq(Utc::now()),
            ))
            .execute(&mut conn)
            .map_err(AppError::from)?;

        insert_audit_log(&mut conn, NewAuditLog {
            id: Uuid::new_v4(),
            correlation_id,
            actor_user_id: Some(actor_id),
            action: "redemption_frozen".to_string(),
            entity_type: "member".to_string(),
            entity_id: user_id.to_string(),
            old_value: None,
            new_value: Some(serde_json::json!({
                "duration_days": FREEZE_DAYS,
                "frozen_until": until,
                "note": note
            })),
            metadata: None,
            created_at: Utc::now(),
            row_hash: String::new(),
            previous_hash: None,
        })?;

        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Blacklist a member and record an audit event.
pub async fn blacklist_member(
    pool: &DbPool,
    user_id: Uuid,
    reason: BlacklistReason,
    note: Option<String>,
    actor_id: Uuid,
    correlation_id: Option<String>,
) -> Result<(), AppError> {
    let pool = pool.clone();
    actix_web::web::block(move || -> Result<(), AppError> {
        let mut conn = pool.get()?;
        repository::set_blacklist(&mut conn, user_id, true)?;

        diesel::insert_into(blacklist_events::table)
            .values((
                crate::schema::blacklist_events::id.eq(Uuid::new_v4()),
                crate::schema::blacklist_events::user_id.eq(user_id),
                crate::schema::blacklist_events::action.eq("blacklist"),
                crate::schema::blacklist_events::duration_days.eq(0i32),
                crate::schema::blacklist_events::note.eq(note.clone()),
                crate::schema::blacklist_events::actor_user_id.eq(actor_id),
                crate::schema::blacklist_events::created_at.eq(Utc::now()),
            ))
            .execute(&mut conn)
            .map_err(AppError::from)?;

        insert_audit_log(&mut conn, NewAuditLog {
            id: Uuid::new_v4(),
            correlation_id,
            actor_user_id: Some(actor_id),
            action: "member_blacklisted".to_string(),
            entity_type: "member".to_string(),
            entity_id: user_id.to_string(),
            old_value: None,
            new_value: Some(serde_json::json!({
                "blacklist_flag": true,
                "reason": serde_json::to_value(&reason).unwrap_or_default(),
                "note": note
            })),
            metadata: None,
            created_at: Utc::now(),
            row_hash: String::new(),
            previous_hash: None,
        })?;

        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Force a member's tier to a specific value (Admin override), with audit trail.
pub async fn force_tier(
    pool: &DbPool,
    user_id: Uuid,
    tier: MemberTier,
    actor_id: Uuid,
    correlation_id: Option<String>,
) -> Result<(), AppError> {
    let pool = pool.clone();
    actix_web::web::block(move || -> Result<(), AppError> {
        let mut conn = pool.get()?;
        let member = repository::find_member(&mut conn, user_id)?;
        let old_tier = member.tier.clone();
        repository::update_tier(&mut conn, user_id, tier.clone())?;

        insert_audit_log(&mut conn, NewAuditLog {
            id: Uuid::new_v4(),
            correlation_id,
            actor_user_id: Some(actor_id),
            action: "tier_forced".to_string(),
            entity_type: "member".to_string(),
            entity_id: user_id.to_string(),
            old_value: Some(serde_json::json!({ "tier": old_tier.as_str() })),
            new_value: Some(serde_json::json!({ "tier": tier.as_str() })),
            metadata: None,
            created_at: Utc::now(),
            row_hash: String::new(),
            previous_hash: None,
        })?;

        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

pub async fn recalculate_tier(pool: &DbPool, user_id: Uuid) -> Result<MemberTier, AppError> {
    // Helper to extract a single i64 total from a raw SQL aggregate.
    #[derive(diesel::QueryableByName)]
    struct AmountSum {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        total: i64,
    }

    let pool = pool.clone();
    actix_web::web::block(move || -> Result<MemberTier, AppError> {
        let mut conn = pool.get()?;

        // Derive rolling_12m_spend from the timestamped payments ledger so that
        // payments that aged out of the 12-month window are automatically excluded.
        let cutoff = Utc::now() - Duration::days(365);

        let pay_row: AmountSum = diesel::sql_query(
            "SELECT COALESCE(SUM(amount_cents), 0) AS total \
             FROM payments WHERE member_id = $1 AND state = 'completed' AND created_at > $2"
        )
        .bind::<diesel::sql_types::Uuid, _>(user_id)
        .bind::<diesel::sql_types::Timestamptz, _>(cutoff)
        .get_result(&mut conn)
        .map_err(AppError::from)?;

        let ref_row: AmountSum = diesel::sql_query(
            "SELECT COALESCE(SUM(r.amount_cents), 0) AS total \
             FROM refunds r JOIN payments p ON r.payment_id = p.id \
             WHERE p.member_id = $1 AND r.state = 'approved' AND r.created_at > $2"
        )
        .bind::<diesel::sql_types::Uuid, _>(user_id)
        .bind::<diesel::sql_types::Timestamptz, _>(cutoff)
        .get_result(&mut conn)
        .map_err(AppError::from)?;

        let rolling_12m = (pay_row.total - ref_row.total).max(0);

        // Persist the recomputed value so real-time reads stay accurate between job runs.
        diesel::update(crate::schema::members::table.filter(crate::schema::members::user_id.eq(user_id)))
            .set(crate::schema::members::rolling_12m_spend.eq(rolling_12m))
            .execute(&mut conn)
            .map_err(AppError::from)?;

        let member = repository::find_member(&mut conn, user_id)?;
        let new_tier = MemberTier::from_spend_cents(rolling_12m);
        if new_tier != member.tier {
            let old_tier = member.tier.clone();
            // Insert tier history
            diesel::insert_into(crate::schema::member_tier_history::table)
                .values((
                    crate::schema::member_tier_history::id.eq(Uuid::new_v4()),
                    crate::schema::member_tier_history::user_id.eq(user_id),
                    crate::schema::member_tier_history::from_tier.eq(&old_tier),
                    crate::schema::member_tier_history::to_tier.eq(&new_tier),
                    crate::schema::member_tier_history::reason.eq("rolling_12m_recalc"),
                    crate::schema::member_tier_history::created_at.eq(Utc::now()),
                ))
                .execute(&mut conn)
                .map_err(AppError::from)?;
            repository::update_tier(&mut conn, user_id, new_tier.clone())?;

            // Tamper-evident audit entry for the tier transition. The batch
            // job has no HTTP actor, so `actor_user_id` is None (system-driven)
            // and the reason explicitly identifies the recalc pathway. This
            // closes the gap where force_tier emitted `tier_forced` but the
            // nightly batch recalc silently changed tiers.
            insert_audit_log(&mut conn, NewAuditLog {
                id: Uuid::new_v4(),
                correlation_id: None,
                actor_user_id: None,
                action: "tier_recalculated".to_string(),
                entity_type: "member".to_string(),
                entity_id: user_id.to_string(),
                old_value: Some(serde_json::json!({ "tier": old_tier.as_str() })),
                new_value: Some(serde_json::json!({
                    "tier": new_tier.as_str(),
                    "reason": "rolling_12m_recalc",
                    "rolling_12m_spend_cents": rolling_12m,
                })),
                metadata: None,
                created_at: Utc::now(),
                row_hash: String::new(),
                previous_hash: None,
            })?;
        }
        Ok(new_tier)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
