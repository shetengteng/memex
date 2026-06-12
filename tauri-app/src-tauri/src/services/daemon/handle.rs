//! In-process daemon host —— Tauri 主进程持有的 daemon 句柄 / 状态。
//!
//! Phase 2 起 daemon 跑在 Tauri 主进程的 tokio runtime 上（`tauri::async_runtime::spawn`），
//! 跟主进程共享 PID / runtime / db handle。Phase 6 起 daemon 的源代码物理上
//! 也下沉到 `crate::services::daemon::*`，本文件只负责 lifecycle 管理（spawn /
//! shutdown / restart），HTTP server 实现见 [`super::server`]。
//!
//! 关键差异 vs 早期 standalone binary 模式：
//! * **lock 文件**：仍写 `~/.memex/daemon.lock`，但 `pid` 字段是**主进程 PID**，
//!   memex-cli 顶层 client + mcp 子模块 client 通过这把 lock 找到 daemon HTTP 端口。
//! * **signal handler**：不装。`shutdown` 由 Tauri 的 `ExitRequested` 调
//!   [`DaemonState::shutdown_blocking`] 触发。
//! * **db handle**：daemon 自己 open。Phase 7 可以再下沉到 Tauri State，跟前端
//!   commands 共享。
//!
//! ## State 模型
//!
//! `DaemonState` 是 `tauri::State` 持有的可变容器（`Mutex<Option<DaemonHandle>>`）。
//! 这样 setup() 阶段可以**先** `app.manage(DaemonState::new())`，然后异步 spawn
//! 任务里再 `state.install(handle).await`。前端 `daemon_restart` IPC 也能通过
//! State 替换内部 handle，而不必 `unmanage + manage` 折腾。

use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use memex_core::config::ensure_memex_dir;
use memex_core::storage::db::Db;
use socket2::{Domain, Socket, Type};
use tokio::sync::Mutex;
use tokio::sync::oneshot;

use super::server::PREFERRED_PORT;

/// `bind_listener` 端口 fallback 探测的最大尝试次数（含首选端口本身）。
/// 9999 被占用时会依次尝试 10000..=10010，10 个备选端口对个人桌面应用足够。
const PORT_FALLBACK_MAX: u16 = 10;

/// 一份 in-process daemon 任务的所有运行时句柄。
///
/// 不暴露公共字段：所有访问都走 [`DaemonState`] 的方法，避免外部不一致地
/// 引用其中某一项（比如只调 shutdown_tx 没 await join 就完事）。
pub struct DaemonHandle {
    port: u16,
    started_at: String,
    pid: u32,
    shutdown_tx: Option<oneshot::Sender<()>>,
    join: Option<tauri::async_runtime::JoinHandle<Result<()>>>,
}

impl DaemonHandle {
    /// 触发 graceful shutdown，await daemon 任务真正退出，然后清 lock。
    async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.join.take() {
            match handle.await {
                Ok(Ok(())) => tracing::info!("in-process daemon stopped cleanly"),
                Ok(Err(e)) => tracing::warn!("in-process daemon returned error: {e}"),
                Err(join_err) => tracing::warn!("in-process daemon join failed: {join_err}"),
            }
        }
        super::lockfile::remove_lock(&memex_core::memex_dir());
    }
}

/// 简版可序列化快照，给 IPC 层用（commands/daemon.rs 的 DaemonStatus 也是同 shape）。
#[derive(Debug, Clone)]
pub struct DaemonSnapshot {
    pub pid: u32,
    pub port: u16,
    pub started_at: String,
}

/// 主进程注册到 `tauri::State` 的可变容器。所有对 in-process daemon 句柄的
/// 访问都通过它，从而保证 install / shutdown / restart 之间不会出现状态交错。
pub struct DaemonState {
    inner: Mutex<Option<DaemonHandle>>,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub async fn snapshot(&self) -> Option<DaemonSnapshot> {
        self.inner.lock().await.as_ref().map(|h| DaemonSnapshot {
            pid: h.pid,
            port: h.port,
            started_at: h.started_at.clone(),
        })
    }

    pub async fn install(&self, handle: DaemonHandle) {
        // 如果已经有旧 handle，先 shutdown，避免端口冲突 / 双重 task。
        // 正常 setup 路径下不会有旧 handle，这是防御式 install。
        if let Some(old) = self.inner.lock().await.take() {
            old.shutdown().await;
        }
        *self.inner.lock().await = Some(handle);
    }

    pub async fn shutdown(&self) {
        if let Some(h) = self.inner.lock().await.take() {
            h.shutdown().await;
        }
    }

    /// 阻塞包装，给 `RunEvent::ExitRequested` 同步钩子用。
    /// Tauri 的 RunEvent 回调不在 tokio context 里，所以用 block_on 桥接。
    pub fn shutdown_blocking(&self) {
        tauri::async_runtime::block_on(self.shutdown());
    }

