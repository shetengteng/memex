//! 会话 markdown 文件的 frontmatter 解析。
//!
//! 归一化目录里每个 md 文件头部带 YAML frontmatter：
//!
//! ```yaml
//! ---
//! session_id: <id>
//! source: <adapter>
//! project: <abs path or null>
//! created: <rfc3339>
//! ---
//! ```
//!
//! 这里只做轻量提取（前 50 行 + `key: value` split），不引入完整 YAML 解析依赖。
//! frontmatter 缺失时由调用方按文件名 / 父目录名兜底（见 [`session_id_from_filename`]
//! / [`adapter_from_path`]）。

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Default)]
pub(super) struct Frontmatter {
    pub session_id: Option<String>,
    pub source: Option<String>,
    pub project: Option<String>,
}

/// 解析文件头的 YAML frontmatter。
///
/// 任何 IO 错误或格式异常都返回 `Frontmatter::default()` —— 这里的 frontmatter
/// 只是元信息加速通道，缺失时调用方走文件名兜底，不应该让 raw 工具因此报错。
pub(super) fn parse_frontmatter(path: &Path) -> Frontmatter {
    let mut fm = Frontmatter::default();
    let Ok(file) = File::open(path) else {
        return fm;
    };
    let reader = BufReader::new(file);
    let mut in_block = false;
    let mut seen_first_dash = false;
    for line in reader.lines().take(50).map_while(|l| l.ok()) {
        let trimmed = line.trim();
        if trimmed == "---" {
            if !seen_first_dash {
                seen_first_dash = true;
                in_block = true;
                continue;
            }
            break;
        }
        if !in_block {
            // 第一行不是 `---`，没有 frontmatter
            break;
        }
        let Some((k, v)) = trimmed.split_once(':') else {
            continue;
        };
        let key = k.trim();
        let value = v.trim().trim_matches(|c| c == '"' || c == '\'');
        if value.is_empty() || value == "null" {
            continue;
        }
        match key {
            "session_id" => fm.session_id = Some(value.to_string()),
            "source" => fm.source = Some(value.to_string()),
            "project" => fm.project = Some(value.to_string()),
            _ => {}
        }
    }
    fm
}

/// 从文件路径推断 session_id：取文件名（去掉 `.md` 后缀）。
pub(super) fn session_id_from_filename(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// 从文件路径推断 adapter：取父目录名。
pub(super) fn adapter_from_path(path: &Path) -> Option<String> {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// 生成 `memex://session/<id>` deep link。
pub(super) fn deep_link(session_id: &str) -> String {
    format!("memex://session/{}", session_id)
}
