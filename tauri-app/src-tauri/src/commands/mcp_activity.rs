//! MCP 工具调用活动的查询 IPC，给 Connect 页「MCP 工具与活动」卡片用。
//!
//! 数据源：`mcp_call_log` 表（由 memex-cli `commands::mcp::server::tools::handle_tool_call`
//! 写入）。前端 3s 轮询：先拉 [`mcp_call_stats_24h`] 拿顶部指标，再拉
//! [`mcp_recent_calls`] 拿事件流；diff 出新事件用于"准实时"渲染。
//!
//! Db 不存在（fresh install 或被 reset）时返回空结构，不报错 —— 让 UI 显示
//! 「暂无调用」而不是一片红。

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
    let db = Db::open(&db_path)?;
    Ok(db.recent_mcp_calls(limit as usize)?)
}

/// 滚动 24 小时窗口的 MCP 调用聚合：总数、成功 / 失败、平均延迟、按工具拆分。
#[tauri::command]
pub async fn mcp_call_stats_24h() -> CmdResult<McpCallStats24h> {
    let dir = memex_dir();
    let db_path = dir.join("memex.db");
    if !db_path.exists() {
        return Ok(McpCallStats24h {
            total: 0,
            success: 0,
            failed: 0,
            avg_latency_ms: 0.0,
            by_tool: Vec::<ToolBreakdown>::new(),
            last_call_at: None,
        });
    }
    let db = Db::open(&db_path)?;
    Ok(db.mcp_call_stats_24h()?)
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
}
