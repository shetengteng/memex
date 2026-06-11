//! Tauri 主进程内嵌的"服务层"——把过去跑在独立 binary 里的后台能力（首先是
//! `memex-daemon`）作为 in-process tokio task 托管。
//!
//! 跟 `crate::commands` 的关系：`commands` 是面向前端 webview 的 IPC handler，
//! `services` 是后端常驻服务的生命周期管理 + 句柄持有。`commands` 通过
//! `tauri::State<...>` 读取 `services` 暴露的句柄来响应前端请求。

pub mod daemon;
