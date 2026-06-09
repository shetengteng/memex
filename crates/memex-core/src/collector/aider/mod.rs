//! Aider adapter —— 解析 `~/<dir>/.aider.chat.history.md` 这类多会话 markdown
//! 历史文件。
//!
//! 拆分：
//! - `parser` —— `split_sessions` / `parse_session_messages` 的纯函数
//! - `tests`  —— 集成 fixture 测试
//!
//! 本文件只负责 adapter 配置（扫描目录）+ `Adapter` trait 实现。

mod parser;
#[cfg(test)]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::debug;

use crate::collector::Adapter;
use crate::storage::models::{RawMessage, SessionMeta};
use parser::{parse_session_messages, split_sessions};

pub struct AiderAdapter {
    scan_dirs: Vec<PathBuf>,
}

const MAX_SCAN_DEPTH: usize = 4;
const HISTORY_FILENAME: &str = ".aider.chat.history.md";

impl Default for AiderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AiderAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("cannot determine home directory");
        let scan_dirs: Vec<PathBuf> = [
            "Documents",
            "Projects",
            "projects",
            "code",
            "dev",
            "repos",
            "work",
            "src",
        ]
        .iter()
        .map(|d| home.join(d))
        .filter(|p| p.exists())
        .collect();
        Self { scan_dirs }
    }

    #[cfg(test)]
    pub fn with_scan_dirs(scan_dirs: Vec<PathBuf>) -> Self {
        Self { scan_dirs }
    }

    fn discover_history_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for dir in &self.scan_dirs {
            if !dir.exists() {
                continue;
            }
            for entry in walkdir::WalkDir::new(dir)
                .max_depth(MAX_SCAN_DEPTH)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.file_name().is_some_and(|n| n == HISTORY_FILENAME) {
                    files.push(path.to_path_buf());
                }
            }
        }
        files.sort();
        files.dedup();
        Ok(files)
    }

    fn project_path_from_file(path: &Path) -> Option<String> {
        path.parent().map(|p| p.to_string_lossy().to_string())
    }
}

impl Adapter for AiderAdapter {
    fn name(&self) -> &str {
        "aider"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let files = self.discover_history_files()?;
        let mut sessions = Vec::new();

        for file_path in files {
            let meta = match fs::metadata(&file_path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let content = match fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    debug!("aider: failed to read {}: {}", file_path.display(), e);
                    continue;
                }
            };

            let sess_blocks = split_sessions(&content);
            let project = Self::project_path_from_file(&file_path);

            for (i, (ts, _block)) in sess_blocks.iter().enumerate() {
                let hash_hex = blake3::hash(file_path.to_string_lossy().as_bytes()).to_hex();
                let sid = format!("aider-{}-{}", &hash_hex[..12], i);
                sessions.push(SessionMeta {
                    id: sid,
                    source: "aider".to_string(),
                    project_path: project.clone(),
                    file_path: file_path.to_string_lossy().to_string(),
                    last_offset: i as u64,
                    mtime: mtime + i as u64,
                    created_secs: 0,
                    title: None,
                });
                let _ = ts;
            }
        }

        Ok(sessions)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        let path = Path::new(&session.file_path);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)?;
        let sess_blocks = split_sessions(&content);
        let idx = session.last_offset as usize;

        if idx >= sess_blocks.len() {
            return Ok(Vec::new());
        }

        let (_ts, block) = &sess_blocks[idx];
        Ok(parse_session_messages(&session.id, block))
    }
}
