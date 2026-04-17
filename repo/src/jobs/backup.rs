use std::time::Duration;
use tracing::{error, info};

use crate::common::db::DbPool;
use crate::config::AppConfig;
use crate::jobs::{job_finish, job_start};

/// Daily database backup via pg_dump at the hour specified in config.
pub async fn run(pool: DbPool, cfg: AppConfig, database_url: String) {
    let run_hour = cfg.jobs.backup_hour;
    let tz_offset = cfg.backup.timezone_offset_minutes;
    loop {
        let next_run = next_daily_run_at(run_hour, tz_offset);
        let wait_dur = next_run
            .signed_duration_since(chrono::Utc::now())
            .to_std()
            .unwrap_or(Duration::from_secs(3600));

        tokio::time::sleep(wait_dur).await;
        let run_id = job_start(&pool, "backup").await;

        match do_backup(&cfg, &database_url).await {
            Ok(()) => {
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "completed", Some(1), None).await;
                }
            }
            Err(e) => {
                error!("backup job: {}", e);
                if let Some(run_id) = run_id {
                    job_finish(&pool, run_id, "failed", None, Some(e.to_string())).await;
                }
            }
        }
    }
}

/// Compute the next daily run time in UTC, scheduled at the given hour in the
/// operator's local timezone (expressed as a UTC offset in minutes).
fn next_daily_run_at(hour: u32, tz_offset_minutes: i32) -> chrono::DateTime<chrono::Utc> {
    use chrono::Utc;
    let offset_secs = tz_offset_minutes * 60;
    let offset = chrono::FixedOffset::east_opt(offset_secs)
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());

    let local_now = Utc::now().with_timezone(&offset);
    let today = local_now.date_naive();
    let run_time = chrono::NaiveTime::from_hms_opt(hour, 0, 0).unwrap();
    let today_run_naive = today.and_time(run_time);

    let today_run_utc = today_run_naive
        .and_local_timezone(offset)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1));

    if today_run_utc > Utc::now() {
        today_run_utc
    } else {
        today_run_utc + chrono::Duration::days(1)
    }
}

async fn do_backup(
    cfg: &AppConfig,
    database_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let backup_dir = &cfg.backup.dir;

    // Create backup directory if it doesn't exist
    tokio::fs::create_dir_all(backup_dir).await?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let output_path = format!("{}/{}.sql.gz", backup_dir, timestamp);

    // Direct invocation: spawn pg_dump and stream its stdout straight into a
    // gzip writer on the file. No shell, no quoting hazards, no dependence on
    // an external `gzip` binary or shell-builtin redirection. The DATABASE_URL
    // is passed via the standard pg_dump `-d` flag so it appears in argv only
    // (no subshell expansion); operators concerned about argv visibility can
    // export `PGPASSWORD`/`PGSERVICE` and pass the connection params via env.
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;

    let mut child = tokio::process::Command::new("pg_dump")
        .arg("-d")
        .arg(database_url)
        .arg("--no-password")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn pg_dump (is it on PATH?): {}", e))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or("pg_dump child has no stdout handle")?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or("pg_dump child has no stderr handle")?;

    // Stream pg_dump stdout through a gzip encoder onto disk. The gzip writer
    // runs on a blocking task because flate2 is synchronous; chunks are
    // forwarded over a bounded sync_channel so we do not buffer the entire
    // dump in memory.
    use std::sync::mpsc::sync_channel;
    let (tx, rx) = sync_channel::<Vec<u8>>(64);
    let output_path_for_writer = output_path.clone();
    let writer_handle = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::fs::File;
        use std::io::{BufWriter, Write};
        let file = File::create(&output_path_for_writer)?;
        let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
        for chunk in rx {
            encoder.write_all(&chunk)?;
        }
        encoder.finish()?.flush()?;
        Ok(())
    });

    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = stdout.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        // Send may fail only if writer thread has died; surface that error.
        tx.send(buf[..n].to_vec())
            .map_err(|_| "gzip writer task terminated unexpectedly")?;
    }
    drop(tx); // signal writer to finish
    writer_handle
        .await
        .map_err(|e| format!("gzip writer join error: {}", e))?
        .map_err(|e| format!("gzip writer io error: {}", e))?;

    // Drain stderr for diagnostics (only logged on failure).
    let mut stderr_buf = Vec::new();
    stderr.read_to_end(&mut stderr_buf).await.ok();

    let status = child.wait().await?;
    if status.success() {
        info!(path = %output_path, "Database backup completed");
        Ok(())
    } else {
        let stderr_text = String::from_utf8_lossy(&stderr_buf);
        Err(format!(
            "pg_dump exited with status {}: {}",
            status,
            stderr_text.trim()
        )
        .into())
    }
}
