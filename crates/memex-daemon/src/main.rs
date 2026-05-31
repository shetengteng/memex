use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(memex_daemon::DEFAULT_PORT);

    memex_daemon::run(port).await
}
