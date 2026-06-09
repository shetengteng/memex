use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::params;
use rusqlite::types::ValueRef;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::collector::{Adapter, safe_prefix};
use crate::storage::models::{RawMessage, Role, SessionMeta};

const COMPOSER_KEY_PREFIX: &str = "composerData:";
const BUBBLE_KEY_PREFIX: &str = "bubbleId:";
/// 全局 K-V 表里存对话标题 + 工作区信息的 key（Cursor 新版引入）。
/// 这条记录里是一个 JSON 对象 `{ allComposers: [...] }`，每个 composer 元素
/// 带 `name`（对话标题）和 `workspaceIdentifier.uri.path`（真实 cwd）。
/// 注：composerData 总数 >> composerHeaders.allComposers，
/// 所以 scan 主源仍走 composerData，headers 只做 enrich。
const HEADERS_KEY: &str = "composer.composerHeaders";

pub struct CursorSqliteAdapter {
    db_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ComposerData {
    #[serde(rename = "composerId")]
    composer_id: Option<String>,
    name: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<i64>,
    #[serde(rename = "lastUpdatedAt")]
    last_updated_at: Option<i64>,
    #[serde(rename = "fullConversationHeadersOnly")]
    headers: Option<Vec<ConversationHeader>>,
}

#[derive(Debug, Deserialize)]
struct ConversationHeader {
    #[serde(rename = "bubbleId")]
    bubble_id: String,
    #[serde(default, rename = "type")]
    type_: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct Bubble {
    #[serde(rename = "type")]
    type_: Option<i64>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "richText")]
    rich_text: Option<String>,
    #[serde(default, rename = "toolFormerData")]
    tool_former_data: Option<ToolFormerData>,
}

#[derive(Debug, Deserialize)]
struct ToolFormerData {
    name: Option<String>,
    #[serde(default)]
    result: Option<String>,
    #[serde(default, rename = "rawArgs")]
    raw_args: Option<String>,
}

/// `ItemTable.composer.composerHeaders` 的 JSON 顶层 schema。
#[derive(Debug, Deserialize)]
struct ComposerHeadersEnvelope {
    #[serde(rename = "allComposers", default)]
    all_composers: Vec<ComposerHeader>,
}

