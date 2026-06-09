use anyhow::Result;
use memex_core::memex_dir;
use memex_core::storage::db::Db;

pub fn run(session_id: &str, json: bool) -> Result<()> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        if json {
            crate::io::json(&serde_json::json!({"error": "database not found"}))?;
        } else {
            crate::err!("Database not found. Run `memex ingest` first.");
        }
        return Ok(());
    }

    let db = Db::open(&db_path)?;

    let resolved_id = if session_id.len() < 36 {
        db.find_session_by_prefix(session_id)?
            .unwrap_or_else(|| session_id.to_string())
    } else {
        session_id.to_string()
    };

    let Some(d) = db.get_session_detail(&resolved_id)? else {
        if json {
            crate::io::json(&serde_json::json!({
                "error": "session not found",
                "id": session_id,
            }))?;
        } else {
            crate::err!("Session \"{}\" not found.", session_id);
        }
        return Ok(());
    };

    if json {
        crate::io::json(&d)?;
        return Ok(());
    }

    crate::out!("Session: {}", d.id);
    crate::out!("Source:  {}", d.source);
    if let Some(ref proj) = d.project_path {
        crate::out!("Project: {}", proj);
    }
    crate::out!("Created: {}", d.created_at);
    crate::out!("Updated: {}", d.updated_at);
    crate::out!("Messages: {}\n", d.message_count);

    for (i, msg) in d.messages.iter().enumerate() {
        let role_icon = match msg.role.as_str() {
            "user" => "User",
            "assistant" => "Assistant",
            "system" => "System",
            "tool" => "Tool",
            _ => &msg.role,
        };
        let ts = msg.timestamp.as_deref().unwrap_or("");
        crate::out!("--- Message {} ({}) {} ---", i + 1, role_icon, ts);
        let preview = if msg.content.len() > 500 {
            format!("{}...", &msg.content[..500])
        } else {
            msg.content.clone()
        };
        crate::out!("{}\n", preview);
    }

    Ok(())
}
