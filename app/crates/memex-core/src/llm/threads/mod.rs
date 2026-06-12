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
//!
//! 模块拆分：
//! - [`prompts`]：2 个 LLM system prompt
//! - [`cluster`]：批量聚类（喂 ≤80 session 给 LLM）
//! - [`query`]：按用户关键词的单 thread 查询
//! - [`fallback`]：LLM 不可用时的确定性 fallback

mod cluster;
mod fallback;
mod prompts;
mod query;
#[cfg(test)]
mod tests;

pub use cluster::cluster_threads;
pub use fallback::{fallback_cluster, fallback_query_match};
pub use query::query_threads_by_keyword;

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

/// Shared by `cluster` / `query` —— LLM 偶尔会用 ```json fence 包裹输出。
fn strip_code_fences(text: &str) -> String {
    let t = text.trim();
    if let Some(stripped) = t.strip_prefix("```json") {
        return stripped.trim_end_matches("```").trim().to_string();
    }
    if let Some(stripped) = t.strip_prefix("```") {
        return stripped.trim_end_matches("```").trim().to_string();
    }
    t.to_string()
}
