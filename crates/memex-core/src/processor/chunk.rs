use anyhow::Result;
use crate::storage::models::{Chunk, ChunkMetadata, ChunkType, RawMessage};

const MAX_CHUNK_CHARS: usize = 2000;

pub fn split_into_chunks(msg: &RawMessage) -> Result<Vec<Chunk>> {
    let content = &msg.content;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    let mut position: u32 = 0;

    let segments = split_by_structure(content);

    for seg in segments {
        let chunk_type = detect_chunk_type(&seg);
        let sub_chunks = if seg.len() > MAX_CHUNK_CHARS {
            split_long_segment(&seg, MAX_CHUNK_CHARS)
        } else {
            vec![seg]
        };

        for piece in sub_chunks {
            if piece.trim().is_empty() {
                continue;
            }
            let token_count = estimate_tokens(&piece);
            chunks.push(Chunk {
                id: None,
                message_id: msg.id.clone(),
                session_id: msg.session_id.clone(),
                chunk_type,
                content: piece,
                redacted_content: None,
                position,
                token_count,
                metadata: ChunkMetadata::default(),
            });
            position += 1;
        }
    }

    Ok(chunks)
}

fn split_by_structure(content: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_code_block = false;
    let mut code_block = String::new();

    for line in content.lines() {
        if line.starts_with("```") {
            if in_code_block {
                code_block.push_str(line);
                code_block.push('\n');
                segments.push(code_block.clone());
                code_block.clear();
                in_code_block = false;
            } else {
                if !current.trim().is_empty() {
                    segments.push(current.clone());
                    current.clear();
                }
                in_code_block = true;
                code_block.push_str(line);
                code_block.push('\n');
            }
        } else if in_code_block {
            code_block.push_str(line);
            code_block.push('\n');
        } else if line.starts_with("## ") || line.starts_with("### ") {
            if !current.trim().is_empty() {
                segments.push(current.clone());
                current.clear();
            }
            current.push_str(line);
            current.push('\n');
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }

    if in_code_block && !code_block.is_empty() {
        segments.push(code_block);
    }
    if !current.trim().is_empty() {
        segments.push(current);
    }

    segments
}

fn detect_chunk_type(content: &str) -> ChunkType {
    let trimmed = content.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        return ChunkType::CodeBlock;
    }
    if trimmed.contains("tool_use") || trimmed.contains("tool_call") {
        return ChunkType::ToolCall;
    }
    if trimmed.contains("tool_result") {
        return ChunkType::ToolResult;
    }
    ChunkType::Text
}

fn split_long_segment(content: &str, max_chars: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut start = 0;
    while start < content.len() {
        let mut end = (start + max_chars).min(content.len());
        while end > start && !content.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            end = content.len();
        }
        let boundary = if end < content.len() {
            content[start..end]
                .rfind('\n')
                .map(|pos| start + pos + 1)
                .unwrap_or(end)
        } else {
            end
        };
        pieces.push(content[start..boundary].to_string());
        start = boundary;
    }
    pieces
}

fn estimate_tokens(text: &str) -> u32 {
    (text.len() as f64 / 4.0).ceil() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{RawMessage, Role};

    fn make_msg(content: &str) -> RawMessage {
        RawMessage {
            id: "m1".to_string(),
            session_id: "s1".to_string(),
            role: Role::Assistant,
            content: content.to_string(),
            timestamp: None,
            source_offset: 0,
        }
    }

    #[test]
    fn test_empty_content() {
        let msg = make_msg("");
        let chunks = split_into_chunks(&msg).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_code_block_detection() {
        let msg = make_msg("Here is code:\n\n```python\nprint('hello')\n```\n\nDone.");
        let chunks = split_into_chunks(&msg).unwrap();
        assert!(chunks.len() >= 2);
        let code_chunk = chunks.iter().find(|c| c.chunk_type == ChunkType::CodeBlock);
        assert!(code_chunk.is_some());
    }

    #[test]
    fn test_heading_split() {
        let content = "## Section 1\nContent 1\n\n## Section 2\nContent 2\n";
        let msg = make_msg(content);
        let chunks = split_into_chunks(&msg).unwrap();
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_mixed_content() {
        let content = "Some text\n\n```rust\nfn main() {}\n```\n\nMore text\n";
        let msg = make_msg(content);
        let chunks = split_into_chunks(&msg).unwrap();
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().any(|c| c.chunk_type == ChunkType::CodeBlock));
        assert!(chunks.iter().any(|c| c.chunk_type == ChunkType::Text));
    }
}
