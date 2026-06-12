//! In-process daemon —— Phase 6 起的最终居所。
//!
//! 这个模块把过去 `memex-daemon` crate 的整个内容（HTTP routes、watcher、
//! 静态资源服务、lock 文件管理）跟 Tauri 主进程的 lifecycle 管理（[`handle`]）
//! 合在一起。daemon 不再是独立 crate / binary / sidecar；唯一调用方就是
//! 当前这个 `memex-menubar` crate。
//!
//! 子模块按职责分：
//!   * [`server`]   —— `run_in_process` 入口 + axum router 装配
//!   * [`routes`]   —— HTTP 端点实现（search / sessions / stats / context / ingest / mcp/log 等）
//!   * [`watcher`]  —— notify-based 文件监听（IDE 写新 jsonl 时触发增量 ingest）
//!   * [`web`]      —— 内嵌静态资源（前端 popup HTML/JS/CSS）
//!   * [`lockfile`] —— `~/.memex/daemon.lock` 读写，给 memex-cli（含 mcp 子模块）探活
//!   * [`handle`]   —— `DaemonHandle` / `DaemonState` —— Tauri State 持有的运行时句柄

mod handle;
mod lockfile;
mod routes;
mod server;
mod watcher;
mod web;

#[cfg(test)]
mod tests;

// 公开给主进程其他模块（commands / lib.rs）的 API：
pub use handle::{DaemonHandle, DaemonSnapshot, DaemonState, spawn_in_process};
pub use server::{PREFERRED_PORT, build_router};

// build_router 公开是给本模块的 `tests` 集成测试用（通过 `super::server::build_router`）。
// 外部 caller 不直接装配 router；想起 daemon 走 `spawn_in_process`。
