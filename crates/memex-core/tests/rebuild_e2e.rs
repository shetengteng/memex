//! End-to-end rebuild test:
//!
//!     1. Seed a fake memex working directory with `sessions/<source>/<id>.md`
//!        Markdown files (the canonical source of truth).
//!     2. Open a fresh on-disk SQLite (simulating the post-`memex backup` /
//!        post-DB-loss state).
//!     3. Run `rebuild_from_markdown` to repopulate `sessions` / `messages` /
//!        `chunks` / `chunks_fts`.
//!     4. Assert `Retriever::search` returns the seeded content with the
//!        adapter / project / snippet metadata intact.
//!
//! This is the Sprint 4 "backup → delete DB → rebuild → search consistent"
//! acceptance test. We deliberately exercise the Markdown layer rather than
//! the tar.gz wrapper — the tarball is a transport artifact, not a semantic
//! one; what matters is that the canonical Markdown set fully reconstitutes
//! the searchable index.

use std::fs;

use memex_core::retriever::Retriever;
use memex_core::storage::db::Db;
use memex_core::storage::rebuild::rebuild_from_markdown;
use tempfile::TempDir;

fn write_session_markdown(memex_dir: &std::path::Path, source: &str, session_id: &str, body: &str) {
    let dir = memex_dir.join("sessions").join(source);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.md", session_id));
    fs::write(path, body).unwrap();
}

#[test]
fn backup_delete_rebuild_search_roundtrip() {
    let memex_dir = TempDir::new().unwrap();
    let memex_path = memex_dir.path();

    write_session_markdown(
        memex_path,
        "claude_code",
        "sess-claude-1",
        "---\nsession_id: sess-claude-1\nsource: claude_code\nproject: memex\n---\n\n\
         ## 👤 User\n\nhow do I tune redis pipeline batch size for high throughput?\n\n---\n\n\
         ## 🤖 Assistant\n\nSet `pipeline_batch_size` to 256 and benchmark with `redis-benchmark`.\n",
    );

    write_session_markdown(
        memex_path,
        "cursor",
        "sess-cursor-1",
        "---\nsession_id: sess-cursor-1\nsource: cursor\nproject: memex\n---\n\n\
         ## 👤 User\n\nhow do we wire shadcn-vue Button into Tauri menubar?\n\n---\n\n\
         ## 🤖 Assistant\n\nImport `Button` from `@/components/ui/button` and bind via Tauri IPC `invoke`.\n",
    );

    let db_path = memex_path.join("memex.db");
    assert!(!db_path.exists(), "DB must not exist before rebuild");

    let db = Db::open(&db_path).expect("open fresh sqlite");
    let stats = rebuild_from_markdown(memex_path, &db).expect("rebuild ok");

    assert_eq!(stats.sessions, 2, "two sessions reconstructed");
    assert_eq!(stats.errors, 0, "no rebuild errors");
    assert!(stats.messages >= 4, "≥4 messages (2 user + 2 assistant)");
    assert!(stats.chunks > 0, "chunks should be produced for FTS5");

    let retriever = Retriever::new(&db);

    let redis_hits = retriever.search("redis", 10).expect("search ok");
    assert!(
        !redis_hits.is_empty(),
        "redis hits should be present after rebuild"
    );
    assert!(
        redis_hits
            .iter()
            .any(|r| r.adapter.as_deref() == Some("claude_code")),
        "claude_code adapter metadata must survive rebuild"
    );

    let shadcn_hits = retriever.search("shadcn", 10).expect("search ok");
    assert!(
        !shadcn_hits.is_empty(),
        "shadcn hits should be present after rebuild"
    );
    assert!(
        shadcn_hits
            .iter()
            .any(|r| r.adapter.as_deref() == Some("cursor")),
        "cursor adapter metadata must survive rebuild"
    );

    let project_filtered = retriever
        .search_filtered(
            "redis",
            10,
            &memex_core::retriever::SearchFilter {
                project: Some("memex".to_string()),
                ..Default::default()
            },
        )
        .expect("filter ok");
    assert!(
        !project_filtered.is_empty(),
        "project filter should still resolve after rebuild"
    );
}
