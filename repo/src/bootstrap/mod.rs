use chrono::Utc;
use diesel::prelude::*;
use diesel::PgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::{info, warn};
use uuid::Uuid;

use crate::common::db::DbPool;
use crate::config::AppConfig;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn run_migrations(pool: &DbPool) {
    let mut conn: diesel::r2d2::PooledConnection<
        diesel::r2d2::ConnectionManager<PgConnection>,
    > = pool.get().expect("Failed to get DB connection for migrations");

    info!("Running pending database migrations...");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
    info!("Migrations complete.");
}

/// Demo users created when `APP__BOOTSTRAP__SEED_DEMO_USERS=true`. Keep in
/// sync with the README "Demo credentials" table and with any role changes.
const DEMO_USERS: &[(&str, crate::users::model::UserRole)] = &[
    ("admin", crate::users::model::UserRole::Administrator),
    ("ops", crate::users::model::UserRole::OperationsManager),
    ("finance", crate::users::model::UserRole::Finance),
    ("asset_mgr", crate::users::model::UserRole::AssetManager),
    ("evaluator", crate::users::model::UserRole::Evaluator),
    ("reviewer", crate::users::model::UserRole::Reviewer),
    ("member", crate::users::model::UserRole::Member),
];

/// Idempotently insert the demo users configured for this deployment.
/// No-op when `cfg.bootstrap.seed_demo_users` is false (the production default).
///
/// For each username:
///   * If the row is absent, insert it with the configured demo password.
///   * If the row already exists, skip — do NOT overwrite operator-modified
///     passwords or roles. This keeps the flag safe to leave on across restarts.
///
/// When the `member` demo user is inserted, a matching `members` row + default
/// `member_preferences` row are also provisioned so the member-domain endpoints
/// work immediately.
pub fn seed_demo_users_if_enabled(pool: &DbPool, cfg: &AppConfig) {
    if !cfg.bootstrap.seed_demo_users {
        return;
    }

    info!("Seeding demo users (APP__BOOTSTRAP__SEED_DEMO_USERS=true)");
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Demo-user seeding skipped: could not get DB connection");
            return;
        }
    };

    // Hash once — the same password applies to every demo account.
    let hash = match crate::auth::service::hash_password(&cfg.bootstrap.demo_password) {
        Ok(h) => h,
        Err(e) => {
            warn!(error = %e, "Demo-user seeding aborted: password hashing failed");
            return;
        }
    };

    for (username, role) in DEMO_USERS {
        if let Err(e) = seed_one_demo_user(&mut conn, username, role, &hash) {
            warn!(username = %username, error = %e, "Demo-user seed failed for user");
        }
    }
    info!("Demo-user seeding complete.");
}

fn seed_one_demo_user(
    conn: &mut PgConnection,
    username: &str,
    role: &crate::users::model::UserRole,
    password_hash: &str,
) -> Result<(), crate::common::errors::AppError> {
    use crate::schema::users;
    let now = Utc::now();
    let new_id = Uuid::new_v4();

    // ON CONFLICT DO NOTHING — never overwrite a pre-existing user.
    let inserted: Vec<Uuid> = diesel::insert_into(users::table)
        .values((
            users::id.eq(new_id),
            users::username.eq(username),
            users::password_hash.eq(password_hash),
            users::role.eq(role),
            users::status.eq(crate::users::model::UserStatus::Active),
            users::created_at.eq(now),
            users::updated_at.eq(now),
        ))
        .on_conflict(users::username)
        .do_nothing()
        .returning(users::id)
        .get_results(conn)
        .map_err(crate::common::errors::AppError::from)?;

    // If we actually inserted a `member` row, provision the member profile too
    // so /members/{id}/* endpoints work immediately. When the row already
    // existed the operator's existing profile (if any) is left untouched.
    if let (Some(actual_id), crate::users::model::UserRole::Member) = (inserted.first(), role) {
        provision_member_profile(conn, *actual_id)?;
    }

    Ok(())
}

fn provision_member_profile(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<(), crate::common::errors::AppError> {
    use crate::schema::{member_preferences, members};
    let now = Utc::now();

    diesel::insert_into(members::table)
        .values((
            members::user_id.eq(user_id),
            members::tier.eq(crate::members::model::MemberTier::Silver),
            members::points_balance.eq(0i32),
            members::wallet_balance.eq(""),
            members::blacklist_flag.eq(false),
            members::rolling_12m_spend.eq(0i64),
            members::version.eq(0i32),
            members::updated_at.eq(now),
        ))
        .on_conflict(members::user_id)
        .do_nothing()
        .execute(conn)
        .map_err(crate::common::errors::AppError::from)?;

    diesel::insert_into(member_preferences::table)
        .values((
            member_preferences::user_id.eq(user_id),
            member_preferences::notification_opt_out.eq(serde_json::Value::Array(vec![])),
            member_preferences::preferred_channel.eq("in_app"),
            member_preferences::timezone_offset_minutes.eq(0),
            member_preferences::updated_at.eq(now),
        ))
        .on_conflict(member_preferences::user_id)
        .do_nothing()
        .execute(conn)
        .map_err(crate::common::errors::AppError::from)?;

    Ok(())
}
