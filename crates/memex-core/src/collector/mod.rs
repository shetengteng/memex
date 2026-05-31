pub mod claude_code;

use anyhow::Result;
use crate::storage::models::{RawMessage, SessionMeta};

pub trait Adapter {
    fn name(&self) -> &str;
    fn scan(&self) -> Result<Vec<SessionMeta>>;
    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>>;
}
