//! 给 MCP server 用的 daemon HTTP client，附带直连 Db 的 fallback。
//!
//! ## 连接模式
//!
//! `McpClient` 内部有两种模式，对 `server/` 和 `tools.rs` 完全透明：
//!
//! - **Http**：daemon 正在运行时走 HTTP RPC，和 5c 以来的行为一致。
//! - **Db**（Phase 5 fallback）：daemon 没跑时直连 SQLite，把 daemon 路由
//!   在本地重放，返回相同的 JSON 形态。MCP log 退化为 no-op。
//!
//! `McpClient::connect()` 失败时调用 `McpClient::connect_or_fallback()` 即
//! 可得到一个 Db 模式的 client，`run()` 入口负责选择。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use memex_core::context::{ContextOptions, build_context, search_by_project};
use memex_core::storage::db::Db;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// MCP 启动时 `/health` 探活的超时。
///
/// 关键：Claude Code 的 `startup_timeout_sec` 默认 30s，如果 daemon HTTP 线程
/// 被 ingest（parking_lot mutex 长事务）阻塞，用主 agent 的 30s 全局超时会
/// 让整个 MCP init 超时失败，用户看到 "MCP client for `memex` timed out"。
///
/// 探活只是判断 daemon 是否可达，3s 足够；超时立即返回一个明确错误让 IDE
/// 上层清晰 surface（"restart Memex.app"），而不是黑洞式挂 30s。真正的 tool
/// 调用仍走 `REQUEST_TIMEOUT` 30s 的主 agent。
const HEALTH_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// HTTP 失败时允许"重读 lock + 切端口 + 重试"的最大次数。跟 [`crate::client`]
/// 保持一致，1 次足够覆盖 daemon 重启 + 端口 fallback 的常见情况。
const TRANSPORT_RETRY_MAX: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockInfo {
    pid: u32,
    port: u16,
    #[allow(dead_code)]
    started_at: String,
}

/// MCP 用的 daemon 客户端，支持 Http / Db 两种模式（对 tools.rs 透明）。
pub struct McpClient {
    mode: Mode,
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.mode {
            Mode::Http { .. } => write!(f, "McpClient(Http)"),
            Mode::Db(_) => write!(f, "McpClient(Db)"),
        }
    }
}

enum Mode {
    /// 走 daemon HTTP RPC（正常情况）。
    Http {
        endpoint: Mutex<Endpoint>,
        agent: ureq::Agent,
        memex_dir: PathBuf,
    },
    /// daemon 不可达时直连 SQLite（Phase 5 fallback）。
    Db(Db),
}

#[derive(Debug, Clone)]
struct Endpoint {
    base_url: String,
    port: u16,
}

impl McpClient {
    /// 标准入口：优先连 daemon HTTP；daemon 不可达时 fallback 到直连 SQLite。
    pub fn connect() -> Result<Self> {
        Self::connect_with_dir(&memex_core::memex_dir())
    }

    /// 显式 memex_dir 入口，便于单测用 tempdir。
    pub fn connect_with_dir(memex_dir: &Path) -> Result<Self> {
        match Self::try_http(memex_dir) {
            Ok(client) => Ok(client),
            Err(_) => Self::open_db_fallback(memex_dir),
        }
    }

    /// 纯 HTTP 连接，不 fallback（单测用）。
    #[cfg(test)]
    pub fn connect_http_only(memex_dir: &Path) -> Result<Self> {
        Self::try_http(memex_dir)
    }

    fn try_http(memex_dir: &Path) -> Result<Self> {
        let info = read_lock(memex_dir).ok_or_else(|| {
            anyhow!(
                "Memex daemon not running (no lock at {})",
                memex_dir.join("daemon.lock").display()
            )
        })?;

        if !is_process_alive(info.pid) {
            let _ = std::fs::remove_file(memex_dir.join("daemon.lock"));
            return Err(anyhow!("Memex daemon lock points to dead pid {}", info.pid));
        }

        let base_url = format!("http://127.0.0.1:{}", info.port);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .into();

        let probe_agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(HEALTH_PROBE_TIMEOUT))
            .build()
            .into();
        probe_agent
            .get(&format!("{}/health", base_url))
            .call()
            .map_err(|e| anyhow!("daemon HTTP not reachable: {}", e))?;

