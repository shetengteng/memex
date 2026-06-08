//! L5「主题线索」LLM 聚类。
//!
//! 输入：一批已经有 L2 摘要的 session 摘要（title / topics / decisions）。
//! 输出：若干条 thread 草稿，每条带名字、一句话主题描述、session_id 列表。
//!
//! 关键设计：
//! - **不让 LLM 输出 session_id**。Session id 是 UUID（如
//!   `42594569-bc2d-4be8-...`），让 LLM 输出 UUID 容易幻觉。改为让它输出
//!   一个 1-based 的"输入序号"，由 Rust 端映射回真实 session_id。
//! - **输出 schema 严格**。失败时 fallback 到"按 topic 关键词桶聚合"的
//!   确定性算法，保证 thread 始终能生成、不让 UI 看空。
//! - **限制输入规模**。一次最多喂 80 个 session 摘要，每条最多 200 字。
//!   超过的话保留最新的 80 个（一年级体量已经足够发现主线索）。

use anyhow::Result;
use serde::Deserialize;

use super::provider::{LlmProvider, LlmRequest};
use super::summarize::SessionSummary;
use crate::storage::db::ThreadDraft;

/// 单次喂给 LLM 的最大 session 数量。超过会被截断。
const MAX_SESSIONS_PER_BATCH: usize = 80;
/// 每条 session 摘要被截断到的长度（中文字符为单位，估算）。
const MAX_SUMMARY_CHARS: usize = 200;
/// 期待 LLM 输出的最大 thread 数（提示词里写明，避免它每个 session 一个 thread）。
const MAX_THREADS_PER_RESPONSE: usize = 12;
/// 单个 thread 的最大 session 容量（避免 LLM 把所有 session 塞一个 thread）。
const MAX_SESSIONS_PER_THREAD: usize = 40;
/// LLM 输出 token 上限。聚类输出 JSON 通常较短，4096 已足够。
const MAX_OUTPUT_TOKENS: usize = 4096;

const THREAD_CLUSTERING_SYSTEM: &str = "\
你是一位资深技术工作流分析师。输入是若干条编程会话的摘要，每条带一个编号。\
你的任务是把它们聚类成若干条「主题线索（Thread）」——同一线索内的会话\
应该围绕同一个高层目标或问题域展开（例如「memex 桌面化迁移」、「cursor 适配器调研」、\
「LLM 摘要 prompt 优化」）。

