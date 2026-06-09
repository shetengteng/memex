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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Db {
        Db::open_in_memory().unwrap()
    }

    #[test]
    fn empty_list() {
        let db = test_db();
        let list = db.provider_list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn upsert_and_get() {
        let db = test_db();
        let row = db
            .provider_upsert(LlmProviderUpsert {
                id: "ds-1".into(),
                name: "DeepSeek".into(),
                kind: "openai_compat".into(),
                base_url: "https://api.deepseek.com/v1".into(),
                model: "deepseek-chat".into(),
                api_key: "sk-test".into(),
                enabled: true,
                is_default: true,
            })
            .unwrap();
        assert_eq!(row.name, "DeepSeek");
        assert!(row.is_default);
        assert_eq!(row.status, "untested");

        let got = db.provider_get("ds-1").unwrap().unwrap();
        assert_eq!(got.api_key, "sk-test");
    }

    #[test]
    fn upsert_preserves_api_key_when_empty() {
        let db = test_db();
        db.provider_upsert(LlmProviderUpsert {
            id: "p1".into(),
            name: "Test".into(),
            kind: "openai_compat".into(),
            base_url: "http://localhost".into(),
            model: "m".into(),
            api_key: "secret".into(),
            enabled: true,
            is_default: false,
        })
        .unwrap();

        db.provider_upsert(LlmProviderUpsert {
            id: "p1".into(),
            name: "Test Updated".into(),
            kind: "openai_compat".into(),
            base_url: "http://localhost".into(),
            model: "m2".into(),
            api_key: "".into(),
            enabled: true,
            is_default: false,
        })
        .unwrap();

        let got = db.provider_get("p1").unwrap().unwrap();
        assert_eq!(
            got.api_key, "secret",
            "empty api_key in upsert should preserve existing"
        );
        assert_eq!(got.model, "m2");
    }

    #[test]
    fn default_flag_is_exclusive() {
        let db = test_db();
        db.provider_upsert(LlmProviderUpsert {
            id: "a".into(),
            name: "A".into(),
            kind: "ollama".into(),
            base_url: "http://localhost:11434".into(),
            model: "llama3".into(),
            api_key: "".into(),
            enabled: true,
            is_default: true,
        })
        .unwrap();
        db.provider_upsert(LlmProviderUpsert {
            id: "b".into(),
            name: "B".into(),
            kind: "openai_compat".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o".into(),
            api_key: "sk".into(),
            enabled: true,
            is_default: true,
        })
        .unwrap();

        let list = db.provider_list().unwrap();
        let defaults: Vec<_> = list.iter().filter(|p| p.is_default).collect();
        assert_eq!(defaults.len(), 1, "only one provider can be default");
        assert_eq!(defaults[0].id, "b");
    }

    #[test]
    fn delete() {
        let db = test_db();
        db.provider_upsert(LlmProviderUpsert {
            id: "x".into(),
            name: "X".into(),
            kind: "ollama".into(),
            base_url: "http://localhost:11434".into(),
            model: "m".into(),
            api_key: "".into(),
            enabled: true,
            is_default: false,
        })
        .unwrap();
        assert_eq!(db.provider_delete("x").unwrap(), 1);
        assert!(db.provider_get("x").unwrap().is_none());
    }

    #[test]
    fn update_status() {
        let db = test_db();
        db.provider_upsert(LlmProviderUpsert {
            id: "s".into(),
            name: "S".into(),
            kind: "openai_compat".into(),
            base_url: "https://example.com".into(),
            model: "m".into(),
            api_key: "k".into(),
            enabled: true,
            is_default: false,
        })
        .unwrap();
        db.provider_update_status("s", "ok", Some(123)).unwrap();
        let got = db.provider_get("s").unwrap().unwrap();
        assert_eq!(got.status, "ok");
        assert_eq!(got.latency_ms, Some(123));
    }
}
