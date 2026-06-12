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

use std::path::{Path, PathBuf};
use std::sync::Mutex;
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

/// 单次 HTTP 失败后允许"重读 lock + 切端口"的最大重试次数。设成 1 已经足够：
/// daemon 重启一次后，端口在 `9999..=10009` 范围内固定下来，CLI 第二次调用直接
/// 用新端口；第三次还失败说明 daemon 真死，再 retry 没意义。
const TRANSPORT_RETRY_MAX: u8 = 1;

/// HTTP 客户端入口。一次 CLI 命令只构造一份，连接池由内部 `Agent` 持有。
///
/// 持有两个独立的 `Agent`：
/// * `agent` —— 默认短 timeout（30s），覆盖 search / sessions / stats 这类快查询。
/// * `long_agent` —— 长 timeout（10 min），覆盖 ingest / rebuild-index 这类扫全库
///   的写操作。两个 agent 不共享连接池，但 CLI 一次命令最多触发其中一个，不会有
///   socket 浪费。
///
/// `endpoint` 用 `Mutex` 包是为了支持「请求失败 → 重读 daemon.lock → 切端口 →
/// 重试一次」的恢复路径。CLI 本质单线程，Mutex 没有竞争开销，只是把 `&self`
/// 下的可变性补回来。
#[derive(Debug)]
pub struct MemexClient {
    endpoint: Mutex<Endpoint>,
    agent: ureq::Agent,
    long_agent: ureq::Agent,
    memex_dir: PathBuf,
    pub pid: u32,
    pub started_at: String,
}