        Ok(Self {
            mode: Mode::Http {
                endpoint: Mutex::new(Endpoint {
                    base_url,
                    port: info.port,
                }),
                agent,
                memex_dir: memex_dir.to_path_buf(),
            },
        })
    }

    fn open_db_fallback(memex_dir: &Path) -> Result<Self> {
        let db_path = memex_dir.join("memex.db");
        let db = Db::open(&db_path)
            .with_context(|| format!("Memex daemon not running and cannot open db at {}", db_path.display()))?;
        Ok(Self { mode: Mode::Db(db) })
    }


    // ── Http-mode helpers ───────────────────────────────────────────────────

    fn http_snapshot_base_url(endpoint: &Mutex<Endpoint>) -> String {
        endpoint.lock().expect("endpoint mutex poisoned").base_url.clone()
    }

    fn http_try_pick_up_new_port(endpoint: &Mutex<Endpoint>, memex_dir: &Path) -> bool {
        let info = match read_lock(memex_dir) {
            Some(i) => i,
            None => return false,
        };
        if !is_process_alive(info.pid) {
            return false;
        }
        let mut ep = endpoint.lock().expect("endpoint mutex poisoned");
        if info.port == ep.port {
            return false;
        }
        ep.base_url = format!("http://127.0.0.1:{}", info.port);
        ep.port = info.port;
        true
    }

    fn http_do_get<T: DeserializeOwned>(
        endpoint: &Mutex<Endpoint>,
        agent: &ureq::Agent,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let url = format!("{}{}", Self::http_snapshot_base_url(endpoint), path);
        let mut req = agent.get(&url);
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        req.call()
            .with_context(|| format!("HTTP GET {} failed", path))?
            .body_mut()
            .read_json::<T>()
            .with_context(|| format!("HTTP GET {} parse json failed", path))
    }

    fn http_do_post<T: DeserializeOwned>(
        endpoint: &Mutex<Endpoint>,
        agent: &ureq::Agent,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let url = format!("{}{}", Self::http_snapshot_base_url(endpoint), path);
        agent
            .post(&url)
            .send_json(body.clone())
            .with_context(|| format!("HTTP POST {} failed", path))?
            .body_mut()
            .read_json::<T>()
            .with_context(|| format!("HTTP POST {} parse json failed", path))
    }

    // ── Public API（对 tools.rs 透明）───────────────────────────────────────

    /// 不带 query string 的 GET。
    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.get_with_query(path, &[])
    }

    /// GET 带 query string。
    pub fn get_with_query<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        match &self.mode {
            Mode::Http { endpoint, agent, memex_dir } => {
                let mut attempts: u8 = 0;
                loop {
                    let result = Self::http_do_get(endpoint, agent, path, query);
                    if result.is_ok() || attempts >= TRANSPORT_RETRY_MAX {
                        return result;
                    }
                    if !result.as_ref().err().is_some_and(looks_like_transport_error)
                        || !Self::http_try_pick_up_new_port(endpoint, memex_dir)
                    {
                        return result;
                    }
                    attempts += 1;
                }
            }
            Mode::Db(db) => {
                let value = db_dispatch(db, path, query)?;
                serde_json::from_value(value).context("Db fallback: deserialize failed")
            }
        }
    }

    /// POST + JSON body。
    pub fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        match &self.mode {
            Mode::Http { endpoint, agent, memex_dir } => {
                let value = serde_json::to_value(body)
                    .with_context(|| format!("serialize POST {} body failed", path))?;
                let mut attempts: u8 = 0;
                loop {
                    let result = Self::http_do_post(endpoint, agent, path, &value);
                    if result.is_ok() || attempts >= TRANSPORT_RETRY_MAX {
                        return result;
                    }
                    if !result.as_ref().err().is_some_and(looks_like_transport_error)
                        || !Self::http_try_pick_up_new_port(endpoint, memex_dir)
                    {
                        return result;
                    }
                    attempts += 1;
                }
            }
            Mode::Db(_) => {
                // Db 模式下 /mcp/log 是 no-op；其他 POST 路径目前不存在。
                serde_json::from_value(serde_json::json!({}))
                    .context("Db fallback: empty POST response")
            }
        }
    }
}

// ── Db dispatch（把 daemon 路由在本地重放）──────────────────────────────────

