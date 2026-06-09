//! `llm_providers` 表的 CRUD + 互斥 default + 健康状态字段。
//!
//! 拆分：tests 移到 sibling `tests.rs`，本文件保留 DTO + 5 个 `impl Db`
//! 方法。`pub use providers::LlmProviderRow` 在 `storage::db::mod.rs` 顶层。

#[cfg(test)]
mod tests;

use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Db;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
    pub enabled: bool,
    pub is_default: bool,
    pub status: String,
    pub latency_ms: Option<i64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderUpsert {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub is_default: bool,
}

fn default_true() -> bool {
    true
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

impl Db {
    pub fn provider_list(&self) -> Result<Vec<LlmProviderRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, kind, base_url, model, api_key, enabled, is_default, \
                    status, latency_ms, updated_at \
             FROM llm_providers \
             ORDER BY is_default DESC, name ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(LlmProviderRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    base_url: row.get(3)?,
                    model: row.get(4)?,
                    api_key: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                    is_default: row.get::<_, i32>(7)? != 0,
                    status: row.get(8)?,
                    latency_ms: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn provider_get(&self, id: &str) -> Result<Option<LlmProviderRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, kind, base_url, model, api_key, enabled, is_default, \
                    status, latency_ms, updated_at \
             FROM llm_providers WHERE id = ?1",
        )?;
        let row = stmt
            .query_row(params![id], |row| {
                Ok(LlmProviderRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    base_url: row.get(3)?,
                    model: row.get(4)?,
                    api_key: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                    is_default: row.get::<_, i32>(7)? != 0,
                    status: row.get(8)?,
                    latency_ms: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .ok();
        Ok(row)
    }

    pub fn provider_upsert(&self, p: LlmProviderUpsert) -> Result<LlmProviderRow> {
        let conn = self.conn.lock();
        let now = now_iso();

        if p.is_default {
            conn.execute(
                "UPDATE llm_providers SET is_default = 0 WHERE is_default = 1",
                [],
            )?;
        }

        conn.execute(
            "INSERT INTO llm_providers (id, name, kind, base_url, model, api_key, enabled, is_default, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'untested', ?9)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                kind = excluded.kind,
                base_url = excluded.base_url,
                model = excluded.model,
                api_key = CASE WHEN excluded.api_key = '' THEN llm_providers.api_key ELSE excluded.api_key END,
                enabled = excluded.enabled,
                is_default = excluded.is_default,
                updated_at = excluded.updated_at",
            params![
                p.id,
                p.name,
                p.kind,
                p.base_url,
                p.model,
                p.api_key,
                p.enabled as i32,
                p.is_default as i32,
                now,
            ],
        )?;
        drop(conn);

        self.provider_get(&p.id)?
            .ok_or_else(|| anyhow::anyhow!("provider upsert failed"))
    }

    pub fn provider_delete(&self, id: &str) -> Result<u64> {
        let conn = self.conn.lock();
        let n = conn.execute("DELETE FROM llm_providers WHERE id = ?1", params![id])?;
        Ok(n as u64)
    }

    pub fn provider_update_status(
        &self,
        id: &str,
        status: &str,
        latency_ms: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE llm_providers SET status = ?1, latency_ms = ?2, updated_at = ?3 WHERE id = ?4",
            params![status, latency_ms, now_iso(), id],
        )?;
        Ok(())
    }
}
