//! `memex` 二进制入口 —— 解析 clap 后转给 [`dispatch::run`]，binary 自身
//! 不承担 ingest / search / context / hook 等业务逻辑，只做 tracing 初始化
//! 和子命令路由。

#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
mod commands;
mod dispatch;
#[macro_use]
mod io;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let parsed = cli::Cli::parse();
    dispatch::run(parsed)
}
