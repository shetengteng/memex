use super::*;

use crate::storage::models::{Chunk, ChunkMetadata, ChunkType};

fn seed_search_chunk(
    db: &Db,
    session_id: &str,
    adapter: &str,
    project: &str,
    message_id: &str,
    content: &str,
    timestamp: &str,
) {
    db.insert_session(
        session_id,
        adapter,
        Some(project),
        "/tmp/session.jsonl",
        0,
        0,
    )
    .unwrap();
    let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
    db.insert_message(
        message_id,
        session_id,
        "user",
        content,
        Some(timestamp),
        0,
        &hash,
    )
    .unwrap();
    db.insert_chunk(&Chunk {
        id: None,
        message_id: message_id.to_string(),
        session_id: session_id.to_string(),
        chunk_type: ChunkType::Text,
        content: content.to_string(),
        redacted_content: None,
        position: 0,
        token_count: 8,
        metadata: ChunkMetadata::default(),
    })
    .unwrap();
}

#[test]
fn test_build_match_reason() {
    let r = SearchResult {
        chunk_id: 1,
        session_id: "s1".into(),
        message_id: "m1".into(),
        chunk_type: "code_block".into(),
        content: "redis pipeline optimization".into(),
        snippet: "redis pipeline".into(),
        rank: 1.0,
        match_reason: String::new(),
        adapter: Some("claude_code".into()),
        project: None,
        timestamp: None,
    };
    let reason = build_match_reason("redis pipeline", &r);
    assert!(reason.contains("keyword match"));
    assert!(reason.contains("chunk_type: code_block"));
}

#[test]
fn test_estimate_age_days() {
    let now = "2026-06-01T12:00:00+00:00";
    let yesterday = "2026-05-31T12:00:00+00:00";
    assert!((estimate_age_days(yesterday, now) - 1.0).abs() < 0.01);
}

#[test]
fn test_recency_boost_sorting() {
    let mut results = vec![
        SearchResult {
            chunk_id: 1,
            session_id: "s1".into(),
            message_id: "m1".into(),
            chunk_type: "text".into(),
            content: "old".into(),
            snippet: "old".into(),
            rank: -5.0,
            match_reason: String::new(),
            adapter: None,
            project: None,
            timestamp: Some("2026-01-01T00:00:00+00:00".into()),
        },
        SearchResult {
            chunk_id: 2,
            session_id: "s2".into(),
            message_id: "m2".into(),
            chunk_type: "text".into(),
            content: "new".into(),
            snippet: "new".into(),
            rank: -5.0,
            match_reason: String::new(),
            adapter: None,
            project: None,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
        },
    ];
    apply_recency_boost(&mut results);
    assert_eq!(results[0].chunk_id, 2);
}

#[test]
fn test_cjk_expand_query_pure_chinese() {
    let expanded = cjk_expand_query("数据库");
    assert_eq!(expanded, "NEAR(数 据 库, 0)");
}

#[test]
fn test_cjk_expand_query_mixed() {
    let expanded = cjk_expand_query("redis 数据库");
    assert_eq!(expanded, "redis NEAR(数 据 库, 0)");
}

#[test]
fn test_cjk_expand_query_no_cjk() {
    let expanded = cjk_expand_query("redis pipeline");
    assert_eq!(expanded, "redis pipeline");
}

#[test]
fn test_cjk_expand_single_char() {
    let expanded = cjk_expand_query("库");
    assert_eq!(expanded, "库");
}

#[test]
fn test_query_with_hyphen_is_quoted() {
    let expanded = cjk_expand_query("ZOOM-1248726");
    assert_eq!(expanded, "\"ZOOM-1248726\"");
}

#[test]
fn test_query_mixed_with_special_chars() {
    let expanded = cjk_expand_query("user@email.com search");
    assert_eq!(expanded, "\"user@email.com\" search");
}

#[test]
fn search_filtered_combines_adapter_project_type_and_date_filters() {
    let db = Db::open_in_memory().unwrap();
    seed_search_chunk(
        &db,
        "s1",
        "claude_code",
        "/workspace/memex",
        "m1",
        "redis cache alpha",
        "2026-06-01T12:00:00+00:00",
    );
    seed_search_chunk(
        &db,
        "s2",
        "cursor",
        "/workspace/memex",
        "m2",
        "redis cache beta",
        "2026-06-01T13:00:00+00:00",
    );
    seed_search_chunk(
        &db,
        "s3",
        "claude_code",
        "/workspace/other",
        "m3",
        "redis cache gamma",
        "2026-06-01T14:00:00+00:00",
    );
    seed_search_chunk(
        &db,
        "s4",
        "claude_code",
        "/workspace/memex",
        "m4",
        "redis cache old",
        "2026-05-01T12:00:00+00:00",
    );

    let retriever = Retriever::new(&db);
    let results = retriever
        .search_filtered(
            "redis",
            10,
            &SearchFilter {
                adapter: Some("claude_code".to_string()),
                project: Some("memex".to_string()),
                session_id: None,
                chunk_type: Some("text".to_string()),
                after: Some("2026-06-01T00:00:00+00:00".to_string()),
                before: Some("2026-06-01T23:59:59+00:00".to_string()),
            },
        )
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, "s1");
    assert_eq!(results[0].adapter.as_deref(), Some("claude_code"));
    assert_eq!(results[0].project.as_deref(), Some("/workspace/memex"));
    assert!(results[0].match_reason.contains("keyword match"));
}

#[test]
fn search_filtered_applies_session_id_and_limit_after_search() {
    let db = Db::open_in_memory().unwrap();
    seed_search_chunk(
        &db,
        "s1",
        "claude_code",
        "/workspace/memex",
        "m1",
        "redis first",
        "2026-06-01T12:00:00+00:00",
    );
    seed_search_chunk(
        &db,
        "s2",
        "claude_code",
        "/workspace/memex",
        "m2",
        "redis second",
        "2026-06-01T13:00:00+00:00",
    );

    let retriever = Retriever::new(&db);
    let results = retriever
        .search_filtered(
            "redis",
            1,
            &SearchFilter {
                session_id: Some("s2".to_string()),
                ..SearchFilter::default()
            },
        )
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, "s2");
}
