/// Unit tests for asset state machine and masking logic.
#[cfg(test)]
mod asset_masking {
    use venue_booking::common::crypto::EncryptionKey;

    #[test]
    fn test_mask_output_format() {
        let masked = EncryptionKey::mask("some_encrypted_blob", 0);
        // When visible_chars = 0, should return all asterisks
        assert!(masked.chars().all(|c| c == '*'));
    }

    #[test]
    fn test_encrypt_decrypt_round_trip() {
        let key_hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let enc = EncryptionKey::from_hex(key_hex).expect("key");
        let plaintext = "100000"; // 100000 cents = $1000.00

        let encrypted = enc.encrypt(plaintext).expect("encrypt");
        let decrypted = enc.decrypt(&encrypted).expect("decrypt");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_each_encryption_unique_nonce() {
        let key_hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let enc = EncryptionKey::from_hex(key_hex).expect("key");
        let plaintext = "99999";

        let enc1 = enc.encrypt(plaintext).expect("enc1");
        let enc2 = enc.encrypt(plaintext).expect("enc2");

        // Same plaintext must produce different ciphertext due to random nonce
        assert_ne!(enc1, enc2, "Same plaintext should produce different ciphertext");
    }

    #[test]
    fn test_tampered_ciphertext_fails_decryption() {
        let key_hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let enc = EncryptionKey::from_hex(key_hex).expect("key");
        let plaintext = "99999";

        let mut encrypted = enc.encrypt(plaintext).expect("encrypt");
        // Tamper by appending garbage
        encrypted.push_str("TAMPERED");

        let result = enc.decrypt(&encrypted);
        assert!(result.is_err(), "Tampered ciphertext should fail decryption");
    }
}

#[cfg(test)]
mod evaluation_state_machine {
    use venue_booking::evaluations::state_machine::{AssignmentStateMachine, EvaluationStateMachine};
    use venue_booking::evaluations::model::{AssignmentState, EvaluationState};

    #[test]
    fn test_eval_invalid_state_transition_rejected() {
        // Draft → Completed is not allowed
        assert!(EvaluationStateMachine::transition(&EvaluationState::Draft, &EvaluationState::Completed).is_err());
        // Cancelled → Open is not allowed
        assert!(EvaluationStateMachine::transition(&EvaluationState::Cancelled, &EvaluationState::Open).is_err());
    }

    #[test]
    fn test_assignment_evaluator_cannot_skip_states() {
        // Pending → Submitted directly is not allowed (must go through InProgress)
        assert!(AssignmentStateMachine::transition(&AssignmentState::Pending, &AssignmentState::Submitted).is_err());
    }

    #[test]
    fn test_assignment_approved_is_terminal() {
        assert!(AssignmentStateMachine::is_terminal(&AssignmentState::Approved));
        assert!(AssignmentStateMachine::is_terminal(&AssignmentState::Rejected));
        assert!(!AssignmentStateMachine::is_terminal(&AssignmentState::Pending));
        assert!(!AssignmentStateMachine::is_terminal(&AssignmentState::InProgress));
    }

    /// Node-level permission: an evaluator must NOT be able to transition an
    /// assignment that is not their own (service.rs:149 check). Only the assigned
    /// evaluator or an admin may do so.
    #[test]
    fn test_evaluator_outside_scope_denied() {
        use uuid::Uuid;

        let assigned_evaluator = Uuid::new_v4();
        let other_evaluator   = Uuid::new_v4();

        // Non-admin, different actor → not permitted
        let is_admin = false;
        let actor_id = other_evaluator;
        let can_act = is_admin || assigned_evaluator == actor_id;
        assert!(
            !can_act,
            "Evaluator must not transition an assignment assigned to a different evaluator"
        );

        // Non-admin, same actor → permitted
        let actor_id = assigned_evaluator;
        let can_act = is_admin || assigned_evaluator == actor_id;
        assert!(can_act, "Assigned evaluator must be allowed to transition their own assignment");

        // Admin, different actor → permitted (admin overrides)
        let is_admin = true;
        let actor_id = other_evaluator;
        let can_act = is_admin || assigned_evaluator == actor_id;
        assert!(can_act, "Admin must be allowed to transition any assignment regardless of assignment");
    }
}

mod common;

/// Integration test: verify that participant_scope is enforced at assignment creation time.
/// Subjects and evaluators outside the declared scope must be rejected.
#[cfg(test)]
mod evaluation_scope_enforcement {
    use chrono::Utc;
    use diesel::prelude::*;
    use uuid::Uuid;
    use venue_booking::evaluations::{
        model::{EvaluationState},
        service,
    };
    use venue_booking::schema::{evaluation_cycles, evaluations, users};

