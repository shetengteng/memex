//! Doctor 诊断 GUI 入口。
//!
//! 直接调 memex-core（不 spawn CLI），返回前端期望的复合结构：
//!   { data_dir, config_present, report: DoctorReport, cursor_probe: CursorProbe }
//!
//! cursor_probe 用 tagged union，前端按 status 字段做 switch。

use serde::Serialize;

use memex_core::collector::cursor::{CursorSqliteAdapter, CursorSqliteProbe};
use memex_core::memex_dir;
use memex_core::storage::db::Db;
use memex_core::storage::queries::DoctorReport;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CursorProbeDto {
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

impl From<CursorSqliteProbe> for CursorProbeDto {
    fn from(p: CursorSqliteProbe) -> Self {
        match p {
            CursorSqliteProbe::Ok {
                composer_count,
                db_path,
            } => CursorProbeDto::Ok {
                composer_count,
                db_path,
            },
            CursorSqliteProbe::NotFound { db_path } => CursorProbeDto::NotFound { db_path },
            CursorSqliteProbe::PermissionDenied { db_path, message } => {
                CursorProbeDto::PermissionDenied { db_path, message }
            }
            CursorSqliteProbe::Error { db_path, message } => {
                CursorProbeDto::Error { db_path, message }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorRunResult {
    pub data_dir: String,
    pub config_present: bool,
    pub report: DoctorReport,
    pub cursor_probe: CursorProbeDto,
}

#[tauri::command]
pub async fn doctor_run() -> Result<DoctorRunResult, String> {
    let memex = memex_dir();
    let db_path = memex.join("memex.db");
    let config_path = memex.join("config.toml");

    let report = if db_path.exists() {
        let db = Db::open(&db_path).map_err(|e| e.to_string())?;
        DoctorReport {
            db_exists: true,
            schema_version: db.schema_version().map_err(|e| e.to_string())?,
            session_count: db.session_count().map_err(|e| e.to_string())?,
            message_count: db.message_count().map_err(|e| e.to_string())?,
            chunk_count: db.chunk_count().map_err(|e| e.to_string())?,
            source_count: db.source_count().map_err(|e| e.to_string())?,
            fts_ok: db.fts_health_check(),
            adapters: db.adapter_statuses().map_err(|e| e.to_string())?,
        }
    } else {
        DoctorReport {
            db_exists: false,
            schema_version: None,
            session_count: 0,
            message_count: 0,
            chunk_count: 0,
            source_count: 0,
            fts_ok: false,
            adapters: Vec::new(),
        }
    };

    let cursor_probe = CursorProbeDto::from(CursorSqliteAdapter::new().probe());

    Ok(DoctorRunResult {
        data_dir: memex.display().to_string(),
        config_present: config_path.exists(),
        report,
        cursor_probe,
    })
}
