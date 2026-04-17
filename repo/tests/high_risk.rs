//! High-risk integration tests covering the four areas the static audit flagged.
//!
//! 1. User suspension immediately invalidates existing session tokens.
//! 2. Refund approval creates an 'adjust' points ledger entry and reduces balance.
//! 3. Cross-thread message receipt guard returns 403 (not 200 or 404).
//! 4. Inventory hold idempotency: same correlation_id never double-decrements qty.
mod common;

use actix_web::{test, web};
use diesel::prelude::*;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

// ─── helpers ──────────────────────────────────────────────────────────────────

fn build_app_data() -> (
    common::DbPool,
    venue_booking::config::AppConfig,
    venue_booking::common::crypto::EncryptionKey,
) {
    let _ = dotenvy::from_filename_override(".env.test");
    let pool = common::build_test_pool();
    common::run_test_migrations(&pool);
    let cfg = venue_booking::config::AppConfig::load().expect("config");
    let enc = venue_booking::common::crypto::EncryptionKey::from_hex(&cfg.encryption.key_hex)
        .expect("enc key");
    (pool, cfg, enc)
}

fn seed_user(
    conn: &mut diesel::PgConnection,
    username: &str,
    role: venue_booking::users::model::UserRole,
) -> Uuid {
    common::seed_user(conn, &resolve_test_username(username), role)
}

fn username_aliases() -> &'static Mutex<HashMap<String, String>> {
    static USERNAME_ALIASES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    USERNAME_ALIASES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn resolve_test_username(alias: &str) -> String {
    let mut aliases = username_aliases().lock().expect("username alias lock");
    aliases
        .entry(alias.to_string())
        .or_insert_with(|| format!("{}-{}", alias, Uuid::new_v4().simple()))
        .clone()
}

/// Log in and return the bearer token. Panics if login fails.
macro_rules! login {
    ($app:expr, $username:expr) => {{
        let username = resolve_test_username($username);
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({"username": username, "password": "Test1234!"}))
            .to_request();
        let resp = test::call_service($app, req).await;
        let body: serde_json::Value = test::read_body_json(resp).await;
        body["token"].as_str().expect("token in login response").to_string()
    }};
}

// ─── Test 1: suspension → session revocation ─────────────────────────────────

