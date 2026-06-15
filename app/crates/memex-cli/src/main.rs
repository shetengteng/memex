//! `memex` 二进制入口 —— 解析 clap 后转给 [`memex_cli::dispatch::run`]，
//! binary 自身不承担 ingest / search / context / hook 等业务逻辑，只做
//! tracing 初始化和子命令路由。
//!
//! 实际逻辑都在 `memex_cli` lib crate 里（`src/lib.rs`），main 直接复用。

#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let parsed = memex_cli::cli::Cli::parse();
    memex_cli::dispatch::run(parsed)
}
