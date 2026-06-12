//! Serializable DTOs returned by query helpers in this module.
//!
//! Keep these in one file so changing the IPC contract is a single
//! pull request; the surrounding sibling modules (`doctor`, `stats`,
//! `workload`) re-export the ones they need via `super::*`.

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub db_exists: bool,
    pub schema_version: Option<u32>,
    pub session_count: u64,
    pub message_count: u64,
    pub chunk_count: u64,
    pub source_count: u64,
    pub fts_ok: bool,
    pub adapters: Vec<AdapterStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterStatus {
    pub name: String,
    pub file_count: u64,
    pub last_scan: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineEntry {
    pub date: String,
    pub adapter: String,
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsBreakdown {
    pub by_adapter: BTreeMap<String, i64>,
    pub by_project: BTreeMap<String, i64>,
    pub recent_7d_sessions: i64,
    pub recent_7d_messages: i64,
    pub recent_30d_sessions: i64,
    pub recent_30d_messages: i64,
}

/// `list_projects` IPC 返回的单个项目聚合行。
///
/// `#[serde(rename_all = "camelCase")]` 让多词字段在 JSON 里变成
/// `projectPath` / `sessionCount` / `messageCount` / `lastTitle` /
/// `lastUpdated` / `byAdapter`。Rust 字段名仍是 snake_case，所以
/// 后端代码（如 `summaries[i].project_path`）不受影响。
///
/// 锁定形态见 `app/desktop/src-tauri/tests/ipc_contract.rs::project_summary_contract`。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub project_path: String,
    pub name: String,
    pub session_count: i64,
    pub message_count: i64,
    pub last_title: Option<String>,
    pub last_updated: String,
    pub by_adapter: BTreeMap<String, i64>,
}

/// Workload 分析数据，对应 Dashboard 的 Workload tab。
/// 所有计数仅覆盖过去 `days` 天（前端选 7/30 等）。
#[derive(Debug, Clone, Serialize)]
pub struct WorkloadReport {
    /// 整个时间窗的天数
    pub days: u32,
    /// 每日 session/message 数（GitHub-style 日历视图原料）。
    /// 仅返回**有活动**的日子；前端按日期补齐空格子。
    pub daily: Vec<WorkloadDailyEntry>,
    /// 时间窗内每个 (weekday, hour) 桶的 session 数（168 个），
    /// 用于渲染 24h × 7-weekday 时段习惯叠加图。weekday=0 即周一。
    pub heatmap: Vec<WorkloadHeatmapCell>,
    /// 按 adapter 聚合的 session 数（饼图）
    pub by_adapter: Vec<WorkloadBucket>,
    /// 工作量最大的项目（条形图），top 10
    pub by_project: Vec<WorkloadProjectBucket>,
    /// 整体高总览：会话总数、消息总数、活跃天数、peak day 描述
    pub overall: WorkloadOverall,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadDailyEntry {
    /// 本地时间的 YYYY-MM-DD
    pub date: String,
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadHeatmapCell {
    pub weekday: u8, // 0=Mon ... 6=Sun
    pub hour: u8,    // 0..24
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadBucket {
    pub key: String,
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadProjectBucket {
    /// 完整 project_path，方便点击跳转
    pub project_path: String,
    /// path 的最后一段，UI 直接显示
    pub name: String,
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadOverall {
    pub sessions: i64,
    pub messages: i64,
    pub active_days: i64,
    /// 这个时间窗里 sessions 最多的那一天（YYYY-MM-DD），可能为空
    pub peak_day: Option<String>,
    pub peak_day_sessions: i64,
}
