use crate::storage::db::Db;
use crate::storage::models::SearchResult;
use anyhow::Result;

#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub adapter: Option<String>,
    pub project: Option<String>,
    pub session_id: Option<String>,
    pub chunk_type: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
}

pub struct Retriever<'a> {
    db: &'a Db,
}

impl<'a> Retriever<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.search_filtered(query, limit, &SearchFilter::default())
    }

    pub fn search_filtered(
        &self,
        query: &str,
        limit: usize,
        filter: &SearchFilter,
    ) -> Result<Vec<SearchResult>> {
        let mut results = self.db.fts_search(query, limit * 3)?;

        if let Some(ref adapter) = filter.adapter {
            results.retain(|r| r.adapter.as_deref() == Some(adapter.as_str()));
        }
        if let Some(ref project) = filter.project {
            let lower = project.to_lowercase();
            results.retain(|r| {
                r.project
                    .as_deref()
                    .is_some_and(|p| p.to_lowercase().contains(&lower))
            });
        }
        if let Some(ref sid) = filter.session_id {
            results.retain(|r| r.session_id == *sid);
        }
        if let Some(ref ct) = filter.chunk_type {
            results.retain(|r| r.chunk_type == *ct);
        }
        if let Some(ref after) = filter.after {
            results.retain(|r| r.timestamp.as_deref().is_some_and(|t| t >= after.as_str()));
        }
        if let Some(ref before) = filter.before {
            results.retain(|r| r.timestamp.as_deref().is_some_and(|t| t <= before.as_str()));
        }

        for r in &mut results {
            r.match_reason = build_match_reason(query, r);
        }

        apply_recency_boost(&mut results);

        results.truncate(limit);
        Ok(results)
    }
}

fn build_match_reason(query: &str, result: &SearchResult) -> String {
    let mut reasons = Vec::new();

    let query_lower = query.to_lowercase();
    let keywords: Vec<&str> = query_lower.split_whitespace().collect();
    let content_lower = result.content.to_lowercase();
    let matched: Vec<&&str> = keywords
        .iter()
        .filter(|k| content_lower.contains(**k))
        .collect();
    if !matched.is_empty() {
        let words: Vec<String> = matched.iter().map(|k| format!("\"{}\"", k)).collect();
        reasons.push(format!("keyword match: {}", words.join(", ")));
    }

    if result.chunk_type != "text" {
        reasons.push(format!("chunk_type: {}", result.chunk_type));
    }
    if let Some(ref adapter) = result.adapter {
        reasons.push(format!("source: {}", adapter));
    }

    if reasons.is_empty() {
        "fts5 match".to_string()
    } else {
        reasons.join("; ")
    }
}

fn apply_recency_boost(results: &mut [SearchResult]) {
    let now = chrono::Utc::now().to_rfc3339();
    for r in results.iter_mut() {
        let ts = r.timestamp.as_deref().unwrap_or("");
        let age_days = estimate_age_days(ts, &now);
        let boost = if age_days < 1.0 {
            0.3
        } else if age_days < 7.0 {
            0.15
        } else if age_days < 30.0 {
            0.05
        } else {
            0.0
        };
        r.rank += boost;
    }
    results.sort_by(|a, b| {
        b.rank
            .partial_cmp(&a.rank)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn estimate_age_days(ts: &str, now: &str) -> f64 {
    let parsed_ts = chrono::DateTime::parse_from_rfc3339(ts).ok();
    let parsed_now = chrono::DateTime::parse_from_rfc3339(now).ok();
    match (parsed_ts, parsed_now) {
        (Some(t), Some(n)) => (n - t).num_hours().max(0) as f64 / 24.0,
        _ => 365.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_match_reason() {
        let r = SearchResult {
            chunk_id: 1,
            session_id: "s1".into(),
            message_id: "m1".into(),
            chunk_type: "code_block".into(),
            content: "redis pipeline optimization".into(),
            snippet: "redis pipeline".into(),
            rank: 1.0,
            match_reason: String::new(),
            adapter: Some("claude_code".into()),
            project: None,
            timestamp: None,
        };
        let reason = build_match_reason("redis pipeline", &r);
        assert!(reason.contains("keyword match"));
        assert!(reason.contains("chunk_type: code_block"));
    }

    #[test]
    fn test_estimate_age_days() {
        let now = "2026-06-01T12:00:00+00:00";
        let yesterday = "2026-05-31T12:00:00+00:00";
        assert!((estimate_age_days(yesterday, now) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_recency_boost_sorting() {
        let mut results = vec![
            SearchResult {
                chunk_id: 1,
                session_id: "s1".into(),
                message_id: "m1".into(),
                chunk_type: "text".into(),
                content: "old".into(),
                snippet: "old".into(),
                rank: -5.0,
                match_reason: String::new(),
                adapter: None,
                project: None,
                timestamp: Some("2026-01-01T00:00:00+00:00".into()),
            },
            SearchResult {
                chunk_id: 2,
                session_id: "s2".into(),
                message_id: "m2".into(),
                chunk_type: "text".into(),
                content: "new".into(),
                snippet: "new".into(),
                rank: -5.0,
                match_reason: String::new(),
                adapter: None,
                project: None,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
            },
        ];
        apply_recency_boost(&mut results);
        assert_eq!(results[0].chunk_id, 2);
    }
}