#[derive(Debug, Deserialize)]
struct ComposerHeader {
    #[serde(rename = "composerId")]
    composer_id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "workspaceIdentifier")]
    workspace_identifier: Option<WorkspaceIdentifier>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceIdentifier {
    /// 单文件夹 workspace。`uri.fsPath` / `uri.path` 都是真实绝对路径。
    #[serde(default)]
    uri: Option<WorkspaceUri>,
    /// 多文件夹 workspace（保存为 `.code-workspace` 文件）。这种情况下没有
    /// 单一 cwd —— 当前刻意不还原成 project_path，保留字段是为了：
    /// 1. 后续若要把 `.code-workspace` 文件名当作"项目名"，可以直接用；
    /// 2. 反序列化时显式声明字段，便于诊断。
    #[allow(dead_code)]
    #[serde(default, rename = "configPath")]
    config_path: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceUri {
    #[serde(default, rename = "fsPath")]
    fs_path: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

/// 单条 composer 的 enrichment 结果。`project_path` / `title` 都是 Option。
#[derive(Debug, Default, Clone)]
struct ComposerEnrichment {
    project_path: Option<String>,
    title: Option<String>,
}

impl CursorSqliteAdapter {
    pub fn new() -> Self {
        let db_path = dirs::home_dir()
            .expect("cannot determine home directory")
            .join("Library/Application Support/Cursor/User/globalStorage/state.vscdb");
        Self { db_path }
    }

    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    /// 从 `ItemTable.composer.composerHeaders` 一次性加载 `composerId -> 元数据` 映射。
    /// 不存在 / 解析失败一律返回空 Map，让 scan 走纯 composerData 路径，
    /// 此时 project_path / title 全部为 None（这是用户的 469 条无 project_path
    /// 会话此前的退化形态，至少不会更差）。
    fn load_header_enrichments(
        &self,
        conn: &rusqlite::Connection,
    ) -> HashMap<String, ComposerEnrichment> {
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM ItemTable WHERE key = ?1",
                params![HEADERS_KEY],
                |row| Ok(value_ref_to_string(row.get_ref(0)?)),
            )
            .ok()
            .flatten();
        let Some(raw) = raw else {
            debug!("cursor[sqlite]: composerHeaders absent; enrichment disabled");
            return HashMap::new();
        };
        let envelope: ComposerHeadersEnvelope = match serde_json::from_str(&raw) {
            Ok(e) => e,
            Err(e) => {
                warn!(
                    "cursor[sqlite]: failed to parse composerHeaders ({}); enrichment disabled",
                    e
                );
                return HashMap::new();
            }
        };

        let mut out = HashMap::with_capacity(envelope.all_composers.len());
        for header in envelope.all_composers {
            let project_path = match header.workspace_identifier {
                Some(WorkspaceIdentifier { uri: Some(u), .. }) => {
                    u.fs_path.or(u.path).filter(|s| !s.is_empty())
                }
                Some(WorkspaceIdentifier {
                    config_path: Some(ref cp),
                    ..
                }) => config_path_to_project(cp),
                _ => None,
            };
            out.insert(
                header.composer_id,
                ComposerEnrichment {
                    project_path,
                    title: header.name.filter(|s| !s.is_empty()),
                },
            );
        }
        out
    }

    fn open_readonly(&self) -> Result<Option<rusqlite::Connection>> {
        if !self.db_path.exists() {
            debug!(
                "cursor[sqlite]: db not found at {}; skipping",
                self.db_path.display()
            );
            return Ok(None);
        }
        let uri = format!(
            "file:{}?mode=ro&immutable=0",
            self.db_path.to_string_lossy()
        );
        let conn = match rusqlite::Connection::open_with_flags(
            &uri,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("unable to open")
                    || msg.contains("authorization denied")
                    || msg.contains("permission")
                {
                    warn!(
                        "cursor[sqlite]: cannot open {} ({msg}).\n  \
                         macOS likely needs Full Disk Access for the terminal running `memex`.\n  \
                         Grant it via System Settings → Privacy & Security → Full Disk Access,\n  \
                         then re-run `memex ingest`. Skipping cursor adapter for now.",
                        self.db_path.display()
                    );
                    return Ok(None);
                }
                return Err(e).with_context(|| {
                    format!("cursor[sqlite]: failed to open {}", self.db_path.display())
                });
            }
        };
        Ok(Some(conn))
    }
}

impl Default for CursorSqliteAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Cursor 的 `cursorDiskKV.value` 列声明类型是 BLOB，
/// 但实际写入既可能是 TEXT JSON（新版 Cursor，绝大多数 row）
/// 也可能是 BLOB 字节（老 fixture / 二进制 cache）。
/// 用 ValueRef 手动区分，避免 rusqlite 的严格类型检查报
/// "Invalid column type Text/Blob at index ... name: value"。
fn value_ref_to_string(value: ValueRef<'_>) -> Option<String> {
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

/// 给 `memex doctor` 和 menubar 设置页用的轻量健康探测。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CursorSqliteProbe {
    Ok {
        composer_count: i64,
        db_path: String,
    },
    NotFound {
        db_path: String,
    },
    PermissionDenied {
        db_path: String,
        message: String,
    },
    Error {
        db_path: String,
        message: String,
    },
}

