//! `raw_find` —— 按文件名 / mtime / 大小定位 session 文件。

use anyhow::{Result, anyhow};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use super::filter::{for_each_session, mtime_of, parse_filter};
use super::frontmatter::{adapter_from_path, deep_link, session_id_from_filename};

const MAX_FIND_LIMIT: usize = 500;

/// `raw_find` 入参。
#[derive(Debug, Deserialize, Default)]
pub struct RawFindRequest {
    /// 文件名匹配。glob（`*` / `?`）或 regex —— 视 `regex` 字段。anchor 到完整文件名。
    pub name_pattern: Option<String>,
    /// 是否把 `name_pattern` 作为 regex。
    #[serde(default)]
    pub regex: bool,
    pub adapter: Option<String>,
    /// 项目路径子串过滤（case-insensitive）。
    pub project: Option<String>,
    /// 文件 mtime >= 这个 ISO 日期。
    pub after: Option<String>,
    /// 文件 mtime <= 这个 ISO 日期。
    pub before: Option<String>,
    #[serde(default)]
    pub min_size_kb: u64,
    pub max_size_kb: Option<u64>,
    /// 返回数上限，默认 50，最大 500。
    #[serde(default = "default_find_limit")]
    pub limit: usize,
}

fn default_find_limit() -> usize {
    50
}

/// 一条 `raw_find` 文件记录。
#[derive(Debug, Serialize)]
pub struct RawFindFile {
    pub path: String,
    pub adapter: String,
    pub session_id: String,
    pub project: Option<String>,
    pub size_bytes: u64,
    pub mtime: Option<String>,
    pub deep_link: String,
}

/// `raw_find` 响应。按 mtime 倒序排列。
#[derive(Debug, Serialize)]
pub struct RawFindResponse {
    pub files: Vec<RawFindFile>,
    pub truncated: bool,
    pub elapsed_ms: u64,
}

/// 按条件列出沙箱内的 session 文件。
///
/// # Errors
///
/// - `name_pattern` 非法（regex 编译失败或 glob 转 regex 后非法）
/// - `after` / `before` 不是合法 ISO 日期
pub fn raw_find(req: RawFindRequest) -> Result<RawFindResponse> {
    let started = std::time::Instant::now();
    let limit = req.limit.clamp(1, MAX_FIND_LIMIT);
    let name_re = match req.name_pattern.as_deref() {
        Some(p) if !p.is_empty() => Some(build_name_matcher(p, req.regex)?),
        _ => None,
    };
    let filter = parse_filter(&req.adapter, &req.project, &req.after, &req.before)?;

    let mut files: Vec<RawFindFile> = Vec::new();
    let mut truncated = false;

    for_each_session(&filter, |path, fm, meta| {
        if let Some(ref re) = name_re {
            let fname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if !re.is_match(fname) {
                return true;
            }
        }
        let size = meta.len();
        if size < req.min_size_kb.saturating_mul(1024) {
            return true;
        }
        if let Some(max_kb) = req.max_size_kb
            && size > max_kb.saturating_mul(1024)
        {
            return true;
        }
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
        let mtime = mtime_of(path).map(|t| t.to_rfc3339());
        files.push(RawFindFile {
            path: path.to_string_lossy().into_owned(),
            adapter,
            session_id: session_id.clone(),
            project: fm.project.clone(),
            size_bytes: size,
            mtime,
            deep_link: deep_link(&session_id),
        });
        if files.len() >= limit.saturating_mul(2).max(limit) {
            truncated = true;
            return false;
        }
        true
    })?;

    files.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    if files.len() > limit {
        files.truncate(limit);
        truncated = true;
    }

    Ok(RawFindResponse {
        files,
        truncated,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

/// 把 glob（`*` / `?`）转成 regex；`is_regex=true` 时直接当 regex 用。
/// regex 始终大小写不敏感，并 anchor 到完整文件名。
fn build_name_matcher(pattern: &str, is_regex: bool) -> Result<Regex> {
    let pat = if is_regex {
        pattern.to_string()
    } else {
        let mut out = String::from("^");
        for c in pattern.chars() {
            match c {
                '*' => out.push_str(".*"),
                '?' => out.push('.'),
                '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                    out.push('\\');
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        out.push('$');
        out
    };
    RegexBuilder::new(&pat)
        .case_insensitive(true)
        .build()
        .map_err(|e| anyhow!("invalid name_pattern: {}", e))
}
