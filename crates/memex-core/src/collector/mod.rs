pub mod aider;
pub mod claude_code;
pub mod cline;
pub mod codex;
pub mod continue_dev;
pub mod cursor;
pub mod opencode;

use crate::storage::models::{RawMessage, SessionMeta};
use anyhow::Result;

pub trait Adapter {
    fn name(&self) -> &str;
    fn scan(&self) -> Result<Vec<SessionMeta>>;
    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>>;
}

pub fn safe_prefix(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude_code::ClaudeCodeAdapter::new()),
        Box::new(cursor::CursorAdapter::new()),
        Box::new(codex::CodexAdapter::new()),
        Box::new(opencode::OpenCodeAdapter::new()),
        Box::new(aider::AiderAdapter::new()),
        Box::new(continue_dev::ContinueAdapter::new()),
        Box::new(cline::ClineAdapter::new()),
    ]
}
