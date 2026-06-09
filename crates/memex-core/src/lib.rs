pub mod collector;
pub mod config;
pub mod context;
pub mod ingest;
pub mod llm;
pub mod maintenance;
pub mod mcp;
pub mod processor;
pub mod reflect;
pub mod retriever;
pub mod storage;

use std::path::PathBuf;

/// 解析 Memex 的工作目录。
///
/// 如果设置了 `MEMEX_HOME` 环境变量，优先使用它（方便测试、CI、多用户场景，
/// 或者在同一台机器上并排跑多个 Memex 实例）。
/// 否则回退到 `~/.memex`。
pub fn memex_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("MEMEX_HOME")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom);
    }
    // INVARIANT: HOME (or platform equivalent) is always set in the
    // environments memex targets (macOS, Linux, Windows desktop). The CLI is
    // unusable without it, so panicking here is honest.
    dirs::home_dir()
        .expect("INVARIANT: home directory must be resolvable")
        .join(".memex")
}
