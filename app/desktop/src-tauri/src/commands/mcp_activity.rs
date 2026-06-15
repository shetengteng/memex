//! MCP 工具调用活动的查询 IPC，给 Connect 页「MCP 工具与活动」卡片用。
//!
//! 数据源：`mcp_call_log` 表（由 memex-cli `commands::mcp::server::tools::handle_tool_call`
//! 写入）。前端 3s 轮询：先拉 [`mcp_call_stats_24h`] 拿顶部指标，再拉
//! [`mcp_recent_calls`] 拿事件流；diff 出新事件用于"准实时"渲染。
//!
//! Db 不存在（fresh install 或被 reset）时返回空结构，不报错 —— 让 UI 显示
//! 「暂无调用」而不是一片红。
//!
//! ## Db 打开失败的兜底（real-world race）
//!
//! `system_reset_index` / `system_reset_all` 把 db 删掉之后，daemon 异步重启需要
//! ~300ms 才会拿到新 db 句柄。窗口期里前端 3s polling 仍然在跑：如果命中
//! "db 文件已经被 daemon 创建但 PRAGMA WAL 还没跑完" 的小窗口，前端 [`Db::open`]
//! 自己也想跑 PRAGMA WAL，就会撞 SQLite 的 SQLITE_CANTOPEN (Error code 14)。
//!
//! 设计决策：**不让这个瞬时错误冒泡到 UI**。daemon 启动路径上有 `open_db_with_recovery`
//! 负责持久性损坏的兜底；前端 polling 此时只需把它当成"db 暂时不可读"处理，
//! 跟 db 不存在走同一条返回空的路径，UI 显示"暂无调用"，下一拍 polling 自然恢复。
//! 失败原因保留在 daemon log（tracing::warn），便于排障。

use std::path::Path;

use memex_core::memex_dir;
use memex_core::storage::db::Db;
use memex_core::storage::mcp_call_log::{McpCallEntry, McpCallStats24h, ToolBreakdown};

use super::error::CmdResult;

/// 最近 N 条 MCP 调用记录，按时间倒序。limit 上限 500，超出由 core 层截断。
#[tauri::command]
pub async fn mcp_recent_calls(limit: u32) -> CmdResult<Vec<McpCallEntry>> {
    let dir = memex_dir();
    let db_path = dir.join("memex.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let Some(db) = open_db_or_log(&db_path, "mcp_recent_calls") else {
        return Ok(Vec::new());
    };
    Ok(db.recent_mcp_calls(limit as usize)?)
}

/// 滚动 24 小时窗口的 MCP 调用聚合：总数、成功 / 失败、平均延迟、按工具拆分。
#[tauri::command]
pub async fn mcp_call_stats_24h() -> CmdResult<McpCallStats24h> {
    let dir = memex_dir();
    let db_path = dir.join("memex.db");
    if !db_path.exists() {
        return Ok(empty_stats());
    }
    let Some(db) = open_db_or_log(&db_path, "mcp_call_stats_24h") else {
        return Ok(empty_stats());
    };
    Ok(db.mcp_call_stats_24h()?)
}

/// 尝试打开 db；失败时记 warn 并返回 None，由调用方走"返回空"的兜底路径。
///
/// 失败的常见原因都是瞬时的（reset 流程中、daemon 重启竞态、文件系统抖动）；
/// 持久性损坏由 daemon 启动时的 `open_db_with_recovery` 负责兜底，不在这里修。
fn open_db_or_log(db_path: &Path, op: &str) -> Option<Db> {
    match Db::open(db_path) {
        Ok(db) => Some(db),
        Err(e) => {
            tracing::warn!(
                error = %e,
                path = %db_path.display(),
                op = op,
                "activity panel db open failed; treating as empty (likely reset/restart race)",
            );
            None
        }
    }
}

fn empty_stats() -> McpCallStats24h {
    McpCallStats24h {
        total: 0,
        success: 0,
        failed: 0,
        avg_latency_ms: 0.0,
        by_tool: Vec::<ToolBreakdown>::new(),
        last_call_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// 用专属 MEMEX_HOME 跑闭包，结束后恢复原值。`#[serial(memex_home)]` 保证
    /// 同进程内不同测试不会互相覆盖环境。
    fn with_temp_memex<F: FnOnce()>(f: F) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::var("MEMEX_HOME").ok();
        // SAFETY: 由 #[serial(memex_home)] 串行化。
        unsafe { std::env::set_var("MEMEX_HOME", tmp.path()) };
        f();
        match prev {
            Some(v) => unsafe { std::env::set_var("MEMEX_HOME", v) },
            None => unsafe { std::env::remove_var("MEMEX_HOME") },
        }
    }

    /// db 不存在时 recent 不应报错，而是返回空列表 —— 否则前端首次启动看到的
    /// 是 red toast「读取失败」而不是「暂无调用」。
    #[test]
    #[serial(memex_home)]
    fn recent_returns_empty_when_db_missing() {
        with_temp_memex(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let rows = rt.block_on(mcp_recent_calls(20)).expect("ok");
            assert!(rows.is_empty());
        });
    }

    /// db 不存在时 stats 应返回零值 struct，与 UI 期望对齐。
    #[test]
    #[serial(memex_home)]
    fn stats_returns_zeros_when_db_missing() {
        with_temp_memex(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let stats = rt.block_on(mcp_call_stats_24h()).expect("ok");
            assert_eq!(stats.total, 0);
            assert_eq!(stats.success, 0);
            assert_eq!(stats.failed, 0);
            assert_eq!(stats.avg_latency_ms, 0.0);
            assert!(stats.by_tool.is_empty());
            assert!(stats.last_call_at.is_none());
        });
    }

    /// Regression：模拟 reset / restart 窗口期 db 暂时不可读。db_path **存在**
    /// 但 [`Db::open`] 必失败（用目录代替文件触发 SQLITE_CANTOPEN），IPC 必须
    /// 走兜底路径返回空 Vec，绝不能把 SQLite 错误冒泡到前端 UI 显示
    /// "读取失败：unable to open database file: Error code 14"。
    #[test]
    #[serial(memex_home)]
    fn recent_returns_empty_when_db_open_fails() {
        with_temp_memex(|| {
            // 让 memex.db 是目录而非文件 —— SQLite 必报 "unable to open database file"
            let bad_db = memex_dir().join("memex.db");
            std::fs::create_dir_all(&bad_db).unwrap();
            assert!(
                bad_db.exists(),
                "precondition: db_path.exists() must be true"
            );

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let rows = rt
                .block_on(mcp_recent_calls(20))
                .expect("must NOT propagate SQLite error to UI");
            assert!(rows.is_empty(), "open-failure path must return empty list");
        });
    }

    /// Regression：与上一条同源，只是覆盖 stats IPC。
    #[test]
    #[serial(memex_home)]
    fn stats_returns_zeros_when_db_open_fails() {
        with_temp_memex(|| {
            let bad_db = memex_dir().join("memex.db");
            std::fs::create_dir_all(&bad_db).unwrap();

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let stats = rt
                .block_on(mcp_call_stats_24h())
                .expect("must NOT propagate SQLite error to UI");
            assert_eq!(stats.total, 0);
            assert_eq!(stats.success, 0);
            assert_eq!(stats.failed, 0);
            assert_eq!(stats.avg_latency_ms, 0.0);
            assert!(stats.by_tool.is_empty());
            assert!(stats.last_call_at.is_none());
        });
    }
}
