use std::time::Duration;
use tokio::time;
use tracing::{error, info};

use crate::common::db::DbPool;
use crate::jobs::{job_finish, job_start};

/// Find published items with available_qty=0 and unpublish them + raise restock alerts.
pub async fn run(pool: DbPool, interval_secs: u64) {
    let mut ticker = time::interval(Duration::from_secs(interval_secs));
    loop {
        ticker.tick().await;
        let run_id = job_start(&pool, "zero_qty_unpublish").await;

        match check_zero_qty(&pool).await {
            Ok(()) => {
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "completed", None, None).await;
                }
            }
            Err(e) => {
                error!("zero_qty_unpublish: {}", e);
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "failed", None, Some(e.to_string())).await;
                }
            }
        }
    }
}

async fn check_zero_qty(pool: &DbPool) -> Result<(), crate::common::errors::AppError> {
    use crate::inventory::model::PublishStatus;
    use crate::schema::inventory_items;
    use chrono::Utc;
    use diesel::prelude::*;

    let pool_c = pool.clone();
    let zero_items: Vec<uuid::Uuid> = actix_web::web::block(move || -> Result<Vec<uuid::Uuid>, crate::common::errors::AppError> {
        let mut conn = pool_c.get()?;
        inventory_items::table
            .filter(inventory_items::available_qty.eq(0))
            .filter(inventory_items::publish_status.eq(PublishStatus::Published))
            .select(inventory_items::id)
            .load(&mut conn)
            .map_err(crate::common::errors::AppError::from)
    })
    .await
    .map_err(|e| crate::common::errors::AppError::Internal(e.to_string()))??;

    if !zero_items.is_empty() {
        info!(count = zero_items.len(), "Unpublishing zero-qty items and generating restock alerts");
        let pool_c2 = pool.clone();
        actix_web::web::block(move || -> Result<(), crate::common::errors::AppError> {
            let mut conn = pool_c2.get()?;
            for item_id in &zero_items {
                // Load each item's safety_stock for the centralized alert check
                let item = crate::inventory::repository::find_item(&mut conn, *item_id)?;
                // Use the centralized helper for consistent unpublish + restock alert
                crate::inventory::service::check_qty_alerts(
                    &mut conn,
                    *item_id,
                    0,                     // new_qty (zero)
                    item.available_qty,    // old_qty
                    item.safety_stock,
                )?;
                diesel::update(
                    inventory_items::table.filter(inventory_items::id.eq(*item_id)),
                )
                .set(inventory_items::updated_at.eq(Utc::now()))
                .execute(&mut conn)
                .map_err(crate::common::errors::AppError::from)?;
            }
            Ok(())
        })
        .await
        .map_err(|e| crate::common::errors::AppError::Internal(e.to_string()))??;
    }

    Ok(())
}