#[derive(Debug, Clone)]
struct Endpoint {
    base_url: String,
    port: u16,
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
            endpoint: Mutex::new(Endpoint {
                base_url,
                port: info.port,
            }),
            agent,
            long_agent,
            memex_dir: memex_dir.to_path_buf(),
            pid: info.pid,
            started_at: info.started_at,
        })
    }

    /// 生产路径：自动从 `memex_core::memex_dir()` 取 ~/.memex/。
    pub fn connect() -> Result<Self> {
        Self::connect_with_dir(&memex_core::memex_dir())
    }

    /// 当前监听端口（可能在请求中途被「重读 lock」更新）。
    pub fn port(&self) -> u16 {
        self.endpoint.lock().expect("endpoint mutex poisoned").port
    }

    fn snapshot_endpoint(&self) -> Endpoint {
        self.endpoint
            .lock()
            .expect("endpoint mutex poisoned")
            .clone()
    }

    /// 重读 `daemon.lock`，端口变化时更新内部 `endpoint`。
    ///
    /// 返回值：
    /// * `true`  —— 端口确实变了，caller 可以重试一次请求
    /// * `false` —— 端口没变 / 主进程已死 / lock 不可解析，retry 没意义
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
        tracing::info!(
            old_port = ep.port,
            new_port = info.port,
            "Memex daemon port changed mid-flight, switching"
        );
        ep.base_url = format!("http://127.0.0.1:{}", info.port);
        ep.port = info.port;
        true
    }

    /// GET 接口；返回反序列化好的 `T`。
    /// 首次失败若像是 transport 错误就重读 lock，端口变了重试一次。
    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let mut attempts: u8 = 0;
        loop {
            let result = self.do_get(path);
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

    fn do_get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.snapshot_endpoint().base_url, path);
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
        self.post_retrying(&self.agent, path, body)
    }

    /// 长操作专用 POST，10 分钟 timeout。
    ///
    /// 当前调用方：`memex-cli ingest`。后续 rebuild-index 切 RPC 后也走这个。
    pub fn post_long<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.post_retrying(&self.long_agent, path, body)
    }

    /// 跟 `get` 一样的"once-retry on transport error"包装。
    fn post_retrying<B: Serialize, T: DeserializeOwned>(
        &self,
        agent: &ureq::Agent,
        path: &str,
        body: &B,
    ) -> Result<T> {
        // body 需要序列化两次（首次 + retry），用 Value 做中间表示比 reborrow B 简单。
        let value = serde_json::to_value(body)
            .with_context(|| format!("serialize POST {} body failed", path))?;
        let mut attempts: u8 = 0;
        loop {
            let result = self.do_post(agent, path, &value);
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

    fn do_post<T: DeserializeOwned>(
        &self,
        agent: &ureq::Agent,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let url = format!("{}{}", self.snapshot_endpoint().base_url, path);
        agent
            .post(&url)
            .send_json(body.clone())
            .with_context(|| format!("HTTP POST {} failed", path))?
            .body_mut()
            .read_json::<T>()
            .with_context(|| format!("HTTP POST {} parse json failed", path))
    }
}

/// 启发式判断一个 anyhow::Error 是不是「连不上 daemon」类的 transport error。
///
/// 之所以走字符串匹配而不是 ureq 的 Error variant：
/// 1. `with_context` 之后底层 error 已经被 wrap 进 anyhow::Error 的 chain，
///    直接 `downcast_ref::<ureq::Error>()` 在跨 ureq 版本时易碎
/// 2. ureq 3.x 的 Error variants 还在演化（HostNotFound / ConnectionFailed /
///    Io(_) 等），版本升级时枚举名容易漂
/// 3. CLI 这边的判定只是"要不要重试一次"，假阳性最多多浪费一次 lock 读取，
///    没有副作用 —— 字符串匹配宽松一点反而更稳。
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

    /// transport-error 检测：connection refused / reset / broken pipe 都应识别，
    /// HTTP 200 风格的"parse json failed"不应误判。
    #[test]
    fn transport_error_detection_matches_io_keywords() {
        for needle in [
            "connection refused",
            "Connection Reset by peer",
            "BROKEN PIPE",
            "io: connection aborted",
        ] {
            let err = anyhow!("HTTP GET /stats failed").context(needle.to_string());
            assert!(
                looks_like_transport_error(&err),
                "should detect transport error in: {}",
                needle
            );
        }
        // 反例：5xx / parse 错误 不是 transport，不能 retry
        let parse_err = anyhow!("parse json failed: expected `,` at line 1");
        assert!(!looks_like_transport_error(&parse_err));
        let http500 = anyhow!("HTTP/1.1 500 Internal Server Error");
        assert!(!looks_like_transport_error(&http500));
    }

    /// `try_pick_up_new_port`：lock 文件端口变了应返回 true 并更新内部 base_url；
    /// 端口没变应返回 false（避免无效重试）。
    #[test]
    fn pick_up_new_port_returns_true_only_when_port_changes() {
        let tmp = TempDir::new().unwrap();
        let my_pid = std::process::id();
        // 第一次连接：lock 写自己的 pid + 9999。connect 会做 /health 探活，
        // 这里走不通就让 connect 提前失败 —— 但我们只想测 pick_up_new_port 这段逻辑，
        // 直接手工构造 MemexClient 跳过 connect 网络部分。
        write_lock(tmp.path(), my_pid, 9999);
        let client = MemexClient {
            endpoint: Mutex::new(Endpoint {
                base_url: "http://127.0.0.1:9999".into(),
                port: 9999,
            }),
            agent: ureq::Agent::config_builder().build().into(),
            long_agent: ureq::Agent::config_builder().build().into(),
            memex_dir: tmp.path().to_path_buf(),
            pid: my_pid,
            started_at: "test".into(),
        };

        // 端口没变 → 不应该切。
        assert!(!client.try_pick_up_new_port(), "no change should yield false");
        assert_eq!(client.port(), 9999);

        // 端口跳到 10001（模拟 daemon 重启 + fallback）→ 应该切，并更新 base_url。
        write_lock(tmp.path(), my_pid, 10001);
        assert!(client.try_pick_up_new_port(), "port change should yield true");
        assert_eq!(client.port(), 10001);
        assert_eq!(
            client.snapshot_endpoint().base_url,
            "http://127.0.0.1:10001"
        );

        // pid 死了 → 不应切（即使 lock 端口变了），避免在 daemon 真死时假性重试。
        write_lock(tmp.path(), 999_999, 10002);
        assert!(
            !client.try_pick_up_new_port(),
            "dead pid should block port switch"
        );
        assert_eq!(client.port(), 10001, "endpoint stays at last live port");
    }
}
