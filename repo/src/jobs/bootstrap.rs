use crate::common::db::DbPool;
use crate::config::AppConfig;

/// Spawn all background job loops. Each runs indefinitely in a separate tokio task.
pub fn start_all_jobs(pool: DbPool, cfg: AppConfig, database_url: String) {
    let hold_interval = cfg.jobs.hold_expiry_interval_secs;
    let payment_interval = cfg.jobs.payment_timeout_interval_secs;
    let reminder_interval = cfg.jobs.reminder_interval_secs;
    let dnd_interval = cfg.jobs.dnd_resolve_interval_secs;
    let zero_qty_interval = cfg.jobs.zero_qty_interval_secs;

    // Hold expiry: expire stale inventory holds and held bookings
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            crate::jobs::hold_expiry::run(pool, hold_interval).await;
        });
    }

    // Payment timeout: close expired payment intents
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            crate::jobs::payment_timeout::run(pool, payment_interval).await;
        });
    }

    // T-24h booking reminders
    {
        let pool = pool.clone();
        let cfg_clone = cfg.clone();
        tokio::spawn(async move {
            crate::jobs::reminders::run(pool, cfg_clone, reminder_interval).await;
        });
    }

    // DND queue resolver: re-deliver suppressed notifications after DND window
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            crate::jobs::dnd_resolver::run(pool, dnd_interval).await;
        });
    }

    // Daily tier recalculation at hour specified by jobs.tier_recalc_hour
    {
        let pool = pool.clone();
        let cfg_clone = cfg.clone();
        tokio::spawn(async move {
            crate::jobs::tier_recalc::run(pool, cfg_clone).await;
        });
    }

    // Zero-qty unpublish check
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            crate::jobs::zero_qty_unpublish::run(pool, zero_qty_interval).await;
        });
    }

    // Daily database backup at 03:00 UTC
    {
        let pool = pool.clone();
        let cfg_clone = cfg.clone();
        tokio::spawn(async move {
            crate::jobs::backup::run(pool, cfg_clone, database_url).await;
        });
    }
}
