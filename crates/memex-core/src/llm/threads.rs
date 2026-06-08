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
你是一位资深技术工作流分析师。输入是若干条编程会话的摘要，每条带一个编号\
和所属项目名。你的任务是把它们聚类成若干条「主题线索（Thread）」——同一线索内\
的会话应该围绕同一个高层目标或问题域展开（例如「memex 桌面化迁移」、\
「cursor 适配器调研」、「LLM 摘要 prompt 优化」）。

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
3. **不同 project 的 session 默认不能聚到同一个 thread**。只有当两个项目\
   讨论的是完全相同的主题（如同一个跨项目的库重构）时才可以跨项目合并；\
   仅仅「都讨论了 prompts.txt」、「都在讨论 markdown 渲染」这种弱相关不算\
4. 不要让所有 session 挤在一个 thread；单一主题强行拆分也不要——\
   宁可少一些 thread，每个聚焦
5. 同一个 session 可以属于多个 thread（如 'memex 桌面化' + 'Tauri 多窗口'）
6. 偶尔出现的边缘会话可以不归属任何 thread——直接不出现在任何 session_indices 即可
7. 不要复述输入的 session 标题，要抽象出更高层的主题
8. 所有自然语言用简体中文。技术标识保持原样

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

/// LLM 解析得到的"按关键词命题搜索"返回结构。
#[derive(Debug, Clone, Deserialize)]
struct LlmQueryThread {
    #[serde(default)]
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    session_indices: Vec<usize>,
}

const QUERY_THREAD_SYSTEM: &str = "\
你是一位资深技术工作流分析师。用户给你一个关键词或主题描述，并给你一批最近的\
编程会话摘要（每条带编号、所属项目、标题、主题、正文）。请挑选出**与用户关键词\
确实相关**的会话，组成一条「主题线索（Thread）」。

