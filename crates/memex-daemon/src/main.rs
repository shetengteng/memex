//! `memex-daemon` 二进制入口 —— 解析端口参数后调到 [`memex_daemon::run`]。

#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(memex_daemon::DEFAULT_PORT);

    memex_daemon::run(port).await
}
