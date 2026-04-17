mod common;

use actix_web::{test, web};
use serde_json::json;

/// Integration test: full auth flow — login, use token, logout, verify 401.
#[actix_web::test]
async fn test_login_and_logout_flow() {
    let _ = dotenvy::dotenv();
    let pool = common::build_test_pool();
    common::run_test_migrations(&pool);

    let cfg = venue_booking::config::AppConfig::load().expect("config");
    let enc = venue_booking::common::crypto::EncryptionKey::from_hex(&cfg.encryption.key_hex)
        .expect("enc key");

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    // Create a test user first (seed)
    {
        use diesel::prelude::*;
        use venue_booking::schema::users;
        let mut conn = pool.get().unwrap();
        // Clean existing test user
        diesel::delete(users::table.filter(users::username.eq("test_admin")))
            .execute(&mut conn)
            .ok();

        let password_hash = venue_booking::auth::service::hash_password("Test1234!")
            .expect("hash password");
        diesel::insert_into(users::table)
            .values((
                users::id.eq(uuid::Uuid::new_v4()),
                users::username.eq("test_admin"),
                users::password_hash.eq(&password_hash),
                users::role.eq(venue_booking::users::model::UserRole::Administrator),
                users::status.eq(venue_booking::users::model::UserStatus::Active),
                users::created_at.eq(chrono::Utc::now()),
                users::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .expect("insert test user");
    }

    // Step 1: Login
    let login_req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({
            "username": "test_admin",
            "password": "Test1234!"
        }))
        .to_request();

    let login_resp = test::call_service(&app, login_req).await;
    assert_eq!(login_resp.status(), 200, "Expected 200 on login");

    let login_body: serde_json::Value = test::read_body_json(login_resp).await;
    let token = login_body["token"].as_str().expect("token in login response");

    // Step 2: Access protected route /api/v1/auth/me
    let me_req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let me_resp = test::call_service(&app, me_req).await;
    assert_eq!(me_resp.status(), 200, "Expected 200 on /me");

    // Step 3: Logout
    let logout_req = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let logout_resp = test::call_service(&app, logout_req).await;
    assert_eq!(logout_resp.status(), 200, "Expected 200 on logout");

    // Step 4: Access /me with revoked token — should be 401
    let me_after_req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let me_after_resp = test::call_service(&app, me_after_req).await;
    assert_eq!(me_after_resp.status(), 401, "Expected 401 after logout");
}

#[actix_web::test]
async fn test_login_wrong_password_returns_401() {
    let _ = dotenvy::dotenv();
    let pool = common::build_test_pool();
    common::run_test_migrations(&pool);

    let cfg = venue_booking::config::AppConfig::load().expect("config");
    let enc = venue_booking::common::crypto::EncryptionKey::from_hex(&cfg.encryption.key_hex)
        .expect("enc key");

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({
            "username": "test_admin",
            "password": "WrongPassword!"
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn test_member_forbidden_from_admin_route() {
    let _ = dotenvy::dotenv();
    let pool = common::build_test_pool();
    common::run_test_migrations(&pool);

    let cfg = venue_booking::config::AppConfig::load().expect("config");
    let enc = venue_booking::common::crypto::EncryptionKey::from_hex(&cfg.encryption.key_hex)
        .expect("enc key");

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    // Seed a member user
    let member_id = uuid::Uuid::new_v4();
    {
        use diesel::prelude::*;
        use venue_booking::schema::users;
        let mut conn = pool.get().unwrap();
        diesel::delete(users::table.filter(users::username.eq("test_member")))
            .execute(&mut conn)
            .ok();
        let hash = venue_booking::auth::service::hash_password("Test1234!").unwrap();
        diesel::insert_into(users::table)
            .values((
                users::id.eq(member_id),
                users::username.eq("test_member"),
                users::password_hash.eq(&hash),
                users::role.eq(venue_booking::users::model::UserRole::Member),
                users::status.eq(venue_booking::users::model::UserStatus::Active),
                users::created_at.eq(chrono::Utc::now()),
                users::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .expect("insert member");
    }

    // Login as member
    let login = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({"username": "test_member", "password": "Test1234!"}))
        .to_request();
    let login_resp = test::call_service(&app, login).await;
    let body: serde_json::Value = test::read_body_json(login_resp).await;
    let token = body["token"].as_str().unwrap().to_string();

    // Try admin-only route: GET /api/v1/users
    let req = test::TestRequest::get()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403, "Member should be forbidden from /api/v1/users");
}
