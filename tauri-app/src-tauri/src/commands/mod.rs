//! IPC command modules. 每个子模块对应一组 `#[tauri::command]` + 其 DTO。
//!
//! 历史上这里写的是 19 行 `pub use xxx::*;` —— rust.mdc §7.3 / §6.4 把它列
//! 为反模式：任何子模块新加 `pub fn` 都会静悄悄变成 IPC 表面。`tauri::generate_handler!`
//! 又依赖 `__cmd__<name>` 隐藏宏符号（re-export 不带），所以 lib.rs 也必须走
//! 原模块路径而非顶层 re-export。两边一致：lib.rs 用 `commands::backup::backup_now`
//! 列每个 handler；DTO struct/enum 由各业务模块按需 `use commands::backup::BackupResult`
//! 引入，不再走顶层 re-export。

pub mod backup;
pub mod cli_path;
pub mod config;
pub mod daemon;
pub mod doctor;
pub mod hooks;
pub mod ide_integration;
pub mod ingest;
pub mod llm_providers;
pub mod llm_test;
pub mod logs;
pub mod maintenance;
pub mod reflect;
pub mod reports;
pub mod search;
pub mod sessions;
pub mod stats;
pub mod threads;
pub mod update;