/// 把 `get_with_query(path, query)` 路由到对应的 Db 操作，返回与 daemon
/// HTTP 响应**相同 JSON 形态**的 `Value`，使 tools.rs 不需要任何改动。
fn db_dispatch(db: &Db, path: &str, query: &[(&str, &str)]) -> Result<serde_json::Value> {
    fn q<'a>(query: &[(&'a str, &'a str)], key: &str) -> Option<&'a str> {
        query.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    if path == "/search" {
        let raw_q = q(query, "q").unwrap_or("");
        let limit: usize = q(query, "limit").and_then(|v| v.parse().ok()).unwrap_or(10);
        let adapter_filter = q(query, "adapter");
        let project_filter = q(query, "project");

        let mut results = db.fts_search(raw_q, limit * 2)?;
        if let Some(a) = adapter_filter {
            results.retain(|r| r.adapter.as_deref() == Some(a));
        }
        if let Some(p) = project_filter {
            results.retain(|r| r.project.as_deref() == Some(p));
        }
        results.truncate(limit);
        return Ok(serde_json::json!({ "results": results }));
    }

    if path == "/sessions" {
        let limit: usize = q(query, "limit").and_then(|v| v.parse().ok()).unwrap_or(10);
        let sessions = db.list_sessions(limit)?;
        return Ok(serde_json::json!({ "sessions": sessions }));
    }

    if path == "/stats" {
        let sessions = db.session_count()?;
        let messages = db.message_count()?;
        let chunks = db.chunk_count()?;
        return Ok(serde_json::json!({ "sessions": sessions, "messages": messages, "chunks": chunks }));
    }

    if path == "/context" {
        let top: usize = q(query, "top").and_then(|v| v.parse().ok()).unwrap_or(3);
        // project 优先；没有则用 cwd 匹配
        let project_path = if let Some(p) = q(query, "project") {
            p.to_string()
        } else {
            let cwd_str = q(query, "cwd").unwrap_or(".");
            let cwd = std::path::Path::new(cwd_str);
            match search_by_project(db, cwd)? {
                Some(m) => m.project_path,
                None => {
                    return Ok(serde_json::json!({ "markdown": "Memex 当前目录暂无关联会话记忆。" }));
                }
            }
        };
        let md = build_context(db, &project_path, &ContextOptions { top_n: top, redact: false })?;
        return Ok(serde_json::json!({ "markdown": md }));
    }

    if path == "/sessions/range" {
        let after = q(query, "after").unwrap_or("");
        let before = q(query, "before").unwrap_or("");
        let limit: usize = q(query, "limit").and_then(|v| v.parse().ok()).unwrap_or(100);
        let project_filter = q(query, "project");

        let mut sessions = db.list_sessions_in_range(after, before)?;
        if let Some(p) = project_filter {
            sessions.retain(|s| s.project_path.as_deref() == Some(p));
        }
        sessions.truncate(limit);
        // tools.rs 里 range 路径期望 snake_case（见 tool_list_sessions_by_range 注释）
        let snake: Vec<serde_json::Value> = sessions
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "source": s.source,
                    "project_path": s.project_path,
                    "title": s.title,
                    "message_count": s.message_count,
                    "created_at": s.created_at,
                    "updated_at": s.updated_at,
                    "summary_title": s.summary_title,
                    "first_user_message": s.first_user_message,
                    "intent": s.intent,
                    "is_private": s.is_private,
                })
            })
            .collect();
        let total = snake.len();
        return Ok(serde_json::json!({
            "range": { "after": after, "before": before },
            "total": total,
            "sessions": snake,
        }));
    }

    // /sessions/<id>
    if let Some(sid) = path.strip_prefix("/sessions/") {
        // 前缀匹配：sid 可能是完整 id 也可能是前缀
        let resolved = if sid.len() < 36 {
            let all = db.list_sessions(200)?;
            all.into_iter()
                .find(|s| s.id.starts_with(sid))
                .map(|s| s.id)
                .unwrap_or_else(|| sid.to_string())
        } else {
            sid.to_string()
        };
        let detail = db
            .get_session_detail(&resolved)?
            .ok_or_else(|| anyhow!("session not found: {}", sid))?;
        return Ok(serde_json::to_value(&detail)?);
    }

    Err(anyhow!("Db fallback: unknown path {}", path))
}

