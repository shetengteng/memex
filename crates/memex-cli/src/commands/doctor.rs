use anyhow::Result;
use memex_core::config::MemexConfig;
use memex_core::memex_dir;
use memex_core::storage::db::Db;
use memex_core::storage::queries::DoctorReport;

pub fn run(json: bool) -> Result<()> {
    let memex = memex_dir();
    let db_path = memex.join("memex.db");
    let config_path = memex.join("config.toml");

    if !db_path.exists() {
        let report = DoctorReport {
            db_exists: false,
            schema_version: None,
            session_count: 0,
            message_count: 0,
            chunk_count: 0,
            source_count: 0,
            fts_ok: false,
            adapters: Vec::new(),
        };
        return print_report(&report, &memex, json);
    }

    let db = Db::open(&db_path)?;
    let report = DoctorReport {
        db_exists: true,
        schema_version: db.schema_version()?,
        session_count: db.session_count()?,
        message_count: db.message_count()?,
        chunk_count: db.chunk_count()?,
        source_count: db.source_count()?,
        fts_ok: db.fts_health_check(),
        adapters: db.adapter_statuses()?,
    };

    print_report(&report, &memex, json)?;

    if !json {
        let metrics = db.get_today_metrics().unwrap_or_default();
        let adapter_errors = metrics
            .iter()
            .find(|m| m.name == "adapter_errors")
            .map(|m| m.value)
            .unwrap_or(0);
        if adapter_errors > 0 {
            crate::out!("\n⚠️  Adapter errors today: {}", adapter_errors);
        }

        crate::out!("\nConfig:");
        if config_path.exists() {
            let config = MemexConfig::load(&memex)?;
            let adapters = [
                ("claude_code", config.adapters.claude_code),
                ("cursor", config.adapters.cursor),
                ("codex", config.adapters.codex),
                ("opencode", config.adapters.opencode),
            ];
            for (name, enabled) in &adapters {
                let status = if *enabled { "enabled" } else { "disabled" };
                crate::out!("  adapter.{}: {}", name, status);
            }
            let llm = if config.llm.ollama_enabled {
                "ollama"
            } else {
                "none (use Settings → LLM Providers to register one)"
            };
            crate::out!("  llm: {}", llm);
        } else {
            crate::out!("  config.toml not found (using defaults)");
        }
    }

    Ok(())
}

fn print_report(report: &DoctorReport, memex: &std::path::Path, json: bool) -> Result<()> {
    if json {
        crate::io::json(report)?;
        return Ok(());
    }

    crate::out!("Memex Doctor Report");
    crate::out!("===================");
    crate::out!("Data dir:  {}", memex.display());
    crate::out!(
        "Database:  {}",
        if report.db_exists { "OK" } else { "NOT FOUND" }
    );

    if !report.db_exists {
        crate::out!("\nRun `memex ingest` to initialize the database.");
        return Ok(());
    }

    crate::out!(
        "Schema:    v{}",
        report
            .schema_version
            .map_or("?".to_string(), |v| v.to_string())
    );
    crate::out!("FTS5:      {}", if report.fts_ok { "OK" } else { "ERROR" });
    crate::out!("\nData:");
    crate::out!("  Sessions:  {}", report.session_count);
    crate::out!("  Messages:  {}", report.message_count);
    crate::out!("  Chunks:    {}", report.chunk_count);
    crate::out!("  Sources:   {}", report.source_count);

    if !report.adapters.is_empty() {
        crate::out!("\nAdapter Sources:");
        for a in &report.adapters {
            let scan = a.last_scan.as_deref().unwrap_or("never");
            crate::out!(
                "  {}: {} file(s), last scan: {}",
                a.name,
                a.file_count,
                scan
            );
        }
    }

    crate::out!("\nAdapter Health:");
    crate::out!("  cursor (SQLite): {}", check_cursor_sqlite());

    Ok(())
}

fn check_cursor_sqlite() -> String {
    use memex_core::collector::cursor::{CursorSqliteAdapter, CursorSqliteProbe};
    let probe = CursorSqliteAdapter::new().probe();
    match probe {
        CursorSqliteProbe::Ok { composer_count, .. } => {
            format!("OK ({} composer sessions)", composer_count)
        }
        CursorSqliteProbe::NotFound { db_path } => format!("not found at {}", db_path),
        CursorSqliteProbe::PermissionDenied { db_path, .. } => format!(
            "PERMISSION DENIED at {}\n     → Grant Full Disk Access to your terminal via\n       System Settings → Privacy & Security → Full Disk Access,\n       then re-run `memex doctor`.",
            db_path
        ),
        CursorSqliteProbe::Error { message, .. } => format!("ERROR: {}", message),
    }
}
