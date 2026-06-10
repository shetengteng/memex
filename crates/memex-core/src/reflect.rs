//! Reflect 模块编排：
//!
//! 1. 把用户传入的 `--period` 解析成 (scope_keys, period_label) 列表
//!    - `week`  → 当前自然周内的所有 daily summaries (周一到今天，UTC)
//!    - `month` → 过去 30 个自然日的 daily summaries
//!    - `Nd`    → 过去 N 天的 daily summaries
//! 2. 从 `aggregate_summaries` 拉 daily/weekly 行 → PeriodDigest 列表
//! 3. 调 `llm::reflect::generate_reflection` 生成 ReflectionOutput
//! 4. 存回 `aggregate_summaries`（scope_type='reflect'，scope_key 形如 `week:2026-W23`）
//!    + 同时把 markdown 写到 `~/.memex/reports/reflect-<key>.md`
//!
//! 不持久化任何额外状态，依赖现有 daily summaries。

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use chrono::{Datelike, Duration, NaiveDate, Utc};

use crate::llm::provider::LlmProvider;
use crate::llm::reflect::{PeriodDigest, ReflectionOutput, generate_reflection};
use crate::storage::db::Db;

/// 用户在 CLI 上选择的周期，解析后得到一组要拉取的 daily/weekly scope_key
/// 和写回的反思 scope_key。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReflectPeriod {
    /// 本周（ISO 周：周一到周日）。回看到本周一为止。
    Week,
    /// 过去 30 天
    Month,
    /// 过去 N 天
    Days(u32),
}

impl ReflectPeriod {
    /// 解析 CLI 入参。支持 `week` / `month` / `Nd` / `N`（纯数字也当 days 处理）。
    pub fn parse(input: &str) -> Result<ReflectPeriod> {
        let s = input.trim().to_ascii_lowercase();
        if s == "week" {
            return Ok(ReflectPeriod::Week);
        }
        if s == "month" {
            return Ok(ReflectPeriod::Month);
        }
        // 1d / 7d / 30d 或 纯数字
        let n_str = s.strip_suffix('d').unwrap_or(&s);
        if let Ok(n) = n_str.parse::<u32>() {
            if n == 0 {
                return Err(anyhow!("period 天数必须 ≥ 1"));
            }
            return Ok(ReflectPeriod::Days(n));
        }
        Err(anyhow!(
            "无法解析 period {:?}（支持 week / month / Nd / N）",
            input
        ))
    }

    /// 给 UI 看的标签，例如 "Week 2026-W23" / "Last 30 days"。
    /// 内含当前日期上下文，便于 reflect 内容追溯。
    pub fn label(&self, today: NaiveDate) -> String {
        match self {
            ReflectPeriod::Week => format!(
                "Week {}-W{:02}",
                today.iso_week().year(),
                today.iso_week().week()
            ),
            ReflectPeriod::Month => format!("Last 30 days ending {}", today),
            ReflectPeriod::Days(n) => format!("Last {n} days ending {today}"),
        }
    }

    /// 写回 aggregate_summaries.scope_key 的稳定值。
    /// 同周期内多次运行只更新同一行（依赖 upsert）。
    pub fn scope_key(&self, today: NaiveDate) -> String {
        match self {
            ReflectPeriod::Week => format!(
                "week:{}-W{:02}",
                today.iso_week().year(),
                today.iso_week().week()
            ),
            ReflectPeriod::Month => format!("month:{}", today.format("%Y-%m")),
            ReflectPeriod::Days(n) => format!("days{n}:{today}"),
        }
    }

    /// 返回当前周期覆盖的日期范围 (start_inclusive, end_inclusive)。
    pub fn date_range(&self, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        match self {
            ReflectPeriod::Week => {
                let week_start =
                    today - Duration::days(today.weekday().num_days_from_monday() as i64);
                (week_start, today)
            }
            ReflectPeriod::Month => (today - Duration::days(29), today),
            ReflectPeriod::Days(n) => (today - Duration::days((*n - 1) as i64), today),
        }
    }
}

/// 从 DB 拉出周期内的 daily 聚合摘要，转成 PeriodDigest 列表。
/// 没有 LLM 摘要的日子不会出现在结果里（这是预期行为 — 那一天没工作就跳过）。
pub fn collect_digests_for_period(
    db: &Db,
    period: &ReflectPeriod,
    today: NaiveDate,
) -> Result<Vec<PeriodDigest>> {
    let (start, end) = period.date_range(today);
    let mut digests = Vec::new();
    let mut d = start;
    while d <= end {
        let scope_key = format!("daily:{}", d);
        if let Ok(Some(row)) = db.get_aggregate_summary("daily", &scope_key) {
            digests.push(PeriodDigest {
                scope_key: d.to_string(),
                title: row.title.unwrap_or_else(|| format!("Daily {}", d)),
                summary: row.summary,
                topics: row.topics,
                decisions: row.decisions,
            });
        }
        d += Duration::days(1);
    }
    Ok(digests)
}

#[derive(Debug, Clone)]
pub struct ReflectArtifacts {
    pub period_label: String,
    pub scope_key: String,
    pub digest_count: usize,
    pub output: ReflectionOutput,
    pub markdown: String,
    pub markdown_path: Option<std::path::PathBuf>,
}

