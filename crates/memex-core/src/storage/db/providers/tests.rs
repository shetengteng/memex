use super::*;

fn test_db() -> Db {
    Db::open_in_memory().unwrap()
}

#[test]
fn empty_list() {
    let db = test_db();
    let list = db.provider_list().unwrap();
    assert!(list.is_empty());
}

#[test]
fn upsert_and_get() {
    let db = test_db();
    let row = db
        .provider_upsert(LlmProviderUpsert {
            id: "ds-1".into(),
            name: "DeepSeek".into(),
            kind: "openai_compat".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            model: "deepseek-chat".into(),
            api_key: "sk-test".into(),
            enabled: true,
            is_default: true,
        })
        .unwrap();
    assert_eq!(row.name, "DeepSeek");
    assert!(row.is_default);
    assert_eq!(row.status, "untested");

    let got = db.provider_get("ds-1").unwrap().unwrap();
    assert_eq!(got.api_key, "sk-test");
}

#[test]
fn upsert_preserves_api_key_when_empty() {
    let db = test_db();
    db.provider_upsert(LlmProviderUpsert {
        id: "p1".into(),
        name: "Test".into(),
        kind: "openai_compat".into(),
        base_url: "http://localhost".into(),
        model: "m".into(),
        api_key: "secret".into(),
        enabled: true,
        is_default: false,
    })
    .unwrap();

    db.provider_upsert(LlmProviderUpsert {
        id: "p1".into(),
        name: "Test Updated".into(),
        kind: "openai_compat".into(),
        base_url: "http://localhost".into(),
        model: "m2".into(),
        api_key: "".into(),
        enabled: true,
        is_default: false,
    })
    .unwrap();

    let got = db.provider_get("p1").unwrap().unwrap();
    assert_eq!(
        got.api_key, "secret",
        "empty api_key in upsert should preserve existing"
    );
    assert_eq!(got.model, "m2");
}

#[test]
fn default_flag_is_exclusive() {
    let db = test_db();
    db.provider_upsert(LlmProviderUpsert {
        id: "a".into(),
        name: "A".into(),
        kind: "ollama".into(),
        base_url: "http://localhost:11434".into(),
        model: "llama3".into(),
        api_key: "".into(),
        enabled: true,
        is_default: true,
    })
    .unwrap();
    db.provider_upsert(LlmProviderUpsert {
        id: "b".into(),
        name: "B".into(),
        kind: "openai_compat".into(),
        base_url: "https://api.openai.com/v1".into(),
        model: "gpt-4o".into(),
        api_key: "sk".into(),
        enabled: true,
        is_default: true,
    })
    .unwrap();

    let list = db.provider_list().unwrap();
    let defaults: Vec<_> = list.iter().filter(|p| p.is_default).collect();
    assert_eq!(defaults.len(), 1, "only one provider can be default");
    assert_eq!(defaults[0].id, "b");
}

#[test]
fn delete() {
    let db = test_db();
    db.provider_upsert(LlmProviderUpsert {
        id: "x".into(),
        name: "X".into(),
        kind: "ollama".into(),
        base_url: "http://localhost:11434".into(),
        model: "m".into(),
        api_key: "".into(),
        enabled: true,
        is_default: false,
    })
    .unwrap();
    assert_eq!(db.provider_delete("x").unwrap(), 1);
    assert!(db.provider_get("x").unwrap().is_none());
}

#[test]
fn update_status() {
    let db = test_db();
    db.provider_upsert(LlmProviderUpsert {
        id: "s".into(),
        name: "S".into(),
        kind: "openai_compat".into(),
        base_url: "https://example.com".into(),
        model: "m".into(),
        api_key: "k".into(),
        enabled: true,
        is_default: false,
    })
    .unwrap();
    db.provider_update_status("s", "ok", Some(123)).unwrap();
    let got = db.provider_get("s").unwrap().unwrap();
    assert_eq!(got.status, "ok");
    assert_eq!(got.latency_ms, Some(123));
}
