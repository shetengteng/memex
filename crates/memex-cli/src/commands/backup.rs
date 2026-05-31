use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use memex_core::memex_dir;

pub fn run(output_path: &str, json: bool) -> Result<()> {
    let memex = memex_dir();
    if !memex.exists() {
        if json {
            println!(
                "{}",
                serde_json::json!({"error": "memex directory not found"})
            );
        } else {
            eprintln!("Memex directory not found at {}", memex.display());
        }
        return Ok(());
    }

    let output = Path::new(output_path);
    if let Some(parent) = output.parent().filter(|p| !p.exists()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let tar_gz = fs::File::create(output)
        .with_context(|| format!("failed to create {}", output.display()))?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut builder = tar::Builder::new(enc);

    let mut file_count = 0u64;

    let db_path = memex.join("memex.db");
    if db_path.exists() {
        builder.append_path_with_name(&db_path, "memex.db")?;
        file_count += 1;
    }

    let config_path = memex.join("config.toml");
    if config_path.exists() {
        builder.append_path_with_name(&config_path, "config.toml")?;
        file_count += 1;
    }

    let redactions_path = memex.join("redactions.yaml");
    if redactions_path.exists() {
        builder.append_path_with_name(&redactions_path, "redactions.yaml")?;
        file_count += 1;
    }

    let sessions_dir = memex.join("sessions");
    if sessions_dir.exists() {
        for entry in walkdir::WalkDir::new(&sessions_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                let relative = path.strip_prefix(&memex).unwrap_or(path);
                builder.append_path_with_name(path, relative)?;
                file_count += 1;
            }
        }
    }

    let enc = builder.into_inner()?;
    enc.finish()?;

    let size = fs::metadata(output)?.len();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "path": output_path,
                "files": file_count,
                "size_bytes": size,
            })
        );
    } else {
        println!(
            "Backup complete: {} ({} files, {} bytes)",
            output_path, file_count, size
        );
    }

    Ok(())
}
