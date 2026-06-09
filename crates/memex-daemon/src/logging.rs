use std::fs;
use std::path::Path;

use anyhow::Result;
use tracing::info;

const MAX_LOG_AGE_DAYS: u64 = 7;
const MAX_LOG_SIZE_BYTES: u64 = 50 * 1024 * 1024;

pub fn setup_file_logging(memex_dir: &Path) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = memex_dir.join("logs");
    fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "daemon.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    Ok(guard)
}

pub fn rotate_old_logs(memex_dir: &Path) {
    let log_dir = memex_dir.join("logs");
    if !log_dir.exists() {
        return;
    }

    let cutoff =
        std::time::SystemTime::now() - std::time::Duration::from_secs(MAX_LOG_AGE_DAYS * 86400);

    let entries = match fs::read_dir(&log_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut total_size: u64 = 0;
    let mut files: Vec<(std::path::PathBuf, std::time::SystemTime, u64)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let size = meta.len();
            total_size += size;
            files.push((path, modified, size));
        }
    }

    files.sort_by_key(|(_, m, _)| *m);

    for (path, modified, size) in &files {
        let should_delete = *modified < cutoff || total_size > MAX_LOG_SIZE_BYTES;
        if should_delete {
            if fs::remove_file(path).is_ok() {
                total_size = total_size.saturating_sub(*size);
                info!("rotated old log: {}", path.display());
            }
        }
    }
}
