//! Reflect — 在 L4 之上的 L5 反思层。
//!
//! 与 daily/weekly summary 的本质区别：
//!   - daily/weekly 是事实流水，回答"做了什么"
//!   - reflect 是 meta 层，回答"我在反复琢磨什么 / 哪些事情卡住了"
//!
//! 输入：一段时间内的 daily/weekly 摘要列表（已经是聚合过的 L4，不要直接吃 L2，太碎）
//! 输出：3 段 markdown
//!   ## Shipped     — 真正交付的产出（合并、上线、关闭的事项）
//!   ## Patterns    — 反复出现的工作主题或方法论
//!   ## Open Loops  — 未闭合的事项：提出了想做的事但还没继续 / 卡在某个点
//!
//! Prompt 写成"分析师"角色而不是"摘要器"，要求基于证据列举，避免空话。

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::llm::provider::{LlmProvider, LlmRequest};

const MAX_INPUT_CHARS: usize = 12_000;

const REFLECTION_SYSTEM: &str = "\
你是一名资深工程师的工作反思助手。输入是一段时间内的工作日报/周报摘要，\
请站在用户视角，做一次**反思级别**的回顾，而不是简单的摘要复述。\n\
输出 JSON，三个字段都必须有：\n\
- shipped: 字符串数组，列出这段时间真正交付的具体产出（功能、修复、文档、决策落地）。\
每条以动词开头，引用具体内容。最多 6 条。如果没有可靠依据就给空数组。\n\
- patterns: 字符串数组，列出反复出现的工作主题、技术栈、解决问题的方法论或卡点类型。\
每条 1-2 句话，要有洞察价值（即不是简单罗列 topics）。最多 5 条。\n\
- open_loops: 字符串数组，列出未闭合的事项：提到过但没下文的想法、TODO、决定不了的方向、\
反复出现的疑问。每条具体到事项。最多 5 条。\n\
\n\
严格只输出 JSON，不要加 markdown 围栏，不要前言后语。\
";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionOutput {
    #[serde(default)]
    pub shipped: Vec<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub open_loops: Vec<String>,
}