impl CursorSqliteAdapter {
    pub fn probe(&self) -> CursorSqliteProbe {
        if !self.db_path.exists() {
            return CursorSqliteProbe::NotFound {
                db_path: self.db_path.to_string_lossy().to_string(),
            };
        }
        let uri = format!(
            "file:{}?mode=ro&immutable=0",
            self.db_path.to_string_lossy()
        );
        match rusqlite::Connection::open_with_flags(
            &uri,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(conn) => match conn.query_row(
                "SELECT COUNT(*) FROM cursorDiskKV WHERE key LIKE 'composerData:%'",
                [],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(n) => CursorSqliteProbe::Ok {
                    composer_count: n,
                    db_path: self.db_path.to_string_lossy().to_string(),
                },
                Err(e) => CursorSqliteProbe::Error {
                    db_path: self.db_path.to_string_lossy().to_string(),
                    message: e.to_string(),
                },
            },
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("unable to open")
                    || msg.contains("authorization")
                    || msg.contains("permission")
                {
                    CursorSqliteProbe::PermissionDenied {
                        db_path: self.db_path.to_string_lossy().to_string(),
                        message: msg,
                    }
                } else {
                    CursorSqliteProbe::Error {
                        db_path: self.db_path.to_string_lossy().to_string(),
                        message: msg,
                    }
                }
            }
        }
    }
}

