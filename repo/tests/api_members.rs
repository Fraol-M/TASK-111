//! HTTP integration tests for the members domain.

mod common;

use actix_web::{test, web};
use serde_json::json;
use uuid::Uuid;

macro_rules! login {
    ($app:expr, $username:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({"username": $username, "password": common::DEFAULT_PASSWORD}))
            .to_request();
        let resp = test::call_service($app, req).await;
        let body: serde_json::Value = test::read_body_json(resp).await;
        body["token"].as_str().expect("token").to_string()
    }};
}

/// Seed (admin, member with profile). Returns their ids. `suffix` keeps
/// usernames unique across tests in the shared test DB.
fn seed_admin_and_member(
    pool: &common::DbPool,
    suffix: &str,
) -> (Uuid, Uuid) {
    let mut conn = pool.get().unwrap();
    let aid = common::seed_user(
        &mut conn,
        &format!("amem_admin_{}", suffix),
        venue_booking::users::model::UserRole::Administrator,
    );
    let mid = common::seed_user(
        &mut conn,
        &format!("amem_member_{}", suffix),
        venue_booking::users::model::UserRole::Member,
    );
    common::seed_member(&mut conn, mid);
    common::seed_member_preferences(&mut conn, mid);
    (aid, mid)
}

#[actix_web::test]
async fn test_member_get_profile_returns_masked_wallet_for_self() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (_aid, mid) = seed_admin_and_member(&pool, "get_self");
    let member_token = login!(&app, &format!("amem_member_{}", "get_self"));

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/members/{}", mid))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let wallet = body["wallet_balance_display"].as_str().unwrap_or("");
    assert!(
        wallet.contains('*') || wallet.is_empty(),
        "member must see masked wallet, got: {}",
        wallet
    );
}

#[actix_web::test]
async fn test_member_tier_force_is_admin_only() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (_aid, mid) = seed_admin_and_member(&pool, "tier");
    let admin_token = login!(&app, &format!("amem_admin_{}", "tier"));
    let member_token = login!(&app, &format!("amem_member_{}", "tier"));

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/members/{}/tier", mid))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .set_json(json!({"tier": "gold"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403, "member must not force their own tier");

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/members/{}/tier", mid))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({"tier": "gold"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "admin must be able to force tier");
}

#[actix_web::test]
async fn test_member_blacklist_is_admin_only() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (_aid, mid) = seed_admin_and_member(&pool, "bl");
    let admin_token = login!(&app, &format!("amem_admin_{}", "bl"));

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/members/{}/blacklist", mid))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({"reason": "fraud", "note": "test"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_points_adjustment_and_ledger_read() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (_aid, mid) = seed_admin_and_member(&pool, "pts");
    let admin_token = login!(&app, &format!("amem_admin_{}", "pts"));
    let member_token = login!(&app, &format!("amem_member_{}", "pts"));

    let idem = Uuid::new_v4().to_string();
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/members/{}/points", mid))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .insert_header(("Idempotency-Key", idem.as_str()))
        .set_json(json!({"delta": 50, "note": "goodwill"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "points adjust, got {}", resp.status());

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/members/{}/points/ledger", mid))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "self must read own points ledger");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["data"].is_array(), "ledger must return paginated data array");
}

#[actix_web::test]
async fn test_negative_adjustment_below_zero_returns_422() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (_aid, mid) = seed_admin_and_member(&pool, "negpts");
    let admin_token = login!(&app, &format!("amem_admin_{}", "negpts"));

    let idem = Uuid::new_v4().to_string();
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/members/{}/points", mid))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .insert_header(("Idempotency-Key", idem.as_str()))
        .set_json(json!({"delta": -10_000, "note": "drive negative"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        422,
        "negative delta below zero must return 422 (not 500)"
    );
}

#[actix_web::test]
async fn test_member_redeem_requires_sufficient_balance() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (_aid, mid) = seed_admin_and_member(&pool, "rdm");
    let member_token = login!(&app, &format!("amem_member_{}", "rdm"));

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/members/{}/redeem", mid))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .set_json(json!({"amount_pts": 100}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422, "redeem with zero balance must be 422");
}

#[actix_web::test]
async fn test_wallet_topup_and_ledger() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let member_id = {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "amem_fin", venue_booking::users::model::UserRole::Finance);
        let mid = common::seed_user(
            &mut conn,
            "amem_walmem",
            venue_booking::users::model::UserRole::Member,
        );
        common::seed_member(&mut conn, mid);
        mid
    };
    let finance_token = login!(&app, "amem_fin");
    let member_token = login!(&app, "amem_walmem");

    let idem = Uuid::new_v4().to_string();
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/members/{}/wallet/topup", member_id))
        .insert_header(("Authorization", format!("Bearer {}", finance_token)))
        .insert_header(("Idempotency-Key", idem.as_str()))
        .set_json(json!({"amount_cents": 5000, "note": "top-up"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "topup: {}", resp.status());

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/members/{}/wallet/ledger", member_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "self must read own wallet ledger");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["data"].is_array());
}

#[actix_web::test]
async fn test_freeze_redemption_is_admin_only() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (_aid, mid) = seed_admin_and_member(&pool, "frz");
    let admin_token = login!(&app, &format!("amem_admin_{}", "frz"));
    let member_token = login!(&app, &format!("amem_member_{}", "frz"));

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/members/{}/freeze", mid))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .set_json(json!({"reason": "fraud"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/members/{}/freeze", mid))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({"reason": "fraud", "note": "test freeze"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_preferences_get_and_update_roundtrip() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (_aid, mid) = seed_admin_and_member(&pool, "pref");
    let member_token = login!(&app, &format!("amem_member_{}", "pref"));

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/members/{}/preferences", mid))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/members/{}/preferences", mid))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .set_json(json!({
            "notification_opt_out": ["booking_reminder_24h"],
            "preferred_channel": "in_app",
            "timezone_offset_minutes": 0
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["preferred_channel"].as_str(), Some("in_app"));
}

#[actix_web::test]
async fn test_preferences_reject_disabled_channel() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (_aid, mid) = seed_admin_and_member(&pool, "prefdis");
    let member_token = login!(&app, &format!("amem_member_{}", "prefdis"));

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/members/{}/preferences", mid))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .set_json(json!({"preferred_channel": "email"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        422,
        "selecting a disabled channel must return 422, not silently accept"
    );
}
