use std::time::Duration;
use tokio::time;
use tracing::{error, info};

use crate::common::db::DbPool;
use crate::jobs::{job_finish, job_start};
use crate::payments::service as payment_service;

pub async fn run(pool: DbPool, interval_secs: u64) {
    let mut ticker = time::interval(Duration::from_secs(interval_secs));
    loop {
        ticker.tick().await;
        let run_id = job_start(&pool, "payment_timeout").await;

        match payment_service::close_expired_intents(&pool).await {
            Ok(count) => {
                if count > 0 {
                    info!(closed = count, "Closed expired payment intents");
                }
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "completed", Some(count as i32), None).await;
                }
            }
            Err(e) => {
                error!("payment_timeout: {}", e);
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "failed", None, Some(e.to_string())).await;
                }
            }
        }
    }
}