/// 同 [`crate::client::looks_like_transport_error`]，独立一份避免跨模块依赖。
fn looks_like_transport_error(err: &anyhow::Error) -> bool {
    let msg = format!("{:#}", err).to_lowercase();
    const NEEDLES: &[&str] = &[
        "connection refused",
        "connection reset",
        "connection aborted",
        "broken pipe",
        "not connected",
        "host unreachable",
        "network unreachable",
        "connect failed",
    ];
    NEEDLES.iter().any(|n| msg.contains(n))
}

fn read_lock(memex_dir: &Path) -> Option<LockInfo> {
    let path = memex_dir.join("daemon.lock");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn connect_falls_back_to_db_when_no_lock() {
        let tmp = TempDir::new().unwrap();
        // no lock file → HTTP fails → fallback to Db (rusqlite creates the file)
        let client = McpClient::connect_with_dir(tmp.path()).expect("should fall back to Db");
        assert!(
            format!("{:?}", client).contains("Db"),
            "expected Db mode, got {:?}",
            client
        );
        // HTTP-only path still fails with the expected message
        let http_err = McpClient::connect_http_only(tmp.path()).unwrap_err();
        assert!(
            format!("{}", http_err).contains("Memex daemon not running"),
            "msg={}", http_err
        );
    }

    #[test]
    fn connect_http_only_fails_when_no_lock() {
        let tmp = TempDir::new().unwrap();
        let err = McpClient::connect_http_only(tmp.path()).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Memex daemon not running"), "msg={}", msg);
    }

    #[test]
    fn connect_http_only_clears_stale_lock() {
        let tmp = TempDir::new().unwrap();
        let info = LockInfo {
            pid: 999_999,
            port: 9999,
            started_at: "2026-06-11T00:00:00+00:00".into(),
        };
        let lock = tmp.path().join("daemon.lock");
        std::fs::write(&lock, serde_json::to_string(&info).unwrap()).unwrap();
        let err = McpClient::connect_http_only(tmp.path()).unwrap_err();
        assert!(
            format!("{}", err).contains("dead pid 999999"),
            "err={:?}",
            err
        );
        assert!(!lock.exists(), "stale lock should be removed");
    }

    /// transport-error 启发式：connection refused / reset 应被识别；
    /// "parse json" / "500 Internal" 不该误判（不重试）。
    #[test]
    fn transport_error_detection_matches_io_keywords() {
        for needle in [
            "connection refused",
            "Connection Reset by peer",
            "BROKEN PIPE",
        ] {
            let err = anyhow!("HTTP GET /stats failed").context(needle.to_string());
            assert!(
                looks_like_transport_error(&err),
                "should detect transport error in: {}",
                needle
            );
        }
        let parse_err = anyhow!("parse json failed");
        assert!(!looks_like_transport_error(&parse_err));
    }

    /// 端口跳变：lock 文件写新端口时，`http_try_pick_up_new_port` 应返回 true
    /// 并把内部 endpoint 切到新端口；端口没变或 pid 已死应返回 false。
    #[test]
    fn pick_up_new_port_updates_endpoint_when_port_changes() {
        let tmp = TempDir::new().unwrap();
        let my_pid = std::process::id();
        let info = LockInfo {
            pid: my_pid,
            port: 9999,
            started_at: "test".into(),
        };
        std::fs::write(
            tmp.path().join("daemon.lock"),
            serde_json::to_string(&info).unwrap(),
        )
        .unwrap();

        let endpoint = Mutex::new(Endpoint {
            base_url: "http://127.0.0.1:9999".into(),
            port: 9999,
        });

        assert!(
            !McpClient::http_try_pick_up_new_port(&endpoint, tmp.path()),
            "no change -> false"
        );
        assert_eq!(endpoint.lock().unwrap().port, 9999);

        let info2 = LockInfo {
            pid: my_pid,
            port: 10001,
            started_at: "test".into(),
        };
        std::fs::write(
            tmp.path().join("daemon.lock"),
            serde_json::to_string(&info2).unwrap(),
        )
        .unwrap();
        assert!(
            McpClient::http_try_pick_up_new_port(&endpoint, tmp.path()),
            "port change -> true"
        );
        assert_eq!(endpoint.lock().unwrap().port, 10001);
        assert_eq!(
            endpoint.lock().unwrap().base_url,
            "http://127.0.0.1:10001"
        );
    }
}