/// When an admin suspends a user account, that user's in-flight JWT must be
/// refused on the next request (401), not silently accepted.
#[actix_web::test]
async fn test_suspension_revokes_existing_session() {
    let (pool, cfg, enc) = build_app_data();

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (admin_id, target_id) = {
        let mut conn = pool.get().unwrap();
        let aid = seed_user(&mut conn, "hr_admin_suspend", venue_booking::users::model::UserRole::Administrator);
        let tid = seed_user(&mut conn, "hr_target_suspend", venue_booking::users::model::UserRole::Member);
        (aid, tid)
    };
    let _ = admin_id; // used indirectly via login

    // Target logs in — gets a valid token
    let target_token = login!(&app, "hr_target_suspend");

    // Confirm /me works before suspension
    let me_req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {}", target_token)))
        .to_request();
    let me_resp = test::call_service(&app, me_req).await;
    assert_eq!(me_resp.status(), 200, "token should be valid before suspension");

    // Admin suspends the target
    let admin_token = login!(&app, "hr_admin_suspend");
    let suspend_req = test::TestRequest::patch()
        .uri(&format!("/api/v1/users/{}/status", target_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({"status": "suspended"}))
        .to_request();
    let suspend_resp = test::call_service(&app, suspend_req).await;
    assert_eq!(suspend_resp.status(), 200, "suspension should succeed");

    // Target's old token must now be rejected
    let me_after = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {}", target_token)))
        .to_request();
    let me_after_resp = test::call_service(&app, me_after).await;
    assert_eq!(
        me_after_resp.status(),
        401,
        "suspended user's session must be invalidated immediately"
    );
}

// ─── Test 2: refund approval reverses points ─────────────────────────────────

/// Approving a refund must create a negative 'adjust' entry in points_ledger
/// (not 'refund', which is not a valid enum value) and reduce points_balance.
#[actix_web::test]
async fn test_refund_approval_creates_adjust_ledger_entry() {
    let (pool, cfg, enc) = build_app_data();

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (_admin_id, member_id, refund_id) = {
        let mut conn = pool.get().unwrap();

        let aid = seed_user(&mut conn, "hr_admin_refund", venue_booking::users::model::UserRole::Administrator);
        let mid = seed_user(&mut conn, "hr_member_refund", venue_booking::users::model::UserRole::Member);

        // Seed member record with 200 points
        {
            use venue_booking::schema::members;
            diesel::delete(members::table.filter(members::user_id.eq(mid)))
                .execute(&mut conn)
                .ok();
            diesel::insert_into(members::table)
                .values((
                    members::user_id.eq(mid),
                    members::tier.eq(venue_booking::members::model::MemberTier::Silver),
                    members::points_balance.eq(200i32),
                    members::wallet_balance.eq("dummy_encrypted"),
                    members::blacklist_flag.eq(false),
                    members::rolling_12m_spend.eq(0i64),
                    members::version.eq(0i32),
                    members::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn)
                .expect("seed member");
        }

        // Seed payment_intent → payment → refund (pending, 5000 cents)
        let intent_id = Uuid::new_v4();
        let payment_id = Uuid::new_v4();
        let rid = Uuid::new_v4();
        {
            use venue_booking::schema::{payment_intents, payments, refunds};
            diesel::insert_into(payment_intents::table)
                .values((
                    payment_intents::id.eq(intent_id),
                    payment_intents::booking_id.eq(None::<Uuid>),
                    payment_intents::member_id.eq(mid),
                    payment_intents::amount_cents.eq(10_000i64),
                    payment_intents::state.eq(venue_booking::payments::model::IntentState::Captured),
                    payment_intents::idempotency_key.eq(Uuid::new_v4().to_string()),
                    payment_intents::expires_at.eq(chrono::Utc::now() + chrono::Duration::hours(1)),
                    payment_intents::created_at.eq(chrono::Utc::now()),
                    payment_intents::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn)
                .expect("seed intent");

            diesel::insert_into(payments::table)
                .values((
                    payments::id.eq(payment_id),
                    payments::intent_id.eq(intent_id),
                    payments::member_id.eq(mid),
                    payments::booking_id.eq(None::<Uuid>),
                    payments::amount_cents.eq(10_000i64),
                    payments::payment_method.eq("card"),
                    payments::state.eq(venue_booking::payments::model::PaymentState::Completed),
                    payments::idempotency_key.eq(Uuid::new_v4().to_string()),
                    payments::external_reference.eq(None::<String>),
                    payments::version.eq(0i32),
                    payments::created_at.eq(chrono::Utc::now()),
                    payments::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn)
                .expect("seed payment");

            diesel::insert_into(refunds::table)
                .values((
                    refunds::id.eq(rid),
                    refunds::payment_id.eq(payment_id),
                    refunds::amount_cents.eq(5_000i64),
                    refunds::reason.eq(Some("test refund")),
                    refunds::state.eq(venue_booking::payments::model::RefundState::Pending),
                    refunds::idempotency_key.eq(Uuid::new_v4().to_string()),
                    refunds::requested_by.eq(mid),
                    refunds::approved_by.eq(None::<Uuid>),
                    refunds::created_at.eq(chrono::Utc::now()),
                    refunds::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn)
                .expect("seed refund");
        }

        (aid, mid, rid)
    };

    // Admin approves the refund via HTTP
    let admin_token = login!(&app, "hr_admin_refund");
    let approve_req = test::TestRequest::patch()
        .uri(&format!("/api/v1/payments/refunds/{}/approve", refund_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let approve_resp = test::call_service(&app, approve_req).await;
    assert_eq!(approve_resp.status(), 200, "approve_refund should succeed");

    // Verify points_ledger has a negative 'adjust' entry (not 'refund')
    {
        let mut conn = pool.get().unwrap();
        let entries: Vec<(String, i32)> = diesel::sql_query(
            "SELECT txn_type::text, delta FROM points_ledger \
             WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1"
        )
        .bind::<diesel::sql_types::Uuid, _>(member_id)
        .load::<crate::PointsLedgerRow>(&mut conn)
        .expect("load ledger")
        .into_iter()
        .map(|r| (r.txn_type, r.delta))
        .collect();

        assert!(!entries.is_empty(), "points_ledger must have an entry after refund approval");
        let (txn_type, delta) = &entries[0];
        assert_eq!(txn_type, "adjust", "txn_type must be 'adjust' (not 'refund')");
        assert!(
            *delta < 0,
            "delta must be negative on refund reversal, got {}",
            delta
        );
        // 5000 cents / 100 = 50 points reversed
        assert_eq!(*delta, -50, "expected -50 points (5000 cents / 100)");

        // Verify points_balance was reduced
        let balance: i32 = diesel::sql_query(
            "SELECT points_balance FROM members WHERE user_id = $1"
        )
        .bind::<diesel::sql_types::Uuid, _>(member_id)
        .load::<crate::BalanceRow>(&mut conn)
        .expect("load balance")
        .into_iter()
        .next()
        .map(|r| r.points_balance)
        .expect("member row");
        assert_eq!(balance, 150, "200 - 50 = 150 points after refund reversal");
    }
}

// ─── Test 3: cross-thread message read → 403 ─────────────────────────────────

/// A user who is a member of both thread A and thread B must get 403 when they
/// try to mark a message from thread A as read while specifying thread B in the
/// URL path. Without the object-level auth check, this would silently succeed.
#[actix_web::test]
async fn test_cross_thread_message_read_is_forbidden() {
    let (pool, cfg, enc) = build_app_data();

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (user_id, thread_a_id, _thread_b_id, message_a_id) = {
        let mut conn = pool.get().unwrap();
        let uid = seed_user(&mut conn, "hr_user_thread", venue_booking::users::model::UserRole::Member);

        use venue_booking::schema::{group_threads, group_members, group_messages};

        // Create two threads
        let ta = Uuid::new_v4();
        let tb = Uuid::new_v4();
        for (tid, tname) in &[(ta, "thread-alpha"), (tb, "thread-beta")] {
            diesel::insert_into(group_threads::table)
                .values((
                    group_threads::id.eq(*tid),
                    group_threads::name.eq(*tname),
                    group_threads::description.eq(None::<String>),
                    group_threads::created_by.eq(uid),
                    group_threads::created_at.eq(chrono::Utc::now()),
                    group_threads::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn)
                .expect("seed thread");

            // Add user as active member of BOTH threads
            diesel::insert_into(group_members::table)
                .values((
                    group_members::id.eq(Uuid::new_v4()),
                    group_members::thread_id.eq(*tid),
                    group_members::user_id.eq(uid),
                    group_members::joined_at.eq(chrono::Utc::now()),
                    group_members::removed_at.eq(None::<chrono::DateTime<chrono::Utc>>),
                ))
                .execute(&mut conn)
                .expect("seed group member");
        }

        // Post a message in thread A
        let msg_id = Uuid::new_v4();
        diesel::insert_into(group_messages::table)
            .values((
                group_messages::id.eq(msg_id),
                group_messages::thread_id.eq(ta),
                group_messages::sender_id.eq(uid),
                group_messages::body.eq("hello thread alpha"),
                group_messages::created_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .expect("seed message");

        (uid, ta, tb, msg_id)
    };
    let _ = user_id;

    let user_token = login!(&app, "hr_user_thread");

    // Try to mark thread-A's message as read, but supply thread-B's ID in path
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/groups/{}/messages/{}/read", _thread_b_id, message_a_id))
        .insert_header(("Authorization", format!("Bearer {}", user_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        403,
        "marking a message from thread A as read via thread B's URL must be 403"
    );

    // Confirm the same request with the CORRECT thread succeeds
    let ok_req = test::TestRequest::patch()
        .uri(&format!("/api/v1/groups/{}/messages/{}/read", thread_a_id, message_a_id))
        .insert_header(("Authorization", format!("Bearer {}", user_token)))
        .to_request();
    let ok_resp = test::call_service(&app, ok_req).await;
    assert_eq!(
        ok_resp.status(),
        200,
        "marking the same message with the correct thread must succeed"
    );
}

// ─── Test 4: inventory hold idempotency ──────────────────────────────────────

/// Calling create_hold twice with the same correlation_id must NOT decrement
/// available_qty a second time. The guard added at the top of the transaction
/// short-circuits before any mutation on replay.
#[actix_web::test]
async fn test_inventory_hold_correlation_id_is_idempotent() {
    let (pool, cfg, enc) = build_app_data();
    let _ = (cfg, enc); // not needed — calling service directly

    let item_id = Uuid::new_v4();
    let booking_id = Uuid::new_v4();
    let correlation_id = format!("booking:{}:{}", booking_id, item_id);

    // Seed inventory item with qty = 10
    {
        use venue_booking::schema::{bookings, inventory_items};
        let mut conn = pool.get().unwrap();
        let now = chrono::Utc::now();
        let member_id = seed_user(&mut conn, "hr_hold_member", venue_booking::users::model::UserRole::Member);
        diesel::insert_into(bookings::table)
            .values((
                bookings::id.eq(booking_id),
                bookings::member_id.eq(member_id),
                bookings::state.eq(venue_booking::bookings::model::BookingState::Held),
                bookings::start_at.eq(now + chrono::Duration::days(1)),
                bookings::end_at.eq(now + chrono::Duration::days(2)),
                bookings::total_cents.eq(0_i64),
                bookings::version.eq(0),
                bookings::created_at.eq(now),
                bookings::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("seed booking");
        diesel::insert_into(inventory_items::table)
            .values((
                inventory_items::id.eq(item_id),
                inventory_items::sku.eq(format!("TEST-{}", item_id)),
                inventory_items::name.eq("Integration Test Item"),
                inventory_items::description.eq(None::<String>),
                inventory_items::available_qty.eq(10i32),
                inventory_items::safety_stock.eq(1i32),
                inventory_items::publish_status.eq(venue_booking::inventory::model::PublishStatus::Published),
                inventory_items::pickup_point_id.eq(None::<Uuid>),
                inventory_items::zone_id.eq(None::<Uuid>),
                inventory_items::cutoff_hours.eq(24i32),
                inventory_items::version.eq(0i32),
                inventory_items::created_at.eq(chrono::Utc::now()),
                inventory_items::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .expect("seed inventory item");
    }

    // First call — should succeed and decrement qty to 8
    let hold_result = venue_booking::inventory::service::create_hold(
        &pool,
        item_id,
        Some(booking_id),
        2,
        30,
        Some(correlation_id.clone()),
        None,
    )
    .await;
    assert!(hold_result.is_ok(), "first create_hold should succeed: {:?}", hold_result.err());

    let qty_after_first: i32 = {
        use venue_booking::schema::inventory_items;
        let mut conn = pool.get().unwrap();
        inventory_items::table
            .filter(inventory_items::id.eq(item_id))
            .select(inventory_items::available_qty)
            .first(&mut conn)
            .expect("read qty")
    };
    assert_eq!(qty_after_first, 8, "qty should be 10 - 2 = 8 after first hold");

    // Second call with the SAME correlation_id — must NOT decrement again
    let replay_result = venue_booking::inventory::service::create_hold(
        &pool,
        item_id,
        Some(booking_id),
        2,
        30,
        Some(correlation_id.clone()),
        None,
    )
    .await;
    assert!(
        replay_result.is_ok(),
        "replayed create_hold should return the existing hold, not an error: {:?}",
        replay_result.err()
    );

    let qty_after_replay: i32 = {
        use venue_booking::schema::inventory_items;
        let mut conn = pool.get().unwrap();
        inventory_items::table
            .filter(inventory_items::id.eq(item_id))
            .select(inventory_items::available_qty)
            .first(&mut conn)
            .expect("read qty")
    };
    assert_eq!(
        qty_after_replay, 8,
        "qty must still be 8 after idempotent replay — guard prevented double-decrement"
    );
}

// ─── Test 5: role downgrade revokes privileged session ───────────────────────

/// When an admin downgrades a user's role (e.g. Finance → Member), any in-flight
/// JWT carrying the old privileged role must be invalidated immediately so the
/// token cannot be reused to access Finance-only routes.
#[actix_web::test]
async fn test_role_downgrade_revokes_session() {
    let (pool, cfg, enc) = build_app_data();

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (target_id,) = {
        let mut conn = pool.get().unwrap();
        let _aid = seed_user(&mut conn, "hr_admin_downgrade", venue_booking::users::model::UserRole::Administrator);
        let tid = seed_user(&mut conn, "hr_finance_downgrade", venue_booking::users::model::UserRole::Finance);
        (tid,)
    };

    // Finance user logs in — receives a Finance-scoped JWT
    let finance_token = login!(&app, "hr_finance_downgrade");

    // Confirm Finance route accessible before downgrade
    let me_req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {}", finance_token)))
        .to_request();
    let me_resp = test::call_service(&app, me_req).await;
    assert_eq!(me_resp.status(), 200, "finance token should be valid before downgrade");

    // Admin downgrades Finance → Member
    let admin_token = login!(&app, "hr_admin_downgrade");
    let downgrade_req = test::TestRequest::patch()
        .uri(&format!("/api/v1/users/{}", target_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({"role": "member"}))
        .to_request();
    let downgrade_resp = test::call_service(&app, downgrade_req).await;
    assert_eq!(downgrade_resp.status(), 200, "role update should succeed");

    // Old Finance token must now be rejected
    let stale_req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {}", finance_token)))
        .to_request();
    let stale_resp = test::call_service(&app, stale_req).await;
    assert_eq!(
        stale_resp.status(),
        401,
        "stale Finance token must be revoked after role downgrade"
    );
}

// ─── Test 6: adjustment maker-checker enforcement ────────────────────────────

/// The person who created a payment adjustment must not be able to approve it.
/// A different Finance user (or admin) must approve, enforcing separation of duties.
#[actix_web::test]
async fn test_adjustment_self_approval_rejected() {
    let (pool, cfg, enc) = build_app_data();

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (creator_id, approver_id, payment_id) = {
        let mut conn = pool.get().unwrap();
        let cid = seed_user(&mut conn, "hr_finance_creator", venue_booking::users::model::UserRole::Finance);
        let aid = seed_user(&mut conn, "hr_finance_approver", venue_booking::users::model::UserRole::Finance);

        // Seed the payment the adjustment will reference
        let intent_id = Uuid::new_v4();
        let pid = Uuid::new_v4();
        {
            use venue_booking::schema::{payment_intents, payments};
            let member_id = cid; // reuse creator as member for simplicity
            diesel::insert_into(payment_intents::table)
                .values((
                    payment_intents::id.eq(intent_id),
                    payment_intents::booking_id.eq(None::<Uuid>),
                    payment_intents::member_id.eq(member_id),
                    payment_intents::amount_cents.eq(5_000i64),
                    payment_intents::state.eq(venue_booking::payments::model::IntentState::Captured),
                    payment_intents::idempotency_key.eq(Uuid::new_v4().to_string()),
                    payment_intents::expires_at.eq(chrono::Utc::now() + chrono::Duration::hours(1)),
                    payment_intents::created_at.eq(chrono::Utc::now()),
                    payment_intents::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn)
                .expect("seed intent");

            diesel::insert_into(payments::table)
                .values((
                    payments::id.eq(pid),
                    payments::intent_id.eq(intent_id),
                    payments::member_id.eq(member_id),
                    payments::booking_id.eq(None::<Uuid>),
                    payments::amount_cents.eq(5_000i64),
                    payments::payment_method.eq("card"),
                    payments::state.eq(venue_booking::payments::model::PaymentState::Completed),
                    payments::idempotency_key.eq(Uuid::new_v4().to_string()),
                    payments::external_reference.eq(None::<String>),
                    payments::version.eq(0i32),
                    payments::created_at.eq(chrono::Utc::now()),
                    payments::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn)
                .expect("seed payment");
        }

        (cid, aid, pid)
    };
    let _ = (creator_id, approver_id);

    let creator_token = login!(&app, "hr_finance_creator");
    let approver_token = login!(&app, "hr_finance_approver");

    // Creator creates the adjustment
    let create_req = test::TestRequest::post()
        .uri("/api/v1/payments/adjustments")
        .insert_header(("Authorization", format!("Bearer {}", creator_token)))
        .set_json(json!({
            "payment_id": payment_id,
            "amount_cents": 500,
            "reason": "correction for duplicate charge"
        }))
        .to_request();
    let create_resp = test::call_service(&app, create_req).await;
    assert_eq!(create_resp.status(), 201, "adjustment creation should succeed");
    let create_body: serde_json::Value = test::read_body_json(create_resp).await;
    let adj_id = create_body["id"].as_str().expect("adjustment id");

    // Creator tries to approve their own adjustment → must be rejected
    let self_approve_req = test::TestRequest::patch()
        .uri(&format!("/api/v1/payments/adjustments/{}/approve", adj_id))
        .insert_header(("Authorization", format!("Bearer {}", creator_token)))
        .to_request();
    let self_approve_resp = test::call_service(&app, self_approve_req).await;
    assert_eq!(
        self_approve_resp.status(),
        403,
        "self-approval must be rejected (maker-checker rule)"
    );

    // Different Finance user approves → must succeed
    let approve_req = test::TestRequest::patch()
        .uri(&format!("/api/v1/payments/adjustments/{}/approve", adj_id))
        .insert_header(("Authorization", format!("Bearer {}", approver_token)))
        .to_request();
    let approve_resp = test::call_service(&app, approve_req).await;
    assert_eq!(
        approve_resp.status(),
        200,
        "a different Finance user must be able to approve the adjustment"
    );
}

// ─── Test 7: refund cap enforced under sequential concurrent pattern ──────────

/// Send two refund requests that together would exceed the payment amount.
/// The second must be rejected regardless of ordering (cap is enforced under
/// the FOR UPDATE lock on the payment row added in F-01).
#[actix_web::test]
async fn test_refund_cap_prevents_over_refund() {
    let (pool, cfg, enc) = build_app_data();

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (member_id, payment_id) = {
        let mut conn = pool.get().unwrap();
        let mid = seed_user(&mut conn, "hr_admin_refundcap", venue_booking::users::model::UserRole::Administrator);

        // Seed member record (required by approve_refund points logic)
        {
            use venue_booking::schema::members;
            diesel::delete(members::table.filter(members::user_id.eq(mid))).execute(&mut conn).ok();
            diesel::insert_into(members::table)
                .values((
                    members::user_id.eq(mid),
                    members::tier.eq(venue_booking::members::model::MemberTier::Silver),
                    members::points_balance.eq(0i32),
                    members::wallet_balance.eq("dummy"),
                    members::blacklist_flag.eq(false),
                    members::rolling_12m_spend.eq(0i64),
                    members::version.eq(0i32),
                    members::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn).ok();
        }

        let intent_id = Uuid::new_v4();
        let pid = Uuid::new_v4();
        {
            use venue_booking::schema::{payment_intents, payments};
            diesel::insert_into(payment_intents::table)
                .values((
                    payment_intents::id.eq(intent_id),
                    payment_intents::booking_id.eq(None::<Uuid>),
                    payment_intents::member_id.eq(mid),
                    payment_intents::amount_cents.eq(10_000i64),
                    payment_intents::state.eq(venue_booking::payments::model::IntentState::Captured),
                    payment_intents::idempotency_key.eq(Uuid::new_v4().to_string()),
                    payment_intents::expires_at.eq(chrono::Utc::now() + chrono::Duration::hours(1)),
                    payment_intents::created_at.eq(chrono::Utc::now()),
                    payment_intents::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn).expect("intent");

            diesel::insert_into(payments::table)
                .values((
                    payments::id.eq(pid),
                    payments::intent_id.eq(intent_id),
                    payments::member_id.eq(mid),
                    payments::booking_id.eq(None::<Uuid>),
                    payments::amount_cents.eq(10_000i64),
                    payments::payment_method.eq("card"),
                    payments::state.eq(venue_booking::payments::model::PaymentState::Completed),
                    payments::idempotency_key.eq(Uuid::new_v4().to_string()),
                    payments::external_reference.eq(None::<String>),
                    payments::version.eq(0i32),
                    payments::created_at.eq(chrono::Utc::now()),
                    payments::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn).expect("payment");
        }
        (mid, pid)
    };
    let _ = member_id;

    let token = login!(&app, "hr_admin_refundcap");

    // First refund: 7000 cents (within 10000 cap)
    let r1_idem = Uuid::new_v4().to_string();
    let r1 = test::TestRequest::post()
        .uri(&format!("/api/v1/payments/{}/refunds", payment_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .insert_header(("Idempotency-Key", r1_idem.clone()))
        .set_json(json!({"amount_cents": 7000, "reason": "first partial refund", "idempotency_key": r1_idem}))
        .to_request();
    let r1_resp = test::call_service(&app, r1).await;
    assert_eq!(r1_resp.status(), 201, "first refund (7000/10000) should be accepted");

    // Second refund: 4000 cents — would bring total to 11000 > 10000
    let r2_idem = Uuid::new_v4().to_string();
    let r2 = test::TestRequest::post()
        .uri(&format!("/api/v1/payments/{}/refunds", payment_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .insert_header(("Idempotency-Key", r2_idem.clone()))
        .set_json(json!({"amount_cents": 4000, "reason": "second refund exceeding cap", "idempotency_key": r2_idem}))
        .to_request();
    let r2_resp = test::call_service(&app, r2).await;
    assert_eq!(
        r2_resp.status(),
        412,
        "second refund (7000+4000 > 10000) must be rejected with 412"
    );
}

// ─── Test 8: audit log API access control and event coverage ─────────────────

/// GET /api/v1/audit/logs is Administrator-only. A Finance user must receive 403.
/// An Administrator must receive 200 with a JSON page object.
#[actix_web::test]
async fn test_audit_log_endpoint_is_admin_only() {
    let (pool, cfg, enc) = build_app_data();

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    {
        let mut conn = pool.get().unwrap();
        seed_user(&mut conn, "hr_admin_audit", venue_booking::users::model::UserRole::Administrator);
        seed_user(&mut conn, "hr_finance_audit", venue_booking::users::model::UserRole::Finance);
    }

    let finance_token = login!(&app, "hr_finance_audit");
    let admin_token   = login!(&app, "hr_admin_audit");

    // Finance → 403
    let forbidden = test::TestRequest::get()
        .uri("/api/v1/audit/logs")
        .insert_header(("Authorization", format!("Bearer {}", finance_token)))
        .to_request();
    let forbidden_resp = test::call_service(&app, forbidden).await;
    assert_eq!(forbidden_resp.status(), 403, "Finance must not access audit logs");

    // Admin → 200 with data/total fields
    let allowed = test::TestRequest::get()
        .uri("/api/v1/audit/logs")
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let allowed_resp = test::call_service(&app, allowed).await;
    assert_eq!(allowed_resp.status(), 200, "Admin must be able to query audit logs");
    let body: serde_json::Value = test::read_body_json(allowed_resp).await;
    assert!(body.get("data").is_some(), "response must have 'data' field");
    assert!(body.get("total").is_some(), "response must have 'total' field");
}

/// Performing an audited action (refund approval) must produce a retrievable
/// audit_log row with the expected action string and entity_type, queryable
/// through the /api/v1/audit/logs endpoint.
#[actix_web::test]
async fn test_audited_action_produces_log_entry() {
    let (pool, cfg, enc) = build_app_data();

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (admin_id, member_id, refund_id) = {
        let mut conn = pool.get().unwrap();
        let aid = seed_user(&mut conn, "hr_admin_auditevt", venue_booking::users::model::UserRole::Administrator);
        let mid = seed_user(&mut conn, "hr_member_auditevt", venue_booking::users::model::UserRole::Member);

        // Member record
        {
            use venue_booking::schema::members;
            diesel::delete(members::table.filter(members::user_id.eq(mid))).execute(&mut conn).ok();
            diesel::insert_into(members::table)
                .values((
                    members::user_id.eq(mid),
                    members::tier.eq(venue_booking::members::model::MemberTier::Silver),
                    members::points_balance.eq(0i32),
                    members::wallet_balance.eq("dummy"),
                    members::blacklist_flag.eq(false),
                    members::rolling_12m_spend.eq(0i64),
                    members::version.eq(0i32),
                    members::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn).ok();
        }

        // Seed payment + refund
        let intent_id = Uuid::new_v4();
        let payment_id = Uuid::new_v4();
        let rid = Uuid::new_v4();
        {
            use venue_booking::schema::{payment_intents, payments, refunds};
            diesel::insert_into(payment_intents::table)
                .values((
                    payment_intents::id.eq(intent_id),
                    payment_intents::booking_id.eq(None::<Uuid>),
                    payment_intents::member_id.eq(mid),
                    payment_intents::amount_cents.eq(5_000i64),
                    payment_intents::state.eq(venue_booking::payments::model::IntentState::Captured),
                    payment_intents::idempotency_key.eq(Uuid::new_v4().to_string()),
                    payment_intents::expires_at.eq(chrono::Utc::now() + chrono::Duration::hours(1)),
                    payment_intents::created_at.eq(chrono::Utc::now()),
                    payment_intents::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn).expect("intent");

            diesel::insert_into(payments::table)
                .values((
                    payments::id.eq(payment_id),
                    payments::intent_id.eq(intent_id),
                    payments::member_id.eq(mid),
                    payments::booking_id.eq(None::<Uuid>),
                    payments::amount_cents.eq(5_000i64),
                    payments::payment_method.eq("card"),
                    payments::state.eq(venue_booking::payments::model::PaymentState::Completed),
                    payments::idempotency_key.eq(Uuid::new_v4().to_string()),
                    payments::external_reference.eq(None::<String>),
                    payments::version.eq(0i32),
                    payments::created_at.eq(chrono::Utc::now()),
                    payments::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn).expect("payment");

            diesel::insert_into(refunds::table)
                .values((
                    refunds::id.eq(rid),
                    refunds::payment_id.eq(payment_id),
                    refunds::amount_cents.eq(1_000i64),
                    refunds::reason.eq(Some("audit event test")),
                    refunds::state.eq(venue_booking::payments::model::RefundState::Pending),
                    refunds::idempotency_key.eq(Uuid::new_v4().to_string()),
                    refunds::requested_by.eq(mid),
                    refunds::approved_by.eq(None::<Uuid>),
                    refunds::created_at.eq(chrono::Utc::now()),
                    refunds::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn).expect("refund");
        }

        (aid, mid, rid)
    };
    let _ = (admin_id, member_id);

    let admin_token = login!(&app, "hr_admin_auditevt");

    // Perform the audited action
    let approve = test::TestRequest::patch()
        .uri(&format!("/api/v1/payments/refunds/{}/approve", refund_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let approve_resp = test::call_service(&app, approve).await;
    assert_eq!(approve_resp.status(), 200, "refund approval must succeed");

    // Query audit log filtered by both entity_type and entity_id for determinism
    // in noisy shared test DBs (entity_id is supported by AuditLogFilter in handler).
    let query = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/audit/logs?entity_type=refund&entity_id={}",
            refund_id
        ))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let query_resp = test::call_service(&app, query).await;
    assert_eq!(query_resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(query_resp).await;

    let data = body["data"].as_array().expect("'data' array in Page response");
    let refund_approved = data.iter().find(|log| {
        log["action"].as_str() == Some("refund_approved")
            && log["entity_id"].as_str() == Some(&refund_id.to_string())
    });
    assert!(
        refund_approved.is_some(),
        "audit_logs must contain a 'refund_approved' entry for refund {}",
        refund_id
    );
}

// ─── QueryableByName helpers for raw SQL results ──────────────────────────────

#[derive(diesel::QueryableByName)]
struct PointsLedgerRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    txn_type: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    delta: i32,
}

#[derive(diesel::QueryableByName)]
struct BalanceRow {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    points_balance: i32,
}

/// Verify that the audit log hash chain is tamper-evident: inserting rows builds
/// a valid chain, and verify_audit_chain confirms integrity.
#[actix_web::test]
async fn test_audit_hash_chain_integrity() {
    let _ = dotenvy::from_filename_override(".env.test");
    let pool = common::build_test_pool();
    common::run_test_migrations(&pool);

    let now = chrono::Utc::now();
    let mut conn = pool.get().unwrap();
    // Remove only this test's prior rows so reruns stay deterministic without
    // deleting fixtures owned by sibling tests running in parallel.
    diesel::delete(
        venue_booking::schema::audit_logs::table
            .filter(venue_booking::schema::audit_logs::action.eq_any(vec![
                "chain_test_1",
                "chain_test_2",
            ])),
    )
    .execute(&mut conn)
    .ok();

    // Insert two audit log entries via the production path
    venue_booking::audit::repository::insert_audit_log(
        &mut conn,
        venue_booking::audit::model::NewAuditLog {
            id: uuid::Uuid::new_v4(),
            correlation_id: None,
            actor_user_id: None,
            action: "chain_test_1".into(),
            entity_type: "test".into(),
            entity_id: "a".into(),
            old_value: None,
            new_value: None,
            metadata: None,
            created_at: now,
            row_hash: String::new(),
            previous_hash: None,
        },
    )
    .expect("insert first audit log");

    venue_booking::audit::repository::insert_audit_log(
        &mut conn,
        venue_booking::audit::model::NewAuditLog {
            id: uuid::Uuid::new_v4(),
            correlation_id: None,
            actor_user_id: None,
            action: "chain_test_2".into(),
            entity_type: "test".into(),
            entity_id: "b".into(),
            old_value: None,
            new_value: None,
            metadata: None,
            created_at: now + chrono::Duration::seconds(1),
            row_hash: String::new(),
            previous_hash: None,
        },
    )
    .expect("insert second audit log");

    // Verify chain integrity
    let result = venue_booking::audit::repository::verify_audit_chain(&mut conn);
    assert!(
        result.is_ok(),
        "Audit hash chain verification should pass: {:?}",
        result.err()
    );
    assert!(
        result.unwrap() >= 2,
        "Should have verified at least 2 rows"
    );
}

// ─── Test: booking ownership enforcement ─────────────────────────────────────

/// A member must not be able to read or modify another member's booking (403).
#[actix_web::test]
async fn test_booking_ownership_returns_403_for_non_owner() {
    let (pool, cfg, enc) = build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (owner_id, _other_id, booking_id) = {
        let mut conn = pool.get().unwrap();
        let owner = seed_user(&mut conn, "hr_booking_owner", venue_booking::users::model::UserRole::Member);
        let other = seed_user(&mut conn, "hr_booking_other", venue_booking::users::model::UserRole::Member);

        // Seed a booking owned by `owner`
        use venue_booking::schema::bookings;
        let bid = Uuid::new_v4();
        let now = chrono::Utc::now();
        diesel::insert_into(bookings::table)
            .values((
                bookings::id.eq(bid),
                bookings::member_id.eq(owner),
                bookings::state.eq(venue_booking::bookings::model::BookingState::Confirmed),
                bookings::start_at.eq(now + chrono::Duration::days(7)),
                bookings::end_at.eq(now + chrono::Duration::days(8)),
                bookings::total_cents.eq(10000_i64),
                bookings::version.eq(0),
                bookings::created_at.eq(now),
                bookings::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("seed booking");

        (owner, other, bid)
    };
    let _ = owner_id;

    // The other member tries to read the owner's booking → 403
    let other_token = login!(&app, "hr_booking_other");
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/bookings/{}", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", other_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403, "non-owner member must get 403 on another's booking");

    // The other member tries to cancel the owner's booking → 403
    let cancel_req = test::TestRequest::patch()
        .uri(&format!("/api/v1/bookings/{}/cancel", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", other_token)))
        .set_json(json!({"reason": "test"}))
        .to_request();
    let cancel_resp = test::call_service(&app, cancel_req).await;
    assert_eq!(cancel_resp.status(), 403, "non-owner member must get 403 when cancelling another's booking");

    // The owner can read their own booking → 200
    let owner_token = login!(&app, "hr_booking_owner");
    let own_req = test::TestRequest::get()
        .uri(&format!("/api/v1/bookings/{}", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", owner_token)))
        .to_request();
    let own_resp = test::call_service(&app, own_req).await;
    assert_eq!(own_resp.status(), 200, "owner must be able to read their own booking");
}

// ─── Test: member data isolation ─────────────────────────────────────────────

/// A member must not be able to view another member's profile (403).
#[actix_web::test]
async fn test_member_profile_isolation() {
    let (pool, cfg, enc) = build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (member_a_id, member_b_id) = {
        let mut conn = pool.get().unwrap();
        let a = seed_user(&mut conn, "hr_member_a", venue_booking::users::model::UserRole::Member);
        let b = seed_user(&mut conn, "hr_member_b", venue_booking::users::model::UserRole::Member);

        // Seed member records
        use venue_booking::schema::members;
        for uid in &[a, b] {
            diesel::insert_into(members::table)
                .values((
                    members::user_id.eq(uid),
                    members::tier.eq(venue_booking::members::model::MemberTier::Silver),
                    members::points_balance.eq(0),
                    members::rolling_12m_spend.eq(0_i64),
                    members::updated_at.eq(chrono::Utc::now()),
                ))
                .on_conflict(members::user_id)
                .do_nothing()
                .execute(&mut conn)
                .ok();
        }

        (a, b)
    };

    let token_a = login!(&app, "hr_member_a");

    // Member A tries to view member B's profile → 403
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/members/{}", member_b_id))
        .insert_header(("Authorization", format!("Bearer {}", token_a)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403, "member must not view another member's profile");

    // Member A can view their own profile → 200
    let own_req = test::TestRequest::get()
        .uri(&format!("/api/v1/members/{}", member_a_id))
        .insert_header(("Authorization", format!("Bearer {}", token_a)))
        .to_request();
    let own_resp = test::call_service(&app, own_req).await;
    assert_eq!(own_resp.status(), 200, "member must be able to view their own profile");
}

// ─── Test: notification read isolation ───────────────────────────────────────

/// A member must not be able to mark another member's notification as read.
#[actix_web::test]
async fn test_notification_read_isolation() {
    let (pool, cfg, enc) = build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let (user_a_id, _user_b_id, notif_id) = {
        let mut conn = pool.get().unwrap();
        let a = seed_user(&mut conn, "hr_notif_owner", venue_booking::users::model::UserRole::Member);
        let b = seed_user(&mut conn, "hr_notif_other", venue_booking::users::model::UserRole::Member);

        // Seed a notification owned by user A
        use venue_booking::schema::notifications;
        let nid = Uuid::new_v4();
        let now = chrono::Utc::now();
        diesel::insert_into(notifications::table)
            .values((
                notifications::id.eq(nid),
                notifications::user_id.eq(a),
                notifications::trigger_type.eq(venue_booking::notifications::model::TemplateTrigger::BookingConfirmed),
                notifications::channel.eq(venue_booking::notifications::model::NotificationChannel::InApp),
                notifications::body.eq("Test notification"),
                notifications::payload_hash.eq("testhash"),
                notifications::delivery_state.eq(venue_booking::notifications::model::DeliveryState::Delivered),
                notifications::dnd_suppressed.eq(false),
                notifications::created_at.eq(now),
                notifications::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("seed notification");

        (a, b, nid)
    };
    let _ = user_a_id;

    // User B tries to mark user A's notification as read → 403
    let token_b = login!(&app, "hr_notif_other");
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/notifications/{}/read", notif_id))
        .insert_header(("Authorization", format!("Bearer {}", token_b)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403, "must not mark another user's notification as read");

    // User A can mark their own → 200
    let token_a = login!(&app, "hr_notif_owner");
    let own_req = test::TestRequest::patch()
        .uri(&format!("/api/v1/notifications/{}/read", notif_id))
        .insert_header(("Authorization", format!("Bearer {}", token_a)))
        .to_request();
    let own_resp = test::call_service(&app, own_req).await;
    assert_eq!(own_resp.status(), 200, "owner must be able to mark their own notification as read");
}

// ─── Test: member-role 403 on finance endpoints ──────────────────────────────

/// A Member must be denied access to finance-only endpoints.
#[actix_web::test]
async fn test_member_forbidden_from_payments_and_reconciliation() {
    let (pool, cfg, enc) = build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    {
        let mut conn = pool.get().unwrap();
        seed_user(&mut conn, "hr_member_fin", venue_booking::users::model::UserRole::Member);
    }

    let token = login!(&app, "hr_member_fin");

    // Member → POST /payments/intents → 403
    let req = test::TestRequest::post()
        .uri("/api/v1/payments/intents")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({"booking_id": Uuid::new_v4(), "member_id": Uuid::new_v4(), "amount_cents": 1000, "idempotency_key": "k1"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403, "member must be denied from payment intents");

    // Member → GET /payments → 403
    let req2 = test::TestRequest::get()
        .uri("/api/v1/payments")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), 403, "member must be denied from payments list");

    // Member → POST /reconciliation/import → 403
    let req3 = test::TestRequest::post()
        .uri("/api/v1/reconciliation/import")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp3 = test::call_service(&app, req3).await;
    assert_eq!(resp3.status(), 403, "member must be denied from reconciliation import");
}

// ─── Test: assets cost masking by role ───────────────────────────────────────

/// A member sees masked procurement_cost, while finance/admin sees the real value.
#[actix_web::test]
async fn test_asset_cost_masking_by_role() {
    let (pool, cfg, enc) = build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let asset_id = {
        let mut conn = pool.get().unwrap();
        seed_user(&mut conn, "hr_asset_member", venue_booking::users::model::UserRole::Member);
        seed_user(&mut conn, "hr_asset_admin", venue_booking::users::model::UserRole::Administrator);

        // Seed an asset with encrypted procurement cost
        use venue_booking::schema::assets;
        let aid = Uuid::new_v4();
        let now = chrono::Utc::now();
        let encrypted_cost = enc.encrypt("50000").unwrap();
        diesel::insert_into(assets::table)
            .values((
                assets::id.eq(aid),
                assets::asset_code.eq(format!("ASSET-{}", &aid.to_string()[..8])),
                assets::name.eq("Test Asset"),
                assets::procurement_cost.eq(&encrypted_cost),
                assets::version.eq(0),
                assets::created_at.eq(now),
                assets::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("seed asset");

        aid
    };

    // Member reads asset → procurement_cost is masked
    let member_token = login!(&app, "hr_asset_member");
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/assets/{}", asset_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let cost_field = body["procurement_cost"].as_str().unwrap_or("");
    assert!(cost_field.contains('*'), "member must see masked cost, got: {}", cost_field);

    // Admin reads asset → procurement_cost is visible
    let admin_token = login!(&app, "hr_asset_admin");
    let admin_req = test::TestRequest::get()
        .uri(&format!("/api/v1/assets/{}", asset_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let admin_resp = test::call_service(&app, admin_req).await;
    assert_eq!(admin_resp.status(), 200);
    let admin_body: serde_json::Value = test::read_body_json(admin_resp).await;
    let admin_cost = admin_body["procurement_cost"].as_str().unwrap_or("");
    assert!(!admin_cost.contains('*'), "admin must see real cost, got: {}", admin_cost);
    assert_eq!(admin_cost, "50000", "admin must see decrypted cost value");
}

// ─── Test: tier recalculation emits tamper-evident audit event ────────────────

/// When the nightly `recalculate_tier` batch job transitions a member across
/// a spend threshold (Silver → Gold), it MUST write a hash-chained
/// `tier_recalculated` audit entry. A previous audit found this path silently
/// updated tier + `member_tier_history` without an `audit_logs` row, leaving
/// the tamper-evident chain incomplete for automated tier transitions.
#[actix_web::test]
async fn test_tier_recalculation_emits_audit_event() {
    let (pool, cfg, enc) = build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    // Seed: admin (for audit query), member (for tier transition).
    let (admin_id, member_id) = {
        let mut conn = pool.get().unwrap();
        let aid = seed_user(
            &mut conn,
            "hr_admin_tieraudit",
            venue_booking::users::model::UserRole::Administrator,
        );
        let mid = seed_user(
            &mut conn,
            "hr_member_tieraudit",
            venue_booking::users::model::UserRole::Member,
        );

        // Seed the member profile at Silver with 0 rolling spend.
        use venue_booking::schema::members;
        diesel::delete(members::table.filter(members::user_id.eq(mid)))
            .execute(&mut conn)
            .ok();
        diesel::insert_into(members::table)
            .values((
                members::user_id.eq(mid),
                members::tier.eq(venue_booking::members::model::MemberTier::Silver),
                members::points_balance.eq(0i32),
                members::wallet_balance.eq("dummy_encrypted"),
                members::blacklist_flag.eq(false),
                members::rolling_12m_spend.eq(0i64),
                members::version.eq(0i32),
                members::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .expect("seed member");

        // Seed a captured intent + completed payment of 600_000 cents ($6000),
        // which pushes rolling_12m spend over the Gold threshold (500_000).
        use venue_booking::schema::{payment_intents, payments};
        let intent_id = Uuid::new_v4();
        diesel::insert_into(payment_intents::table)
            .values((
                payment_intents::id.eq(intent_id),
                payment_intents::booking_id.eq(None::<Uuid>),
                payment_intents::member_id.eq(mid),
                payment_intents::amount_cents.eq(600_000i64),
                payment_intents::state.eq(venue_booking::payments::model::IntentState::Captured),
                payment_intents::idempotency_key.eq(Uuid::new_v4().to_string()),
                payment_intents::expires_at.eq(chrono::Utc::now() + chrono::Duration::hours(1)),
                payment_intents::created_at.eq(chrono::Utc::now()),
                payment_intents::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .expect("intent");

        diesel::insert_into(payments::table)
            .values((
                payments::id.eq(Uuid::new_v4()),
                payments::intent_id.eq(intent_id),
                payments::member_id.eq(mid),
                payments::booking_id.eq(None::<Uuid>),
                payments::amount_cents.eq(600_000i64),
                payments::payment_method.eq("card"),
                payments::state.eq(venue_booking::payments::model::PaymentState::Completed),
                payments::idempotency_key.eq(Uuid::new_v4().to_string()),
                payments::external_reference.eq(None::<String>),
                payments::version.eq(0i32),
                payments::created_at.eq(chrono::Utc::now()),
                payments::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .expect("payment");

        (aid, mid)
    };
    let _ = admin_id; // login picks up by username

    // Drive the tier-recalc service path directly — the same function the
    // nightly batch job invokes. Assert the transition happened (Silver → Gold)
    // AND that an audit entry was written.
    let new_tier = venue_booking::members::service::recalculate_tier(&pool, member_id)
        .await
        .expect("recalculate_tier must succeed");
    assert_eq!(
        new_tier,
        venue_booking::members::model::MemberTier::Gold,
        "spend of $6000 should upgrade Silver → Gold"
    );

    // Query audit_logs via the admin endpoint filtered by entity_type + entity_id
    // so shared-DB test noise doesn't cause false positives.
    let admin_token = login!(&app, "hr_admin_tieraudit");
    let query = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/audit/logs?entity_type=member&entity_id={}",
            member_id
        ))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let query_resp = test::call_service(&app, query).await;
    assert_eq!(query_resp.status(), 200, "audit query must succeed");
    let body: serde_json::Value = test::read_body_json(query_resp).await;
    let data = body["data"].as_array().expect("'data' array in Page response");

    let tier_audit = data.iter().find(|log| {
        log["action"].as_str() == Some("tier_recalculated")
            && log["entity_id"].as_str() == Some(&member_id.to_string())
    });
    let entry = tier_audit.unwrap_or_else(|| {
        panic!(
            "audit_logs must contain a 'tier_recalculated' entry for member {}",
            member_id
        )
    });

    // Old/new tier values must be recorded so the chain evidences the exact
    // transition, not just that something happened.
    assert_eq!(
        entry["old_value"]["tier"].as_str(),
        Some("silver"),
        "old_value.tier must be 'silver' in tier_recalculated audit entry"
    );
    assert_eq!(
        entry["new_value"]["tier"].as_str(),
        Some("gold"),
        "new_value.tier must be 'gold' in tier_recalculated audit entry"
    );
    // The automatic batch path has no HTTP actor.
    assert!(
        entry["actor_user_id"].is_null(),
        "tier_recalculated from batch job must have null actor_user_id"
    );
    // Tamper-evidence: the row_hash must be set by the chained insert.
    let row_hash = entry["row_hash"].as_str().unwrap_or("");
    assert_eq!(
        row_hash.len(),
        64,
        "tier_recalculated audit row_hash must be a 64-char sha256 hex, got len {}",
        row_hash.len()
    );
}