impl Adapter for CursorSqliteAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let Some(conn) = self.open_readonly()? else {
            return Ok(Vec::new());
        };

        let enrichments = self.load_header_enrichments(&conn);

        let mut stmt = conn
            .prepare("SELECT key, value FROM cursorDiskKV WHERE key LIKE ?1")
            .context("cursor[sqlite]: prepare composerData query failed")?;
        let pattern = format!("{}%", COMPOSER_KEY_PREFIX);
        let rows = stmt
            .query_map(params![pattern], |row| {
                let key: String = row.get(0)?;
                let value = value_ref_to_string(row.get_ref(1)?);
                Ok((key, value))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut sessions = Vec::with_capacity(rows.len());
        for (key, value) in rows {
            let text = match value {
                Some(s) => s,
                None => continue,
            };
            let composer: ComposerData = match serde_json::from_str(&text) {
                Ok(c) => c,
                Err(e) => {
                    debug!("cursor[sqlite]: skip malformed composer {}: {}", key, e);
                    continue;
                }
            };
            let composer_id = composer
                .composer_id
                .clone()
                .or_else(|| key.strip_prefix(COMPOSER_KEY_PREFIX).map(String::from))
                .unwrap_or_default();
            if composer_id.is_empty() {
                continue;
            }

            let mtime_ms = composer
                .last_updated_at
                .or(composer.created_at)
                .unwrap_or(0);
            let mtime = if mtime_ms > 0 {
                (mtime_ms / 1000) as u64
            } else {
                0
            };
            let created_ms = composer.created_at.unwrap_or(0);
            let created_secs = if created_ms > 0 {
                (created_ms / 1000) as u64
            } else {
                0
            };

            // composer.name 历来被错放进 project_path —— 实际是对话标题。
            // 新版 Cursor 把它和 workspaceIdentifier 都搬去了 composerHeaders，
            // 所以这里以 enrichment 为准；enrichment 缺失时退回 composer.name
            // 当 title（至少给 UI 留下可读字符串），project_path 留空。
            let enrichment = enrichments.get(&composer_id).cloned().unwrap_or_default();
            let project_path = enrichment
                .project_path
                .or_else(|| infer_project_from_raw_json(&text));
            let title = enrichment.title.or_else(|| {
                composer
                    .name
                    .clone()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && !is_generic_title(s))
            });

            sessions.push(SessionMeta {
                id: format!("cursor-{}", composer_id),
                source: "cursor".to_string(),
                project_path,
                // 关键：每个 composer 必须有独一无二的 file_path。
                // 否则 ingest 会把所有 cursor session 的 collect 进度
                // 共享到 `sources` 表的同一行（key = file_path），
                // 一条大会话把 last_offset 推到 N 后，所有 headers 长度
                // < N 的会话都会被 `collect()` 里的 `start >= headers.len()`
                // 判 0 直接吞掉。这里的 fragment 不是真路径，但 cursor
                // 的 collect() 不会读它（只用 db_path + session.id），
                // 只参与 sources 表的 key 隔离 + 调试可见。
                file_path: format!(
                    "{}#composer={}",
                    self.db_path.to_string_lossy(),
                    composer_id
                ),
                last_offset: 0,
                mtime,
                created_secs,
                title,
            });
        }
        Ok(sessions)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        let composer_id = session
            .id
            .strip_prefix("cursor-")
            .unwrap_or(&session.id)
            .to_string();

        let Some(conn) = self.open_readonly()? else {
            return Ok(Vec::new());
        };

        let composer_key = format!("{}{}", COMPOSER_KEY_PREFIX, composer_id);
        let composer_text: Option<String> = conn
            .query_row(
                "SELECT value FROM cursorDiskKV WHERE key = ?1",
                params![composer_key],
                |row| Ok(value_ref_to_string(row.get_ref(0)?)),
            )
            .ok()
            .flatten();
        let Some(composer_text) = composer_text else {
            return Ok(Vec::new());
        };
        let composer: ComposerData = serde_json::from_str(&composer_text)
            .with_context(|| format!("cursor[sqlite]: parse composer {composer_id}"))?;

        let headers = composer.headers.unwrap_or_default();
        let start = session.last_offset as usize;
        if start >= headers.len() {
            return Ok(Vec::new());
        }

        let mut messages = Vec::with_capacity(headers.len() - start);
        for (idx, header) in headers.iter().enumerate().skip(start) {
            let key = format!("{}{}:{}", BUBBLE_KEY_PREFIX, composer_id, header.bubble_id);
            let bubble_text: Option<String> = conn
                .query_row(
                    "SELECT value FROM cursorDiskKV WHERE key = ?1",
                    params![&key],
                    |row| Ok(value_ref_to_string(row.get_ref(0)?)),
                )
                .ok()
                .flatten();
            let Some(bubble_text) = bubble_text else {
                continue;
            };
            let bubble: Bubble = match serde_json::from_str(&bubble_text) {
                Ok(b) => b,
                Err(e) => {
                    debug!("cursor[sqlite]: skip malformed bubble {}: {}", key, e);
                    continue;
                }
            };

            let type_id = bubble.type_.or(header.type_);
            let role = match type_id {
                Some(1) => Role::User,
                Some(2) => Role::Assistant,
                _ => continue,
            };

            let content = bubble_content(&bubble);
            let content = match content {
                Some(c) if !c.trim().is_empty() => c,
                _ => continue,
            };

            let offset_ix = (idx as u64) + 1;
            let id = blake3::hash(
                format!(
                    "{}{}{}",
                    session.id,
                    header.bubble_id,
                    safe_prefix(&content, 100)
                )
                .as_bytes(),
            )
            .to_hex()
            .to_string();

            messages.push(RawMessage {
                id,
                session_id: session.id.clone(),
                role,
                content,
                timestamp: None,
                source_offset: offset_ix,
            });
        }

        Ok(messages)
    }
}

fn is_generic_title(s: &str) -> bool {
    const GENERIC: &[&str] = &[
        "conversation initiation",
        "conversation start",
        "start the conversation",
        "start of the conversation",
        "new conversation",
        "开始对话",
        "新对话",
        "新的对话",
        "继续讨论",
        "prompts file discussion",
        "prompts from prompts.txt",
    ];
    let lower = s.to_lowercase();
    GENERIC.iter().any(|g| lower == *g)
}

