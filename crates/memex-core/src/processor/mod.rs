pub mod chunk;
pub mod metadata;
pub mod privacy;
pub mod redact;

use crate::storage::models::{Chunk, RawMessage};
use anyhow::Result;

pub fn process_messages(messages: &[RawMessage]) -> Result<Vec<Chunk>> {
    let mut all_chunks = Vec::new();
    for msg in messages {
        let chunks = chunk::split_into_chunks(msg)?;
        let chunks: Vec<Chunk> = chunks
            .into_iter()
            .map(|mut c| {
                c.redacted_content = Some(redact::redact(&c.content));
                c.metadata = metadata::extract(&c.content);
                c
            })
            .collect();
        all_chunks.extend(chunks);
    }
    Ok(all_chunks)
}
