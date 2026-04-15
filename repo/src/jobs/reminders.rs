use std::time::Duration;
use tokio::time;
use tracing::{error, info};

use crate::common::db::DbPool;
use crate::config::AppConfig;
use crate::jobs::{job_finish, job_start};

/// Find confirmed bookings with start_at in [now+23h, now+25h] and send T-24h reminders.
pub async fn run(pool: DbPool, cfg: AppConfig, interval_secs: u64) {
    let mut ticker = time::interval(Duration::from_secs(interval_secs));
    loop {
        ticker.tick().await;
        let run_id = job_start(&pool, "reminders").await;

        match send_reminders(&pool, &cfg).await {
            Ok(()) => {
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "completed", None, None).await;
                }
            }
            Err(e) => {
                error!("reminders job: {}", e);
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "failed", None, Some(e.to_string())).await;
                }
            }
        }
    }
}

async fn send_reminders(
    pool: &DbPool,
    cfg: &AppConfig,
) -> Result<(), crate::common::errors::AppError> {
    use crate::bookings::model::BookingState;
    use crate::schema::{bookings, notifications};
    use chrono::Utc;
    use diesel::prelude::*;

    let now = Utc::now();
    let window_start = now + chrono::Duration::hours(23);
    let window_end = now + chrono::Duration::hours(25);

    let pool_c = pool.clone();
    let upcoming: Vec<crate::bookings::model::Booking> = actix_web::web::block(move || -> Result<_, crate::common::errors::AppError> {
        let mut conn = pool_c.get()?;
        bookings::table
            .filter(bookings::state.eq(BookingState::Confirmed))
            .filter(bookings::start_at.gt(window_start))
            .filter(bookings::start_at.lt(window_end))
            .load(&mut conn)
            .map_err(crate::common::errors::AppError::from)
    })
    .await
    .map_err(|e| crate::common::errors::AppError::Internal(e.to_string()))??;

    for booking in upcoming {
        // Check if reminder already sent by looking at notifications
        let pool_c2 = pool.clone();
        let booking_id = booking.id;
        let member_id = booking.member_id;

        let already_sent: bool = actix_web::web::block(move || -> Result<bool, crate::common::errors::AppError> {
            let mut conn = pool_c2.get()?;
            use crate::notifications::model::TemplateTrigger;
            let count: i64 = notifications::table
                .filter(notifications::user_id.eq(member_id))
                .filter(notifications::trigger_type.eq(TemplateTrigger::BookingReminder24h))
                .filter(notifications::reference_id.eq(Some(booking_id)))
                .count()
                .get_result(&mut conn)
                .map_err(crate::common::errors::AppError::from)?;
            Ok(count > 0)
        })
        .await
        .map_err(|e| crate::common::errors::AppError::Internal(e.to_string()))??;

        if !already_sent {
            let mut vars = std::collections::HashMap::new();
            vars.insert("booking_id".into(), serde_json::Value::String(booking_id.to_string()));
            vars.insert("start_at".into(), serde_json::Value::String(booking.start_at.to_rfc3339()));

            // Resolve channel from member preferences (fallback: InApp).
            // Non-in_app dispatch failures still produce an InApp fallback notification
            // inside send_notification, so the user always receives the reminder.
            let channel = crate::notifications::service::resolve_user_channel(pool, cfg, member_id).await;

            match crate::notifications::service::send_notification(
                pool,
                cfg,
                member_id,
                crate::notifications::model::TemplateTrigger::BookingReminder24h,
                channel,
                vars,
                Some(booking_id),
            )
            .await
            {
                Ok(_) => {
                    info!(booking_id = %booking_id, "Sent T-24h reminder");
                }
                Err(e) => {
                    error!(
                        booking_id = %booking_id,
                        member_id = %member_id,
                        error = %e,
                        "Failed to send T-24h reminder notification"
                    );
                }
            }
        }
    }

    Ok(())
}
