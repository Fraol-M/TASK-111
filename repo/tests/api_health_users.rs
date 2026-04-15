//! HTTP integration tests for the health endpoint + users domain.
//!
//! Covers:
//!   GET  /health
//!   POST /api/v1/users                   (admin; 403 for non-admin)
//!   GET  /api/v1/users                   (admin)
//!   GET  /api/v1/users/{id}              (admin; self also allowed)
//!   PATCH /api/v1/users/{id}             (covered by high_risk role-downgrade)
//!   PATCH /api/v1/users/{id}/status      (covered by high_risk suspension)
//!   POST  /api/v1/users/{id}/password    (self + admin reset)

mod common;

use actix_web::{test, web};
use serde_json::json;

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

#[actix_web::test]
async fn test_health_returns_200_ok() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;

    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "health must return 200 when DB is reachable");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"].as_str(), Some("ok"), "status must be 'ok'");
    assert_eq!(body["db"].as_str(), Some("ok"), "db must be 'ok'");
}

#[actix_web::test]
async fn test_users_list_and_create_admin_only() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "ah_admin", venue_booking::users::model::UserRole::Administrator);
    }
    let admin_token = login!(&app, "ah_admin");

    // Admin creates a new user.
    let unique_name = format!("ah_new_{}", uuid::Uuid::new_v4());
    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({
            "username": unique_name,
            "password": "Test1234!",
            "role": "member"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201, "admin must be able to create users");
    let body: serde_json::Value = test::read_body_json(resp).await;
    let new_user_id = body["id"].as_str().expect("created user id").to_string();
    assert_eq!(body["username"].as_str(), Some(unique_name.as_str()));

    // Admin lists users.
    let req = test::TestRequest::get()
        .uri("/api/v1/users?page=1&per_page=5")
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["data"].is_array(), "list response must carry a data array");

    // Admin reads the newly-created user by id.
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/users/{}", new_user_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "admin must be able to read any user");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["username"].as_str(), Some(unique_name.as_str()));
}

#[actix_web::test]
async fn test_non_admin_forbidden_from_user_create() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "ah_member", venue_booking::users::model::UserRole::Member);
    }
    let token = login!(&app, "ah_member");

    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({"username":"x","password":"Test1234!","role":"member"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403, "member must not be able to create users");
}

#[actix_web::test]
async fn test_self_can_read_own_user_row() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let self_id = {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "ah_self", venue_booking::users::model::UserRole::Member)
    };
    let token = login!(&app, "ah_self");

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/users/{}", self_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "user must be able to read their own row");
}

#[actix_web::test]
async fn test_self_can_change_own_password() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let self_id = {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "ah_pw", venue_booking::users::model::UserRole::Member)
    };
    let token = login!(&app, "ah_pw");

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/users/{}/password", self_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "current_password": common::DEFAULT_PASSWORD,
            "new_password": "NewStronger!9"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "self password change must succeed");

    // Old password now fails.
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({"username": "ah_pw", "password": common::DEFAULT_PASSWORD}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401, "old password must no longer authenticate");

    // New password works.
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({"username": "ah_pw", "password": "NewStronger!9"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "new password must authenticate");
}

#[actix_web::test]
async fn test_password_change_requires_current_for_non_admin() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let self_id = {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "ah_pw_wrong", venue_booking::users::model::UserRole::Member)
    };
    let token = login!(&app, "ah_pw_wrong");

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/users/{}/password", self_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "current_password": "wrong-password",
            "new_password": "Another!9"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        403,
        "wrong current_password must be rejected with 403"
    );
}
