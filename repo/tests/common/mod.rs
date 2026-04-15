//! Shared harness for integration tests.
//!
//! Each `tests/*.rs` file is its own crate and pulls these helpers in via
//! `mod common;`. Per-test-file `login!` macros sit inside the test file
//! because naming the Actix service type across tests is awkward; the macro
//! form stays terse.

use chrono::Utc;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::PgConnection;
use uuid::Uuid;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// Canonical password used by every seeded test user. Integration tests call
/// `login_as(app, username, DEFAULT_PASSWORD)` / `login!(app, username)` which
/// expand to this constant.
pub const DEFAULT_PASSWORD: &str = "Test1234!";

/// Build an isolated test database pool from TEST_DATABASE_URL env var.
///
/// Uses `from_filename_override` so that .env.test always wins, even if a
/// test file already called `dotenvy::dotenv()` which loaded the production
/// .env (DATABASE_URL pointing to "db" instead of "db_test").
pub fn build_test_pool() -> DbPool {
    let _ = dotenvy::from_filename_override(".env.test");

    let database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("TEST_DATABASE_URL or DATABASE_URL must be set for integration tests");

    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .max_size(5)
        .build(manager)
        .expect("Failed to create test DB pool")
}

/// Run all pending migrations on the test database.
pub fn run_test_migrations(pool: &DbPool) {
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    let mut conn = pool.get().expect("Failed to get test DB connection");
    conn.run_pending_migrations(MIGRATIONS).expect("Failed to run test migrations");
}

/// Build `(pool, cfg, enc)` for an integration test. Runs migrations and
/// loads `AppConfig` from env — most test files want all three.
pub fn build_app_data() -> (
    DbPool,
    venue_booking::config::AppConfig,
    venue_booking::common::crypto::EncryptionKey,
) {
    let _ = dotenvy::from_filename_override(".env.test");
    let pool = build_test_pool();
    run_test_migrations(&pool);
    let cfg = venue_booking::config::AppConfig::load().expect("config");
    let enc = venue_booking::common::crypto::EncryptionKey::from_hex(&cfg.encryption.key_hex)
        .expect("enc key");
    (pool, cfg, enc)
}

/// Insert (or replace) a user with the given role and canonical password.
/// Returns the user's UUID.
pub fn seed_user(
    conn: &mut PgConnection,
    username: &str,
    role: venue_booking::users::model::UserRole,
) -> Uuid {
    use venue_booking::schema::users;
    let id = Uuid::new_v4();
    diesel::delete(users::table.filter(users::username.eq(username)))
        .execute(conn)
        .ok();
    let hash = venue_booking::auth::service::hash_password(DEFAULT_PASSWORD).unwrap();
    diesel::insert_into(users::table)
        .values((
            users::id.eq(id),
            users::username.eq(username),
            users::password_hash.eq(&hash),
            users::role.eq(role),
            users::status.eq(venue_booking::users::model::UserStatus::Active),
            users::created_at.eq(Utc::now()),
            users::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .expect("seed user");
    id
}

/// Seed a `members` row for an existing user with sane defaults. Used by
/// tests that need to hit `/members/{id}/...` endpoints without going through
/// the admin `update_user` provisioning path.
pub fn seed_member(conn: &mut PgConnection, user_id: Uuid) {
    use venue_booking::schema::members;
    diesel::delete(members::table.filter(members::user_id.eq(user_id)))
        .execute(conn)
        .ok();
    diesel::insert_into(members::table)
        .values((
            members::user_id.eq(user_id),
            members::tier.eq(venue_booking::members::model::MemberTier::Silver),
            members::points_balance.eq(0i32),
            members::wallet_balance.eq(""),
            members::blacklist_flag.eq(false),
            members::rolling_12m_spend.eq(0i64),
            members::version.eq(0i32),
            members::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .expect("seed member");
}

/// Seed a `member_preferences` row with default values so notification
/// resolution works without a 404.
pub fn seed_member_preferences(conn: &mut PgConnection, user_id: Uuid) {
    use venue_booking::schema::member_preferences;
    diesel::delete(member_preferences::table.filter(member_preferences::user_id.eq(user_id)))
        .execute(conn)
        .ok();
    diesel::insert_into(member_preferences::table)
        .values((
            member_preferences::user_id.eq(user_id),
            member_preferences::notification_opt_out.eq(serde_json::Value::Array(vec![])),
            member_preferences::preferred_channel.eq("in_app"),
            member_preferences::timezone_offset_minutes.eq(0),
            member_preferences::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .expect("seed member_preferences");
}

/// Seed an inventory pickup point and return its id.
pub fn seed_pickup_point(conn: &mut PgConnection, name: &str) -> Uuid {
    use venue_booking::schema::pickup_points;
    let id = Uuid::new_v4();
    diesel::insert_into(pickup_points::table)
        .values((
            pickup_points::id.eq(id),
            pickup_points::name.eq(name),
            pickup_points::address.eq(None::<String>),
            pickup_points::active.eq(true),
            pickup_points::created_at.eq(Utc::now()),
            pickup_points::cutoff_hours.eq(None::<i32>),
        ))
        .execute(conn)
        .expect("seed pickup_point");
    id
}

/// Seed an inventory item with plenty of stock.
pub fn seed_inventory_item(conn: &mut PgConnection, sku: &str) -> Uuid {
    use venue_booking::schema::inventory_items;
    let id = Uuid::new_v4();
    diesel::insert_into(inventory_items::table)
        .values((
            inventory_items::id.eq(id),
            inventory_items::sku.eq(sku),
            inventory_items::name.eq(sku),
            inventory_items::description.eq(None::<String>),
            inventory_items::available_qty.eq(100i32),
            inventory_items::safety_stock.eq(5i32),
            inventory_items::publish_status
                .eq(venue_booking::inventory::model::PublishStatus::Published),
            inventory_items::pickup_point_id.eq(None::<Uuid>),
            inventory_items::zone_id.eq(None::<Uuid>),
            inventory_items::cutoff_hours.eq(2i32),
            inventory_items::version.eq(0i32),
            inventory_items::created_at.eq(Utc::now()),
            inventory_items::updated_at.eq(Utc::now()),
        ))
        .execute(conn)
        .expect("seed inventory_item");
    id
}