输出严格合法的 JSON（不带 ```json 标记），结构如下：

{
  \"threads\": [
    {
      \"name\": \"线索名（≤30 字符的简体中文短语，不要带引号）\",
      \"summary\": \"一句话主题描述，≤100 字符\",
      \"session_indices\": [1, 3, 7]
    }
  ]
}

硬性要求：
1. 输出的 threads 数组长度不超过 12 个
2. 每个 thread 的 session_indices 长度不超过 40，且必须是 1-based 整数，对应输入序号
3. 不要让所有 session 挤在一个 thread；单一主题强行拆分也不要——\
   宁可少一些 thread，每个聚焦
4. 同一个 session 可以属于多个 thread（如 'memex 桌面化' + 'Tauri 多窗口'）
5. 偶尔出现的边缘会话可以不归属任何 thread——直接不出现在任何 session_indices 即可
6. 不要复述输入的 session 标题，要抽象出更高层的主题
7. 所有自然语言用简体中文。技术标识保持原样

只输出 JSON，不要解释。";

/// LLM 解析得到的单个 thread 草稿（带 1-based 输入序号）。
#[derive(Debug, Clone, Deserialize)]
struct LlmThread {
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    session_indices: Vec<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmThreadResponse {
    #[serde(default)]
    threads: Vec<LlmThread>,
}

/// 把一批 (session_id, SessionSummary) 喂给 LLM，让它聚类成 thread。
/// 返回值是落库前的 ThreadDraft 列表。失败时返回 Err，调用方应使用 fallback。
pub fn cluster_threads(
    provider: &dyn LlmProvider,
    sessions: &[(String, SessionSummary)],
) -> Result<Vec<ThreadDraft>> {
    if sessions.is_empty() {
        return Ok(Vec::new());
    }

    // 截到 MAX_SESSIONS_PER_BATCH 个最新会话（调用方保证顺序：最新在前）。
    let batch: &[(String, SessionSummary)] = if sessions.len() > MAX_SESSIONS_PER_BATCH {
        &sessions[..MAX_SESSIONS_PER_BATCH]
    } else {
        sessions
    };

    let prompt = build_clustering_prompt(batch);
    let request = LlmRequest::with_prompt(prompt)
        .with_system(THREAD_CLUSTERING_SYSTEM)
        .with_max_tokens(MAX_OUTPUT_TOKENS);
    let response = provider.generate(&request)?;

    let parsed = parse_thread_response(&response.text)?;
    Ok(map_to_drafts(&parsed.threads, batch))
}

/// 当 LLM 不可用或解析失败时的确定性 fallback：
/// 按 SessionSummary.topics 第一项作为桶 key，把 session 分桶。
/// 不会产生跨 session 智能合并（同一主题但 topic 字段不一致的会被分开），
/// 但保证 UI 至少有线索可看。
pub fn fallback_cluster(sessions: &[(String, SessionSummary)]) -> Vec<ThreadDraft> {
    use std::collections::BTreeMap;
    let mut by_topic: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (sid, summary) in sessions {
        let key = summary
            .topics
            .first()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "未分类".to_string());
        by_topic.entry(key).or_default().push(sid.clone());
    }

    // 只保留 session_count >= 2 的桶（单 session 的"线索"没有意义）。
    by_topic
        .into_iter()
        .filter(|(_, ids)| ids.len() >= 2)
        .take(MAX_THREADS_PER_RESPONSE)
        .map(|(name, ids)| ThreadDraft {
            name,
            summary: String::new(),
            session_ids: ids,
        })
        .collect()
}

fn build_clustering_prompt(batch: &[(String, SessionSummary)]) -> String {
    let mut prompt = String::with_capacity(8000);
    prompt.push_str("以下是若干条编程会话的摘要，每条带一个 1-based 编号：\n\n");
    for (i, (_, summary)) in batch.iter().enumerate() {
        let n = i + 1;
        let title = if summary.title.is_empty() {
            "(无标题)"
        } else {
            &summary.title
        };
        let body = if summary.summary.chars().count() > MAX_SUMMARY_CHARS {
            // chars().take + collect 是处理多字节安全的截断
            let s: String = summary.summary.chars().take(MAX_SUMMARY_CHARS).collect();
            format!("{}…", s)
        } else {
            summary.summary.clone()
        };
        let topics = if summary.topics.is_empty() {
            String::new()
        } else {
            format!("（主题：{}）", summary.topics.join("、"))
        };
        prompt.push_str(&format!("[{}] {}{}\n  {}\n\n", n, title, topics, body));
    }
    prompt.push_str("请把上面的会话聚类成主题线索，按要求输出 JSON。");
    prompt
}

fn parse_thread_response(text: &str) -> Result<LlmThreadResponse> {
    let cleaned = strip_code_fences(text);
    let parsed = serde_json::from_str::<LlmThreadResponse>(&cleaned)
        .map_err(|e| anyhow::anyhow!("parse thread JSON failed: {e}; raw: {}", &cleaned[..cleaned.len().min(200)]))?;
    Ok(parsed)
}

fn strip_code_fences(text: &str) -> String {
    let t = text.trim();
    // 去掉 ```json ... ``` 包装
    if let Some(stripped) = t.strip_prefix("```json") {
        return stripped.trim_end_matches("```").trim().to_string();
    }
    if let Some(stripped) = t.strip_prefix("```") {
        return stripped.trim_end_matches("```").trim().to_string();
    }
    t.to_string()
}