impl ReflectionOutput {
    /// 把三段塞进一份 markdown 字符串，方便存到 aggregate_summaries.summary 字段
    /// 与写入 reports/*.md 文件。
    pub fn to_markdown(&self, period_label: &str) -> String {
        let mut out = String::with_capacity(1024);
        out.push_str(&format!("# Reflect — {period_label}\n\n"));

        out.push_str("## Shipped\n\n");
        if self.shipped.is_empty() {
            out.push_str("_这段时间没有识别到明显的交付动作。_\n\n");
        } else {
            for item in &self.shipped {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        out.push_str("## Patterns\n\n");
        if self.patterns.is_empty() {
            out.push_str("_暂未识别到值得反思的反复模式。_\n\n");
        } else {
            for item in &self.patterns {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        out.push_str("## Open Loops\n\n");
        if self.open_loops.is_empty() {
            out.push_str("_暂未识别到未闭合的事项。_\n\n");
        } else {
            for item in &self.open_loops {
                out.push_str(&format!("- {item}\n"));
            }
            out.push('\n');
        }

        out
    }
}

/// 单个 daily/weekly 输入条目。从 `aggregate_summaries` 行里抽出来。
#[derive(Debug, Clone)]
pub struct PeriodDigest {
    /// 例如 "2026-06-01" 或 "2026-W22"
    pub scope_key: String,
    /// daily/weekly 摘要的 title
    pub title: String,
    /// daily/weekly 的 summary 正文
    pub summary: String,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
}

/// 生成 reflection。`period_label` 例：`"Week 2026-W23"` 或 `"Last 30 days"`。
pub fn generate_reflection(
    provider: &dyn LlmProvider,
    period_label: &str,
    digests: &[PeriodDigest],
) -> Result<ReflectionOutput> {
    let mut prompt = String::with_capacity(MAX_INPUT_CHARS);
    prompt.push_str(&format!(
        "以下是 {} 期间的工作日报/周报摘要（按时间正序）：\n\n",
        period_label
    ));

    for (i, d) in digests.iter().enumerate() {
        let mut entry = format!("【{} · {}】{}\n", i + 1, d.scope_key, d.title);
        entry.push_str(&format!("  摘要：{}\n", d.summary));
        if !d.topics.is_empty() {
            entry.push_str(&format!("  主题：{}\n", d.topics.join("、")));
        }
        if !d.decisions.is_empty() {
            entry.push_str(&format!("  决策：{}\n", d.decisions.join("；")));
        }
        entry.push('\n');

        if prompt.len() + entry.len() > MAX_INPUT_CHARS {
            prompt.push_str("…（更早的部分因篇幅省略）\n");
            break;
        }
        prompt.push_str(&entry);
    }

    prompt.push_str(
        "\n请基于上述材料做一次反思级别的回顾，按 system 描述输出 JSON。",
    );

    let request = LlmRequest::with_prompt(prompt).with_system(REFLECTION_SYSTEM);
    let response = provider.generate(&request)?;
    parse_reflection(&response.text)
}

fn parse_reflection(text: &str) -> Result<ReflectionOutput> {
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // 直接 try parse
    if let Ok(r) = serde_json::from_str::<ReflectionOutput>(cleaned) {
        return Ok(r);
    }

    // 容错：LLM 可能在 JSON 外加了寒暄。截取第一个 `{` 到最后一个 `}` 再尝试。
    if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if end > start {
            if let Ok(r) = serde_json::from_str::<ReflectionOutput>(&cleaned[start..=end]) {
                return Ok(r);
            }
        }
    }

    // 最终兜底：把整段文本塞进 patterns，保证不丢失信息
    Ok(ReflectionOutput {
        shipped: Vec::new(),
        patterns: vec![format!("LLM 返回未能解析为结构化 JSON：{}", text.chars().take(300).collect::<String>())],
        open_loops: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider::LlmResponse;

    struct FixedResponseProvider {
        response: String,
    }
    impl LlmProvider for FixedResponseProvider {
        fn name(&self) -> &str {
            "fixed"
        }
        fn is_available(&self) -> bool {
            true
        }
        fn generate(&self, _req: &LlmRequest) -> anyhow::Result<LlmResponse> {
            Ok(LlmResponse {
                text: self.response.clone(),
                model: "test".into(),
                tokens_used: 0,
            })
        }
    }

    #[test]
    fn reflection_to_markdown_three_sections_present() {
        let r = ReflectionOutput {
            shipped: vec!["上线 Doctor GUI".into(), "修复 SessionsTab 换行".into()],
            patterns: vec!["反复在 Tauri 命令注册上踩坑".into()],
            open_loops: vec!["TARS Radar 模块是否要做".into()],
        };
        let md = r.to_markdown("Week 2026-W23");
        assert!(md.contains("# Reflect — Week 2026-W23"));
        assert!(md.contains("## Shipped"));
        assert!(md.contains("- 上线 Doctor GUI"));
        assert!(md.contains("## Patterns"));
        assert!(md.contains("## Open Loops"));
        assert!(md.contains("TARS Radar"));
    }

    #[test]
    fn reflection_empty_sections_have_placeholder_text() {
        let r = ReflectionOutput::default();
        let md = r.to_markdown("Empty");
        assert!(md.contains("没有识别到明显的交付动作"));
        assert!(md.contains("暂未识别到值得反思的反复模式"));
        assert!(md.contains("暂未识别到未闭合的事项"));
    }

    impl Default for ReflectionOutput {
        fn default() -> Self {
            ReflectionOutput {
                shipped: Vec::new(),
                patterns: Vec::new(),
                open_loops: Vec::new(),
            }
        }
    }

    #[test]
    fn generate_reflection_parses_clean_json() {
        let json = r#"{"shipped":["ship a"],"patterns":["pattern b"],"open_loops":["loop c"]}"#;
        let provider = FixedResponseProvider {
            response: json.into(),
        };
        let digests = vec![PeriodDigest {
            scope_key: "2026-06-01".into(),
            title: "Daily".into(),
            summary: "x".into(),
            topics: vec![],
            decisions: vec![],
        }];
        let r = generate_reflection(&provider, "Test", &digests).unwrap();
        assert_eq!(r.shipped, vec!["ship a".to_string()]);
        assert_eq!(r.patterns, vec!["pattern b".to_string()]);
        assert_eq!(r.open_loops, vec!["loop c".to_string()]);
    }

    #[test]
    fn generate_reflection_handles_markdown_fence() {
        let provider = FixedResponseProvider {
            response: "```json\n{\"shipped\":[\"a\"],\"patterns\":[],\"open_loops\":[]}\n```"
                .into(),
        };
        let digests = vec![PeriodDigest {
            scope_key: "x".into(),
            title: "x".into(),
            summary: "x".into(),
            topics: vec![],
            decisions: vec![],
        }];
        let r = generate_reflection(&provider, "Test", &digests).unwrap();
        assert_eq!(r.shipped, vec!["a".to_string()]);
    }

    #[test]
    fn generate_reflection_falls_back_when_unparseable() {
        let provider = FixedResponseProvider {
            response: "totally not json".into(),
        };
        let digests = vec![PeriodDigest {
            scope_key: "x".into(),
            title: "x".into(),
            summary: "x".into(),
            topics: vec![],
            decisions: vec![],
        }];
        let r = generate_reflection(&provider, "Test", &digests).unwrap();
        // patterns 兜底会装入原文片段
        assert_eq!(r.shipped.len(), 0);
        assert_eq!(r.patterns.len(), 1);
        assert!(r.patterns[0].contains("totally not json"));
    }
}
