//! HTTP integration tests for the inventory domain:
//! items CRUD, pickup points, delivery zones, alerts.

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

fn seed_ops(pool: &common::DbPool, suffix: &str) -> Uuid {
    let mut conn = pool.get().unwrap();
    common::seed_user(
        &mut conn,
        &format!("ainv_ops_{}", suffix),
        venue_booking::users::model::UserRole::OperationsManager,
    )
}

fn seed_member(pool: &common::DbPool, suffix: &str) -> Uuid {
    let mut conn = pool.get().unwrap();
    common::seed_user(
        &mut conn,
        &format!("ainv_mem_{}", suffix),
        venue_booking::users::model::UserRole::Member,
    )
}

// ─────────────────────── Items ───────────────────────

#[actix_web::test]
async fn test_inventory_item_crud_flow() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    seed_ops(&pool, "crud");
    seed_member(&pool, "crud");
    let ops_token = login!(&app, &format!("ainv_ops_{}", "crud"));
    let mem_token = login!(&app, &format!("ainv_mem_{}", "crud"));

    // Create
    let sku = format!("SKU-{}", Uuid::new_v4());
    let req = test::TestRequest::post()
        .uri("/api/v1/inventory")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({
            "sku": sku,
            "name": "Test item",
            "description": null,
            "available_qty": 10,
            "safety_stock": 2,
            "pickup_point_id": null,
            "zone_id": null,
            "cutoff_hours": 2
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let id = body["id"].as_str().unwrap().to_string();
    let version = body["version"].as_i64().unwrap_or(0) as i32;

    // List as member (any authenticated user can read)
    let req = test::TestRequest::get()
        .uri("/api/v1/inventory")
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // GET one
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/inventory/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // PATCH — optimistic version required
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/inventory/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({
            "name": "Renamed item",
            "version": version
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "patch succeeds with matching version");

    // PATCH with stale version → 412 precondition
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/inventory/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({
            "name": "Again",
            "version": version
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 412, "stale version must fail 412");

    // Restock
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/inventory/{}/restock", id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({"quantity": 5}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "restock succeeds");
}

#[actix_web::test]
async fn test_inventory_create_requires_ops_role() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    seed_member(&pool, "rolecheck");
    let token = login!(&app, &format!("ainv_mem_{}", "rolecheck"));

    let req = test::TestRequest::post()
        .uri("/api/v1/inventory")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "sku": "nope", "name": "nope",
            "available_qty": 0, "safety_stock": 0
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403);
}

#[actix_web::test]
async fn test_inventory_alerts_list_and_ack() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    seed_ops(&pool, "alerts");
    let ops_token = login!(&app, &format!("ainv_ops_{}", "alerts"));

    // Seed an open restock alert so the list is non-empty and we have an id to ack.
    let alert_id = {
        use diesel::prelude::*;
        use venue_booking::schema::restock_alerts;
        let mut conn = pool.get().unwrap();
        let item_id = common::seed_inventory_item(&mut conn, &format!("SKU-ALERT-{}", Uuid::new_v4()));
        let aid = Uuid::new_v4();
        diesel::insert_into(restock_alerts::table)
            .values((
                restock_alerts::id.eq(aid),
                restock_alerts::inventory_item_id.eq(item_id),
                restock_alerts::triggered_qty.eq(0i32),
                restock_alerts::triggered_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .expect("seed alert");
        aid
    };

    let req = test::TestRequest::get()
        .uri("/api/v1/inventory/alerts")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "alerts list must succeed");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["data"].is_array());

    // Acknowledge the alert
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/inventory/alerts/{}/ack", alert_id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "ack must succeed");
}

// ─────────────────────── Pickup points ───────────────────────

