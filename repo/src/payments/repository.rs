use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::common::{db::DbConn, errors::AppError};
use crate::payments::model::{
    IntentState, NewPayment, NewPaymentAdjustment, NewPaymentIntent, NewRefund, Payment,
    PaymentAdjustment, PaymentIntent, PaymentState, Refund, RefundState,
};
use crate::schema::{payment_adjustments, payment_intents, payments, refunds};

pub fn create_intent(conn: &mut DbConn, intent: NewPaymentIntent) -> Result<PaymentIntent, AppError> {
    diesel::insert_into(payment_intents::table)
        .values(&intent)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_intent(conn: &mut DbConn, intent_id: Uuid) -> Result<PaymentIntent, AppError> {
    payment_intents::table
        .filter(payment_intents::id.eq(intent_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Payment intent {} not found", intent_id)))
}

pub fn find_intent_by_idempotency_key(
    conn: &mut DbConn,
    key: &str,
) -> Result<Option<PaymentIntent>, AppError> {
    payment_intents::table
        .filter(payment_intents::idempotency_key.eq(key))
        .first(conn)
        .optional()
        .map_err(AppError::from)
}

pub fn capture_intent(
    conn: &mut DbConn,
    intent_id: Uuid,
) -> Result<PaymentIntent, AppError> {
    // FOR UPDATE serializes concurrent captures; the version-checked WHERE clause
    // below provides lost-update detection for any other code path that races to
    // transition the same intent (e.g. the timeout sweeper).
    let intent: PaymentIntent = payment_intents::table
        .filter(payment_intents::id.eq(intent_id))
        .for_update()
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Intent {} not found", intent_id)))?;

    if intent.state != IntentState::Open {
        return Err(AppError::PreconditionFailed(format!(
            "Intent is {:?}, not open",
            intent.state
        )));
    }

    if intent.expires_at < Utc::now() {
        return Err(AppError::PreconditionFailed("Payment intent has expired".into()));
    }

    let expected_version = intent.version;
    let rows = diesel::update(
        payment_intents::table
            .filter(payment_intents::id.eq(intent_id))
            .filter(payment_intents::version.eq(expected_version)),
    )
    .set((
        payment_intents::state.eq(IntentState::Captured),
        payment_intents::version.eq(expected_version + 1),
        payment_intents::updated_at.eq(Utc::now()),
    ))
    .execute(conn)
    .map_err(AppError::from)?;

    if rows == 0 {
        return Err(AppError::PreconditionFailed(format!(
            "Concurrent modification detected on payment intent {}: expected version {}",
            intent_id, expected_version
        )));
    }

    find_intent(conn, intent_id)
}

pub fn close_expired_intents(conn: &mut DbConn) -> Result<usize, AppError> {
    // Batch expire-sweep: each row's version is incremented so any concurrent
    // capture on the same id will see a mismatched version and abort.
    diesel::update(
        payment_intents::table
            .filter(payment_intents::state.eq(IntentState::Open))
            .filter(payment_intents::expires_at.lt(Utc::now())),
    )
    .set((
        payment_intents::state.eq(IntentState::TimedOut),
        payment_intents::version.eq(payment_intents::version + 1),
        payment_intents::updated_at.eq(Utc::now()),
    ))
    .execute(conn)
    .map_err(AppError::from)
}

pub fn create_payment(conn: &mut DbConn, payment: NewPayment) -> Result<Payment, AppError> {
    diesel::insert_into(payments::table)
        .values(&payment)
        .get_result(conn)
        .map_err(AppError::from)
}

/// Transition a payment's state with optimistic concurrency. The caller supplies
/// the version they observed; the update only succeeds if the row's version still
/// matches (and the version is then incremented). Returns the reloaded payment.
///
/// Callers that hold a row-level lock (SELECT ... FOR UPDATE) still benefit from
/// this because version progression lets independent readers detect changes, and
/// because lost-update protection is needed on any aggregate state transition
/// whether or not the current code path serializes access.
pub fn update_payment_state_versioned(
    conn: &mut DbConn,
    payment_id: Uuid,
    new_state: &PaymentState,
    expected_version: i32,
) -> Result<Payment, AppError> {
    let rows = diesel::update(
        payments::table
            .filter(payments::id.eq(payment_id))
            .filter(payments::version.eq(expected_version)),
    )
    .set((
        payments::state.eq(new_state),
        payments::version.eq(expected_version + 1),
        payments::updated_at.eq(Utc::now()),
    ))
    .execute(conn)
    .map_err(AppError::from)?;

    if rows == 0 {
        return Err(AppError::PreconditionFailed(format!(
            "Concurrent modification detected on payment {}: expected version {}",
            payment_id, expected_version
        )));
    }

    find_payment(conn, payment_id)
}

pub fn find_payment(conn: &mut DbConn, payment_id: Uuid) -> Result<Payment, AppError> {
    payments::table
        .filter(payments::id.eq(payment_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Payment {} not found", payment_id)))
}

pub fn find_payment_by_idempotency_key(
    conn: &mut DbConn,
    key: &str,
) -> Result<Option<Payment>, AppError> {
    payments::table
        .filter(payments::idempotency_key.eq(key))
        .first(conn)
        .optional()
        .map_err(AppError::from)
}

pub fn list_payments(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Payment>, i64), AppError> {
    let total: i64 = payments::table.count().get_result(conn).map_err(AppError::from)?;
    let records = payments::table
        .order(payments::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((records, total))
}

pub fn create_refund(conn: &mut DbConn, refund: NewRefund) -> Result<Refund, AppError> {
    diesel::insert_into(refunds::table)
        .values(&refund)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_refund(conn: &mut DbConn, refund_id: Uuid) -> Result<Refund, AppError> {
    refunds::table
        .filter(refunds::id.eq(refund_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Refund {} not found", refund_id)))
}

pub fn find_refund_by_idempotency_key(
    conn: &mut DbConn,
    key: &str,
) -> Result<Option<Refund>, AppError> {
    refunds::table
        .filter(refunds::idempotency_key.eq(key))
        .first(conn)
        .optional()
        .map_err(AppError::from)
}

pub fn list_refunds_for_payment(
    conn: &mut DbConn,
    payment_id: Uuid,
) -> Result<Vec<Refund>, AppError> {
    refunds::table
        .filter(refunds::payment_id.eq(payment_id))
        .load(conn)
        .map_err(AppError::from)
}

pub fn approve_refund(
    conn: &mut DbConn,
    refund_id: Uuid,
    approver_id: Uuid,
) -> Result<Refund, AppError> {
    // Version-checked transition so two finance approvers racing on the same
    // refund cannot both succeed. The state filter still guards against
    // approve-after-reject; the version filter guards against any other
    // concurrent mutation that would silently get clobbered.
    let current = find_refund(conn, refund_id)?;
    if current.state != RefundState::Pending {
        return Err(AppError::PreconditionFailed("Refund is not in Pending state".into()));
    }
    let expected_version = current.version;

    let rows = diesel::update(
        refunds::table
            .filter(refunds::id.eq(refund_id))
            .filter(refunds::version.eq(expected_version))
            .filter(refunds::state.eq(RefundState::Pending)),
    )
    .set((
        refunds::state.eq(RefundState::Approved),
        refunds::approved_by.eq(Some(approver_id)),
        refunds::version.eq(expected_version + 1),
        refunds::updated_at.eq(Utc::now()),
    ))
    .execute(conn)
    .map_err(AppError::from)?;

    if rows == 0 {
        return Err(AppError::PreconditionFailed(format!(
            "Concurrent modification detected on refund {}: expected version {}",
            refund_id, expected_version
        )));
    }

    find_refund(conn, refund_id)
}

pub fn create_adjustment(
    conn: &mut DbConn,
    adj: NewPaymentAdjustment,
) -> Result<PaymentAdjustment, AppError> {
    diesel::insert_into(payment_adjustments::table)
        .values(&adj)
        .get_result(conn)
        .map_err(AppError::from)
}
