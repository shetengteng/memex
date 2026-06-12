//! 通知中心 IPC，给 SiteHeader Bell 按钮 + Popover 列表用。
//!
//! 数据源：`notifications` 表（由 `services::daemon::routes` 上的 `/ingest`
//! 失败、reflect 后台扫描等业务路径调 `Db::insert_notification` 写入）。
//!
//! 前端 3s 轮询 `unread_count` 拿 Bell badge，用户打开 Popover 时再拉 `list`。
//! 点击单条通知 → 弹 Dialog 显示详情（payload_json 解析后渲染）+ 调
//! `mark_read` 标记已读。
//!
//! Db 不存在（fresh install 或被 reset）时返回空 / 0，跟 mcp_activity 一致 ——
//! 让 UI 显示「暂无通知」而不是 red toast。

use memex_core::memex_dir;
use memex_core::storage::db::Db;
use memex_core::storage::notifications::NotificationEntry;

use super::error::CmdResult;

fn open_db_optional() -> CmdResult<Option<Db>> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(None);
    }
    Ok(Some(Db::open(&db_path)?))
}

/// 拉最近 N 条通知。`unread_only=true` 时只返回未读。limit 上限由 core 层
/// 截断到 500，超出无报错。
#[tauri::command]
pub async fn notifications_list(limit: u32, unread_only: bool) -> CmdResult<Vec<NotificationEntry>> {
    let Some(db) = open_db_optional()? else {
        return Ok(Vec::new());
    };
    Ok(db.list_notifications(limit as usize, unread_only)?)
}

/// 当前未读通知数。Bell badge 3s 轮询用这个值。
#[tauri::command]
pub async fn notifications_unread_count() -> CmdResult<i64> {
    let Some(db) = open_db_optional()? else {
        return Ok(0);
    };
    Ok(db.count_unread_notifications()?)
}

/// 标记单条已读。已读再次标记是 no-op；返回是否真正发生了状态变更。
#[tauri::command]
pub async fn notification_mark_read(id: i64) -> CmdResult<bool> {
    let Some(db) = open_db_optional()? else {
        return Ok(false);
    };
    Ok(db.mark_notification_read(id)? > 0)
}

/// 全部已读。返回被标记的行数。
#[tauri::command]
pub async fn notifications_mark_all_read() -> CmdResult<u32> {
    let Some(db) = open_db_optional()? else {
        return Ok(0);
    };
    Ok(db.mark_all_notifications_read()? as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

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

    #[test]
    #[serial(memex_home)]
    fn list_returns_empty_when_db_missing() {
        with_temp_memex(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let rows = rt.block_on(notifications_list(20, false)).expect("ok");
            assert!(rows.is_empty());
        });
    }

    #[test]
    #[serial(memex_home)]
    fn unread_count_returns_zero_when_db_missing() {
        with_temp_memex(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let n = rt.block_on(notifications_unread_count()).expect("ok");
            assert_eq!(n, 0);
        });
    }
}