#[actix_web::test]
async fn test_pickup_point_full_crud() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    seed_ops(&pool, "pp");
    seed_member(&pool, "pp");
    let ops_token = login!(&app, &format!("ainv_ops_{}", "pp"));
    let mem_token = login!(&app, &format!("ainv_mem_{}", "pp"));

    // CREATE (ops)
    let name = format!("PP-{}", Uuid::new_v4());
    let req = test::TestRequest::post()
        .uri("/api/v1/inventory/pickup-points")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({
            "name": name,
            "address": "123 Test St",
            "cutoff_hours": 4
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let id = body["id"].as_str().unwrap().to_string();
    assert_eq!(body["cutoff_hours"].as_i64(), Some(4));

    // LIST as member (must be allowed — members choose pickup at booking time)
    let req = test::TestRequest::get()
        .uri("/api/v1/inventory/pickup-points")
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // GET (member ok)
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/inventory/pickup-points/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // PATCH (ops only) — clear_cutoff resets to null
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/inventory/pickup-points/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({"clear_cutoff": true}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["cutoff_hours"].is_null(), "clear_cutoff must NULL the value");

    // PATCH as member (should 403)
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/inventory/pickup-points/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .set_json(json!({"name": "Hack"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 403, "member must not be able to mutate pickup point");

    // DELETE (ops)
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/inventory/pickup-points/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 204, "ops must be able to delete unreferenced pickup");
}

#[actix_web::test]
async fn test_pickup_point_delete_blocked_when_referenced() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    seed_ops(&pool, "ppref");
    let ops_token = login!(&app, &format!("ainv_ops_{}", "ppref"));

    // Seed pickup point + item that references it
    let pickup_id = {
        let mut conn = pool.get().unwrap();
        common::seed_pickup_point(&mut conn, &format!("PP-ref-{}", Uuid::new_v4()))
    };
    // Link an inventory item to this pickup
    {
        use diesel::prelude::*;
        use venue_booking::schema::inventory_items;
        let mut conn = pool.get().unwrap();
        diesel::insert_into(inventory_items::table)
            .values((
                inventory_items::id.eq(Uuid::new_v4()),
                inventory_items::sku.eq(format!("refsku-{}", Uuid::new_v4())),
                inventory_items::name.eq("refitem"),
                inventory_items::description.eq(None::<String>),
                inventory_items::available_qty.eq(10i32),
                inventory_items::safety_stock.eq(0i32),
                inventory_items::publish_status
                    .eq(venue_booking::inventory::model::PublishStatus::Published),
                inventory_items::pickup_point_id.eq(Some(pickup_id)),
                inventory_items::zone_id.eq(None::<Uuid>),
                inventory_items::cutoff_hours.eq(2i32),
                inventory_items::version.eq(0i32),
                inventory_items::created_at.eq(chrono::Utc::now()),
                inventory_items::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .expect("seed referenced item");
    }

    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/inventory/pickup-points/{}", pickup_id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        409,
        "DELETE must 409 when a pickup is still referenced"
    );
}

// ─────────────────────── Delivery zones ───────────────────────

#[actix_web::test]
async fn test_zone_full_crud() {
    let (pool, cfg, enc) = common::build_app_data();
    let app = test::init_service(venue_booking::app::build_app(
        web::Data::new(pool.clone()),
        web::Data::new(cfg),
        web::Data::new(enc),
    ))
    .await;
    seed_ops(&pool, "zn");
    seed_member(&pool, "zn");
    let ops_token = login!(&app, &format!("ainv_ops_{}", "zn"));
    let mem_token = login!(&app, &format!("ainv_mem_{}", "zn"));

    let name = format!("Z-{}", Uuid::new_v4());
    let req = test::TestRequest::post()
        .uri("/api/v1/inventory/zones")
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({
            "name": name,
            "description": "Zone A",
            "cutoff_hours": 8
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let id = body["id"].as_str().unwrap().to_string();

    // List (member ok)
    let req = test::TestRequest::get()
        .uri("/api/v1/inventory/zones")
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // GET (member ok)
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/inventory/zones/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", mem_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // PATCH — update cutoff to a new value
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/inventory/zones/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .set_json(json!({"cutoff_hours": 12}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["cutoff_hours"].as_i64(), Some(12));

    // DELETE (ops)
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/inventory/zones/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", ops_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 204);
}