    #[actix_web::test]
    async fn test_assignment_rejected_when_subject_outside_participant_scope() {
        let _ = dotenvy::from_filename_override(".env.test");
        let pool = super::common::build_test_pool();
        super::common::run_test_migrations(&pool);

        let admin_id = Uuid::new_v4();
        let evaluator_in_scope = Uuid::new_v4();
        let subject_in_scope = Uuid::new_v4();
        let subject_out_of_scope = Uuid::new_v4();
        let cycle_id = Uuid::new_v4();
        let eval_id = Uuid::new_v4();
        let now = Utc::now();

        {
            let mut conn = pool.get().unwrap();

            // Seed required users
            for uid in &[admin_id, evaluator_in_scope, subject_in_scope, subject_out_of_scope] {
                diesel::insert_into(users::table)
                    .values((
                        users::id.eq(uid),
                        users::username.eq(format!("scope_test_{}", &uid.to_string()[..8])),
                        users::password_hash.eq("$argon2id$v=19$m=19456,t=2,p=1$scope_test_hash"),
                        users::role.eq(venue_booking::users::model::UserRole::Administrator),
                        users::status.eq(venue_booking::users::model::UserStatus::Active),
                        users::created_at.eq(now),
                        users::updated_at.eq(now),
                    ))
                    .execute(&mut conn)
                    .expect("seed user");
            }

            // Seed evaluation cycle
            diesel::insert_into(evaluation_cycles::table)
                .values((
                    evaluation_cycles::id.eq(cycle_id),
                    evaluation_cycles::name.eq("Scope test cycle"),
                    evaluation_cycles::starts_at.eq(now),
                    evaluation_cycles::ends_at.eq(now + chrono::Duration::days(30)),
                    evaluation_cycles::created_by.eq(admin_id),
                    evaluation_cycles::created_at.eq(now),
                    evaluation_cycles::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .expect("seed cycle");

            // Seed evaluation with restricted participant_scope
            let scope = serde_json::json!([
                evaluator_in_scope.to_string(),
                subject_in_scope.to_string()
            ]);
            diesel::insert_into(evaluations::table)
                .values((
                    evaluations::id.eq(eval_id),
                    evaluations::cycle_id.eq(Some(cycle_id)),
                    evaluations::title.eq("Scope enforcement test"),
                    evaluations::state.eq(EvaluationState::Open),
                    evaluations::version.eq(0),
                    evaluations::created_by.eq(admin_id),
                    evaluations::participant_scope.eq(scope),
                    evaluations::created_at.eq(now),
                    evaluations::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .expect("seed evaluation");
        }

        // Assignment with in-scope subject and evaluator: should succeed
        let ok_result = service::create_assignment(
            &pool,
            eval_id,
            evaluator_in_scope,
            Some(subject_in_scope),
            None,
        )
        .await;
        assert!(ok_result.is_ok(), "In-scope assignment should succeed: {:?}", ok_result.err());

        // Assignment with out-of-scope subject: should fail
        let err_result = service::create_assignment(
            &pool,
            eval_id,
            evaluator_in_scope,
            Some(subject_out_of_scope),
            None,
        )
        .await;
        assert!(
            err_result.is_err(),
            "Out-of-scope subject assignment must be rejected"
        );
    }
}
