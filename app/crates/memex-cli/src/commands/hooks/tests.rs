//! `commands::hooks` 的端到端测试 —— 用临时目录模拟 `~/.cursor/hooks.json` 等
//! 配置文件，验证 install / uninstall / probe 的幂等性、协议形态、不破坏用户既有 hook。

use std::fs;
use std::path::Path;

use serde_json::json;

use super::super::setup::Ide;
use super::claude::{remove_hook as remove_claude_hook, upsert_hook as upsert_claude_hook};
use super::codex::upsert_hook as upsert_codex_hook;
use super::cursor::{remove_hook as remove_cursor_hook, upsert_hook as upsert_cursor_hook};
use super::status;

fn write_tmp_json(path: &Path, v: &serde_json::Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_string_pretty(v).unwrap()).unwrap();
}

#[test]
fn cursor_install_then_uninstall_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("cursor").join("hooks.json");
    // 模拟已有的用户 hook（项目自己的 audit.sh），install 不应把它干掉
    write_tmp_json(
        &cfg,
        &json!({
            "version": 1,
            "hooks": {
                "sessionStart": [{ "command": "./hooks/audit.sh" }]
            }
        }),
    );

    let wrapper = tmp
        .path()
        .join(".memex")
        .join("hooks")
        .join("cursor-session-start.sh");
    fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
    fs::write(&wrapper, "#!/bin/sh\n").unwrap();

    upsert_cursor_hook(&cfg, &wrapper).unwrap();

    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    let arr = v["hooks"]["sessionStart"].as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "memex 写入应保留用户既有 hook，结果:\n{:#}",
        v
    );
    assert!(arr.iter().any(|e| e["command"] == "./hooks/audit.sh"));

    // 第二次 install：不应产生重复 memex 条目（幂等）
    upsert_cursor_hook(&cfg, &wrapper).unwrap();
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    let arr = v["hooks"]["sessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 2, "幂等性失败，重复写入应去重，结果:\n{:#}", v);

    // uninstall：只移除 memex 那条
    remove_cursor_hook(&cfg).unwrap();
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    let arr = v["hooks"]["sessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 1, "卸载后应只剩用户原有 hook，结果:\n{:#}", v);
    assert_eq!(arr[0]["command"], "./hooks/audit.sh");
}

#[test]
fn claude_install_uses_session_start_envelope_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("claude").join("settings.json");
    // 用户已经手工配过一个 SessionStart hook
    write_tmp_json(
        &cfg,
        &json!({
            "hooks": {
                "SessionStart": [{
                    "matcher": "",
                    "hooks": [{ "type": "command", "command": "/usr/local/bin/their-script.sh" }]
                }]
            }
        }),
    );

    let wrapper = tmp
        .path()
        .join(".memex")
        .join("hooks")
        .join("claude-code-session-start.sh");
    fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
    fs::write(&wrapper, "#!/bin/sh\n").unwrap();

    upsert_claude_hook(&cfg, &wrapper).unwrap();

    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    let arr = v["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 2, "memex 与用户 hook 共存\n{:#}", v);
    // memex 条目应位于末尾
    assert_eq!(arr[1]["hooks"][0]["type"], "command");
    assert!(
        arr[1]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("/.memex/hooks/"),
        "memex 条目的 command 没指向 wrapper:\n{:#}",
        v
    );

    // 二次 install 仍幂等
    upsert_claude_hook(&cfg, &wrapper).unwrap();
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    assert_eq!(v["hooks"]["SessionStart"].as_array().unwrap().len(), 2);

    // 卸载只删 memex
    remove_claude_hook(&cfg).unwrap();
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    let arr = v["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(
        arr[0]["hooks"][0]["command"],
        "/usr/local/bin/their-script.sh"
    );
}

#[test]
fn codex_install_adds_matcher_for_all_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("codex").join("hooks.json");
    fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    let wrapper = tmp
        .path()
        .join(".memex")
        .join("hooks")
        .join("codex-session-start.sh");
    fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
    fs::write(&wrapper, "#!/bin/sh\n").unwrap();

    upsert_codex_hook(&cfg, &wrapper).unwrap();
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    let group = &v["hooks"]["SessionStart"][0];
    // 确保 startup / resume / clear / compact 都会触发
    assert_eq!(group["matcher"], "startup|resume|clear|compact");
    assert!(
        group["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("/.memex/hooks/")
    );
}

#[test]
fn opencode_is_marked_unsupported() {
    let st = status(Ide::OpenCode).unwrap();
    assert!(
        !st.supported,
        "OpenCode 应标记为 unsupported（plugin 体系）"
    );
    assert!(!st.installed);
}
