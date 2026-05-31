pub mod collector;
pub mod config;
pub mod mcp;
pub mod processor;
pub mod retriever;
pub mod storage;

use std::path::PathBuf;

pub fn memex_dir() -> PathBuf {
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".memex")
}
