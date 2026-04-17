mod common;

/// Unit tests for the booking state-machine transition table.
///
/// Scope: these tests exercise `BookingStateMachine::transition` and
/// `BookingStateMachine::is_terminal` directly — no HTTP handlers, no DB, no
/// inventory side effects. They verify that the declarative transition table
/// accepts the happy-path lifecycle (Draft → Held → Confirmed → Completed),
/// rejects transitions out of terminal states, and allows exception/change
/// branches.
///
/// NOT in scope here (lives in `tests/high_risk.rs` and related suites):
/// - Full HTTP booking lifecycle (create/confirm/change/cancel via the router)
/// - Inventory hold creation/release and ledger side effects
/// - Ownership/authorization checks on booking endpoints
/// - Immediate-strategy deduction / restore on cancel
///
/// If you need to add a regression test for end-to-end booking behavior that
/// touches the DB or HTTP layer, add it to `tests/high_risk.rs` (or a new
/// `booking_integration.rs` using the same `common` harness) — not here.
#[cfg(test)]
mod booking_state_machine {
    use venue_booking::bookings::model::BookingState;
    use venue_booking::bookings::state_machine::BookingStateMachine;

    #[test]
    fn test_full_lifecycle_state_transitions() {
        // Draft → Held
        assert!(BookingStateMachine::transition(&BookingState::Draft, &BookingState::Held).is_ok());
        // Held → Confirmed
        assert!(BookingStateMachine::transition(&BookingState::Held, &BookingState::Confirmed).is_ok());
        // Confirmed → Completed
        assert!(BookingStateMachine::transition(&BookingState::Confirmed, &BookingState::Completed).is_ok());
        // Completed → Cancelled: rejected (terminal)
        assert!(BookingStateMachine::transition(&BookingState::Completed, &BookingState::Cancelled).is_err());
    }

    #[test]
    fn test_expired_path() {
        // Held → Expired (via job)
        assert!(BookingStateMachine::transition(&BookingState::Held, &BookingState::Expired).is_ok());
        // Expired → any: rejected (terminal)
        assert!(BookingStateMachine::transition(&BookingState::Expired, &BookingState::Confirmed).is_err());
    }

    #[test]
    fn test_exception_path() {
        // Confirmed → ExceptionPending
        assert!(BookingStateMachine::transition(&BookingState::Confirmed, &BookingState::ExceptionPending).is_ok());
        // ExceptionPending → Confirmed (resolved)
        assert!(BookingStateMachine::transition(&BookingState::ExceptionPending, &BookingState::Confirmed).is_ok());
        // ExceptionPending → Cancelled
        assert!(BookingStateMachine::transition(&BookingState::ExceptionPending, &BookingState::Cancelled).is_ok());
    }

    #[test]
    fn test_change_path() {
        // Confirmed → Changed
        assert!(BookingStateMachine::transition(&BookingState::Confirmed, &BookingState::Changed).is_ok());
        // Changed → Confirmed
        assert!(BookingStateMachine::transition(&BookingState::Changed, &BookingState::Confirmed).is_ok());
        // Changed → Completed
        assert!(BookingStateMachine::transition(&BookingState::Changed, &BookingState::Completed).is_ok());
    }

    #[test]
    fn test_all_terminal_states() {
        assert!(BookingStateMachine::is_terminal(&BookingState::Cancelled));
        assert!(BookingStateMachine::is_terminal(&BookingState::Completed));
        assert!(BookingStateMachine::is_terminal(&BookingState::Expired));
        assert!(!BookingStateMachine::is_terminal(&BookingState::Draft));
        assert!(!BookingStateMachine::is_terminal(&BookingState::Held));
        assert!(!BookingStateMachine::is_terminal(&BookingState::Confirmed));
    }
}
