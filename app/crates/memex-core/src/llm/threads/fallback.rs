//! Deterministic fallbacks for thread clustering & keyword querying.
//!
//! Activated when the LLM is unavailable or parses badly. Weaker than
//! the LLM path but always works.

use crate::llm::summarize::SessionSummary;
use crate::storage::db::ThreadDraft;

use super::MAX_THREADS_PER_RESPONSE;

/// 关键词检索的确定性 fallback：在 title / topics / summary / decisions 里
/// 做大小写不敏感的字面子串匹配。
pub fn fallback_query_match(sessions: &[(String, SessionSummary)], query: &str) -> ThreadDraft {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return ThreadDraft {
            name: query.to_string(),
            summary: String::new(),
            session_ids: vec![],
        };
    }
    let mut session_ids = Vec::new();
    for (sid, s) in sessions {
        let hit = s.title.to_lowercase().contains(&needle)
            || s.summary.to_lowercase().contains(&needle)
            || s.topics.iter().any(|t| t.to_lowercase().contains(&needle))
            || s.decisions
                .iter()
                .any(|d| d.to_lowercase().contains(&needle));
        if hit {
            session_ids.push(sid.clone());
        }
    }
    ThreadDraft {
        name: query.to_string(),
        summary: String::new(),
        session_ids,
    }
}

/// 当 LLM 不可用或解析失败时的确定性 fallback：
/// 按 (project_name, topics[0]) 作为桶 key 分桶。把 project 放进 key 是为了
/// 避免跨项目错聚类——同样讨论"prompts.txt"的两个 session 来自不同项目时
/// 应该是两条独立线索。
pub fn fallback_cluster(sessions: &[(String, SessionSummary)]) -> Vec<ThreadDraft> {
    use std::collections::BTreeMap;
    use crate::locale::PromptLocale;

    let loc = PromptLocale::current();
    let unknown_proj = match loc {
        PromptLocale::Zh => "未知项目",
        PromptLocale::En => "Unknown project",
    };
    let uncategorized = match loc {
        PromptLocale::Zh => "未分类",
        PromptLocale::En => "Uncategorized",
    };

    let mut by_bucket: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();

    for (sid, summary) in sessions {
        let project = summary
            .project_name
            .as_deref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| unknown_proj.to_string());
        let topic = summary
            .topics
            .first()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| uncategorized.to_string());
        by_bucket
            .entry((project, topic))
            .or_default()
            .push(sid.clone());
    }

    by_bucket
        .into_iter()
        .filter(|(_, ids)| ids.len() >= 2)
        .take(MAX_THREADS_PER_RESPONSE)
        .map(|((project, topic), ids)| ThreadDraft {
            name: format!("{} · {}", project, topic),
            summary: String::new(),
            session_ids: ids,
        })
        .collect()
}
