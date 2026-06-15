//! Memex CLI library form —— 把原本只作为 `memex` 二进制存在的 commands
//! 暴露给 memex-menubar（Tauri GUI）直接调用，避免 release-bundled GUI
//! spawn sidecar 时在 macOS hardened runtime 下偶发 EBADF。
//!
//! main.rs 通过 `use memex_cli::*` 复用同一份 module 树，binary 行为不变；
//! 一份代码两种调用形态。
//!
//! 公开 API 边界：只 pub mod 到 module 级，不 re-export 个别 fn —— 调用方
//! 用 `memex_cli::commands::setup::list_status()` 这种全限定路径，可读且不
//! 会被自动补全展开成几十个无关 symbol。

#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

pub mod cli;
pub mod client;
pub mod commands;
pub mod dispatch;
#[macro_use]
pub mod io;
