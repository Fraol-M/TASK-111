use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::bookings::model::{Booking, BookingItem, BookingState, BookingStatusHistory, NewBooking, NewBookingItem};
use crate::common::{db::DbConn, errors::AppError};
use crate::schema::{booking_items, booking_status_history, bookings};

pub fn create_booking(conn: &mut DbConn, new_booking: NewBooking) -> Result<Booking, AppError> {
    diesel::insert_into(bookings::table)
        .values(&new_booking)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn find_booking(conn: &mut DbConn, booking_id: Uuid) -> Result<Booking, AppError> {
    bookings::table
        .filter(bookings::id.eq(booking_id))
        .first(conn)
        .map_err(|_| AppError::NotFound(format!("Booking {} not found", booking_id)))
}

pub fn list_bookings_for_member(
    conn: &mut DbConn,
    member_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Booking>, i64), AppError> {
    let total: i64 = bookings::table
        .filter(bookings::member_id.eq(member_id))
        .count()
        .get_result(conn)
        .map_err(AppError::from)?;

    let records = bookings::table
        .filter(bookings::member_id.eq(member_id))
        .order(bookings::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;

    Ok((records, total))
}

pub fn list_all_bookings(
    conn: &mut DbConn,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Booking>, i64), AppError> {
    let total: i64 = bookings::table.count().get_result(conn).map_err(AppError::from)?;
    let records = bookings::table
        .order(bookings::created_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(AppError::from)?;
    Ok((records, total))
}

/// Transition booking state with optimistic concurrency and status history record.
pub fn transition_state(
    conn: &mut DbConn,
    booking_id: Uuid,
    from_state: BookingState,
    to_state: BookingState,
    reason: Option<String>,
    actor_id: Option<Uuid>,
    expected_version: i32,
) -> Result<Booking, AppError> {
    let rows = diesel::update(
        bookings::table
            .filter(bookings::id.eq(booking_id))
            .filter(bookings::version.eq(expected_version)),
    )
    .set((
        bookings::state.eq(&to_state),
        bookings::change_reason.eq(reason.as_deref()),
        bookings::version.eq(expected_version + 1),
        bookings::updated_at.eq(Utc::now()),
    ))
    .execute(conn)
    .map_err(AppError::from)?;

    if rows == 0 {
        return Err(AppError::PreconditionFailed(
            "Concurrent modification on booking".into(),
        ));
    }

    // Insert status history
    diesel::insert_into(booking_status_history::table)
        .values((
            booking_status_history::id.eq(Uuid::new_v4()),
            booking_status_history::booking_id.eq(booking_id),
            booking_status_history::from_state.eq(Some(&from_state)),
            booking_status_history::to_state.eq(&to_state),
            booking_status_history::reason.eq(reason.as_deref()),
            booking_status_history::actor_user_id.eq(actor_id),
            booking_status_history::created_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(AppError::from)?;

    find_booking(conn, booking_id)
}

pub fn create_booking_item(conn: &mut DbConn, item: NewBookingItem) -> Result<BookingItem, AppError> {
    diesel::insert_into(booking_items::table)
        .values(&item)
        .get_result(conn)
        .map_err(AppError::from)
}

pub fn list_booking_items(conn: &mut DbConn, booking_id: Uuid) -> Result<Vec<BookingItem>, AppError> {
    booking_items::table
        .filter(booking_items::booking_id.eq(booking_id))
        .load(conn)
        .map_err(AppError::from)
}

pub fn get_status_history(
    conn: &mut DbConn,
    booking_id: Uuid,
) -> Result<Vec<BookingStatusHistory>, AppError> {
    booking_status_history::table
        .filter(booking_status_history::booking_id.eq(booking_id))
        .order(booking_status_history::created_at.asc())
        .load(conn)
        .map_err(AppError::from)
}

/// Find all held bookings whose hold has expired (for the expiry job).
pub fn find_expired_held_bookings(conn: &mut DbConn) -> Result<Vec<Booking>, AppError> {
    bookings::table
        .filter(bookings::state.eq(BookingState::Held))
        .filter(bookings::inventory_hold_expires_at.lt(Utc::now()))
        .load(conn)
        .map_err(AppError::from)
}

pub fn update_hold_expiry(
    conn: &mut DbConn,
    booking_id: Uuid,
    expires_at: Option<chrono::DateTime<Utc>>,
) -> Result<(), AppError> {
    diesel::update(bookings::table.filter(bookings::id.eq(booking_id)))
        .set((
            bookings::inventory_hold_expires_at.eq(expires_at),
            bookings::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .map_err(AppError::from)?;
    Ok(())
}
