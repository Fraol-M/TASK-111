use std::time::Duration;
use tokio::time;
use tracing::{error, info};

use crate::common::db::DbPool;
use crate::jobs::{job_finish, job_start};
use crate::notifications::service as notif_service;

/// Periodically re-deliver notifications that were suppressed by DND.
pub async fn run(pool: DbPool, interval_secs: u64) {
    let mut ticker = time::interval(Duration::from_secs(interval_secs));
    loop {
        ticker.tick().await;
        let run_id = job_start(&pool, "dnd_resolver").await;

        match notif_service::deliver_dnd_queue(&pool).await {
            Ok(count) => {
                if count > 0 {
                    info!(delivered = count, "DND queue processed");
                }
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "completed", Some(count as i32), None).await;
                }
            }
            Err(e) => {
                error!("dnd_resolver: {}", e);
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "failed", None, Some(e.to_string())).await;
                }
            }
        }
    }
}
