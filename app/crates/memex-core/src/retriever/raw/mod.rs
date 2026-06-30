//! 原始文件兜底检索：在 `<data_dir>/sessions/<adapter>/<session_id>.md` 这一
//! 棵已归一化的会话目录上做 grep / find / read，作为 FTS5 主路径的兜底。
//!
//! 设计文档：`design/specs/20260630-01-Memex-原始文件兜底检索设计.md`。
//!
//! 模块拆分：
//! - [`sandbox`] — 沙箱根目录与路径校验
//! - [`frontmatter`] — YAML frontmatter 解析、文件名兜底反查
//! - [`filter`] — 通用文件遍历与 adapter / project / 时间窗过滤
//! - [`grep`] / [`find`] / [`read`] — 三个公开工具
//!
//! 公开 API 在本模块 re-export：`raw_grep` / `raw_find` / `raw_read` 及其
//! Request/Response 类型。

mod filter;
mod find;
mod frontmatter;
mod grep;
mod read;
mod sandbox;

#[cfg(test)]
mod tests;

pub use find::{RawFindFile, RawFindRequest, RawFindResponse, raw_find};
pub use grep::{RawGrepHit, RawGrepRequest, RawGrepResponse, raw_grep};
pub use read::{RawReadLine, RawReadRequest, RawReadResponse, raw_read};
pub use sandbox::sessions_root;
