use std::time::Duration;
use tokio::time;
use tracing::{error, info};

use crate::common::db::DbPool;
use crate::config::AppConfig;
use crate::jobs::{job_finish, job_start};

/// Daily tier recalculation: updates rolling_12m_spend from payments and recalculates tiers.
pub async fn run(pool: DbPool, cfg: AppConfig) {
    let run_hour = cfg.jobs.tier_recalc_hour;
    loop {
        let next_run = next_daily_run_at(run_hour);
        let wait_dur = next_run
            .signed_duration_since(chrono::Utc::now())
            .to_std()
            .unwrap_or(Duration::from_secs(3600));

        tokio::time::sleep(wait_dur).await;
        let run_id = job_start(&pool, "tier_recalc").await;

        match recalculate_all_tiers(&pool).await {
            Ok(()) => {
                info!("tier_recalc: daily tier recalculation completed");
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "completed", None, None).await;
                }
            }
            Err(e) => {
                error!("tier_recalc: {}", e);
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "failed", None, Some(e.to_string())).await;
                }
            }
        }
    }
}

fn next_daily_run_at(hour: u32) -> chrono::DateTime<chrono::Utc> {
    use chrono::{TimeZone, Utc};
    let now = Utc::now();
    let today = now.date_naive();
    let run_time = chrono::NaiveTime::from_hms_opt(hour, 0, 0).unwrap();
    let today_run = chrono::Utc.from_utc_datetime(&today.and_time(run_time));
    if today_run > now {
        today_run
    } else {
        today_run + chrono::Duration::days(1)
    }
}

async fn recalculate_all_tiers(pool: &DbPool) -> Result<(), crate::common::errors::AppError> {
    use crate::schema::members;
    use diesel::prelude::*;

    let pool_c = pool.clone();
    let member_ids: Vec<uuid::Uuid> = actix_web::web::block(move || -> Result<Vec<uuid::Uuid>, crate::common::errors::AppError> {
        let mut conn = pool_c.get()?;
        members::table
            .select(members::user_id)
            .load(&mut conn)
            .map_err(crate::common::errors::AppError::from)
    })
    .await
    .map_err(|e| crate::common::errors::AppError::Internal(e.to_string()))??;

    for member_id in member_ids {
        if let Err(e) = crate::members::service::recalculate_tier(pool, member_id).await {
            error!(member_id = %member_id, "tier_recalc: failed for member: {}", e);
        }
    }

    Ok(())
}