输出严格合法的 JSON（不带 ```json 标记）：

{
  \"name\": \"线索名（≤30 字符的简体中文短语，应能体现关键词）\",
  \"summary\": \"一句话主题描述，≤100 字符\",
  \"session_indices\": [1, 3, 7]
}

硬性要求：
1. session_indices 必须是 1-based 整数，对应输入序号
2. 宁缺毋滥：只有真正讨论该关键词的会话才入选；标题/正文里仅仅\
   提到一句无关上下文的会话不算
3. 如果没有任何会话相关，输出 session_indices=[] 并把 name 设为关键词原文
4. 不要复述输入的 session 标题，要抽象出更高层的主题
5. 所有自然语言用简体中文。技术标识保持原样

只输出 JSON，不要解释。";

/// 按用户关键词，让 LLM 从一批 session 摘要里挑出相关 session。
/// 失败时调用方应使用 fallback：在标题/主题/摘要字面包含关键词。
pub fn query_threads_by_keyword(
    provider: &dyn LlmProvider,
    sessions: &[(String, SessionSummary)],
    query: &str,
) -> Result<ThreadDraft> {
    if sessions.is_empty() {
        return Ok(ThreadDraft {
            name: query.to_string(),
            summary: String::new(),
            session_ids: vec![],
        });
    }
    let batch: &[(String, SessionSummary)] = if sessions.len() > MAX_SESSIONS_PER_BATCH {
        &sessions[..MAX_SESSIONS_PER_BATCH]
    } else {
        sessions
    };

    let mut prompt = String::with_capacity(8000);
    prompt.push_str(&format!("用户关键词：{}\n\n", query));
    prompt.push_str("以下是若干条编程会话的摘要，每条带 1-based 编号和所属项目：\n\n");
    for (i, (_, summary)) in batch.iter().enumerate() {
        let n = i + 1;
        let title = if summary.title.is_empty() {
            "(无标题)"
        } else {
            &summary.title
        };
        let body = if summary.summary.chars().count() > MAX_SUMMARY_CHARS {
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
        let project = summary
            .project_name
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .unwrap_or("(未知项目)");
        prompt.push_str(&format!(
            "[{}] project={} | {}{}\n  {}\n\n",
            n, project, title, topics, body,
        ));
    }
    prompt.push_str("请按要求挑选与关键词相关的会话，输出 JSON。");

    let request = LlmRequest::with_prompt(prompt)
        .with_system(QUERY_THREAD_SYSTEM)
        .with_max_tokens(MAX_OUTPUT_TOKENS);
    let response = provider.generate(&request)?;
    let cleaned = strip_code_fences(&response.text);
    let parsed: LlmQueryThread = serde_json::from_str(&cleaned).map_err(|e| {
        anyhow::anyhow!(
            "parse query thread JSON failed: {e}; raw: {}",
            &cleaned[..cleaned.len().min(200)]
        )
    })?;

    // 把 1-based session_indices 映射到真实 session_id，去重并丢掉越界的。
    let mut seen = std::collections::HashSet::new();
    let mut session_ids = Vec::new();
    for idx in parsed.session_indices {
        if idx == 0 || idx > batch.len() {
            continue;
        }
        let sid = batch[idx - 1].0.clone();
        if seen.insert(sid.clone()) {
            session_ids.push(sid);
        }
    }

    let name = if parsed.name.trim().is_empty() {
        query.to_string()
    } else {
        parsed.name.trim().to_string()
    };

    Ok(ThreadDraft {
        name,
        summary: parsed.summary.trim().to_string(),
        session_ids,
    })
}

/// 关键词检索的确定性 fallback：在 title / topics / summary 里做大小写不敏感的
/// 字面子串匹配。比 LLM 弱很多但永远可用。
pub fn fallback_query_match(
    sessions: &[(String, SessionSummary)],
    query: &str,
) -> ThreadDraft {
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
            || s
                .decisions
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
    let mut by_bucket: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();

    for (sid, summary) in sessions {
        let project = summary
            .project_name
            .as_deref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "未知项目".to_string());
        let topic = summary
            .topics
            .first()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "未分类".to_string());
        by_bucket.entry((project, topic)).or_default().push(sid.clone());
    }

    // 只保留 session_count >= 2 的桶（单 session 的"线索"没有意义）。
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

fn build_clustering_prompt(batch: &[(String, SessionSummary)]) -> String {
    let mut prompt = String::with_capacity(8000);
    prompt.push_str("以下是若干条编程会话的摘要，每条带一个 1-based 编号和所属项目：\n\n");
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
        // 把 project_name 单独高亮一行，让 LLM 第一眼就能看到归属信号。
        let project = summary
            .project_name
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .unwrap_or("(未知项目)");
        prompt.push_str(&format!(
            "[{}] project={} | {}{}\n  {}\n\n",
            n, project, title, topics, body,
        ));
    }
    prompt.push_str(
        "请把上面的会话聚类成主题线索，按要求输出 JSON。\
         记住：不同 project 的 session 默认不能聚到同一个 thread。",
    );
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

    fn sp(title: &str, summary: &str, topics: &[&str], project: &str) -> SessionSummary {
        SessionSummary {
            title: title.into(),
            summary: summary.into(),
            topics: topics.iter().map(|t| (*t).into()).collect(),
            decisions: vec![],
            project_name: Some(project.into()),
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
        // project_name 缺失时桶 key 是 (未知项目, 桌面化)
        assert_eq!(drafts[0].name, "未知项目 · 桌面化");
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
        assert_eq!(drafts[0].name, "未知项目 · 未分类");
    }

    /// 同样的 topic 来自不同项目，fallback 必须分成两条独立线索
    /// （这是用户反馈的 tt-qimen 误聚类问题的回归用例）。
    #[test]
    fn fallback_does_not_merge_across_projects() {
        let sessions = vec![
            ("s1".into(), sp("a", "x", &["prompts.txt"], "memex")),
            ("s2".into(), sp("b", "y", &["prompts.txt"], "memex")),
            ("s3".into(), sp("c", "z", &["prompts.txt"], "tt-qimen")),
            ("s4".into(), sp("d", "w", &["prompts.txt"], "tt-qimen")),
        ];
        let drafts = fallback_cluster(&sessions);
        // 两个项目，每个 2 session，应该产生 2 条线索
        assert_eq!(drafts.len(), 2);
        let names: Vec<_> = drafts.iter().map(|d| d.name.clone()).collect();
        assert!(names.iter().any(|n| n.contains("memex")));
        assert!(names.iter().any(|n| n.contains("tt-qimen")));
        // memex 线索只包含 memex 的 session
        let memex_draft = drafts.iter().find(|d| d.name.contains("memex")).unwrap();
        assert_eq!(memex_draft.session_ids, vec!["s1".to_string(), "s2".to_string()]);
    }

    #[test]
    fn build_prompt_includes_project_signal() {
        let batch = vec![
            ("s1".into(), sp("写文档", "做文档相关工作", &["docs"], "memex")),
            ("s2".into(), sp("跑命盘", "排八字", &["命理"], "tt-qimen")),
        ];
        let prompt = build_clustering_prompt(&batch);
        assert!(prompt.contains("project=memex"), "应包含 project=memex 信号:\n{}", prompt);
        assert!(prompt.contains("project=tt-qimen"), "应包含 project=tt-qimen 信号:\n{}", prompt);
    }

    /// 关键词字面 fallback：在 title / topics / summary / decisions 任一命中即收录。
    #[test]
    fn fallback_query_match_hits_title_topics_summary_decisions() {
        let mut a = sp("修 Tauri 多窗口", "无关内容", &[], "memex");
        let mut b = sp("写 Markdown", "讨论 Tauri 事件循环", &[], "memex");
        let mut c = sp("修 bug", "不沾边", &["tauri"], "memex");
        let mut d = sp("改 schema", "不沾边", &[], "memex");
        d.decisions = vec!["Tauri 升级 v2".into()];
        // 完全不相关的 e 不应入选
        let e = sp("命理预测", "不沾边", &[], "tt-qimen");
        b.title = b.title.clone();
        a.title = a.title.clone();
        c.title = c.title.clone();
        let sessions = vec![
            ("s_a".into(), a),
            ("s_b".into(), b),
            ("s_c".into(), c),
            ("s_d".into(), d),
            ("s_e".into(), e),
        ];
        let draft = fallback_query_match(&sessions, "Tauri");
        assert_eq!(draft.name, "Tauri");
        assert_eq!(draft.session_ids.len(), 4);
        assert!(!draft.session_ids.contains(&"s_e".to_string()));
    }

    #[test]
    fn fallback_query_match_empty_query_returns_empty() {
        let sessions = vec![("s1".into(), s("a", "x", &["topic"]))];
        let draft = fallback_query_match(&sessions, "   ");
        assert_eq!(draft.session_ids.len(), 0);
    }
}
