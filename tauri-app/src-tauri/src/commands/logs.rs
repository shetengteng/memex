use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

use memex_core::memex_dir;
use serde::Serialize;

const MAX_TAIL_LINES: usize = 5000;
const MAX_READ_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct DaemonLogFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_secs: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DaemonLogRead {
    pub file: String,
    pub lines: Vec<String>,
    pub total_lines_returned: usize,
    pub truncated: bool,
}

fn logs_dir() -> std::path::PathBuf {
    memex_dir().join("logs")
}

/// 列出 daemon 日志目录里所有 `daemon.log*` 文件，按修改时间倒序。
///
/// 该目录由 `memex-daemon` 在启动时 `setup_file_logging` 写入，
/// 使用 `tracing_appender::rolling::daily` 滚动，文件名形如 `daemon.log.2026-06-07`。
#[tauri::command]
pub async fn list_daemon_log_files() -> Result<Vec<DaemonLogFile>, String> {
    let dir = logs_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let entries = fs::read_dir(&dir).map_err(|e| format!("读取日志目录失败：{e}"))?;
    let mut files: Vec<DaemonLogFile> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if !name.starts_with("daemon.log") {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified_secs = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        files.push(DaemonLogFile {
            name,
            path: path.to_string_lossy().into_owned(),
            size: meta.len(),
            modified_secs,
        });
    }
    files.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));
    Ok(files)
}

/// 读 daemon 日志最后 `lines` 行（默认 500，上限 MAX_TAIL_LINES）。
///
/// - 不传 `file_name` → 默认读最新一份（list 的第 0 个）。
/// - 为避免一次性把 GB 级日志全读进内存，只 seek 到末尾向前读 MAX_READ_BYTES（8 MiB），
///   超出该窗口的旧日志会被截断（`truncated=true` 让前端能提示用户）。
#[tauri::command]
pub async fn read_daemon_log(
    file_name: Option<String>,
    lines: Option<usize>,
) -> Result<DaemonLogRead, String> {
    let files = list_daemon_log_files().await?;
    let target = match file_name {
        Some(n) => files
            .into_iter()
            .find(|f| f.name == n)
            .ok_or_else(|| format!("找不到日志文件：{n}"))?,
        None => files
            .into_iter()
            .next()
            .ok_or_else(|| "日志目录为空（daemon 可能尚未写入日志）".to_string())?,
    };
    let want_lines = lines.unwrap_or(500).min(MAX_TAIL_LINES).max(1);

    let path = std::path::PathBuf::from(&target.path);
    let mut file = fs::File::open(&path).map_err(|e| format!("打开日志失败：{e}"))?;

    let file_size = target.size;
    let start = file_size.saturating_sub(MAX_READ_BYTES);
    if start > 0 {
        file.seek(SeekFrom::Start(start))
            .map_err(|e| format!("seek 日志失败：{e}"))?;
    }
    let reader = BufReader::new(file);
    let mut all_lines: Vec<String> = Vec::new();
    let mut first_partial_dropped = false;
    for (i, line) in reader.lines().enumerate() {
        match line {
            Ok(s) => {
                if start > 0 && i == 0 {
                    first_partial_dropped = true;
                    continue;
                }
                all_lines.push(s);
            }
            Err(_) => continue,
        }
    }

    let truncated = first_partial_dropped || all_lines.len() > want_lines;
    let total = all_lines.len();
    let tail = if total > want_lines {
        all_lines.split_off(total - want_lines)
    } else {
        all_lines
    };

    Ok(DaemonLogRead {
        file: target.name,
        total_lines_returned: tail.len(),
        lines: tail,
        truncated,
    })
}
