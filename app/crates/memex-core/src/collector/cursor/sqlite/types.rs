//! 所有跨 Cursor SQLite 子模块共享的反序列化 DTO + 常量 + 字节解码。
//!
//! Cursor 把数据塞在 `state.vscdb` 的两张表里：
//! - `ItemTable.composer.composerHeaders` —— 全局 `allComposers` 索引（带
//!   workspace 路径与对话标题），用于 enrichment。
//! - `cursorDiskKV.composerData:<id>` —— 单条对话主体（headers + 元数据），
//!   是 scan 主源。
//! - `cursorDiskKV.bubbleId:<composer>:<bubbleId>` —— 单条消息内容，
//!   collect 时按需读取。

use rusqlite::types::ValueRef;
use serde::Deserialize;

pub(super) const COMPOSER_KEY_PREFIX: &str = "composerData:";
pub(super) const BUBBLE_KEY_PREFIX: &str = "bubbleId:";
/// 全局 K-V 表里存对话标题 + 工作区信息的 key（Cursor 新版引入）。
/// 这条记录里是一个 JSON 对象 `{ allComposers: [...] }`，每个 composer 元素
/// 带 `name`（对话标题）和 `workspaceIdentifier.uri.path`（真实 cwd）。
/// 注：composerData 总数 >> composerHeaders.allComposers，
/// 所以 scan 主源仍走 composerData，headers 只做 enrich。
pub(super) const HEADERS_KEY: &str = "composer.composerHeaders";

#[derive(Debug, Deserialize)]
pub(super) struct ComposerData {
    #[serde(rename = "composerId")]
    pub(super) composer_id: Option<String>,
    pub(super) name: Option<String>,
    #[serde(rename = "createdAt")]
    pub(super) created_at: Option<i64>,
    #[serde(rename = "lastUpdatedAt")]
    pub(super) last_updated_at: Option<i64>,
    #[serde(rename = "fullConversationHeadersOnly")]
    pub(super) headers: Option<Vec<ConversationHeader>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConversationHeader {
    #[serde(rename = "bubbleId")]
    pub(super) bubble_id: String,
    #[serde(default, rename = "type")]
    pub(super) type_: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Bubble {
    #[serde(rename = "type")]
    pub(super) type_: Option<i64>,
    #[serde(default)]
    pub(super) text: Option<String>,
    #[serde(default, rename = "richText")]
    pub(super) rich_text: Option<String>,
    #[serde(default, rename = "toolFormerData")]
    pub(super) tool_former_data: Option<ToolFormerData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ToolFormerData {
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) result: Option<String>,
    #[serde(default, rename = "rawArgs")]
    pub(super) raw_args: Option<String>,
}

/// `ItemTable.composer.composerHeaders` 的 JSON 顶层 schema。
#[derive(Debug, Deserialize)]
pub(super) struct ComposerHeadersEnvelope {
    #[serde(rename = "allComposers", default)]
    pub(super) all_composers: Vec<ComposerHeader>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ComposerHeader {
    #[serde(rename = "composerId")]
    pub(super) composer_id: String,
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default, rename = "workspaceIdentifier")]
    pub(super) workspace_identifier: Option<WorkspaceIdentifier>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WorkspaceIdentifier {
    /// 单文件夹 workspace。`uri.fsPath` / `uri.path` 都是真实绝对路径。
    #[serde(default)]
    pub(super) uri: Option<WorkspaceUri>,
    /// 多文件夹 workspace（保存为 `.code-workspace` 文件）。这种情况下没有
    /// 单一 cwd —— 当前刻意不还原成 project_path，保留字段是为了：
    /// 1. 后续若要把 `.code-workspace` 文件名当作"项目名"，可以直接用；
    /// 2. 反序列化时显式声明字段，便于诊断。
    #[allow(dead_code)]
    #[serde(default, rename = "configPath")]
    pub(super) config_path: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WorkspaceUri {
    #[serde(default, rename = "fsPath")]
    pub(super) fs_path: Option<String>,
    #[serde(default)]
    pub(super) path: Option<String>,
}

/// 单条 composer 的 enrichment 结果。`project_path` / `title` 都是 Option。
#[derive(Debug, Default, Clone)]
pub(super) struct ComposerEnrichment {
    pub(super) project_path: Option<String>,
    pub(super) title: Option<String>,
}

/// Cursor 的 `cursorDiskKV.value` 列声明类型是 BLOB，
/// 但实际写入既可能是 TEXT JSON（新版 Cursor，绝大多数 row）
/// 也可能是 BLOB 字节（老 fixture / 二进制 cache）。
/// 用 ValueRef 手动区分，避免 rusqlite 的严格类型检查报
/// "Invalid column type Text/Blob at index ... name: value"。
pub(super) fn value_ref_to_string(value: ValueRef<'_>) -> Option<String> {
    match value {
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
            if bytes.is_empty() {
                None
            } else {
                std::str::from_utf8(bytes).ok().map(|s| s.to_string())
            }
        }
        ValueRef::Null => None,
        _ => None,
    }
}
