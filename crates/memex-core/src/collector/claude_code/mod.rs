mod parser;

use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{debug, warn};

use super::Adapter;
use crate::storage::models::{RawMessage, SessionMeta};
use parser::{ClaudeMessage, convert_claude_message};

/// 只用来扫前几行 JSONL 抓 cwd，不需要解析整个消息体。
#[derive(Debug, Deserialize)]
struct CwdProbe {
    cwd: Option<String>,
}

pub struct ClaudeCodeAdapter {
    base_dir: PathBuf,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".claude")
            .join("projects");
        Self { base_dir }
    }

    #[cfg(test)]
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn discover_session_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if !self.base_dir.exists() {
            return Ok(files);
        }
        for entry in walkdir::WalkDir::new(&self.base_dir)
            .min_depth(1)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jsonl") {
                files.push(path.to_path_buf());
            }
        }
        Ok(files)
    }

    /// 解析会话所在的真实工作目录绝对路径。
    /// 优先级：
    /// 1. JSONL 前若干行里的 `cwd` 字段（Claude Code 最权威的来源）。
    /// 2. 文件父目录在 `~/.claude/projects/` 下的 dash-encoded 名（如
    ///    `-Users-Foo-Documents-bar` → `/Users/Foo/Documents/bar`）。
    ///    对 subagent 文件（`<project>/<uuid>/subagents/<file>.jsonl`），
    ///    向上回退到 `<project>` 那一层。
    fn extract_project_path(&self, file_path: &Path) -> Option<String> {
        if let Some(cwd) = probe_cwd_in_jsonl(file_path) {
            return Some(cwd);
        }

        let rel = file_path
            .parent()
            .and_then(|p| p.strip_prefix(&self.base_dir).ok())?;
        // subagent: `<encoded-project>/<uuid>/subagents/` → 回退到 `<encoded-project>`。
        let first_component = rel.components().next()?;
        let encoded = first_component.as_os_str().to_string_lossy().to_string();
        Some(dash_decode_to_absolute(&encoded))
    }

    fn session_id_from_path(path: &Path) -> String {
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                blake3::hash(path.to_string_lossy().as_bytes())
                    .to_hex()
                    .to_string()
            })
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// 扫 JSONL 前 8 行找 `cwd` 字段。多数 Claude Code 文件第 1 行就有；
/// 边界场景（首行是 leafUuid summary，没有 cwd）顺势再多读几行，
/// 命中即返回。不命中返回 None，让调用方走 dash-decode fallback。
fn probe_cwd_in_jsonl(file_path: &Path) -> Option<String> {
    let file = fs::File::open(file_path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines().take(8).map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(probe) = serde_json::from_str::<CwdProbe>(trimmed)
            && let Some(cwd) = probe.cwd.filter(|s| !s.is_empty())
        {
            return Some(cwd);
        }
    }
    None
}

/// 把 Claude Code 的目录命名 `-Users-Foo-Documents-bar`（绝对路径
/// 里的 `/` 替换为 `-` 得到）还原成 `/Users/Foo/Documents/bar`。
/// Claude Code 自己就是按这个规则把 cwd 折叠成目录名的，
/// 反向操作大部分情况下能还原；遇到原路径段里就带 `-` 的就有歧义
/// （无法在这层无损还原），此时返回的字符串看起来像绝对路径，
/// 但与真实 cwd 可能不完全一致。优先用 `probe_cwd_in_jsonl`。
fn dash_decode_to_absolute(encoded: &str) -> String {
    if let Some(body) = encoded.strip_prefix('-') {
        format!("/{}", body.replace('-', "/"))
    } else {
        // 不带前导 `-` 的（理论上 Claude Code 不会这样存），原样返回。
        encoded.to_string()
    }
}

