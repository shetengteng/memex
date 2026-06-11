//! memex-cli 的 HTTP 客户端 —— Phase 5 起 CLI 不再直连 db / 配置，全部走 daemon
//! 暴露的 HTTP 接口（127.0.0.1:<port>）。
//!
//! 设计契约：
//! * [`MemexClient::connect`] 是"硬"探活：读 `~/.memex/daemon.lock` →
//!   `kill -0 <pid>` 判活 → HTTP `GET /health` 200。任何一步失败都直接
//!   返回带 user-facing 文案的 `anyhow::Error`，**不退化到本地 db**。
//! * 主进程未跑的提示统一是
//!   `"Memex 服务未启动，请打开 Memex.app（菜单栏 M 图标）后重试"`，
//!   方便用户一眼定位修复路径。
//! * 所有 HTTP 调用走同一个 `ureq::Agent`，复用连接池。

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// 单次 RPC 的 hard timeout（短操作 default）。
///
/// search / sessions / stats 这类查询通常 < 1s，30 秒能覆盖到冷启动 fts 索引
/// 加载的 worst case。过短会让 release 模式 CLI 看上去 flaky。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// 长操作专用 timeout（ingest / rebuild-index 等扫全库的命令）。
///
/// cursor 全量首扫 + claude_code 全量首扫合计可能 60-120s（个例机器上看到过
/// 180s）。给 10 分钟硬上限，既能拦死死锁的 daemon，又不会在合理负载下误杀。
const LONG_REQUEST_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub pid: u32,
    pub port: u16,
    pub started_at: String,
}

/// HTTP 客户端入口。一次 CLI 命令只构造一份，连接池由内部 `Agent` 持有。
///
/// 持有两个独立的 `Agent`：
/// * `agent` —— 默认短 timeout（30s），覆盖 search / sessions / stats 这类快查询。
/// * `long_agent` —— 长 timeout（10 min），覆盖 ingest / rebuild-index 这类扫全库
///   的写操作。两个 agent 不共享连接池，但 CLI 一次命令最多触发其中一个，不会有
///   socket 浪费。
#[derive(Debug)]
pub struct MemexClient {
    base_url: String,
    agent: ureq::Agent,
    long_agent: ureq::Agent,
    pub pid: u32,
    pub port: u16,
    pub started_at: String,
}

impl MemexClient {
    /// 三步联通性检查；任意一步失败都直接 bail user-facing 错误。
    /// `memex_dir` 显式传入是为了让单测可以指向 tempdir。
    pub fn connect_with_dir(memex_dir: &Path) -> Result<Self> {
        let info = read_lock(memex_dir).ok_or_else(|| {
            anyhow!(
                "Memex 服务未启动（找不到 {}），请打开 Memex.app（菜单栏 M 图标）后重试。",
                memex_dir.join("daemon.lock").display()
            )
        })?;

        if !is_process_alive(info.pid) {
            // 清理过期 lock，避免下一次还误判
            let _ = std::fs::remove_file(memex_dir.join("daemon.lock"));
            return Err(anyhow!(
                "Memex 服务未启动（lock 指向已退出的 PID {}），请打开 Memex.app（菜单栏 M 图标）后重试。",
                info.pid
            ));
        }

        let base_url = format!("http://127.0.0.1:{}", info.port);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .into();
        let long_agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(LONG_REQUEST_TIMEOUT))
            .build()
            .into();

        // /health 探活：拿不到 200 说明 daemon 进程还在但 HTTP server 未就绪，
        // 例如刚启动还没 bind 完。提示用户重试比 silent 卡住更友好。
        let health_url = format!("{}/health", base_url);
        agent.get(&health_url).call().map_err(|e| {
            anyhow!(
                "Memex 服务运行中但 HTTP 不可达（port {}, error: {}），请重启 Memex.app 后重试。",
                info.port,
                e
            )
        })?;

        Ok(Self {
            base_url,
            agent,
            long_agent,
            pid: info.pid,
            port: info.port,
            started_at: info.started_at,
        })
    }

    /// 生产路径：自动从 `memex_core::memex_dir()` 取 ~/.memex/。
    pub fn connect() -> Result<Self> {
        Self::connect_with_dir(&memex_core::memex_dir())
    }

    /// GET 接口；返回反序列化好的 `T`。
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

    /// GET 接口的 generic 弱类型版本，返回 `serde_json::Value`，方便在
    /// CLI 上做不强类型的"按 key 取字段"。
    pub fn get_value(&self, path: &str) -> Result<serde_json::Value> {
        self.get::<serde_json::Value>(path)
    }

    /// POST 接口；request body 自动 JSON 序列化，response 反序列化为 `T`。
    ///
    /// 默认用 30s timeout。需要扫全库 / 跑 LLM 的命令请改用 [`Self::post_long`]。
    #[allow(dead_code)]
    pub fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        self.post_with(&self.agent, path, body)
    }

    /// 长操作专用 POST，10 分钟 timeout。
    ///
    /// 当前调用方：`memex ingest`。后续 rebuild-index 切 RPC 后也走这个。
    pub fn post_long<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.post_with(&self.long_agent, path, body)
    }

    fn post_with<B: Serialize, T: DeserializeOwned>(
        &self,
        agent: &ureq::Agent,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let value = serde_json::to_value(body)
            .with_context(|| format!("serialize POST {} body failed", path))?;
        agent
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
    // kill -0 在进程存在且我们能给它发信号时返回 0；CLI 跟主进程同 UID，权限不会失败。
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_lock(dir: &Path, pid: u32, port: u16) {
        let info = LockInfo {
            pid,
            port,
            started_at: "2026-06-11T00:00:00+00:00".into(),
        };
        std::fs::write(dir.join("daemon.lock"), serde_json::to_string(&info).unwrap()).unwrap();
    }

    /// lock 不存在 → 应给出"打开 Memex.app"的错误，且 message 包含 lock 路径。
    #[test]
    fn connect_fails_when_no_lock() {
        let tmp = TempDir::new().unwrap();
        let err = MemexClient::connect_with_dir(tmp.path()).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Memex 服务未启动"), "msg={}", msg);
        assert!(msg.contains("Memex.app"), "msg={}", msg);
    }

    /// lock 指向已死 PID（用大值 999_999 保证不存在）→ 应清掉过期 lock 并报错。
    #[test]
    fn connect_clears_stale_lock_and_errors() {
        let tmp = TempDir::new().unwrap();
        let lock = tmp.path().join("daemon.lock");
        write_lock(tmp.path(), 999_999, 9999);
        assert!(lock.exists(), "precondition: lock written");

        let err = MemexClient::connect_with_dir(tmp.path()).unwrap_err();
        assert!(
            format!("{}", err).contains("Memex 服务未启动"),
            "err={:?}",
            err
        );
        assert!(!lock.exists(), "stale lock should be deleted after connect");
    }

    /// lock 解析失败（文件存在但内容不是 JSON）→ 也应当作 "not running" 处理。
    #[test]
    fn connect_treats_unparsable_lock_as_no_lock() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("daemon.lock"), "not-json").unwrap();
        let err = MemexClient::connect_with_dir(tmp.path()).unwrap_err();
        assert!(
            format!("{}", err).contains("Memex 服务未启动"),
            "err={:?}",
            err
        );
    }
}
