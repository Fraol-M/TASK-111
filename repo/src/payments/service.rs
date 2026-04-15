use chrono::Utc;
use uuid::Uuid;

use crate::audit::model::NewAuditLog;
use crate::audit::repository::insert_audit_log;
use crate::common::{db::DbPool, errors::AppError};
use crate::config::AppConfig;
use crate::payments::{
    model::{IntentState, NewPayment, NewPaymentAdjustment, NewPaymentIntent, NewRefund, Payment, PaymentAdjustment, PaymentIntent, PaymentState, Refund, RefundState},
    repository,
};

/// Create a payment intent. Idempotent by idempotency_key.
/// `tax_cents` is the non-loyalty-accruing portion of `amount_cents` and must
/// satisfy 0 <= tax_cents <= amount_cents.
pub async fn create_intent(
    pool: &DbPool,
    cfg: &AppConfig,
    booking_id: Option<Uuid>,
    member_id: Uuid,
    amount_cents: i64,
    tax_cents: i64,
    idempotency_key: String,
) -> Result<PaymentIntent, AppError> {
    if tax_cents < 0 || tax_cents > amount_cents {
        return Err(AppError::UnprocessableEntity(format!(
            "tax_cents must be between 0 and amount_cents ({}), got {}",
            amount_cents, tax_cents
        )));
    }

    let timeout_mins = cfg.payment.intent_timeout_minutes as i64;
    let pool_c = pool.clone();

    actix_web::web::block(move || -> Result<PaymentIntent, AppError> {
        let mut conn = pool_c.get()?;

        // Idempotency check
        if let Some(existing) = repository::find_intent_by_idempotency_key(&mut conn, &idempotency_key)? {
            return Ok(existing);
        }

        let now = Utc::now();
        let expires_at = now + chrono::Duration::minutes(timeout_mins);

        repository::create_intent(
            &mut conn,
            NewPaymentIntent {
                id: Uuid::new_v4(),
                booking_id,
                member_id,
                amount_cents,
                tax_cents,
                state: IntentState::Open,
                idempotency_key,
                expires_at,
                created_at: now,
                updated_at: now,
                version: 0,
            },
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Capture a payment from an open intent. Idempotent by idempotency_key.
pub async fn capture_payment(
    pool: &DbPool,
    intent_id: Uuid,
    payment_method: String,
    idempotency_key: String,
    external_reference: Option<String>,
) -> Result<Payment, AppError> {
    let pool_c = pool.clone();

    actix_web::web::block(move || -> Result<Payment, AppError> {
        let mut conn = pool_c.get()?;

        // Idempotency check (outside transaction — just a read)
        if let Some(existing) = repository::find_payment_by_idempotency_key(&mut conn, &idempotency_key)? {
            return Ok(existing);
        }

        // Capture intent, insert payment, record points, and update member in one transaction
        use diesel::prelude::*;
        conn.transaction::<_, AppError, _>(|conn| {
            // Lock and validate the intent
            let intent = repository::capture_intent(conn, intent_id)?;

            let now = Utc::now();
            let payment = repository::create_payment(
                conn,
                NewPayment {
                    id: Uuid::new_v4(),
                    intent_id,
                    member_id: intent.member_id,
                    booking_id: intent.booking_id,
                    amount_cents: intent.amount_cents,
                    tax_cents: intent.tax_cents,
                    payment_method,
                    state: PaymentState::Completed,
                    idempotency_key,
                    external_reference,
                    version: 0,
                    created_at: now,
                    updated_at: now,
                },
            )?;

            // Earn points on the net (taxable) amount only:
            //   points = floor((amount_cents - tax_cents) / 100)
            // Tax is excluded because the loyalty rule is "1 point per $1 net of tax".
            let net_cents = intent.amount_cents - intent.tax_cents;
            let points_delta = (net_cents / 100) as i32;
            let member_id = intent.member_id;

            use crate::schema::members;

            // Read current points balance to compute balance_after
            let current_balance: i32 = members::table
                .filter(members::user_id.eq(member_id))
                .select(members::points_balance)
                .first(conn)
                .map_err(AppError::from)?;

            let balance_after: i32 = current_balance + points_delta;

            // Append points ledger entry — column names and enum value must match schema
            diesel::sql_query(
                "INSERT INTO points_ledger (id, user_id, txn_type, delta, balance_after, reference_id, note, created_at) \
                 VALUES ($1, $2, 'earn', $3, $4, $5, 'Points earned from payment', $6)"
            )
            .bind::<diesel::sql_types::Uuid, _>(Uuid::new_v4())
            .bind::<diesel::sql_types::Uuid, _>(member_id)
            .bind::<diesel::sql_types::Integer, _>(points_delta)
            .bind::<diesel::sql_types::Integer, _>(balance_after)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Uuid>, _>(Some(payment.id))
            .bind::<diesel::sql_types::Timestamptz, _>(now)
            .execute(conn)
            .map_err(AppError::from)?;

            // Update member points balance.
            // rolling_12m_spend is owned by the recalculate_tier batch job, which derives it
            // from the timestamped payments ledger. Incrementing it here would create a
            // dual-write conflict: the batch job resets the field, making real-time increments
            // double-count between runs.
            diesel::update(members::table.filter(members::user_id.eq(member_id)))
                .set((
                    members::points_balance.eq(balance_after),
                    members::updated_at.eq(now),
                ))
                .execute(conn)
                .map_err(AppError::from)?;

            Ok(payment)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Request a refund. Enforces refund cap. Idempotent by idempotency_key.
pub async fn request_refund(
    pool: &DbPool,
    payment_id: Uuid,
    amount_cents: i64,
    reason: Option<String>,
    idempotency_key: String,
    requested_by: Uuid,
    correlation_id: Option<String>,
) -> Result<Refund, AppError> {
    let pool_c = pool.clone();

    actix_web::web::block(move || -> Result<Refund, AppError> {
        let mut conn = pool_c.get()?;

        // Idempotency check before acquiring any lock
        if let Some(existing) = repository::find_refund_by_idempotency_key(&mut conn, &idempotency_key)? {
            return Ok(existing);
        }

        // Wrap cap check + insert in a transaction with a row-level lock on the payment.
        // The FOR UPDATE lock ensures two concurrent requests cannot both read the same
        // total_refunded value and both pass the cap check — one will block until the
        // other commits, then re-read the updated total.
        use diesel::prelude::*;
        use crate::schema::payments;
        conn.transaction::<Refund, AppError, _>(|conn| {

            // Lock the payment row to serialize concurrent refund requests
            let payment: Payment = payments::table
                .filter(payments::id.eq(payment_id))
                .for_update()
                .first(conn)
                .map_err(|_| AppError::NotFound(format!("Payment {} not found", payment_id)))?;

            // Only completed or partially-refunded payments are refundable
            if payment.state != PaymentState::Completed && payment.state != PaymentState::PartiallyRefunded {
                return Err(AppError::PreconditionFailed(format!(
                    "Payment {} is in state {:?} and cannot be refunded (must be completed or partially_refunded)",
                    payment_id, payment.state
                )));
            }

            // Cap check — reads committed refund totals under the lock
            let existing_refunds = repository::list_refunds_for_payment(conn, payment_id)?;
            let total_refunded: i64 = existing_refunds
                .iter()
                .filter(|r| r.state != RefundState::Rejected)
                .map(|r| r.amount_cents)
                .sum();

            if total_refunded + amount_cents > payment.amount_cents {
                return Err(AppError::PreconditionFailed(format!(
                    "Refund of {} cents would exceed original payment of {} cents (already refunded: {})",
                    amount_cents, payment.amount_cents, total_refunded
                )));
            }

            let now = Utc::now();
            let refund = repository::create_refund(
                conn,
                NewRefund {
                    id: Uuid::new_v4(),
                    payment_id,
                    amount_cents,
                    reason: reason.clone(),
                    state: RefundState::Pending,
                    idempotency_key,
                    requested_by,
                    created_at: now,
                    updated_at: now,
                    version: 0,
                },
            )?;

            insert_audit_log(conn, NewAuditLog {
                id: Uuid::new_v4(),
                correlation_id,
                actor_user_id: Some(requested_by),
                action: "refund_requested".to_string(),
                entity_type: "payment".to_string(),
                entity_id: payment_id.to_string(),
                old_value: None,
                new_value: Some(serde_json::json!({
                    "refund_id": refund.id,
                    "amount_cents": amount_cents,
                    "reason": reason
                })),
                metadata: None,
                created_at: now,
                row_hash: String::new(),
                previous_hash: None,
            })?;

            Ok(refund)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Approve a refund (Finance only). Reverses points and rolling spend earned on the original capture.
pub async fn approve_refund(
    pool: &DbPool,
    refund_id: Uuid,
    approver_id: Uuid,
    correlation_id: Option<String>,
) -> Result<Refund, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<Refund, AppError> {
        let mut conn = pool_c.get()?;

        use diesel::prelude::*;
        use crate::schema::members;

        conn.transaction::<_, AppError, _>(|conn| {
            // Load & lock the payment BEFORE approving the refund so cumulative state
            // transitions (partially_refunded → refunded) remain serialized against
            // concurrent approvals. The FOR UPDATE lock also pins the version we read
            // for the optimistic-concurrency check below.
            let refund_payment_id: Uuid = crate::schema::refunds::table
                .filter(crate::schema::refunds::id.eq(refund_id))
                .select(crate::schema::refunds::payment_id)
                .first(conn)
                .map_err(|_| AppError::NotFound(format!("Refund {} not found", refund_id)))?;

            let payment: Payment = crate::schema::payments::table
                .filter(crate::schema::payments::id.eq(refund_payment_id))
                .for_update()
                .first(conn)
                .map_err(|_| AppError::NotFound(format!("Payment {} not found", refund_payment_id)))?;
            let payment_version_at_read = payment.version;

            let refund = repository::approve_refund(conn, refund_id, approver_id)?;
            let member_id = payment.member_id;
            let now = Utc::now();

            // Reverse the points that were earned on the refunded portion's net amount.
            // Apportion tax out of the refund proportionally to keep symmetry with capture:
            //   refund_net = refund.amount - round(refund.amount * tax / amount)
            //   points_to_reverse = floor(refund_net / 100)
            // Clamp to the current balance so points_balance never goes negative
            // (CHECK constraint: balance_after >= 0).
            let current_balance: i32 = members::table
                .filter(members::user_id.eq(member_id))
                .select(members::points_balance)
                .first(conn)
                .map_err(AppError::from)?;

            // Derive refund net by apportioning tax to the refund amount.
            // For a fully-refunded payment this recovers exactly amount - tax.
            let refund_tax = if payment.amount_cents > 0 {
                (refund.amount_cents as i128 * payment.tax_cents as i128
                    / payment.amount_cents as i128) as i64
            } else {
                0
            };
            let refund_net = refund.amount_cents - refund_tax;
            let points_delta = ((refund_net / 100) as i32).min(current_balance).max(0);
            let balance_after = current_balance - points_delta;

            // Append negative 'adjust' ledger entry ('refund' is not a valid points_txn_type).
            diesel::sql_query(
                "INSERT INTO points_ledger (id, user_id, txn_type, delta, balance_after, reference_id, note, created_at) \
                 VALUES ($1, $2, 'adjust', $3, $4, $5, 'Points reversed on refund approval', $6)"
            )
            .bind::<diesel::sql_types::Uuid, _>(Uuid::new_v4())
            .bind::<diesel::sql_types::Uuid, _>(member_id)
            .bind::<diesel::sql_types::Integer, _>(-points_delta)
            .bind::<diesel::sql_types::Integer, _>(balance_after)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Uuid>, _>(Some(refund.payment_id))
            .bind::<diesel::sql_types::Timestamptz, _>(now)
            .execute(conn)
            .map_err(AppError::from)?;

            // Update member points balance only.
            // rolling_12m_spend is owned by the recalculate_tier batch job; modifying it here
            // would fight the job's authoritative recomputation from the payments ledger.
            diesel::sql_query(
                "UPDATE members SET points_balance = $1, updated_at = $2 WHERE user_id = $3"
            )
            .bind::<diesel::sql_types::Integer, _>(balance_after)
            .bind::<diesel::sql_types::Timestamptz, _>(now)
            .bind::<diesel::sql_types::Uuid, _>(member_id)
            .execute(conn)
            .map_err(AppError::from)?;

            // Update payment state based on cumulative approved refund amount.
            // Optimistic concurrency: the update is gated on the version we observed
            // under the FOR UPDATE lock; a competing transition would have incremented
            // version and would cause this update to fail-fast with PreconditionFailed.
            let all_refunds = repository::list_refunds_for_payment(conn, refund.payment_id)?;
            let total_approved: i64 = all_refunds
                .iter()
                .filter(|r| r.state == crate::payments::model::RefundState::Approved)
                .map(|r| r.amount_cents)
                .sum();
            // Include the just-approved refund (its state was updated above)
            let new_payment_state = if total_approved >= payment.amount_cents {
                crate::payments::model::PaymentState::Refunded
            } else {
                crate::payments::model::PaymentState::PartiallyRefunded
            };
            repository::update_payment_state_versioned(
                conn,
                refund.payment_id,
                &new_payment_state,
                payment_version_at_read,
            )?;

            insert_audit_log(conn, NewAuditLog {
                id: Uuid::new_v4(),
                correlation_id,
                actor_user_id: Some(approver_id),
                action: "refund_approved".to_string(),
                entity_type: "refund".to_string(),
                entity_id: refund_id.to_string(),
                old_value: Some(serde_json::json!({ "state": "pending" })),
                new_value: Some(serde_json::json!({
                    "state": "approved",
                    "approved_by": approver_id,
                    "payment_id": refund.payment_id,
                    "amount_cents": refund.amount_cents,
                    "points_reversed": points_delta,
                    "payment_state": format!("{:?}", new_payment_state),
                })),
                metadata: None,
                created_at: now,
                row_hash: String::new(),
                previous_hash: None,
            })?;

            Ok(refund)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Create a payment adjustment. Starts in 'pending' state, requires explicit approval.
/// Emits a tamper-evident audit_logs entry at creation time so the compensation
/// trail is complete — `approve_adjustment` audits the approval, and now
/// `create_adjustment` audits the request itself (actor + payment + amount +
/// reason snapshot). Without this, a finance reviewer auditing a compensation
/// chain would see the approval event but not the original request.
pub async fn create_adjustment(
    pool: &DbPool,
    payment_id: Uuid,
    amount_cents: i64,
    reason: String,
    created_by: Uuid,
    correlation_id: Option<String>,
) -> Result<PaymentAdjustment, AppError> {
    let now = Utc::now();
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<PaymentAdjustment, AppError> {
        let mut conn = pool_c.get()?;
        // Single transaction so the adjustment row and the audit event either
        // both land or both roll back.
        use diesel::prelude::*;
        conn.transaction::<_, AppError, _>(|conn| {
            // Verify payment exists
            repository::find_payment(conn, payment_id)?;
            let adj = repository::create_adjustment(
                conn,
                NewPaymentAdjustment {
                    id: Uuid::new_v4(),
                    payment_id,
                    amount_cents,
                    reason: reason.clone(),
                    created_by,
                    state: "pending".to_string(),
                    created_at: now,
                    updated_at: now,
                },
            )?;

            insert_audit_log(conn, NewAuditLog {
                id: Uuid::new_v4(),
                correlation_id,
                actor_user_id: Some(created_by),
                action: "adjustment_created".to_string(),
                entity_type: "payment_adjustment".to_string(),
                entity_id: adj.id.to_string(),
                old_value: None,
                new_value: Some(serde_json::json!({
                    "adjustment_id": adj.id,
                    "payment_id": payment_id,
                    "amount_cents": amount_cents,
                    "reason": reason,
                    "state": "pending",
                })),
                metadata: None,
                created_at: now,
                row_hash: String::new(),
                previous_hash: None,
            })?;

            Ok(adj)
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Approve a pending payment adjustment (Finance only).
pub async fn approve_adjustment(
    pool: &DbPool,
    adjustment_id: Uuid,
    approver_id: Uuid,
    correlation_id: Option<String>,
) -> Result<PaymentAdjustment, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<PaymentAdjustment, AppError> {
        use crate::schema::payment_adjustments;
        use diesel::prelude::*;

        let mut conn = pool_c.get()?;

        let adj: PaymentAdjustment = payment_adjustments::table
            .filter(payment_adjustments::id.eq(adjustment_id))
            .first(&mut conn)
            .map_err(|_| AppError::NotFound(format!("Adjustment {} not found", adjustment_id)))?;

        if adj.state != "pending" {
            return Err(AppError::PreconditionFailed(format!(
                "Adjustment is already in '{}' state",
                adj.state
            )));
        }

        // Maker-checker: the person who created the adjustment cannot approve it.
        if adj.created_by == approver_id {
            return Err(AppError::Forbidden(
                "Self-approval is not permitted: the adjustment creator cannot also approve it".into(),
            ));
        }

        let updated: PaymentAdjustment = diesel::update(
            payment_adjustments::table.filter(payment_adjustments::id.eq(adjustment_id)),
        )
        .set((
            payment_adjustments::state.eq("approved"),
            payment_adjustments::approved_by.eq(Some(approver_id)),
            payment_adjustments::updated_at.eq(Utc::now()),
        ))
        .get_result(&mut conn)
        .map_err(AppError::from)?;

        insert_audit_log(&mut conn, NewAuditLog {
            id: Uuid::new_v4(),
            correlation_id,
            actor_user_id: Some(approver_id),
            action: "adjustment_approved".to_string(),
            entity_type: "payment_adjustment".to_string(),
            entity_id: adjustment_id.to_string(),
            old_value: Some(serde_json::json!({ "state": "pending" })),
            new_value: Some(serde_json::json!({
                "state": "approved",
                "approved_by": approver_id,
                "payment_id": updated.payment_id,
                "amount_cents": updated.amount_cents
            })),
            metadata: None,
            created_at: Utc::now(),
            row_hash: String::new(),
            previous_hash: None,
        })?;

        Ok(updated)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Close all expired open intents (background job).
pub async fn close_expired_intents(pool: &DbPool) -> Result<usize, AppError> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<usize, AppError> {
        let mut conn = pool_c.get()?;
        repository::close_expired_intents(&mut conn)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