fn map_to_drafts(
    llm_threads: &[LlmThread],
    batch: &[(String, SessionSummary)],
) -> Vec<ThreadDraft> {
    let mut drafts = Vec::with_capacity(llm_threads.len());
    for t in llm_threads.iter().take(MAX_THREADS_PER_RESPONSE) {
        let name = t.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let mut session_ids: Vec<String> = t
            .session_indices
            .iter()
            // 1-based → 0-based，跳过越界
            .filter_map(|&i| {
                if i == 0 || i > batch.len() {
                    None
                } else {
                    Some(batch[i - 1].0.clone())
                }
            })
            .collect();
        // 去重（LLM 偶尔会把同一序号写两次）
        session_ids.sort();
        session_ids.dedup();
        if session_ids.is_empty() {
            continue;
        }
        if session_ids.len() > MAX_SESSIONS_PER_THREAD {
            session_ids.truncate(MAX_SESSIONS_PER_THREAD);
        }
        drafts.push(ThreadDraft {
            name,
            summary: t.summary.trim().to_string(),
            session_ids,
        });
    }
    drafts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(title: &str, summary: &str, topics: &[&str]) -> SessionSummary {
        SessionSummary {
            title: title.into(),
            summary: summary.into(),
            topics: topics.iter().map(|t| (*t).into()).collect(),
            decisions: vec![],
            project_name: None,
            intent: None,
        }
    }

    #[test]
    fn parse_well_formed_json_into_drafts() {
        let batch = vec![
            ("s1".into(), s("a", "x", &["t"])),
            ("s2".into(), s("b", "y", &["t"])),
            ("s3".into(), s("c", "z", &["u"])),
        ];
        let llm_out = r#"{
            "threads": [
              {"name": "桌面化", "summary": "整体迁移", "session_indices": [1, 2]},
              {"name": "其他", "summary": "杂项", "session_indices": [3]}
            ]
        }"#;
        let parsed = parse_thread_response(llm_out).unwrap();
        let drafts = map_to_drafts(&parsed.threads, &batch);
        assert_eq!(drafts.len(), 2);
        assert_eq!(drafts[0].name, "桌面化");
        assert_eq!(drafts[0].session_ids, vec!["s1", "s2"]);
        assert_eq!(drafts[1].name, "其他");
        assert_eq!(drafts[1].session_ids, vec!["s3"]);
    }

    #[test]
    fn out_of_range_indices_are_dropped() {
        let batch = vec![("s1".into(), s("a", "x", &[]))];
        let llm_out =
            r#"{"threads":[{"name":"x","summary":"","session_indices":[0, 1, 2, 99]}]}"#;
        let parsed = parse_thread_response(llm_out).unwrap();
        let drafts = map_to_drafts(&parsed.threads, &batch);
        // 0 和 2/99 越界都被丢，只留 1 → s1
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].session_ids, vec!["s1"]);
    }

    #[test]
    fn duplicate_indices_are_deduplicated() {
        let batch = vec![("s1".into(), s("a", "x", &[])), ("s2".into(), s("b", "y", &[]))];
        let llm_out =
            r#"{"threads":[{"name":"x","summary":"","session_indices":[1, 1, 2, 2]}]}"#;
        let parsed = parse_thread_response(llm_out).unwrap();
        let drafts = map_to_drafts(&parsed.threads, &batch);
        assert_eq!(drafts[0].session_ids, vec!["s1", "s2"]);
    }

    #[test]
    fn empty_threads_array_returns_no_drafts() {
        let llm_out = r#"{"threads":[]}"#;
        let parsed = parse_thread_response(llm_out).unwrap();
        let drafts = map_to_drafts(&parsed.threads, &[]);
        assert!(drafts.is_empty());
    }

    #[test]
    fn code_fence_wrapping_is_stripped() {
        let llm_out = "```json\n{\"threads\":[]}\n```";
        let parsed = parse_thread_response(llm_out).unwrap();
        assert_eq!(parsed.threads.len(), 0);
    }

    #[test]
    fn threads_with_empty_name_are_filtered() {
        let batch = vec![("s1".into(), s("a", "x", &[]))];
        let llm_out =
            r#"{"threads":[{"name":"  ","summary":"","session_indices":[1]},
                             {"name":"x","summary":"","session_indices":[1]}]}"#;
        let parsed = parse_thread_response(llm_out).unwrap();
        let drafts = map_to_drafts(&parsed.threads, &batch);
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].name, "x");
    }

    #[test]
    fn fallback_clusters_by_topic_minimum_two_sessions() {
        let sessions = vec![
            ("s1".into(), s("a", "x", &["桌面化"])),
            ("s2".into(), s("b", "y", &["桌面化"])),
            ("s3".into(), s("c", "z", &["独立主题"])),
        ];
        let drafts = fallback_cluster(&sessions);
        // 只有"桌面化"有 2 个 session，独立主题只有 1 个被过滤掉
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].name, "桌面化");
        assert_eq!(drafts[0].session_ids.len(), 2);
    }

    #[test]
    fn fallback_handles_empty_topics_as_uncategorized() {
        let sessions = vec![
            ("s1".into(), s("a", "x", &[])),
            ("s2".into(), s("b", "y", &[])),
        ];
        let drafts = fallback_cluster(&sessions);
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].name, "未分类");
    }
}
