use super::*;

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
