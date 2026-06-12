//! `CursorAdapter` 的回归测试入口。原 `cursor/tests.rs` 拆为：
//! - `jsonl` —— 早期 JSONL 后端（含 permission-denied 边界）
//! - `sqlite_backend` —— 新版 SQLite + composer headers enrichment

mod jsonl;
mod sqlite_backend;
