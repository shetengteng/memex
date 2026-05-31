pub mod collector;
pub mod config;
pub mod ingest;
pub mod llm;
pub mod mcp;
pub mod processor;
pub mod retriever;
pub mod storage;

use std::path::PathBuf;

/// Resolve the Memex working directory.
///
/// Honors the `MEMEX_HOME` environment variable when set (useful for tests,
/// CI, multi-user setups, or running multiple Memex instances side-by-side).
/// Falls back to `~/.memex` otherwise.
pub fn memex_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("MEMEX_HOME") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".memex")
}
