use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::db::Db;

pub const METRIC_SEARCH_COUNT: &str = "search_count";
pub const METRIC_INGEST_COUNT: &str = "ingest_count";
pub const METRIC_INGEST_MESSAGES: &str = "ingest_messages";
pub const METRIC_MCP_CALLS: &str = "mcp_calls";
pub const METRIC_SLOW_QUERIES: &str = "slow_queries";
pub const METRIC_ADAPTER_ERRORS: &str = "adapter_errors";

const SLOW_QUERY_THRESHOLD_MS: u64 = 500;

impl Db {
    pub fn increment_metric(&self, name: &str) -> Result<()> {
        let today = self.now_utc().format("%Y-%m-%d").to_string();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO metrics (date, metric_name, metric_value) VALUES (?1, ?2, 1)
             ON CONFLICT(date, metric_name) DO UPDATE SET metric_value = metric_value + 1",
            params![today, name],
        )?;
        Ok(())
    }

    pub fn increment_metric_by(&self, name: &str, amount: i64) -> Result<()> {
        let today = self.now_utc().format("%Y-%m-%d").to_string();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO metrics (date, metric_name, metric_value) VALUES (?1, ?2, ?3)
             ON CONFLICT(date, metric_name) DO UPDATE SET metric_value = metric_value + ?3",
            params![today, name, amount],
        )?;
        Ok(())
    }

    pub fn get_today_metrics(&self) -> Result<Vec<MetricEntry>> {
        let today = self.now_utc().format("%Y-%m-%d").to_string();
        self.get_metrics_for_date(&today)
    }

    pub fn get_metrics_for_date(&self, date: &str) -> Result<Vec<MetricEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT metric_name, metric_value FROM metrics WHERE date = ?1 ORDER BY metric_name",
        )?;
        let rows = stmt
            .query_map(params![date], |row| {
                Ok(MetricEntry {
                    name: row.get(0)?,
                    value: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_metrics_range(&self, days: u32) -> Result<Vec<DailyMetrics>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT date, metric_name, metric_value FROM metrics
             WHERE date >= date('now', ?1)
             ORDER BY date DESC, metric_name",
        )?;
        let offset = format!("-{} days", days);
        let rows: Vec<(String, String, i64)> = stmt
            .query_map(params![offset], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut result: Vec<DailyMetrics> = Vec::new();
        for (date, name, value) in rows {
            if result.last().map(|d| &d.date) != Some(&date) {
                result.push(DailyMetrics {
                    date: date.clone(),
                    entries: Vec::new(),
                });
            }
            if let Some(day) = result.last_mut() {
                day.entries.push(MetricEntry { name, value });
            }
        }
        Ok(result)
    }

    pub fn record_search_latency(&self, latency_ms: u64) -> Result<()> {
        self.increment_metric(METRIC_SEARCH_COUNT)?;
        if latency_ms > SLOW_QUERY_THRESHOLD_MS {
            self.increment_metric(METRIC_SLOW_QUERIES)?;
            tracing::warn!(
                latency_ms,
                "slow query detected (>{SLOW_QUERY_THRESHOLD_MS}ms)"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricEntry {
    pub name: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyMetrics {
    pub date: String,
    pub entries: Vec<MetricEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_metric() {
        let db = Db::open_in_memory().unwrap();
        db.increment_metric("test_count").unwrap();
        db.increment_metric("test_count").unwrap();
        let metrics = db.get_today_metrics().unwrap();
        let entry = metrics.iter().find(|m| m.name == "test_count").unwrap();
        assert_eq!(entry.value, 2);
    }

    #[test]
    fn test_increment_by() {
        let db = Db::open_in_memory().unwrap();
        db.increment_metric_by("msg_count", 5).unwrap();
        db.increment_metric_by("msg_count", 3).unwrap();
        let metrics = db.get_today_metrics().unwrap();
        let entry = metrics.iter().find(|m| m.name == "msg_count").unwrap();
        assert_eq!(entry.value, 8);
    }

    #[test]
    fn test_search_latency_normal() {
        let db = Db::open_in_memory().unwrap();
        db.record_search_latency(100).unwrap();
        let metrics = db.get_today_metrics().unwrap();
        assert!(metrics.iter().any(|m| m.name == METRIC_SEARCH_COUNT));
        assert!(!metrics.iter().any(|m| m.name == METRIC_SLOW_QUERIES));
    }

    #[test]
    fn test_search_latency_slow() {
        let db = Db::open_in_memory().unwrap();
        db.record_search_latency(600).unwrap();
        let metrics = db.get_today_metrics().unwrap();
        assert!(metrics.iter().any(|m| m.name == METRIC_SEARCH_COUNT));
        assert!(metrics.iter().any(|m| m.name == METRIC_SLOW_QUERIES));
    }
}
