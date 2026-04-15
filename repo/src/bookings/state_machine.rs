use crate::bookings::model::BookingState;
use crate::common::errors::AppError;

pub struct BookingStateMachine;

impl BookingStateMachine {
    /// Returns all allowed next states from `from`.
    pub fn allowed_transitions(from: &BookingState) -> Vec<BookingState> {
        use BookingState::*;
        match from {
            Draft => vec![Held, Confirmed, Cancelled],
            Held => vec![Confirmed, Cancelled, Expired],
            Confirmed => vec![Changed, Cancelled, Completed, ExceptionPending],
            Changed => vec![Confirmed, Cancelled, Completed],
            ExceptionPending => vec![Confirmed, Cancelled],
            // Terminal states — no transitions
            Cancelled | Completed | Expired => vec![],
        }
    }

    /// Validate and perform transition. Returns Err if transition is not allowed.
    pub fn transition(from: &BookingState, to: &BookingState) -> Result<(), AppError> {
        if Self::allowed_transitions(from).contains(to) {
            Ok(())
        } else {
            Err(AppError::PreconditionFailed(format!(
                "Cannot transition booking from {:?} to {:?}",
                from, to
            )))
        }
    }

    pub fn is_terminal(state: &BookingState) -> bool {
        matches!(
            state,
            BookingState::Cancelled | BookingState::Completed | BookingState::Expired
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookings::model::BookingState::*;

    #[test]
    fn test_draft_to_held_allowed() {
        assert!(BookingStateMachine::transition(&Draft, &Held).is_ok());
    }

    #[test]
    fn test_draft_to_confirmed_allowed() {
        // Allowed for "immediate" inventory strategy (no hold-based reservation).
        assert!(BookingStateMachine::transition(&Draft, &Confirmed).is_ok());
    }

    #[test]
    fn test_held_to_expired_allowed() {
        assert!(BookingStateMachine::transition(&Held, &Expired).is_ok());
    }

    #[test]
    fn test_confirmed_to_changed_allowed() {
        assert!(BookingStateMachine::transition(&Confirmed, &Changed).is_ok());
    }

    #[test]
    fn test_cancelled_is_terminal() {
        assert!(BookingStateMachine::is_terminal(&Cancelled));
        assert!(BookingStateMachine::allowed_transitions(&Cancelled).is_empty());
    }

    #[test]
    fn test_completed_to_cancelled_rejected() {
        assert!(BookingStateMachine::transition(&Completed, &Cancelled).is_err());
    }

    #[test]
    fn test_exception_pending_to_confirmed_allowed() {
        assert!(BookingStateMachine::transition(&ExceptionPending, &Confirmed).is_ok());
    }
}
