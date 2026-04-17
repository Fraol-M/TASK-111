//! Integration tests for the `/reconciliation/import` endpoint covering the
//! paths the prior audit found uncovered:
//!
//! 1. Successful import returns 201 and the on-disk staging file exists.
//! 2. Re-uploading bytes with the same content returns 409 (checksum dedupe).
//! 3. A multipart upload missing the file field returns 422.
//! 4. Oversized uploads are rejected by the streaming size cap.
//!
//! These tests exercise the full handler → service → repository path against
//! the test database and the local filesystem (`cfg.storage.reconciliation_dir`).

mod common;

use actix_web::{test, web};
use serde_json::json;
use uuid::Uuid;

/// Build app data (pool, cfg, enc) from `.env.test` and run migrations.
fn build_app_data() -> (
    common::DbPool,
    venue_booking::config::AppConfig,
    venue_booking::common::crypto::EncryptionKey,
) {
    use diesel::r2d2::{self, ConnectionManager};
    use diesel::PgConnection;

    // Load defaults from .env.test without stomping per-test overrides set via
    // std::env::set_var(...) inside this process.
    let _ = dotenvy::from_filename(".env.test");

    let database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("TEST_DATABASE_URL or DATABASE_URL must be set for integration tests");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(5)
        .build(manager)
        .expect("Failed to create test DB pool");

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
    common::seed_user(conn, username, role)
}

/// Build a minimal valid multipart body carrying a single CSV field named
/// "file" with the supplied filename and bytes. Returns `(content_type, body)`.
fn make_multipart(filename: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    let boundary = "----recontestboundary1234";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
            filename
        )
        .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: text/csv\r\n\r\n");
    body.extend_from_slice(bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    (format!("multipart/form-data; boundary={}", boundary), body)
}

/// Login with a seeded user and return the bearer token.
///
/// Implemented as a macro so the test file doesn't need to name the Actix
/// internal `Request` type (which is `actix_http::Request`, not a direct
/// dependency in `Cargo.toml`).
macro_rules! login_as {
    ($app:expr, $username:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(json!({"username": $username, "password": "Test1234!"}))
            .to_request();
        let resp = test::call_service($app, req).await;
        let body: serde_json::Value = test::read_body_json(resp).await;
        body["token"]
            .as_str()
            .expect("token in login response")
            .to_string()
    }};
}

/// Happy path: finance user uploads a valid CSV, gets 201, and the on-disk
/// staging file is present at the checksum-derived path.
#[actix_web::test]
async fn test_import_success_persists_file_and_returns_201() {
    let (pool, cfg, enc) = build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    {
        let mut conn = pool.get().unwrap();
        seed_user(
            &mut conn,
            "recon_finance_ok",
            venue_booking::users::model::UserRole::Finance,
        );
    }
    let token = login_as!(&app, "recon_finance_ok");

    // Use a unique reference in the CSV so its checksum is unique to this run
    // (the table's UNIQUE(file_checksum) constraint would otherwise collide
    // with previous test runs on the same DB).
    let unique_ref = format!("REF-{}", Uuid::new_v4());
    let csv = format!(
        "external_reference,external_amount_cents,transaction_date\n{},1000,2026-01-15\n",
        unique_ref
    );
    let csv_bytes = csv.as_bytes();
    let (ct, body) = make_multipart("recon.csv", csv_bytes);

    let req = test::TestRequest::post()
        .uri("/api/v1/reconciliation/import")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 201, "expected 201 Created on successful import");
    let body: serde_json::Value = test::read_body_json(resp).await;
    let checksum = body["file_checksum"]
        .as_str()
        .expect("file_checksum in response");
    let storage_path = body["storage_path"]
        .as_str()
        .expect("storage_path in response");

    // The storage file must exist and contain the original CSV bytes.
    let on_disk = std::fs::read(storage_path).expect("staged file must exist on disk");
    assert_eq!(on_disk, csv_bytes, "staged file must be byte-identical to the upload");
    assert!(
        storage_path.ends_with(&format!("{}.csv", checksum)),
        "storage_path must be named by checksum, got: {}",
        storage_path
    );
}

