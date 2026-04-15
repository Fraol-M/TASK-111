pub mod backup;
pub mod bootstrap;
pub mod dnd_resolver;
pub mod hold_expiry;
pub mod payment_timeout;
pub mod reminders;
pub mod tier_recalc;
pub mod zero_qty_unpublish;

use crate::common::db::DbPool;
use uuid::Uuid;

/// Record job run start. Returns the run ID, or None if the insert fails (non-fatal).
pub(crate) async fn job_start(pool: &DbPool, job_name: &'static str) -> Option<Uuid> {
    let pool_c = pool.clone();
    actix_web::web::block(move || -> Result<Uuid, crate::common::errors::AppError> {
        let mut conn = pool_c.get()?;
        crate::audit::repository::start_job_run(&mut conn, job_name)
    })
    .await
    .ok()
    .and_then(|r| r.ok())
}

/// Record job run finish. Failures are silently ignored.
pub(crate) async fn job_finish(
    pool: &DbPool,
    run_id: Uuid,
    status: &'static str,
    items: Option<i32>,
    err: Option<String>,
) {
    let pool_c = pool.clone();
    let _ = actix_web::web::block(move || -> Result<(), crate::common::errors::AppError> {
        let mut conn = pool_c.get()?;
        crate::audit::repository::finish_job_run(&mut conn, run_id, status, items, err)
    })
    .await;
}
