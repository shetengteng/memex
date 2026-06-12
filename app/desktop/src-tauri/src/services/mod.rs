//! Tauri 主进程内嵌的"服务层" —— 后台常驻服务的源代码归属。
//!
//! Phase 6 起 daemon 的源代码也物理沉到了 [`daemon`] 子模块下，过去独立的
//! `memex-daemon` crate 已解散。本目录的语义边界：
//!
//! * `crate::commands` —— 面向前端 webview 的 IPC handler，浅、薄、转手即可
//! * `crate::services` —— 后端服务的实现 + lifecycle 管理
//!
//! `commands` 通过 `tauri::State<...>` 拿到 `services` 暴露的句柄来响应前端
//! 请求。两边互不直接 `use` 实现细节，只通过受控的 public API 交互。

pub mod daemon;