/// Infer project_path by scanning the raw composerData JSON for `file:///` URIs
/// in the `codeBlockData` section. Collects up to 10 paths and finds their
/// common ancestor directory. Light-weight: no JSON value parsing required.
fn infer_project_from_raw_json(raw: &str) -> Option<String> {
    let pattern = "\"file:///";
    let mut dirs: Vec<String> = Vec::new();
    for (i, _) in raw.match_indices(pattern) {
        let uri_start = i + 1;
        let uri_rest = &raw[uri_start..];
        if let Some(end) = uri_rest.find('"') {
            let uri = &uri_rest[..end];
            if let Some(path) = uri.strip_prefix("file://") {
                let decoded = percent_decode(path);
                let p = std::path::Path::new(&decoded);
                let dir = p
                    .parent()
                    .map(|d| d.to_string_lossy().to_string())
                    .unwrap_or_default();
                if dir.is_empty() {
                    continue;
                }
                if dir.contains("/.cursor/") || dir.contains("\\.cursor\\") {
                    continue;
                }
                if !dirs.contains(&dir) {
                    dirs.push(dir);
                    if dirs.len() >= 10 {
                        break;
                    }
                }
            }
        }
    }
    if dirs.is_empty() {
        return None;
    }
    find_common_ancestor(&dirs)
}

fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Minimum path depth to be considered a meaningful project path.
/// `/Users/foo` = 2 components, so we require > 3.
const MIN_ANCESTOR_DEPTH: usize = 4;

fn find_common_ancestor(dirs: &[String]) -> Option<String> {
    if dirs.is_empty() {
        return None;
    }
    if dirs.len() == 1 {
        let depth = std::path::Path::new(&dirs[0]).components().count();
        return if depth >= MIN_ANCESTOR_DEPTH {
            Some(dirs[0].clone())
        } else {
            None
        };
    }
    let first = std::path::Path::new(&dirs[0]);
    let components: Vec<_> = first.components().collect();
    let mut common_len = components.len();
    for dir in &dirs[1..] {
        let p = std::path::Path::new(dir);
        let p_comps: Vec<_> = p.components().collect();
        let matching = components
            .iter()
            .zip(p_comps.iter())
            .take_while(|(a, b)| a == b)
            .count();
        common_len = common_len.min(matching);
    }
    if common_len < MIN_ANCESTOR_DEPTH {
        return None;
    }
    let ancestor: std::path::PathBuf = components[..common_len].iter().collect();
    let s = ancestor.to_string_lossy().to_string();
    if s.is_empty() || s == "/" {
        None
    } else {
        Some(s)
    }
}

/// Extract a project path from a multi-folder workspace `configPath`.
/// The value may be a JSON string (`"/path/to/foo.code-workspace"`)
/// or an object with `fsPath`/`path` fields.
/// Returns the parent directory of the `.code-workspace` file.
/// Skips Cursor-internal workspace.json paths (under `Cursor/Workspaces/`).
fn config_path_to_project(val: &serde_json::Value) -> Option<String> {
    let raw = match val {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(obj) => obj
            .get("fsPath")
            .or_else(|| obj.get("path"))
            .and_then(|v| v.as_str())
            .map(String::from),
        _ => None,
    };
    let raw = raw.filter(|s| !s.is_empty())?;
    if raw.contains("Cursor/Workspaces/") || raw.contains("Cursor\\Workspaces\\") {
        return None;
    }
    let p = std::path::Path::new(&raw);
    p.parent()
        .map(|d| d.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
}

fn bubble_content(bubble: &Bubble) -> Option<String> {
    if let Some(text) = bubble.text.as_ref() {
        if !text.trim().is_empty() {
            return Some(text.clone());
        }
    }
    if let Some(rich) = bubble.rich_text.as_ref() {
        if !rich.trim().is_empty() {
            return Some(rich.clone());
        }
    }
    if let Some(tool) = bubble.tool_former_data.as_ref() {
        let name = tool.name.as_deref().unwrap_or("tool");
        let mut parts = Vec::new();
        parts.push(format!("[tool: {}]", name));
        if let Some(args) = &tool.raw_args {
            if !args.trim().is_empty() {
                parts.push(format!("args: {}", args));
            }
        }
        if let Some(result) = &tool.result {
            if !result.trim().is_empty() {
                parts.push(format!("result: {}", result));
            }
        }
        if parts.len() > 1 {
            return Some(parts.join("\n"));
        }
    }
    None
}
