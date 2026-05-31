use anyhow::Result;
use memex_core::config::ensure_memex_dir;
use memex_core::mcp::server;
use memex_core::memex_dir;
use memex_core::storage::db::Db;

pub fn run() -> Result<()> {
    let memex = memex_dir();
    ensure_memex_dir(&memex)?;

    let db_path = memex.join("memex.db");
    let db = Db::open(&db_path)?;

    server::run_stdio(&db)
}