/// Duplicate-checksum dedupe: uploading the same bytes twice must yield 409 on
/// the second attempt.
#[actix_web::test]
async fn test_import_duplicate_checksum_returns_409() {
    let (pool, cfg, enc) = build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    {
        let mut conn = pool.get().unwrap();
        seed_user(
            &mut conn,
            "recon_finance_dup",
            venue_booking::users::model::UserRole::Finance,
        );
    }
    let token = login_as!(&app, "recon_finance_dup");

    let unique_ref = format!("DUP-{}", Uuid::new_v4());
    let csv = format!(
        "external_reference,external_amount_cents,transaction_date\n{},500,2026-02-01\n",
        unique_ref
    );
    let (ct, body) = make_multipart("dup.csv", csv.as_bytes());

    // First upload succeeds.
    let req1 = test::TestRequest::post()
        .uri("/api/v1/reconciliation/import")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .insert_header(("Content-Type", ct.clone()))
        .set_payload(body.clone())
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), 201, "first upload should succeed");

    // Second upload of byte-identical content must 409 Conflict.
    let req2 = test::TestRequest::post()
        .uri("/api/v1/reconciliation/import")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(
        resp2.status(),
        409,
        "duplicate-checksum upload must be rejected with 409"
    );
}

/// Multipart body with no fields at all should 422.
#[actix_web::test]
async fn test_import_empty_multipart_returns_422() {
    let (pool, cfg, enc) = build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    {
        let mut conn = pool.get().unwrap();
        seed_user(
            &mut conn,
            "recon_finance_empty",
            venue_booking::users::model::UserRole::Finance,
        );
    }
    let token = login_as!(&app, "recon_finance_empty");

    // Well-formed multipart envelope but no file field.
    let boundary = "----emptyboundary";
    let body = format!("--{}--\r\n", boundary).into_bytes();
    let ct = format!("multipart/form-data; boundary={}", boundary);

    let req = test::TestRequest::post()
        .uri("/api/v1/reconciliation/import")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        422,
        "empty multipart body must produce 422, got {}",
        resp.status()
    );
}

/// Oversized upload is rejected by the streaming size cap before the full
/// payload is accepted. We drive this by setting `APP__STORAGE__MAX_UPLOAD_BYTES`
/// to a small value via the process environment and running the handler against
/// a larger CSV.
#[actix_web::test]
async fn test_import_oversized_upload_returns_422() {
    // Clamp the upload limit for this test only. AppConfig::load reads env
    // at call time, so we set the override before building cfg.
    std::env::set_var("APP__STORAGE__MAX_UPLOAD_BYTES", "1024"); // 1 KiB

    let (pool, cfg, enc) = build_app_data();

    // Sanity-check the override actually took effect before we build the app.
    assert_eq!(
        cfg.storage.max_upload_bytes, 1024,
        "test env override for max_upload_bytes did not apply"
    );

    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg.clone()),
        web::Data::new(enc.clone()),
    ))
    .await;

    {
        let mut conn = pool.get().unwrap();
        seed_user(
            &mut conn,
            "recon_finance_oversize",
            venue_booking::users::model::UserRole::Finance,
        );
    }
    let token = login_as!(&app, "recon_finance_oversize");

    // Build a CSV whose body is >> 1 KiB by padding external_reference tokens.
    // Header + many rows easily exceeds the cap.
    let mut csv = String::from("external_reference,external_amount_cents,transaction_date\n");
    for i in 0..200 {
        csv.push_str(&format!("REF-{}-PADDING-{}\u{0020},100,2026-03-01\n", i, "x".repeat(32)));
    }
    assert!(
        csv.len() > 1024,
        "test payload must exceed the configured cap"
    );

    let (ct, body) = make_multipart("oversize.csv", csv.as_bytes());
    let req = test::TestRequest::post()
        .uri("/api/v1/reconciliation/import")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        422,
        "oversize upload must return 422, got {}",
        resp.status()
    );

    // Restore env var so later tests in the same process aren't affected.
    // (Integration tests in this repo run with `--test-threads=1` per README.)
    std::env::remove_var("APP__STORAGE__MAX_UPLOAD_BYTES");
}