    /// 用于 daemon_restart IPC：停掉当前 in-process daemon，spawn 新的并 install。
    ///
    /// 端口走 `spawn_in_process` 内部的 fallback 探测（首选 9999，被占就尝试 10000+）。
    /// 这样旧 listener 还在 OS 缓冲里、或者外部进程抢了 9999，都能自动恢复。
    pub async fn restart(&self) -> Result<DaemonSnapshot> {
        self.shutdown().await;
        let handle = spawn_in_process().await?;
        let snapshot = DaemonSnapshot {
            pid: handle.pid,
            port: handle.port,
            started_at: handle.started_at.clone(),
        };
        *self.inner.lock().await = Some(handle);
        Ok(snapshot)
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

/// 启动 in-process daemon，返回 [`DaemonHandle`]。
///
/// 端口策略：先尝试 [`PREFERRED_PORT`]（9999），被外部进程占或自身上次 listener
/// 还没完全释放时，依次 fallback 到下 [`PORT_FALLBACK_MAX`] 个端口。`SO_REUSEADDR`
/// 已经开启，正常重启不会触发 fallback。实际监听端口写到 `daemon.lock`，
/// memex-cli / mcp bridge 都通过读 lock 找端口，所以动态端口对它们透明。
///
/// 调用方负责把返回的 handle 通过 `DaemonState::install` 注册到 Tauri State。
///
/// 失败原因（已知）：
/// * `~/.memex` 目录创建失败（磁盘满 / 权限）
/// * `memex.db` open 失败（schema migration 报错 / 文件损坏）
/// * 端口段全被占用（极小概率，但能给前端清晰错误消息）
/// * lock 写入失败（极小概率，磁盘只读）
///
/// 注意：本函数不检查"是否已有 daemon 在跑"。in-process 模式下整个 app 只起
/// 一份 daemon（由 Tauri setup 钩子调一次），不需要外部去重；`write_lock` 直接
/// 覆盖旧 lock 文件。
pub async fn spawn_in_process() -> Result<DaemonHandle> {
    let memex_dir = memex_core::memex_dir();
    ensure_memex_dir(&memex_dir).context("ensure_memex_dir failed")?;

    let db_path = memex_dir.join("memex.db");
    let db = Arc::new(Db::open(&db_path).context("Db::open failed")?);

    let listener = bind_listener(PREFERRED_PORT)?;
    let port = listener.local_addr().context("listener missing local_addr")?.port();

    super::lockfile::write_lock(&memex_dir, port).context("write daemon.lock failed")?;
    let started_at = chrono::Utc::now().to_rfc3339();
    let pid = std::process::id();
    tracing::info!(
        pid = pid,
        port = port,
        "in-process daemon lock written (pid is Memex main process)",
    );

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let shutdown_future = async move {
        let _ = shutdown_rx.await;
    };

    let join = tauri::async_runtime::spawn(super::server::run_in_process(
        memex_dir,
        db,
        listener,
        shutdown_future,
    ));

    Ok(DaemonHandle {
        port,
        started_at,
        pid,
        shutdown_tx: Some(shutdown_tx),
        join: Some(join),
    })
}

/// 同步 bind 一个带 `SO_REUSEADDR` 的 TCP listener，9999 被占时在
/// `[preferred, preferred + PORT_FALLBACK_MAX]` 范围内依次重试。
///
/// 同步 bind 让 caller 在 `spawn_in_process` 内立即感知失败 —— 老实现把 bind
/// 推迟到 axum task 里，spawn_in_process 已经返回成功的 handle，但实际 task
/// 在第一行 await 就 panic，前端只能从"PID 在 / HTTP 一直异常"反推问题。
///
/// `SO_REUSEADDR` 主要解决：用户连点重启时，旧 listener 的 TCP socket 还在
/// OS 内部清理窗口里，新 bind 会偶发 EADDRINUSE。
fn bind_listener(preferred: u16) -> Result<tokio::net::TcpListener> {
    let mut last_err: Option<std::io::Error> = None;
    for offset in 0..=PORT_FALLBACK_MAX {
        let port = preferred.saturating_add(offset);
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        match try_bind_reuse(addr) {
            Ok(listener) => {
                if offset > 0 {
                    tracing::warn!(
                        preferred = preferred,
                        actual = port,
                        "daemon: preferred port busy, fell back",
                    );
                }
                return Ok(listener);
            }
            Err(e) if e.kind() == ErrorKind::AddrInUse => {
                tracing::debug!(port = port, "daemon: bind candidate in use, trying next");
                last_err = Some(e);
            }
            Err(e) => {
                return Err(anyhow!(e).context(format!("bind 127.0.0.1:{port} failed")));
            }
        }
    }
    Err(anyhow!(
        "no free port in {preferred}..={}: {}",
        preferred.saturating_add(PORT_FALLBACK_MAX),
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "unknown".into())
    ))
}

fn try_bind_reuse(addr: SocketAddr) -> std::io::Result<tokio::net::TcpListener> {
    let domain = match addr {
        SocketAddr::V4(_) => Domain::IPV4,
        SocketAddr::V6(_) => Domain::IPV6,
    };
    let socket = Socket::new(domain, Type::STREAM, None)?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    let std_listener: std::net::TcpListener = socket.into();
    tokio::net::TcpListener::from_std(std_listener)
}
