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

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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

/// MCP 用的 daemon 客户端。一次 `run_stdio` 调用持有一份，下面所有 tool
/// 都通过这个 client 跟 daemon 通信。
///
/// 跟顶层 [`crate::client::MemexClient`] 一样，`endpoint` 用 `Mutex` 包装支持
/// "请求失败 → 重读 lock → 切端口 → 重试一次"，覆盖 daemon 重启 + 端口 fallback
/// 的瞬时窗口。MCP stdio 进程通常长跑（IDE 会话期间一直在），daemon 重启端口
/// 跳到 10001 时若没有这个重试，整段 IDE 会话就丢了 daemon 连接。
#[derive(Debug)]
pub struct McpClient {
    endpoint: Mutex<Endpoint>,
    agent: ureq::Agent,
    memex_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct Endpoint {
    base_url: String,
    port: u16,
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

        Ok(Self {
            endpoint: Mutex::new(Endpoint {
                base_url,
                port: info.port,
            }),
            agent,
            memex_dir: memex_dir.to_path_buf(),
        })
    }

    fn snapshot_base_url(&self) -> String {
        self.endpoint
            .lock()
            .expect("endpoint mutex poisoned")
            .base_url
            .clone()
    }

    /// 重读 `daemon.lock`；端口变了就更新内部 endpoint 并返回 true，否则 false。
    /// 跟 [`crate::client::MemexClient::try_pick_up_new_port`] 同语义。
    fn try_pick_up_new_port(&self) -> bool {
        let info = match read_lock(&self.memex_dir) {
            Some(i) => i,
            None => return false,
        };
        if !is_process_alive(info.pid) {
            return false;
        }
        let mut ep = self.endpoint.lock().expect("endpoint mutex poisoned");
        if info.port == ep.port {
            return false;
        }
        ep.base_url = format!("http://127.0.0.1:{}", info.port);
        ep.port = info.port;
        true
    }

    /// 不带 query string 的 GET。失败若是 transport 错误，会重读 lock + 重试一次。
    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let mut attempts: u8 = 0;
        loop {
            let result = self.do_get(path, &[]);
            if result.is_ok() || attempts >= TRANSPORT_RETRY_MAX {
                return result;
            }
            let err = result.as_ref().err().unwrap();
            if !looks_like_transport_error(err) || !self.try_pick_up_new_port() {
                return result;
            }
            attempts += 1;
        }
    }

    /// GET 带 query string。query 不会再次 URL-encode value（caller 自己负责）。
    pub fn get_with_query<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let mut attempts: u8 = 0;
        loop {
            let result = self.do_get(path, query);
            if result.is_ok() || attempts >= TRANSPORT_RETRY_MAX {
                return result;
            }
            let err = result.as_ref().err().unwrap();
            if !looks_like_transport_error(err) || !self.try_pick_up_new_port() {
                return result;
            }
            attempts += 1;
        }
    }

    fn do_get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T> {
        let url = format!("{}{}", self.snapshot_base_url(), path);
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

    /// POST + JSON body。失败时同样支持一次端口跳变重试。
    pub fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        let value = serde_json::to_value(body)
            .with_context(|| format!("serialize POST {} body failed", path))?;
        let mut attempts: u8 = 0;
        loop {
            let result = self.do_post(path, &value);
            if result.is_ok() || attempts >= TRANSPORT_RETRY_MAX {
                return result;
            }
            let err = result.as_ref().err().unwrap();
            if !looks_like_transport_error(err) || !self.try_pick_up_new_port() {
                return result;
            }
            attempts += 1;
        }
    }

    fn do_post<T: DeserializeOwned>(&self, path: &str, body: &serde_json::Value) -> Result<T> {
        let url = format!("{}{}", self.snapshot_base_url(), path);
        self.agent
            .post(&url)
            .send_json(body.clone())
            .with_context(|| format!("HTTP POST {} failed", path))?
            .body_mut()
            .read_json::<T>()
            .with_context(|| format!("HTTP POST {} parse json failed", path))
    }
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

    /// 端口跳变：lock 文件写新端口时，`try_pick_up_new_port` 应返回 true
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

        // 跳过 connect() 的 /health 探活，直接手工构造 client。
        let client = McpClient {
            endpoint: Mutex::new(Endpoint {
                base_url: "http://127.0.0.1:9999".into(),
                port: 9999,
            }),
            agent: ureq::Agent::config_builder().build().into(),
            memex_dir: tmp.path().to_path_buf(),
        };

        assert!(!client.try_pick_up_new_port(), "no change -> false");
        assert_eq!(client.endpoint.lock().unwrap().port, 9999);

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
        assert!(client.try_pick_up_new_port(), "port change -> true");
        assert_eq!(client.endpoint.lock().unwrap().port, 10001);
        assert_eq!(
            client.endpoint.lock().unwrap().base_url,
            "http://127.0.0.1:10001"
        );
    }
}
