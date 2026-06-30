//! `raw_grep` —— FTS5 兜底的内容搜索。

use std::fs::File;
use std::io::{BufRead, BufReader};

use anyhow::{Result, anyhow, bail};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use super::filter::{for_each_session, parse_filter};
use super::frontmatter::{adapter_from_path, deep_link, session_id_from_filename};

const MAX_GREP_LIMIT: usize = 200;
const MAX_CONTEXT_LINES: usize = 10;

/// `raw_grep` 入参。
///
/// 设计上是 FTS5 路径的兜底：当 `search_memory` 返回 0 条或明显不相关时，由
/// LLM 显式切换到 raw_grep。所有路径限定在 [`super::sandbox::sessions_root`]
/// 之下。
#[derive(Debug, Deserialize, Default)]
pub struct RawGrepRequest {
    /// 必填查询串。`regex=false`（默认）时按字面量匹配；`regex=true` 时按
    /// regex 编译。
    pub query: String,
    /// 是否把 `query` 作为正则。
    #[serde(default)]
    pub regex: bool,
    /// 是否大小写敏感（默认 false）。
    #[serde(default)]
    pub case_sensitive: bool,
    /// 限定 adapter（`claude_code` / `codex` / `cursor` / `continue` / `opencode`）。
    pub adapter: Option<String>,
    /// 项目路径子串过滤（case-insensitive）。
    pub project: Option<String>,
    /// 文件 mtime >= 这个 ISO 日期（`YYYY-MM-DD` 或 RFC3339）。
    pub after: Option<String>,
    /// 文件 mtime <= 这个 ISO 日期。
    pub before: Option<String>,
    /// 命中行前后保留多少行上下文，默认 2，最大 10。
    #[serde(default = "default_context")]
    pub context: usize,
    /// 命中数上限，默认 20，最大 200。
    #[serde(default = "default_grep_limit")]
    pub limit: usize,
    /// 每个文件只回一条（rg -l 等价），用于"哪些 session 提到过 X"这种问题。
    #[serde(default)]
    pub files_only: bool,
}

fn default_context() -> usize {
    2
}
fn default_grep_limit() -> usize {
    20
}

/// 一次 `raw_grep` 命中。
#[derive(Debug, Serialize)]
pub struct RawGrepHit {
    pub session_id: String,
    pub adapter: String,
    pub project: Option<String>,
    pub file: String,
    /// 命中行号（1-based）；`files_only=true` 时为 0。
    pub line: usize,
    pub snippet: String,
    pub deep_link: String,
}

/// `raw_grep` 响应。
#[derive(Debug, Serialize)]
pub struct RawGrepResponse {
    pub hits: Vec<RawGrepHit>,
    /// 命中数被 `limit` / 内部 cap 截断时为 true。
    pub truncated: bool,
    pub elapsed_ms: u64,
}

/// 在沙箱内做内容搜索。
///
/// # Errors
///
/// - `query` 为空
/// - `regex=true` 且 `query` 编译失败（返回 `invalid_regex: ...`）
/// - `after` / `before` 不是合法 ISO 日期
pub fn raw_grep(req: RawGrepRequest) -> Result<RawGrepResponse> {
    let started = std::time::Instant::now();
    if req.query.is_empty() {
        bail!("query is required");
    }
    let limit = req.limit.clamp(1, MAX_GREP_LIMIT);
    let context = req.context.min(MAX_CONTEXT_LINES);

    let regex = build_query_regex(&req.query, req.regex, req.case_sensitive)?;
    let filter = parse_filter(&req.adapter, &req.project, &req.after, &req.before)?;

    let mut hits: Vec<RawGrepHit> = Vec::new();
    let mut truncated = false;
    let internal_cap = limit.saturating_mul(5).max(limit);

    for_each_session(&filter, |path, fm, _meta| {
        let session_id = fm
            .session_id
            .clone()
            .or_else(|| session_id_from_filename(path))
            .unwrap_or_default();
        let adapter = fm
            .source
            .clone()
            .or_else(|| adapter_from_path(path))
            .unwrap_or_default();

        let Ok(file) = File::open(path) else {
            return true;
        };
        let lines: Vec<String> = BufReader::new(file)
            .lines()
            .map_while(|l| l.ok())
            .collect();

        let mut matched_in_file = false;
        for (idx, line) in lines.iter().enumerate() {
            if !regex.is_match(line) {
                continue;
            }
            matched_in_file = true;
            if req.files_only {
                break;
            }
            let from = idx.saturating_sub(context);
            let to = (idx + context + 1).min(lines.len());
            let snippet = lines[from..to].join("\n");
            hits.push(RawGrepHit {
                session_id: session_id.clone(),
                adapter: adapter.clone(),
                project: fm.project.clone(),
                file: path.to_string_lossy().into_owned(),
                line: idx + 1,
                snippet,
                deep_link: deep_link(&session_id),
            });
            if hits.len() >= internal_cap {
                truncated = true;
                return false;
            }
        }

        if req.files_only && matched_in_file {
            hits.push(RawGrepHit {
                session_id: session_id.clone(),
                adapter,
                project: fm.project.clone(),
                file: path.to_string_lossy().into_owned(),
                line: 0,
                snippet: String::new(),
                deep_link: deep_link(&session_id),
            });
            if hits.len() >= internal_cap {
                truncated = true;
                return false;
            }
        }
        true
    })?;

    if hits.len() > limit {
        hits.truncate(limit);
        truncated = true;
    }

    Ok(RawGrepResponse {
        hits,
        truncated,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

fn build_query_regex(query: &str, is_regex: bool, case_sensitive: bool) -> Result<Regex> {
    let pat = if is_regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    RegexBuilder::new(&pat)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| anyhow!("invalid_regex: {}", e))
}
