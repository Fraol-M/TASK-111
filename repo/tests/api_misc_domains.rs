//! HTTP integration tests consolidating the remaining domain surfaces:
//! notifications (templates + preview), groups (CRUD + messages), assets
//! (CRUD + versions + attachments), evaluations (cycles + CRUD + assignments),
//! payments (intent get/capture), reconciliation listing, audit by id.

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

// ─────────────────────── Notifications (templates + preview) ───────────────────────

#[actix_web::test]
async fn test_notifications_inbox_and_templates_crud_and_preview() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    {
        use diesel::RunQueryDsl;
        let mut conn = pool.get().unwrap();
        // This test reuses the shared test database volume across runs. The
        // template table enforces UNIQUE(trigger_type, channel), so a previous
        // interrupted run can leave behind the exact custom/in_app template
        // shape this test creates and turn the POST into a 409 on rerun.
        diesel::sql_query(
            "DELETE FROM notification_attempts
             WHERE notification_id IN (
                 SELECT id FROM notifications
                 WHERE template_id IN (
                     SELECT id FROM notification_templates
                     WHERE trigger_type = 'custom' AND channel = 'in_app'
                 )
             )",
        )
        .execute(&mut conn)
        .unwrap();
        diesel::sql_query(
            "DELETE FROM notifications
             WHERE template_id IN (
                 SELECT id FROM notification_templates
                 WHERE trigger_type = 'custom' AND channel = 'in_app'
             )",
        )
        .execute(&mut conn)
        .unwrap();
        diesel::sql_query(
            "DELETE FROM notification_templates
             WHERE trigger_type = 'custom' AND channel = 'in_app'",
        )
        .execute(&mut conn)
        .unwrap();
        common::seed_user(&mut conn, "aoth_ops", venue_booking::users::model::UserRole::OperationsManager);
        common::seed_user(&mut conn, "aoth_admin", venue_booking::users::model::UserRole::Administrator);
        let mid = common::seed_user(&mut conn, "aoth_mem", venue_booking::users::model::UserRole::Member);
        common::seed_member(&mut conn, mid);
    }
    let ops_token = login!(&app, "aoth_ops");
    let admin_token = login!(&app, "aoth_admin");
    let mem_token = login!(&app, "aoth_mem");

    // GET inbox as member (own)
    let req = test::TestRequest::get()
        .uri("/api/v1/notifications")
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // GET inbox as admin (all)
    let req = test::TestRequest::get()
        .uri("/api/v1/notifications")
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // List templates as ops
    let req = test::TestRequest::get()
        .uri("/api/v1/notifications/templates")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Create template (in_app is the default enabled channel)
    let name = format!("tmpl-{}", Uuid::new_v4());
    let req = test::TestRequest::post()
        .uri("/api/v1/notifications/templates")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({
            "name": name,
            "trigger_type": "custom",
            "channel": "in_app",
            "subject_template": "Hello {{who}}",
            "body_template": "Your code is {{code}}",
            "variable_schema": {"who": "string", "code": "string"},
            "is_critical": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201, "template create must 201");
    let body: serde_json::Value = test::read_body_json(resp).await;
    let template_id = body["id"].as_str().unwrap().to_string();

    // Update template
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/notifications/templates/{}", template_id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({"body_template": "Updated body with {{who}}"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Preview
    let req = test::TestRequest::post()
        .uri("/api/v1/notifications/preview")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({
            "template_id": template_id,
            "variables": {"who": "Alice", "code": "42"}
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["body"].as_str().unwrap_or("").contains("Alice"));
}

#[actix_web::test]
async fn test_template_create_rejects_disabled_channel() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "aoth_ops2", venue_booking::users::model::UserRole::OperationsManager);
    }
    let ops_token = login!(&app, "aoth_ops2");

    let req = test::TestRequest::post()
        .uri("/api/v1/notifications/templates")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({
            "name": "disabled-channel",
            "trigger_type": "custom",
            "channel": "email",   // not in APP__NOTIFICATIONS__ENABLED_CHANNELS (default: in_app only)
            "body_template": "x",
            "is_critical": false
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 422, "disabled channel must be rejected with 422");
}

// ─────────────────────── Groups ───────────────────────

