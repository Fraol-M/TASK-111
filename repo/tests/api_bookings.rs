//! HTTP integration tests for the bookings domain.

mod common;

use actix_web::{test, web};
use chrono::Duration;
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

/// Create a booking via HTTP using an Actix service. Inlined as a macro to
/// avoid naming the Service type in a helper function signature (actix_http is
/// not a direct dep).
macro_rules! create_booking_via_app {
    ($app:expr, $member_token:expr, $item_id:expr) => {{
        let start_at = chrono::Utc::now() + Duration::hours(25);
        let end_at = start_at + Duration::hours(1);
        let req = test::TestRequest::post()
            .uri("/api/v1/bookings")
            .insert_header(("Authorization", format!("Bearer {}", $member_token)))
            .insert_header(("Idempotency-Key", Uuid::new_v4().to_string()))
            .set_json(json!({
                "items": [{
                    "inventory_item_id": $item_id,
                    "quantity": 1,
                    "unit_price_cents": 5000
                }],
                "start_at": start_at,
                "end_at": end_at,
                "pickup_point_id": null,
                "zone_id": null
            }))
            .to_request();
        let resp = test::call_service($app, req).await;
        assert_eq!(
            resp.status(),
            201,
            "booking create must succeed (got {})",
            resp.status()
        );
        let body: serde_json::Value = test::read_body_json(resp).await;
        let booking_id: Uuid = body["id"].as_str().unwrap().parse().unwrap();
        booking_id
    }};
}

/// Seed ops + member + member profile + inventory item. Returns
/// (member_username, ops_username, item_id).
fn seed_for_booking(pool: &common::DbPool, suffix: &str) -> (String, String, Uuid) {
    let mut conn = pool.get().unwrap();
    let unique_id = Uuid::new_v4().to_string().replace("-", "")[..8].to_string();
    let ops_user = format!("abook_ops_{}_{}", suffix, unique_id);
    let mem_user = format!("abook_mem_{}_{}", suffix, unique_id);
    common::seed_user(
        &mut conn,
        &ops_user,
        venue_booking::users::model::UserRole::OperationsManager,
    );
    let mid = common::seed_user(
        &mut conn,
        &mem_user,
        venue_booking::users::model::UserRole::Member,
    );
    common::seed_member(&mut conn, mid);
    let item_id = common::seed_inventory_item(&mut conn, &format!("SKU-BOOK-{}", Uuid::new_v4()));
    (mem_user, ops_user, item_id)
}

#[actix_web::test]
async fn test_create_list_get_booking_happy_path() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (mem_user, _ops_user, item_id) = seed_for_booking(&pool, "happy");
    let member_token = login!(&app, &mem_user);
    let booking_id = create_booking_via_app!(&app, member_token, item_id);

    // Member lists own bookings
    let req = test::TestRequest::get()
        .uri("/api/v1/bookings")
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["data"].is_array());

    // Member GET own booking
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/bookings/{}", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // GET items
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/bookings/{}/items", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_booking_confirm_and_complete() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (mem_user, ops_user, item_id) = seed_for_booking(&pool, "conf");
    let member_token = login!(&app, &mem_user);
    let ops_token = login!(&app, &ops_user);
    let booking_id = create_booking_via_app!(&app, member_token, item_id);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/bookings/{}/confirm", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "confirm from Held must succeed");

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/bookings/{}/complete", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "complete from Confirmed must succeed");

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/bookings/{}/history", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_booking_change_roundtrip() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (mem_user, _ops_user, item_id) = seed_for_booking(&pool, "chng");
    let member_token = login!(&app, &mem_user);
    let booking_id = create_booking_via_app!(&app, member_token, item_id);

    // Confirm first so Change is a valid transition
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/bookings/{}/confirm", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let _ = test::call_service(&app, req).await;

    // Change with same item, new quantity
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/bookings/{}/change", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .set_json(json!({
            "items": [{
                "inventory_item_id": item_id,
                "quantity": 2,
                "unit_price_cents": 5000
            }]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "change from Confirmed must succeed");
}

#[actix_web::test]
async fn test_booking_cancel_by_owner() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (mem_user, _ops_user, item_id) = seed_for_booking(&pool, "cxl");
    let member_token = login!(&app, &mem_user);
    let booking_id = create_booking_via_app!(&app, member_token, item_id);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/bookings/{}/cancel", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .set_json(json!({"reason": "changed plans"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn test_booking_exception_flag_is_ops_only() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let (mem_user, ops_user, item_id) = seed_for_booking(&pool, "exc");
    let member_token = login!(&app, &mem_user);
    let ops_token = login!(&app, &ops_user);
    let booking_id = create_booking_via_app!(&app, member_token, item_id);

    // Confirm first so exception is a valid transition
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/bookings/{}/confirm", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .to_request();
    let _ = test::call_service(&app, req).await;

    // Member → 403
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/bookings/{}/exception", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", member_token)))
        .set_json(json!({"reason": "damage"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);

    // Ops → 200
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/bookings/{}/exception", booking_id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({"reason": "damage"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