impl Adapter for ClaudeCodeAdapter {
    fn name(&self) -> &str {
        "claude_code"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let files = self.discover_session_files()?;
        let mut sessions = Vec::new();

        for file_path in files {
            let meta = fs::metadata(&file_path)?;
            let mtime = meta
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let created_secs = meta
                .created()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let session_id = Self::session_id_from_path(&file_path);
            let project_path = self.extract_project_path(&file_path);

            sessions.push(SessionMeta {
                id: session_id,
                source: "claude_code".to_string(),
                project_path,
                file_path: file_path.to_string_lossy().to_string(),
                last_offset: 0,
                mtime,
                created_secs,
                title: None,
            });
        }

        Ok(sessions)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        let path = Path::new(&session.file_path);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(path)
            .with_context(|| format!("failed to open {}", session.file_path))?;
        let file_size = file.metadata()?.len();

        if file_size <= session.last_offset {
            return Ok(Vec::new());
        }

        let mut reader = BufReader::new(file);
        if session.last_offset > 0 {
            reader.seek(SeekFrom::Start(session.last_offset))?;
        }

        let mut messages = Vec::new();
        let mut current_offset = session.last_offset;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    warn!("failed to read line at offset {}: {}", current_offset, e);
                    break;
                }
            };
            current_offset += line.len() as u64 + 1;

            if line.trim().is_empty() {
                continue;
            }

            let parsed: ClaudeMessage = match serde_json::from_str(&line) {
                Ok(m) => m,
                Err(e) => {
                    debug!("skipping malformed JSON line: {}", e);
                    continue;
                }
            };

            if let Some(raw_msg) = convert_claude_message(&parsed, &session.id, current_offset) {
                messages.push(raw_msg);
            }
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::Role;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_fixture(dir: &Path, filename: &str, content: &str) -> PathBuf {
        let path = dir.join(filename);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_parse_normal_jsonl() {
        let tmp = TempDir::new().unwrap();
        let jsonl = r#"{"type":"human","uuid":"m1","role":"human","message":{"role":"human","content":"hello"},"timestamp":"2026-05-01T10:00:00Z"}
{"type":"assistant","uuid":"m2","role":"assistant","message":{"role":"assistant","content":"hi there"},"timestamp":"2026-05-01T10:00:01Z"}
"#;
        let file_path = write_fixture(tmp.path(), "project/session1.jsonl", jsonl);
        let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
        let session = SessionMeta {
            id: "session1".into(),
            source: "claude_code".into(),
            project_path: Some("project".into()),
            file_path: file_path.to_string_lossy().to_string(),
            last_offset: 0,
            mtime: 0,
            created_secs: 0,
            title: None,
        };
        let messages = adapter.collect(&session).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].role, Role::Assistant);
    }

    #[test]
    fn test_scan_discovers_files() {
        let tmp = TempDir::new().unwrap();
        write_fixture(tmp.path(), "proj_a/session1.jsonl", "{}");
        write_fixture(tmp.path(), "proj_b/session2.jsonl", "{}");
        let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_empty_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = write_fixture(tmp.path(), "proj/empty.jsonl", "");
        let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
        let session = SessionMeta {
            id: "empty".into(),
            source: "claude_code".into(),
            project_path: None,
            file_path: file_path.to_string_lossy().to_string(),
            last_offset: 0,
            mtime: 0,
            created_secs: 0,
            title: None,
        };
        let messages = adapter.collect(&session).unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_extract_project_path_prefers_cwd_in_jsonl() {
        let tmp = TempDir::new().unwrap();
        let jsonl = r#"{"type":"user","cwd":"/Users/foo/work/my-proj","message":{"role":"human","content":"hi"}}
"#;
        let file_path = write_fixture(tmp.path(), "-Users-foo-work-my--proj/session1.jsonl", jsonl);
        let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
        let got = adapter.extract_project_path(&file_path);
        assert_eq!(got.as_deref(), Some("/Users/foo/work/my-proj"));
    }

    #[test]
    fn test_extract_project_path_falls_back_to_dash_decode() {
        let tmp = TempDir::new().unwrap();
        // 没有 cwd 字段的损坏行：必须退到 dash-decode。
        let file_path = write_fixture(
            tmp.path(),
            "-Users-foo-Documents-bar/session1.jsonl",
            "{}\n",
        );
        let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
        let got = adapter.extract_project_path(&file_path);
        assert_eq!(got.as_deref(), Some("/Users/foo/Documents/bar"));
    }

    #[test]
    fn test_extract_project_path_for_subagent_returns_parent_project() {
        let tmp = TempDir::new().unwrap();
        // subagent 文件路径形如 `<encoded-proj>/<uuid>/subagents/agent-x.jsonl`
        // 它的首行 cwd 通常已经是父项目的绝对路径。
        let jsonl = r#"{"type":"assistant","cwd":"/Users/foo/Documents/bar","message":{"role":"assistant","content":"ok"}}
"#;
        let file_path = write_fixture(
            tmp.path(),
            "-Users-foo-Documents-bar/abc-uuid/subagents/agent-x.jsonl",
            jsonl,
        );
        let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
        let got = adapter.extract_project_path(&file_path);
        assert_eq!(got.as_deref(), Some("/Users/foo/Documents/bar"));
    }

    #[test]
    fn test_extract_project_path_subagent_dash_decode_fallback() {
        let tmp = TempDir::new().unwrap();
        // subagent 文件没 cwd 时：fallback 必须落到 `<encoded-proj>` 那一层，
        // 而不是 `subagents` / `<uuid>` / `agent-x`。
        let file_path = write_fixture(
            tmp.path(),
            "-Users-foo-Documents-bar/abc-uuid/subagents/agent-y.jsonl",
            "{}\n",
        );
        let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
        let got = adapter.extract_project_path(&file_path);
        assert_eq!(got.as_deref(), Some("/Users/foo/Documents/bar"));
    }
}
