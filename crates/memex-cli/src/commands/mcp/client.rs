//! 给 MCP server 用的 daemon HTTP client。
//!
//! 跟 memex-cli 顶层的 `client.rs`（`crate::client::MemexClient`）是同族
//! 实现，但 mcp 这里需求更窄：
//! * 只有 GET（带 query string）和 POST（json body）两种调用。
//! * 不需要 long timeout —— MCP tool 调用都是 < 5s 的查询，30s 默认 timeout
//!   足够。真要扫全库的话用户会通过 menubar 直接跑，不会从 IDE MCP 触发。
//! * 不抛 user-facing 文案 —— mcp 是 stdio JSON-RPC，错误会被 wrap 进 tool
//!   response 的 `isError`。caller 决定怎么 surface。
//!
//! Phase 7 起 mcp 已经下沉到 memex-cli 内部，但 `McpClient` 跟 `MemexClient`
//! 仍然保持独立两份：前者面向 IDE 端 stdio JSON-RPC 的错误语义，后者面向
//! CLI 用户的 user-facing 文案。两份实现 ≈ 80 行重复但语义不同，没有强行
//! 抽公共层的诉求。

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockInfo {
    pid: u32,
    port: u16,
    #[allow(dead_code)]
    started_at: String,
}

/// MCP 用的 daemon 客户端。一次 `run_stdio` 调用持有一份，下面所有 tool
/// 都通过这个 client 跟 daemon 通信。
#[derive(Debug)]
pub struct McpClient {
    base_url: String,
    agent: ureq::Agent,
}

impl McpClient {
    /// 标准入口：从 `memex_core::memex_dir()` 找 daemon.lock，做三连探活。
    pub fn connect() -> Result<Self> {
        Self::connect_with_dir(&memex_core::memex_dir())
    }

    /// 显式 memex_dir 入口，便于单测用 tempdir。
    pub fn connect_with_dir(memex_dir: &Path) -> Result<Self> {
        let info = read_lock(memex_dir).ok_or_else(|| {
            anyhow!(
                "Memex daemon not running (no lock at {}); start Memex.app first.",
                memex_dir.join("daemon.lock").display()
            )
        })?;

        if !is_process_alive(info.pid) {
            let _ = std::fs::remove_file(memex_dir.join("daemon.lock"));
            return Err(anyhow!(
                "Memex daemon lock points to dead pid {}; start Memex.app first.",
                info.pid
            ));
        }

        let base_url = format!("http://127.0.0.1:{}", info.port);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .into();

        let health_url = format!("{}/health", base_url);
        agent.get(&health_url).call().map_err(|e| {
            anyhow!(
                "Memex daemon HTTP not reachable (port {}, error: {}); restart Memex.app.",
                info.port,
                e
            )
        })?;

        Ok(Self { base_url, agent })
    }

    /// 不带 query string 的 GET。
    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        self.agent
            .get(&url)
            .call()
            .with_context(|| format!("HTTP GET {} failed", path))?
            .body_mut()
            .read_json::<T>()
            .with_context(|| format!("HTTP GET {} parse json failed", path))
    }

    /// GET 带 query string。query 不会再次 URL-encode value（caller 自己负责）。
    pub fn get_with_query<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.agent.get(&url);
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        req.call()
            .with_context(|| format!("HTTP GET {} failed", path))?
            .body_mut()
            .read_json::<T>()
            .with_context(|| format!("HTTP GET {} parse json failed", path))
    }

    /// POST + JSON body。
    pub fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let value = serde_json::to_value(body)
            .with_context(|| format!("serialize POST {} body failed", path))?;
        self.agent
            .post(&url)
            .send_json(value)
            .with_context(|| format!("HTTP POST {} failed", path))?
            .body_mut()
            .read_json::<T>()
            .with_context(|| format!("HTTP POST {} parse json failed", path))
    }
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
    fn connect_fails_when_no_lock() {
        let tmp = TempDir::new().unwrap();
        let err = McpClient::connect_with_dir(tmp.path()).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Memex daemon not running"), "msg={}", msg);
    }

    #[test]
    fn connect_clears_stale_lock() {
        let tmp = TempDir::new().unwrap();
        let info = LockInfo {
            pid: 999_999,
            port: 9999,
            started_at: "2026-06-11T00:00:00+00:00".into(),
        };
        let lock = tmp.path().join("daemon.lock");
        std::fs::write(&lock, serde_json::to_string(&info).unwrap()).unwrap();
        let err = McpClient::connect_with_dir(tmp.path()).unwrap_err();
        assert!(
            format!("{}", err).contains("dead pid 999999"),
            "err={:?}",
            err
        );
        assert!(!lock.exists(), "stale lock should be removed");
    }
}
