//! 资料库视图的多维过滤查询：把 adapter / project / time / summary / query /
//! sort 全部下推到 SQL，前端不再做内存过滤——避免 facets counts 走全表、
//! filter 走 200 条窗口时的不一致。

use anyhow::Result;
use rusqlite::types::Value as SqlValue;
use serde_rusqlite::from_rows;

use crate::storage::db::Db;
use crate::storage::db::sessions::{SessionListFilter, SessionRow};

impl Db {
    /// 同时支持 adapter / project / time / summary / query / sort 复合过滤的分页查询。
    ///
    /// 设计要点：
    ///   * 跟 [`Db::list_sessions_paged`] 共享同一份 SELECT 形态（含 stale-empty
    ///     过滤 + JOIN L2 summaries + 第一条 user message preview），filter 只追加
    ///     额外的 WHERE 子句和 ORDER BY，调用方拿到的 `SessionRow` 结构完全一致。
    ///   * 动态 SQL 用 `rusqlite::types::Value` 装异构参数（String / i64），
    ///     借 [`rusqlite::params_from_iter`] 一次绑定，避免 `Box<dyn ToSql>` 那套
    ///     生命周期繁文缛节。
    ///   * `time` 字段直接用 SQLite 的 `datetime('now', '-N days', 'start of day')`
    ///     —— 跟前端 `computeTimeLowerBound` 的"含今天的 N 个日历日"语义一致，
    ///     不需要把当前时间从 Rust 传进来。
    ///   * `projects` 不做 path == name 的精确比较——前端只持有 path 末段名
    ///     （如 "memex"），后端用 `LIKE '%/<name>'` 做尾段匹配，避免误命中
    ///     `memex-clone` 这种同前缀目录。
    ///     旧实现用 `LIKE '%/<name>'` 末段匹配会把所有同末段不同前缀的
    ///     路径混算（如 `/A/src` 和 `/B/src` 都被算成 "src"），导致 facet
    ///     上某行写 42、勾选后却命中所有 src 合计 56。前端
    ///     `LibraryFacets.vue` 现在显式传完整路径，并对同末段路径做去歧义
    ///     展示（如 `tt-demo/src`、`metadata-server/src`）。
    pub fn list_sessions_filtered_paged(
        &self,
        filter: &SessionListFilter,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SessionRow>> {
        let mut where_clauses: Vec<String> =
            vec!["NOT (s.message_count = 0 AND s.created_at < datetime('now', '-1 day'))".into()];
        let mut binds: Vec<SqlValue> = Vec::new();

        push_adapters_clause(filter, &mut where_clauses, &mut binds);
        push_projects_clause(filter, &mut where_clauses, &mut binds);
        push_time_clause(filter, &mut where_clauses);
        push_summary_clause(filter, &mut where_clauses);
        push_query_clause(filter, &mut where_clauses, &mut binds);

        let order_by = pick_order_by(filter);

        binds.push(SqlValue::Integer(limit as i64));
        binds.push(SqlValue::Integer(offset as i64));

        let sql = format!(
            "SELECT s.id, s.source, s.project_path, s.title, s.message_count,
                    s.created_at, s.updated_at,
                    sm.title AS summary_title,
                    (SELECT substr(m.content, 1, 120)
                     FROM messages m
                     WHERE m.session_id = s.id AND m.role = 'user'
                     ORDER BY m.source_offset ASC LIMIT 1) AS first_user_message,
                    s.intent
             FROM sessions s
             LEFT JOIN summaries sm
                ON sm.session_id = s.id AND sm.level = 'L2_session'
             WHERE {}
             {}
             LIMIT ? OFFSET ?",
            where_clauses.join(" AND "),
            order_by,
        );

        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query(rusqlite::params_from_iter(binds.iter()))?;
        let out: Vec<SessionRow> =
            from_rows::<SessionRow>(rows).collect::<std::result::Result<_, _>>()?;
        Ok(out)
    }
}

fn push_adapters_clause(
    filter: &SessionListFilter,
    where_clauses: &mut Vec<String>,
    binds: &mut Vec<SqlValue>,
) {
    let Some(adapters) = filter.adapters.as_ref().filter(|v| !v.is_empty()) else {
        return;
    };
    let placeholders = vec!["?"; adapters.len()].join(",");
    where_clauses.push(format!("s.source IN ({placeholders})"));
    for a in adapters {
        binds.push(SqlValue::Text(a.clone()));
    }
}

fn push_projects_clause(
    filter: &SessionListFilter,
    where_clauses: &mut Vec<String>,
    binds: &mut Vec<SqlValue>,
) {
    let Some(projects) = filter.projects.as_ref().filter(|v| !v.is_empty()) else {
        return;
    };
    let parts = vec!["s.project_path LIKE ?"; projects.len()].join(" OR ");
    where_clauses.push(format!("({parts})"));
    for p in projects {
        binds.push(SqlValue::Text(format!("%/{p}")));
    }
}

fn push_time_clause(filter: &SessionListFilter, where_clauses: &mut Vec<String>) {
    let Some(time) = filter.time.as_deref() else {
        return;
    };
    let cutoff_expr = match time {
        "today" => "datetime('now', 'start of day')",
        "7d" => "datetime('now', '-6 days', 'start of day')",
        "30d" => "datetime('now', '-29 days', 'start of day')",
        "90d" => "datetime('now', '-89 days', 'start of day')",
        _ => return,
    };
    where_clauses.push(format!("s.updated_at >= {cutoff_expr}"));
}

fn push_summary_clause(filter: &SessionListFilter, where_clauses: &mut Vec<String>) {
    match filter.summary.as_deref() {
        Some("done") => where_clauses.push("sm.session_id IS NOT NULL".into()),
        Some("pending") => where_clauses.push("sm.session_id IS NULL".into()),
        _ => {}
    }
}

fn push_query_clause(
    filter: &SessionListFilter,
    where_clauses: &mut Vec<String>,
    binds: &mut Vec<SqlValue>,
) {
    let Some(q) = filter
        .query
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    where_clauses.push(
        "(s.title LIKE ? OR s.intent LIKE ? OR sm.title LIKE ? \
         OR EXISTS (SELECT 1 FROM messages m \
                    WHERE m.session_id = s.id AND m.role = 'user' \
                      AND m.content LIKE ? LIMIT 1))"
            .into(),
    );
    let pat = format!("%{q}%");
    for _ in 0..4 {
        binds.push(SqlValue::Text(pat.clone()));
    }
}

fn pick_order_by(filter: &SessionListFilter) -> &'static str {
    match filter.sort.as_deref() {
        Some("duration") => {
            "ORDER BY (strftime('%s', s.updated_at) - strftime('%s', s.created_at)) DESC, \
             s.updated_at DESC"
        }
        Some("messages") => "ORDER BY s.message_count DESC, s.updated_at DESC",
        _ => "ORDER BY s.updated_at DESC",
    }
}
