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
