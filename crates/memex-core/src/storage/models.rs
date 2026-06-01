use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub file_path: String,
    pub last_offset: u64,
    pub mtime: u64,
    /// Unix seconds of the session's true creation/start time, when the
    /// adapter can recover it (cursor composer.created_at, opencode
    /// time_created, file metadata.created()). 0 means unknown; the DB
    /// layer falls back to `now()` only on first insert in that case.
    #[serde(default)]
    pub created_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMessage {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub content: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub source_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
            Role::Tool => write!(f, "tool"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: Option<i64>,
    pub message_id: String,
    pub session_id: String,
    pub chunk_type: ChunkType,
    pub content: String,
    pub redacted_content: Option<String>,
    pub position: u32,
    pub token_count: u32,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    Text,
    CodeBlock,
    ToolCall,
    ToolResult,
    AssistantTurn,
}

impl std::fmt::Display for ChunkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChunkType::Text => write!(f, "text"),
            ChunkType::CodeBlock => write!(f, "code_block"),
            ChunkType::ToolCall => write!(f, "tool_call"),
            ChunkType::ToolResult => write!(f, "tool_result"),
            ChunkType::AssistantTurn => write!(f, "assistant_turn"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkMetadata {
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub has_code: bool,
    #[serde(default)]
    pub tools_used: Vec<String>,
    #[serde(default)]
    pub error_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk_id: i64,
    pub session_id: String,
    pub message_id: String,
    pub chunk_type: String,
    pub content: String,
    pub snippet: String,
    pub rank: f64,
    pub match_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceState {
    pub adapter: String,
    pub file_path: String,
    pub last_offset: u64,
    pub last_mtime: u64,
    pub last_scan: DateTime<Utc>,
}
