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
        println!("\nConfig:");
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
                println!("  adapter.{}: {}", name, status);
            }
            let llm = if config.llm.ollama_enabled {
                "ollama"
            } else if config.llm.cloud_fallback {
                "cloud"
            } else {
                "none"
            };
            println!("  llm: {}", llm);
        } else {
            println!("  config.toml not found (using defaults)");
        }
    }

    Ok(())
}

fn print_report(report: &DoctorReport, memex: &std::path::Path, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    println!("Memex Doctor Report");
    println!("===================");
    println!("Data dir:  {}", memex.display());
    println!(
        "Database:  {}",
        if report.db_exists { "OK" } else { "NOT FOUND" }
    );

    if !report.db_exists {
        println!("\nRun `memex ingest` to initialize the database.");
        return Ok(());
    }

    println!(
        "Schema:    v{}",
        report
            .schema_version
            .map_or("?".to_string(), |v| v.to_string())
    );
    println!("FTS5:      {}", if report.fts_ok { "OK" } else { "ERROR" });
    println!("\nData:");
    println!("  Sessions:  {}", report.session_count);
    println!("  Messages:  {}", report.message_count);
    println!("  Chunks:    {}", report.chunk_count);
    println!("  Sources:   {}", report.source_count);

    if !report.adapters.is_empty() {
        println!("\nAdapter Sources:");
        for a in &report.adapters {
            let scan = a.last_scan.as_deref().unwrap_or("never");
            println!(
                "  {}: {} file(s), last scan: {}",
                a.name, a.file_count, scan
            );
        }
    }

    Ok(())
}
