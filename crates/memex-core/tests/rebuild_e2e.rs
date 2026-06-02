//! 端到端 rebuild 测试：
//!
//!     1. 在一个假的 memex 工作目录里铺一份 `sessions/<source>/<id>.md`
//!        Markdown 文件作为"权威数据源"。
//!     2. 打开一个全新的 on-disk SQLite（模拟 `memex backup` 后 / DB 丢失后
//!        的状态）。
//!     3. 跑 `rebuild_from_markdown`，把 `sessions` / `messages` /
//!        `chunks` / `chunks_fts` 全部重建出来。
//!     4. 断言 `Retriever::search` 能搜到之前的内容，并且 adapter / project /
//!        snippet 元数据都还在。
//!
//! 这是 Sprint 4 的"backup → 删除 DB → rebuild → 搜索结果一致"验收用例。
//! 我们故意走 Markdown 这一层，而不是 tar.gz 那层 —— tarball 只是个传输
//! 载体，没有语义；真正要保证的是这一份"权威 Markdown" 能完整重建出
//! 可搜索的索引。

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
