//! 会话发现：扫一遍 `~/.codex/session_index.jsonl`，把每一条
//! 解析到 `~/.codex/sessions/` 下按日期前缀组织的 rollout JSONL 文件。

use std::fs;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub(super) struct IndexEntry {
    pub id: Option<String>,
    pub updated_at: Option<String>,
    pub thread_name: Option<String>,
}

pub(super) trait DateAccessors {
    fn year(&self) -> i32;
    fn month(&self) -> u32;
    fn day(&self) -> u32;
}

impl DateAccessors for chrono::DateTime<chrono::FixedOffset> {
    fn year(&self) -> i32 {
        chrono::Datelike::year(self)
    }
    fn month(&self) -> u32 {
        chrono::Datelike::month(self)
    }
    fn day(&self) -> u32 {
        chrono::Datelike::day(self)
    }
}

pub(super) fn find_session_file(
    sessions_root: &Path,
    session_id: &str,
    updated_at: Option<&str>,
) -> Option<PathBuf> {
    if !sessions_root.exists() {
        return None;
    }

    if let Some(dt) =
        updated_at.and_then(|ts| DateTime::parse_from_rfc3339(&ts.replace('Z', "+00:00")).ok())
    {
        let date_dir = sessions_root
            .join(format!("{:04}", dt.year()))
            .join(format!("{:02}", dt.month()))
            .join(format!("{:02}", dt.day()));
        if let Some(p) = scan_dir_for_session(&date_dir, session_id) {
            return Some(p);
        }
    }

    for entry in walkdir::WalkDir::new(sessions_root)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file()
            && path.extension().is_some_and(|ext| ext == "jsonl")
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().contains(session_id))
        {
            return Some(path.to_path_buf());
        }
    }
    debug!("codex: session file missing for {}", session_id);
    None
}

fn scan_dir_for_session(dir: &Path, session_id: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let p = entry.path();
        if p.is_file()
            && p.extension().is_some_and(|ext| ext == "jsonl")
            && p.file_name()
                .is_some_and(|n| n.to_string_lossy().contains(session_id))
        {
            Some(p)
        } else {
            None
        }
    })
}
