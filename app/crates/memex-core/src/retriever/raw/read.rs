//! `raw_read` —— 按行号区间读取 session markdown 片段。

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::num::NonZeroUsize;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::filter::for_each_session;
use super::frontmatter::session_id_from_filename;
use super::sandbox::ensure_inside_sandbox;

const MAX_READ_RANGE: usize = 500;

/// `raw_read` 入参。`session_id` 与 `file` 二选一（前者优先）。
///
/// `start_line` / `end_line` 用 [`NonZeroUsize`]，从类型层面排除 0 这种非法状态
/// （Make illegal states unrepresentable）。serde 反序列化时 0 会直接报错。
#[derive(Debug, Deserialize)]
pub struct RawReadRequest {
    pub session_id: Option<String>,
    pub file: Option<String>,
    pub start_line: NonZeroUsize,
    pub end_line: NonZeroUsize,
}

/// 单行读取结果。
#[derive(Debug, Serialize)]
pub struct RawReadLine {
    pub line: usize,
    pub content: String,
}

/// `raw_read` 响应。
#[derive(Debug, Serialize)]
pub struct RawReadResponse {
    pub file: String,
    pub session_id: String,
    pub lines: Vec<RawReadLine>,
    /// 文件实际行数小于 `end_line`，或读取过程出错截断时为 true。
    pub truncated: bool,
}

/// 按行号区间读取一份 session markdown 片段。
///
/// # Errors
///
/// - `end_line < start_line`
/// - 区间跨度超过 500 行（`range_too_large`）
/// - 既没传 `session_id` 也没传 `file`
/// - `session_id` 在沙箱里找不到对应文件
/// - `file` 路径不在沙箱内（`path_outside_sandbox`）
pub fn raw_read(req: RawReadRequest) -> Result<RawReadResponse> {
    let start = req.start_line.get();
    let end = req.end_line.get();
    if end < start {
        bail!("invalid line range: start={}, end={}", start, end);
    }
    let span = end - start + 1;
    if span > MAX_READ_RANGE {
        bail!("range_too_large: {} lines (max {})", span, MAX_READ_RANGE);
    }

    let path = resolve_read_target(&req)?;
    let canonical = ensure_inside_sandbox(&path)?;
    let fm = super::frontmatter::parse_frontmatter(&canonical);
    let session_id = fm
        .session_id
        .clone()
        .or_else(|| session_id_from_filename(&canonical))
        .unwrap_or_default();

    let file = File::open(&canonical)
        .with_context(|| format!("failed to open: {}", canonical.display()))?;
    let mut lines = Vec::new();
    let mut truncated = false;
    let mut current = 0usize;
    for line in BufReader::new(file).lines() {
        current += 1;
        if current < start {
            continue;
        }
        if current > end {
            break;
        }
        match line {
            Ok(content) => lines.push(RawReadLine {
                line: current,
                content,
            }),
            Err(_) => {
                truncated = true;
                break;
            }
        }
    }
    if current < end {
        truncated = true;
    }

    Ok(RawReadResponse {
        file: canonical.to_string_lossy().into_owned(),
        session_id,
        lines,
        truncated,
    })
}

fn resolve_read_target(req: &RawReadRequest) -> Result<PathBuf> {
    if let Some(ref sid) = req.session_id
        && !sid.is_empty()
    {
        let mut found: Option<PathBuf> = None;
        for_each_session(
            &super::filter::ScanFilter {
                adapter: None,
                project: None,
                after: None,
                before: None,
            },
            |path, fm, _meta| {
                let id = fm
                    .session_id
                    .clone()
                    .or_else(|| session_id_from_filename(path))
                    .unwrap_or_default();
                if id == *sid || id.starts_with(sid) {
                    found = Some(path.to_path_buf());
                    return false;
                }
                true
            },
        )?;
        return found.ok_or_else(|| anyhow!("session not found: {}", sid));
    }
    if let Some(ref f) = req.file
        && !f.is_empty()
    {
        return Ok(PathBuf::from(f));
    }
    bail!("either session_id or file is required");
}
