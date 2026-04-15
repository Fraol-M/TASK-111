use crate::evaluations::model::{AssignmentState, EvaluationState};
use crate::common::errors::AppError;

pub struct EvaluationStateMachine;

impl EvaluationStateMachine {
    pub fn allowed_transitions(from: &EvaluationState) -> Vec<EvaluationState> {
        use EvaluationState::*;
        match from {
            Draft => vec![Open, Cancelled],
            Open => vec![InReview, Cancelled],
            InReview => vec![Completed, Open],
            // Terminal
            Completed | Cancelled => vec![],
        }
    }

    pub fn transition(from: &EvaluationState, to: &EvaluationState) -> Result<(), AppError> {
        if Self::allowed_transitions(from).contains(to) {
            Ok(())
        } else {
            Err(AppError::PreconditionFailed(format!(
                "Cannot transition evaluation from {:?} to {:?}",
                from, to
            )))
        }
    }

    pub fn is_terminal(state: &EvaluationState) -> bool {
        matches!(state, EvaluationState::Completed | EvaluationState::Cancelled)
    }
}

pub struct AssignmentStateMachine;

impl AssignmentStateMachine {
    pub fn allowed_transitions(from: &AssignmentState) -> Vec<AssignmentState> {
        use AssignmentState::*;
        match from {
            Pending => vec![InProgress],
            InProgress => vec![Submitted],
            Submitted => vec![Approved, Rejected],
            // Terminal
            Approved | Rejected => vec![],
        }
    }

    pub fn transition(from: &AssignmentState, to: &AssignmentState) -> Result<(), AppError> {
        if Self::allowed_transitions(from).contains(to) {
            Ok(())
        } else {
            Err(AppError::PreconditionFailed(format!(
                "Cannot transition assignment from {:?} to {:?}",
                from, to
            )))
        }
    }

    pub fn is_terminal(state: &AssignmentState) -> bool {
        matches!(state, AssignmentState::Approved | AssignmentState::Rejected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EvaluationState::*;
    use AssignmentState::*;

    #[test]
    fn test_eval_draft_to_open_allowed() {
        assert!(EvaluationStateMachine::transition(&Draft, &Open).is_ok());
    }

    #[test]
    fn test_eval_draft_to_completed_rejected() {
        assert!(EvaluationStateMachine::transition(&Draft, &Completed).is_err());
    }

    #[test]
    fn test_eval_open_to_in_review_allowed() {
        assert!(EvaluationStateMachine::transition(&Open, &InReview).is_ok());
    }

    #[test]
    fn test_eval_in_review_back_to_open_allowed() {
        assert!(EvaluationStateMachine::transition(&InReview, &Open).is_ok());
    }

    #[test]
    fn test_eval_completed_is_terminal() {
        assert!(EvaluationStateMachine::is_terminal(&Completed));
        assert!(EvaluationStateMachine::allowed_transitions(&Completed).is_empty());
    }

    #[test]
    fn test_eval_cancelled_to_open_rejected() {
        assert!(EvaluationStateMachine::transition(&Cancelled, &Open).is_err());
    }

    #[test]
    fn test_assignment_pending_to_in_progress_allowed() {
        assert!(AssignmentStateMachine::transition(&Pending, &InProgress).is_ok());
    }

    #[test]
    fn test_assignment_submitted_to_approved_allowed() {
        assert!(AssignmentStateMachine::transition(&Submitted, &Approved).is_ok());
    }

    #[test]
    fn test_assignment_submitted_to_rejected_allowed() {
        assert!(AssignmentStateMachine::transition(&Submitted, &Rejected).is_ok());
    }

    #[test]
    fn test_assignment_pending_to_submitted_rejected() {
        assert!(AssignmentStateMachine::transition(&Pending, &Submitted).is_err());
    }

    #[test]
    fn test_assignment_approved_is_terminal() {
        assert!(AssignmentStateMachine::is_terminal(&Approved));
        assert!(AssignmentStateMachine::allowed_transitions(&Approved).is_empty());
    }
}
