//! In-process daemon host —— Phase 2 的核心。
//!
//! 过去 `memex-daemon` 是独立 binary，由 Tauri 主进程 `Command::spawn` 出去跑。
//! 现在把它折叠成 `tauri::async_runtime::spawn` 出去的 in-process task，
//! 跟主进程共享同一个 PID / tokio runtime / db handle。
//!
//! 关键差异 vs binary 模式：
//! * **lock 文件**：仍写 `~/.memex/daemon.lock`，但 `pid` 字段是**主进程 PID**。
//!   这是为了向后兼容 `memex-cli daemon_client`（Phase 5 前还得让它能通过 lock
//!   读 port + 探活）。
//! * **signal handler**：不装。`shutdown` 由 Tauri 的 `ExitRequested` 调
//!   [`DaemonState::shutdown_blocking`] 触发。
//! * **db handle**：现阶段还是 daemon 自己 open（跟 binary 模式一致）；Phase 后续
//!   可以再下沉到 Tauri State，跟前端 commands 共享。
//!
//! ## State 模型
//!
//! `DaemonState` 是 `tauri::State` 持有的可变容器（`Mutex<Option<DaemonHandle>>`）。
//! 这样 setup() 阶段可以**先** `app.manage(DaemonState::new())`，然后异步 spawn
//! 任务里再 `state.install(handle).await`。前端 `daemon_restart` IPC 也能通过
//! State 替换内部 handle，而不必 `unmanage + manage` 折腾。

use std::sync::Arc;

use anyhow::{Context, Result};
use memex_core::config::ensure_memex_dir;
use memex_core::storage::db::Db;
use tokio::sync::Mutex;
use tokio::sync::oneshot;

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
        memex_daemon::lockfile::remove_lock(&memex_core::memex_dir());
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
    pub async fn restart(&self, port: u16) -> Result<DaemonSnapshot> {
        self.shutdown().await;
        let handle = spawn_in_process(port).await?;
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
/// 调用方负责把返回的 handle 通过 `DaemonState::install` 注册到 Tauri State。
///
/// 失败原因（已知）：
/// * `~/.memex` 目录创建失败（磁盘满 / 权限）
/// * `memex.db` open 失败（schema migration 报错 / 文件损坏）
/// * lock 写入失败（极小概率，磁盘只读）
///
/// 注意：本函数不检查"是否已有 daemon 在跑"。调用方应在 Tauri 启动早期主动调
/// `stop_daemon_blocking()`（含 self-pid 守卫）清掉历史独立 daemon 的 lock。
pub async fn spawn_in_process(port: u16) -> Result<DaemonHandle> {
    let memex_dir = memex_core::memex_dir();
    ensure_memex_dir(&memex_dir).context("ensure_memex_dir failed")?;

    let db_path = memex_dir.join("memex.db");
    let db = Arc::new(Db::open(&db_path).context("Db::open failed")?);

    // 写 lock：pid=主进程 PID。这样 memex-cli/daemon_client 通过
    // read_lock + is_process_alive 仍能正常发现 daemon。
    memex_daemon::lockfile::write_lock(&memex_dir, port).context("write daemon.lock failed")?;
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

    let join = tauri::async_runtime::spawn(memex_daemon::run_in_process(
        memex_dir,
        db,
        port,
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
