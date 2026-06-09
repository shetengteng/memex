//! Doctor + access-log + session-prefix lookup tests.

use crate::storage::db::Db;

#[test]
fn test_access_log() {
    let db = Db::open_in_memory().unwrap();
    db.write_access_log("redis", 5, 42).unwrap();
}

#[test]
fn test_find_session_by_prefix() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("abc-12345", "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
    assert_eq!(
        db.find_session_by_prefix("abc-1").unwrap().unwrap(),
        "abc-12345"
    );
    assert!(db.find_session_by_prefix("zzz").unwrap().is_none());
}

#[test]
fn test_fts_health() {
    let db = Db::open_in_memory().unwrap();
    assert!(db.fts_health_check());
}

#[test]
fn test_doctor_queries() {
    let db = Db::open_in_memory().unwrap();
    assert!(db.schema_version().unwrap().is_some());
    assert_eq!(db.source_count().unwrap(), 0);
    assert!(db.adapter_statuses().unwrap().is_empty());
}
