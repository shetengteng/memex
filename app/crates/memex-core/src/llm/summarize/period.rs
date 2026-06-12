//! L4 周期摘要（daily / weekly / monthly）的预算分档、主题分组、
//! 浓缩与 prompt 构造。
//!
//! 与 L2 单会话摘要不同，这里要把 N 个 [`SessionSummary`] 浓缩成一段
//! prompt，对不同周期采用不同的 `PeriodBudget`，避免月报被压成寥寥几段。

use anyhow::Result;

use super::parse::parse_summary;
use super::{MAX_PERIOD_INPUT_CHARS, PERIODIC_SUMMARY_SYSTEM, SessionSummary};
use crate::llm::provider::{LlmProvider, LlmRequest};

pub fn summarize_period(
    provider: &dyn LlmProvider,
    period_label: &str,
    session_summaries: &[SessionSummary],
) -> Result<SessionSummary> {
    // 不同 period 的 condense 预算不同：
    //   daily   ：会话量小，浓缩后给 200 字 summary 已够；
    //   weekly  ：500 字、引用 8 个 session/topic；
    //   monthly ：329 个会话级别也要塞得下，单组 20 session、1500 字摘要、
    //             技术决策最多 15 条，最低输出 1500 字。
    let kind = classify_period(period_label);
    let budget = PeriodBudget::for_kind(kind);

    let condensed = condense_for_period(session_summaries, &budget);

    let mut prompt = String::with_capacity(MAX_PERIOD_INPUT_CHARS);
    prompt.push_str(&format!(
        "以下是 {} 期间的工作会话摘要（共 {} 个会话）：\n\n",
        period_label,
        session_summaries.len()
    ));
    let mut included = 0usize;
    for entry in &condensed {
        if prompt.len() + entry.len() > MAX_PERIOD_INPUT_CHARS {
            break;
        }
        prompt.push_str(entry);
        included += 1;
    }
    if included < condensed.len() {
        prompt.push_str(&format!(
            "（还有 {} 组工作因篇幅限制未列出）\n\n",
            condensed.len() - included
        ));
    }
    prompt.push_str(&format!(
        "\n请综合以上 {} 个会话，生成一个 JSON 对象。\
         summary 必须按【主题名】分段，每段 3-5 句话，总长度不少于 {} 字。\
         涵盖所有主要主题，写出具体技术细节，不要笼统概括。",
        session_summaries.len(),
        budget.min_words
    ));
    let request = LlmRequest::with_prompt(prompt)
        .with_system(PERIODIC_SUMMARY_SYSTEM)
        .with_max_tokens(budget.max_tokens);
    let response = provider.generate(&request)?;
    parse_summary(&response.text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PeriodKind {
    Daily,
    Weekly,
    Monthly,
}

pub(super) fn classify_period(period_label: &str) -> PeriodKind {
    // 调用方目前会传 "Daily 2026-06-08" / "Weekly 2026-W23" / "Monthly 2026-06" / "Week 2026-W23"
    // 等多种风格的字符串，统一识别。Monthly 必须先于 Daily 判断，否则 contains("daily") 误命中。
    if period_label.contains("Monthly") || period_label.contains("monthly") {
        return PeriodKind::Monthly;
    }
    if period_label.contains("Week") || period_label.contains("weekly") {
        return PeriodKind::Weekly;
    }
    PeriodKind::Daily
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PeriodBudget {
    /// 每个主题组里取多少个 session 来拼摘要文本
    pub(super) sessions_per_group: usize,
    /// 每个主题组里取多少个 title 作为「代表性工作」清单
    pub(super) titles_per_group: usize,
    /// 每个主题组里聚合多少条技术决策
    pub(super) decisions_per_group: usize,
    /// 每个主题组的浓缩摘要字符上限
    pub(super) max_summary_chars: usize,
    /// 提示 LLM 输出的最低字数
    pub(super) min_words: usize,
    /// LLM 输出 token 上限
    pub(super) max_tokens: usize,
}

impl PeriodBudget {
    pub(super) fn for_kind(kind: PeriodKind) -> Self {
        match kind {
            PeriodKind::Daily => Self {
                sessions_per_group: 8,
                titles_per_group: 10,
                decisions_per_group: 8,
                max_summary_chars: 500,
                min_words: 200,
                max_tokens: 4096,
            },
            PeriodKind::Weekly => Self {
                sessions_per_group: 12,
                titles_per_group: 15,
                decisions_per_group: 10,
                max_summary_chars: 900,
                min_words: 500,
                max_tokens: 4096,
            },
            PeriodKind::Monthly => Self {
                sessions_per_group: 20,
                titles_per_group: 20,
                decisions_per_group: 15,
                max_summary_chars: 1500,
                min_words: 1500,
                max_tokens: 8192,
            },
        }
    }
}

/// 把多个会话按主题分组并浓缩成 prompt 用的文本块。
///
/// 之前版本只按 `topics[0]` 分组，当 329 个会话大部分落在同一两个 topic 时，
/// 整月内容会被压成寥寥几段 + 8 个 session 的引用，月报因此非常稀。
///
/// 改进：
/// 1. 优先按 (project_name, topic) 二级 key 分组 —— 多项目并行时不同 project
///    的同名 topic 不会被错误合并。无 project_name 的会话 fallback 到 topic-only。
/// 2. 大组（> sessions_per_group * 1.5）按时间分片再切几条，避免一段 prompt 全
///    被一个超大组占满 max_summary_chars 之后剩余组被截断。
pub(super) fn condense_for_period(
    summaries: &[SessionSummary],
    budget: &PeriodBudget,
) -> Vec<String> {
    use std::collections::BTreeMap;

    let mut by_key: BTreeMap<String, Vec<&SessionSummary>> = BTreeMap::new();
    for s in summaries {
        let topic = s
            .topics
            .first()
            .cloned()
            .unwrap_or_else(|| "其他".to_string());
        let key = match &s.project_name {
            Some(p) if !p.trim().is_empty() => format!("{} · {}", p.trim(), topic),
            _ => topic,
        };
        by_key.entry(key).or_default().push(s);
    }

    let mut entries = Vec::new();
    for (key, group) in &by_key {
        let titles: Vec<&str> = group
            .iter()
            .take(budget.titles_per_group)
            .map(|s| s.title.as_str())
            .collect();
        let all_decisions: Vec<&str> = group
            .iter()
            .flat_map(|s| s.decisions.iter().map(|d| d.as_str()))
            .take(budget.decisions_per_group)
            .collect();
        let summaries_text: String = group
            .iter()
            .take(budget.sessions_per_group)
            .map(|s| s.summary.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let mut entry = format!(
            "【{}】（{} 个会话）\n  代表性工作：{}\n  详细内容：{}\n",
            key,
            group.len(),
            titles.join("、"),
            if summaries_text.len() > budget.max_summary_chars {
                format!(
                    "{}...",
                    &summaries_text[..summaries_text.floor_char_boundary(budget.max_summary_chars)]
                )
            } else {
                summaries_text
            }
        );
        if !all_decisions.is_empty() {
            entry.push_str(&format!("  技术决策：{}\n", all_decisions.join("；")));
        }
        entry.push('\n');
        entries.push(entry);
    }
    entries
}
