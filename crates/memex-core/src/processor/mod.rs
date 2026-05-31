pub mod chunk;
pub mod metadata;
pub mod privacy;
pub mod redact;

use crate::storage::models::{Chunk, RawMessage};
use anyhow::Result;
use redact::RedactionHit;

pub struct ProcessedChunk {
    pub chunk: Chunk,
    pub redaction_hits: Vec<RedactionHit>,
}

pub fn process_messages(messages: &[RawMessage]) -> Result<Vec<Chunk>> {
    process_messages_with_hits(messages)
        .map(|v| v.into_iter().map(|pc| pc.chunk).collect())
}

pub fn process_messages_with_hits(messages: &[RawMessage]) -> Result<Vec<ProcessedChunk>> {
    let mut all = Vec::new();
    for msg in messages {
        let chunks = chunk::split_into_chunks(msg)?;
        for mut c in chunks {
            let (redacted, hits) = redact::redact_with_hits(&c.content);
            c.redacted_content = Some(redacted);
            c.metadata = metadata::extract(&c.content);
            all.push(ProcessedChunk { chunk: c, redaction_hits: hits });
        }
    }
    Ok(all)
}