/// 一次完整的 reflect 流程：取 digest → LLM → 存回 DB → 落地 markdown 文件。
///
/// `memex_dir` 用于决定 markdown 输出位置（`~/.memex/reports/reflect-*.md`）。
/// 传 None 则跳过文件落地（用于测试或 daemon 自动触发）。
pub fn run_reflect(
    db: &Db,
    provider: &dyn LlmProvider,
    period: ReflectPeriod,
    today: NaiveDate,
    memex_dir: Option<&Path>,
) -> Result<ReflectArtifacts> {
    let period_label = period.label(today);
    let scope_key = period.scope_key(today);

    let digests = collect_digests_for_period(db, &period, today)?;
    if digests.is_empty() {
        return Err(anyhow!(
            "没有可用的 daily 摘要支撑反思（周期：{}）。\n\
            提示：reflect 依赖现有的 daily summaries。先运行 `memex summarize` 或等 daemon 自动生成。",
            period_label
        ));
    }

    let output = generate_reflection(provider, &period_label, &digests)?;
    let markdown = output.to_markdown(&period_label);

    // 1. 存进 aggregate_summaries（scope_type='reflect'）
    //    summary 直接塞 markdown 整体，topics 借用 patterns，decisions 借用 open_loops
    //    （reflect 内部的"决策"语义就是"未闭合事项"，借用 decisions 字段不至于歧义）
    db.upsert_aggregate_summary(crate::storage::db::AggregateSummaryUpsert {
        scope_type: "reflect",
        scope_key: &scope_key,
        title: Some(&period_label),
        summary: &markdown,
        topics: &output.patterns,
        decisions: &output.open_loops,
        session_count: digests.len() as i64,
    })?;

    // 2. 落地 markdown
    let mut path = None;
    if let Some(dir) = memex_dir {
        let reports_dir = dir.join("reports");
        std::fs::create_dir_all(&reports_dir).context("创建 reports/ 失败")?;
        let safe_key = scope_key.replace(':', "-");
        let file_path = reports_dir.join(format!("reflect-{}.md", safe_key));
        std::fs::write(&file_path, &markdown).context("写入 reflect markdown 失败")?;
        path = Some(file_path);
    }

    Ok(ReflectArtifacts {
        period_label,
        scope_key,
        digest_count: digests.len(),
        output,
        markdown,
        markdown_path: path,
    })
}

// 让 Utc::now() 之类的代码在外部可用；CLI 那边会拿当天日期传进来。
pub fn today_utc() -> NaiveDate {
    Utc::now().date_naive()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn parse_period_accepts_week_month_days() {
        assert_eq!(ReflectPeriod::parse("week").unwrap(), ReflectPeriod::Week);
        assert_eq!(ReflectPeriod::parse("WEEK").unwrap(), ReflectPeriod::Week);
        assert_eq!(ReflectPeriod::parse("month").unwrap(), ReflectPeriod::Month);
        assert_eq!(ReflectPeriod::parse("7d").unwrap(), ReflectPeriod::Days(7));
        assert_eq!(ReflectPeriod::parse("14").unwrap(), ReflectPeriod::Days(14));
    }

    #[test]
    fn parse_period_rejects_garbage_and_zero() {
        assert!(ReflectPeriod::parse("yearly").is_err());
        assert!(ReflectPeriod::parse("").is_err());
        assert!(ReflectPeriod::parse("0d").is_err());
    }

    #[test]
    fn date_range_week_starts_on_monday() {
        // 2026-06-04 是周四，本周一应该是 2026-06-01
        let today = ymd(2026, 6, 4);
        let (start, end) = ReflectPeriod::Week.date_range(today);
        assert_eq!(start, ymd(2026, 6, 1));
        assert_eq!(end, today);
    }

    #[test]
    fn date_range_month_is_30_days_inclusive() {
        let today = ymd(2026, 6, 4);
        let (start, end) = ReflectPeriod::Month.date_range(today);
        assert_eq!(end, today);
        // 30 天 = (end - start).num_days() = 29，因为是闭区间
        assert_eq!((end - start).num_days(), 29);
    }

    #[test]
    fn date_range_days_n_is_n_days_inclusive() {
        let today = ymd(2026, 6, 4);
        let (start, end) = ReflectPeriod::Days(7).date_range(today);
        assert_eq!(end, today);
        assert_eq!((end - start).num_days(), 6);
    }

    #[test]
    fn scope_key_is_stable_for_same_day() {
        let today = ymd(2026, 6, 4);
        assert_eq!(ReflectPeriod::Week.scope_key(today), "week:2026-W23");
        assert_eq!(ReflectPeriod::Month.scope_key(today), "month:2026-06");
        assert_eq!(ReflectPeriod::Days(7).scope_key(today), "days7:2026-06-04");
    }

    #[test]
    fn label_includes_today_for_traceability() {
        let today = ymd(2026, 6, 4);
        let week_label = ReflectPeriod::Week.label(today);
        assert!(week_label.contains("Week"));
        let month_label = ReflectPeriod::Month.label(today);
        assert!(month_label.contains("Last 30 days"));
        assert!(month_label.contains("2026-06-04"));
    }
}
