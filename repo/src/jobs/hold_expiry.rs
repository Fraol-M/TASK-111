use std::time::Duration;
use tokio::time;
use tracing::{error, info};

use crate::bookings::service as booking_service;
use crate::common::db::DbPool;
use crate::inventory::service as inv_service;
use crate::jobs::{job_finish, job_start};

pub async fn run(pool: DbPool, interval_secs: u64) {
    let mut ticker = time::interval(Duration::from_secs(interval_secs));
    loop {
        ticker.tick().await;
        let run_id = job_start(&pool, "hold_expiry").await;

        let mut total: i32 = 0;
        let mut failed = false;

        // Expire stale inventory holds
        match inv_service::expire_stale_holds(&pool).await {
            Ok(count) => {
                total += count as i32;
                if count > 0 {
                    info!(released = count, "Expired stale inventory holds");
                }
            }
            Err(e) => {
                failed = true;
                error!("hold_expiry: inventory holds error: {}", e);
            }
        }

        // Expire held bookings whose hold timer ran out
        match booking_service::expire_held_bookings(&pool).await {
            Ok(count) => {
                total += count as i32;
                if count > 0 {
                    info!(expired = count, "Expired held bookings");
                }
            }
            Err(e) => {
                failed = true;
                error!("hold_expiry: booking expiry error: {}", e);
            }
        }

        if let Some(run_id) = run_id {
            let status = if failed { "failed" } else { "completed" };
            job_finish(&pool, run_id, status, Some(total), None).await;
        }
    }
}
