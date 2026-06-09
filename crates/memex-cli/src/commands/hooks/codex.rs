//! Codex `~/.codex/hooks.json` 协议处理。
//!
//! shape:
//! ```json
//! { "hooks": { "SessionStart": [
//!   { "matcher": "startup|resume|clear|compact",
//!     "hooks": [ { "type":"command", "command":"...", "statusMessage": "..." } ] }
//! ] } }
//! ```
//!
//! Codex 协议接受 plain text stdout 作为 developer context（也支持
//! `hookSpecificOutput` JSON 信封，但 plain text 更轻量，无 jq 依赖）。

use std::path::Path;

use anyhow::{Context, Result};

use super::claude;
use super::json_helpers::{purge_memex_grouped, read_json_or_empty, write_json};
use super::wrapper::WRAPPER_BANNER;

pub(super) const HOOK_KEY: &str = "SessionStart";

pub(super) fn upsert_hook(path: &Path, wrapper: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    let obj = v
        .as_object_mut()
        .context("hooks.json is not a JSON object")?;
    let hooks = obj
        .entry("hooks".to_string())
        .or_insert(serde_json::json!({}))
        .as_object_mut()
        .context("hooks field is not an object")?;
    let entries = hooks
        .entry(HOOK_KEY.to_string())
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .context("SessionStart is not an array")?;
    purge_memex_grouped(entries);
    entries.push(serde_json::json!({
        "matcher": "startup|resume|clear|compact",
        "hooks": [{
            "type": "command",
            "command": wrapper.to_string_lossy(),
            "statusMessage": "Memex injecting project memory"
        }]
    }));
    write_json(path, &v)
}

pub(super) fn remove_hook(path: &Path) -> Result<()> {
    // Codex 的 shape 与 Claude Code 完全一致，复用 claude 的实现。
    claude::remove_hook(path)
}

pub(super) fn probe_hook(content: &str) -> bool {
    claude::probe_hook(content)
}

/// Codex wrapper:
///
/// - 协议接受 plain text stdout 作为 developer context（亦支持 JSON envelope）。
/// - **stdout 极敏感**：plain text 模式下 stdout 的每一个字节都会进 developer context。
///   所以诊断日志必须走 stderr，绝不能写 stdout。
pub(super) fn wrapper_body(bin: &dyn std::fmt::Display) -> (&'static str, String) {
    let body = format!(
        r#"#!/bin/sh
{banner}
# Codex SessionStart hook: stdout plain text → developer context
set -e

LOG="$HOME/.memex/hooks/last-run.log"
mkdir -p "$(dirname "$LOG")"
TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

CTX="$('{bin}' context 2>/dev/null || true)"
CTX_LEN="$(printf '%s' "$CTX" | wc -c | tr -d ' ')"
printf '{{"ts":"%s","ide":"codex","cwd":"%s","context_len":%s,"output_len":%s}}\n' \
    "$TS" "$PWD" "$CTX_LEN" "$CTX_LEN" >> "$LOG"
printf 'memex sessionStart fired ide=codex ts=%s ctx_len=%s\n' "$TS" "$CTX_LEN" >&2

printf '%s' "$CTX"
"#,
        banner = WRAPPER_BANNER,
        bin = bin
    );
    ("codex-session-start.sh", body)
}
