use actix_web::{web, App, HttpServer};
use tracing::info;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod app;
mod assets;
mod audit;
mod auth;
mod bookings;
mod bootstrap;
mod common;
mod config;
mod evaluations;
mod groups;
mod inventory;
mod jobs;
mod members;
mod notifications;
mod payments;
mod reconciliation;
mod schema;
mod users;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env if present (ignored in Docker where env vars are set directly)
    let _ = dotenvy::dotenv();

    // Initialize structured logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer().json())
        .init();

    info!("Starting Venue Booking & Operations Management System");

    // Load config from environment
    let cfg = config::AppConfig::load().expect("Failed to load application configuration");

    // Build encryption key
    let enc_key = common::crypto::EncryptionKey::from_hex(&cfg.encryption.key_hex)
        .expect("Invalid ENCRYPTION_KEY_HEX — must be 64 hex characters");

    // Build DB pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = common::db::build_pool(&database_url);

    // Run migrations on startup
    bootstrap::run_migrations(&pool);

    // Idempotently seed demo users when the operator opts in via
    // APP__BOOTSTRAP__SEED_DEMO_USERS=true (default false for production).
    // This turns `docker-compose up` into a zero-manual-step bring-up: the 7
    // role-mapped demo accounts from the README work immediately, with no
    // `docker compose exec db psql` required.
    bootstrap::seed_demo_users_if_enabled(&pool, &cfg);

    // Start background jobs (non-async, spawns tokio tasks internally)
    jobs::bootstrap::start_all_jobs(
        pool.clone(),
        cfg.clone(),
        database_url.clone(),
    );

    let bind_addr = format!("{}:{}", cfg.server.host, cfg.server.port);
    info!("Listening on {}", bind_addr);

    let workers = cfg.server.workers;
    let cfg_data = web::Data::new(cfg.clone());
    let enc_data = web::Data::new(enc_key);
    let pool_data = web::Data::new(pool.clone());

    HttpServer::new(move || {
        app::build_app(
            pool_data.clone(),
            cfg_data.clone(),
            enc_data.clone(),
        )
    })
    .workers(workers)
    .bind(&bind_addr)?
    .run()
    .await
}