#[actix_web::test]
async fn test_groups_full_crud() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let member_id = {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "agrp_ops", venue_booking::users::model::UserRole::OperationsManager);
        let mid = common::seed_user(&mut conn, "agrp_mem", venue_booking::users::model::UserRole::Member);
        common::seed_member(&mut conn, mid);
        mid
    };
    let ops_token = login!(&app, "agrp_ops");
    let mem_token = login!(&app, "agrp_mem");

    // Create group (ops)
    let req = test::TestRequest::post()
        .uri("/api/v1/groups")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({"name": format!("grp-{}", Uuid::new_v4()), "description": "test thread"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let group_id = body["id"].as_str().unwrap().to_string();

    // List groups (ops sees all)
    let req = test::TestRequest::get()
        .uri("/api/v1/groups")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Ops adds the member to the group
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/groups/{}/members", group_id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({"user_id": member_id}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "add member must succeed, got {}",
        resp.status()
    );

    // Member lists their groups
    let req = test::TestRequest::get()
        .uri("/api/v1/groups")
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Member reads the group detail
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/groups/{}", group_id))
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Member lists group members
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/groups/{}/members", group_id))
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Member posts a message
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/groups/{}/messages", group_id))
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .set_json(json!({"body": "hello world"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "post message: {}", resp.status());

    // List messages
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/groups/{}/messages", group_id))
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Ops removes the member
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/groups/{}/members/{}", group_id, member_id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "remove member: {}", resp.status());
}

// ─────────────────────── Assets ───────────────────────

#[actix_web::test]
async fn test_assets_create_list_update_versions() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "aast_mgr", venue_booking::users::model::UserRole::AssetManager);
    }
    let mgr_token = login!(&app, "aast_mgr");

    // Create
    let code = format!("ASSET-{}", Uuid::new_v4());
    let req = test::TestRequest::post()
        .uri("/api/v1/assets")
        .insert_header(("Authorization", format!("Bearer {}", mgr_token)))
        .set_json(json!({
            "asset_code": code,
            "name": "Test asset",
            "description": "test",
            "status": "active",
            "procurement_cost_cents": 50_000,
            "depreciation_method": "none",
            "useful_life_years": null,
            "useful_life_months": null,
            "purchase_date": null,
            "location": null,
            "classification": null,
            "brand": null,
            "model": null,
            "owner_unit": null,
            "responsible_user_id": null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let id = body["id"].as_str().unwrap().to_string();
    let version = body["version"].as_i64().unwrap_or(0) as i32;

    // List (authenticated)
    let req = test::TestRequest::get()
        .uri("/api/v1/assets")
        .insert_header(("Authorization", format!("Bearer {}", mgr_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Update
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/assets/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", mgr_token)))
        .set_json(json!({
            "name": "Renamed asset",
            "expected_version": version
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "update with matching version: got {}", resp.status());

    // List versions
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/assets/{}/versions", id))
        .insert_header(("Authorization", format!("Bearer {}", mgr_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let items = body
        .as_array()
        .cloned()
        .or_else(|| body["data"].as_array().cloned())
        .unwrap_or_default();
    assert!(
        !items.is_empty(),
        "version list must include at least one before-image"
    );
    let first_version = items[0]["version_no"].as_i64().unwrap_or(1);

    // Fetch a specific version
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/assets/{}/versions/{}", id, first_version))
        .insert_header(("Authorization", format!("Bearer {}", mgr_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Upload a small PNG-ish attachment (multipart). Only MIME + byte size are
    // validated; we don't need a real PNG for the upload path to succeed.
    let boundary = "----assetattachboundary";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"doc.png\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
    body.extend_from_slice(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0x00, 0x01]);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/assets/{}/attachments", id))
        .insert_header(("Authorization", format!("Bearer {}", mgr_token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "asset attachment upload must succeed, got {}",
        resp.status()
    );

    // List attachments (now contains the one we just uploaded)
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/assets/{}/attachments", id))
        .insert_header(("Authorization", format!("Bearer {}", mgr_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

// ─────────────────────── Evaluations ───────────────────────

#[actix_web::test]
async fn test_evaluation_cycle_and_evaluation_flow() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let evaluator_id = {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "aeval_admin", venue_booking::users::model::UserRole::Administrator);
        common::seed_user(&mut conn, "aeval_eval", venue_booking::users::model::UserRole::Evaluator)
    };
    let admin_token = login!(&app, "aeval_admin");
    let eval_token = login!(&app, "aeval_eval");

    // Create cycle (admin)
    let starts = chrono::Utc::now();
    let ends = starts + chrono::Duration::days(30);
    let req = test::TestRequest::post()
        .uri("/api/v1/evaluation-cycles")
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({
            "name": format!("cycle-{}", Uuid::new_v4()),
            "description": "test cycle",
            "starts_at": starts,
            "ends_at": ends
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let cycle_id = body["id"].as_str().unwrap().to_string();

    // List cycles (evaluator)
    let req = test::TestRequest::get()
        .uri("/api/v1/evaluation-cycles")
        .insert_header(("Authorization", format!("Bearer {}", eval_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Create evaluation (admin)
    let req = test::TestRequest::post()
        .uri("/api/v1/evaluations")
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({
            "cycle_id": cycle_id,
            "title": "Test evaluation",
            "description": "t",
            "participant_scope": [evaluator_id]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201, "evaluation create: {}", resp.status());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let eval_id = body["id"].as_str().unwrap().to_string();

    // Admin transitions evaluation to open
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/evaluations/{}/state", eval_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({"state": "open"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "evaluation state to open: {}", resp.status());

    // Admin creates assignment for evaluator
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/evaluations/{}/assignments", eval_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .set_json(json!({
            "evaluator_id": evaluator_id,
            "subject_id": evaluator_id
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "assignment create: {}", resp.status());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let assignment_id = body["id"].as_str().unwrap().to_string();

    // Admin lists evaluation assignments
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/evaluations/{}/assignments", eval_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Evaluator reads their evaluation
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/evaluations/{}", eval_id))
        .insert_header(("Authorization", format!("Bearer {}", eval_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "assigned evaluator must read eval: {}",
        resp.status()
    );

    // Evaluator transitions assignment to in_progress
    let req = test::TestRequest::patch()
        .uri(&format!(
            "/api/v1/evaluations/{}/assignments/{}/state",
            eval_id, assignment_id
        ))
        .insert_header(("Authorization", format!("Bearer {}", eval_token)))
        .set_json(json!({"state": "in_progress"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "assignment state: {}",
        resp.status()
    );

    // Evaluator adds an action
    let req = test::TestRequest::post()
        .uri(&format!(
            "/api/v1/evaluations/{}/assignments/{}/actions",
            eval_id, assignment_id
        ))
        .insert_header(("Authorization", format!("Bearer {}", eval_token)))
        .set_json(json!({
            "action_type": "note",
            "notes": "observed behavior",
            "payload": null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "add action: {}",
        resp.status()
    );
}

// ─────────────────────── Payments (intent get / capture) ───────────────────────

#[actix_web::test]
async fn test_payment_intent_create_get_capture() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    let member_id = {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "apay_fin", venue_booking::users::model::UserRole::Finance);
        let mid = common::seed_user(&mut conn, "apay_mem", venue_booking::users::model::UserRole::Member);
        common::seed_member(&mut conn, mid);
        mid
    };
    let finance_token = login!(&app, "apay_fin");

    // Create intent
    let req = test::TestRequest::post()
        .uri("/api/v1/payments/intents")
        .insert_header(("Authorization", format!("Bearer {}", finance_token)))
        .set_json(json!({
            "booking_id": null,
            "member_id": member_id,
            "amount_cents": 10_000,
            "tax_cents": 0,
            "idempotency_key": Uuid::new_v4().to_string()
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let intent_id = body["id"].as_str().unwrap().to_string();

    // Get intent
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/payments/intents/{}", intent_id))
        .insert_header(("Authorization", format!("Bearer {}", finance_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // Capture
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/payments/intents/{}/capture", intent_id))
        .insert_header(("Authorization", format!("Bearer {}", finance_token)))
        .set_json(json!({
            "payment_method": "card",
            "idempotency_key": Uuid::new_v4().to_string(),
            "external_reference": null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201, "capture must 201, got {}", resp.status());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["state"].as_str(), Some("completed"));
}

// ─────────────────────── Reconciliation listing endpoints ───────────────────────

#[actix_web::test]
async fn test_reconciliation_list_endpoints() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "arcn_fin", venue_booking::users::model::UserRole::Finance);
    }
    let token = login!(&app, "arcn_fin");

    // List imports
    let req = test::TestRequest::get()
        .uri("/api/v1/reconciliation/imports")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["data"].is_array());

    // Get by random id → 404
    let random = Uuid::new_v4();
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reconciliation/imports/{}", random))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404, "missing import must be 404");

    // Rows for random id — list returns 200 with empty data array
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reconciliation/imports/{}/rows", random))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

// ─────────────────────── Audit detail by id ───────────────────────

#[actix_web::test]
async fn test_audit_log_detail_by_id() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    {
        let mut conn = pool.get().unwrap();
        common::seed_user(&mut conn, "aadt_admin", venue_booking::users::model::UserRole::Administrator);
    }
    let token = login!(&app, "aadt_admin");

    // Insert one audit log via the production helper so we have a real row
    let audit_row_id = {
        let mut conn = pool.get().unwrap();
        let new = venue_booking::audit::model::NewAuditLog {
            id: Uuid::new_v4(),
            correlation_id: Some("corr-test".to_string()),
            actor_user_id: None,
            action: "coverage_seed".to_string(),
            entity_type: "test".to_string(),
            entity_id: "coverage".to_string(),
            old_value: None,
            new_value: None,
            metadata: None,
            created_at: chrono::Utc::now(),
            row_hash: String::new(),
            previous_hash: None,
        };
        let inserted =
            venue_booking::audit::repository::insert_audit_log(&mut conn, new).expect("insert");
        inserted.id
    };

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/audit/logs/{}", audit_row_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "admin must read audit log by id");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["action"].as_str(), Some("coverage_seed"));
    assert!(
        body["row_hash"].as_str().unwrap_or("").len() == 64,
        "row_hash should be 64 hex chars"
    );
}
