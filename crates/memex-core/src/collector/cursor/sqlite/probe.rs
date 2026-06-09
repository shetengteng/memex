//! 给 `memex doctor` 和 menubar 设置页用的轻量健康探测。
//!
//! 不和 `scan/collect` 共享 connection —— 它必须能在没有 FDA 权限时优雅降级，
//! 单独打开 + 单独错误分类，避免污染主流程。

use std::path::Path;

use super::CursorSqliteAdapter;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CursorSqliteProbe {
    Ok {
        composer_count: i64,
        db_path: String,
    },
    NotFound {
        db_path: String,
    },
    PermissionDenied {
        db_path: String,
        message: String,
    },
    Error {
        db_path: String,
        message: String,
    },
}

impl CursorSqliteAdapter {
    pub fn probe(&self) -> CursorSqliteProbe {
        let db_path: &Path = self.db_path();
        if !db_path.exists() {
            return CursorSqliteProbe::NotFound {
                db_path: db_path.to_string_lossy().to_string(),
            };
        }
        let uri = format!("file:{}?mode=ro&immutable=0", db_path.to_string_lossy());
        match rusqlite::Connection::open_with_flags(
            &uri,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(conn) => match conn.query_row(
                "SELECT COUNT(*) FROM cursorDiskKV WHERE key LIKE 'composerData:%'",
                [],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(n) => CursorSqliteProbe::Ok {
                    composer_count: n,
                    db_path: db_path.to_string_lossy().to_string(),
                },
                Err(e) => CursorSqliteProbe::Error {
                    db_path: db_path.to_string_lossy().to_string(),
                    message: e.to_string(),
                },
            },
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("unable to open")
                    || msg.contains("authorization")
                    || msg.contains("permission")
                {
                    CursorSqliteProbe::PermissionDenied {
                        db_path: db_path.to_string_lossy().to_string(),
                        message: msg,
                    }
                } else {
                    CursorSqliteProbe::Error {
                        db_path: db_path.to_string_lossy().to_string(),
                        message: msg,
                    }
                }
            }
        }
    }
}
