use std::fs;

use super::*;

#[test]
fn default_llm_config_has_nonempty_ollama_url_and_model() {
    let c = MemexConfig::default();
    assert_eq!(
        c.llm.ollama_url, "http://127.0.0.1:11434",
        "MemexConfig::default() 必须给出可用的 ollama_url，否则 OOB 写出来的 config.toml 是空串"
    );
    assert_eq!(c.llm.ollama_model, "llama3.2");
    assert!(!c.llm.ollama_enabled);
    assert_eq!(
        c.llm.summary_cooldown_secs, 600,
        "默认 L2 摘要冷却时间应为 10 分钟（600 秒），与方案 B 的设计一致"
    );
}

#[test]
fn default_adapters_all_disabled() {
    let c = MemexConfig::default();
    assert!(
        !c.adapters.claude_code,
        "OOB adapters 必须默认关闭，由用户按需开启"
    );
    assert!(!c.adapters.cursor);
    assert!(!c.adapters.codex);
    assert!(!c.adapters.opencode);
    assert!(!c.adapters.aider);
    assert!(!c.adapters.continue_dev);
    assert!(!c.adapters.cline);
}

#[test]
fn default_privacy_has_redaction_on() {
    let c = MemexConfig::default();
    assert!(
        c.privacy.redaction_enabled,
        "默认必须开启脱敏（隐私默认安全）"
    );
    assert!(!c.privacy.skip_private_sessions);
}

#[test]
fn load_existing_config_with_empty_ollama_strings_repairs_to_defaults() {
    // 模拟 v0.2.3 存量用户：MemexConfig::default() 在旧版本下把空串写进了
    // config.toml。新版本 load 时必须把空串视为「字段缺失」并回填默认值，
    // 否则升级到新版后行为还是错的（toml 反序列化在「值存在但为空串」时
    // **不会**触发 #[serde(default = "...")]）。
    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    fs::write(
        &cfg_path,
        r#"data_dir = "~/.memex"

[adapters]
claude_code = true
cursor = true
codex = true
opencode = true
aider = true
continue_dev = true
cline = true

[llm]
ollama_enabled = true
ollama_url = ""
ollama_model = ""

[privacy]
redaction_enabled = true
skip_private_sessions = false
"#,
    )
    .unwrap();
    let c = MemexConfig::load(tmp.path()).unwrap();
    assert!(c.llm.ollama_enabled);
    assert_eq!(
        c.llm.ollama_url, "http://127.0.0.1:11434",
        "存量用户配置里 ollama_url='' 必须被当作未设置回填默认值"
    );
    assert_eq!(c.llm.ollama_model, "llama3.2");
}

#[test]
fn ensure_memex_dir_writes_nonempty_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    ensure_memex_dir(tmp.path()).unwrap();
    let written = fs::read_to_string(tmp.path().join("config.toml")).unwrap();
    assert!(
        written.contains("ollama_url = \"http://127.0.0.1:11434\""),
        "OOB config.toml 必须写入完整 ollama_url，否则新用户开关打开后 Ollama provider 选不上。\n实际写入:\n{}",
        written
    );
    assert!(
        written.contains("ollama_model = \"llama3.2\""),
        "实际写入:\n{}",
        written
    );
}
