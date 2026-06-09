//! `Adapter::scan` 主流程：composerData → SessionMeta。
//!
//! 写在独立模块里是因为 `scan` 体非常长（读 composerHeaders enrich、
//! 解析 composerData、推断 project_path、构造 SessionMeta）。
//! `mod.rs` 里的 trait impl 只做一行转发。

use std::collections::HashMap;

use anyhow::{Context, Result};
use rusqlite::params;
use tracing::{debug, warn};

use super::project_path::infer_project_from_raw_json;
use super::types::{
    COMPOSER_KEY_PREFIX, ComposerData, ComposerEnrichment, ComposerHeadersEnvelope, HEADERS_KEY,
    WorkspaceIdentifier, value_ref_to_string,
};
use super::{CursorSqliteAdapter, is_generic_title};
use crate::storage::models::SessionMeta;

pub(super) fn scan_sessions(adapter: &CursorSqliteAdapter) -> Result<Vec<SessionMeta>> {
    let Some(conn) = adapter.open_readonly()? else {
        return Ok(Vec::new());
    };

    let enrichments = load_header_enrichments(&conn);

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
                adapter.db_path().to_string_lossy(),
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

/// 从 `ItemTable.composer.composerHeaders` 一次性加载 `composerId -> 元数据` 映射。
/// 不存在 / 解析失败一律返回空 Map，让 scan 走纯 composerData 路径，
/// 此时 project_path / title 全部为 None。
fn load_header_enrichments(conn: &rusqlite::Connection) -> HashMap<String, ComposerEnrichment> {
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
        // Multi-folder workspace（只有 configPath 没有 uri）→ 不还原 cwd：
        // .code-workspace 的父目录可能完全不是工程 cwd（用户常把 workspace
        // 文件丢在 ~/Documents 或 Desktop，而项目在别处）。
        let project_path = match header.workspace_identifier {
            Some(WorkspaceIdentifier { uri: Some(u), .. }) => {
                u.fs_path.or(u.path).filter(|s| !s.is_empty())
            }
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
