//! Claude Code `~/.claude/settings.json` 协议处理。
//!
//! shape:
//! ```json
//! { "hooks": { "SessionStart": [
//!   { "matcher": "", "hooks": [ { "type":"command","command":"..." } ] }
//! ] } }
//! ```
//!
//! 同时包含 Claude Code 的 wrapper 模板 —— 协议要求 stdout 是
//! `{ hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: "..." } }`。

use std::path::Path;

use anyhow::{Context, Result};

use super::json_helpers::{has_memex_command, purge_memex_grouped, read_json_or_empty, write_json};
use super::wrapper::WRAPPER_BANNER;

pub(super) const HOOK_KEY: &str = "SessionStart";

pub(super) fn upsert_hook(path: &Path, wrapper: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    let obj = v
        .as_object_mut()
        .context("settings.json is not a JSON object")?;
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
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": wrapper.to_string_lossy()
        }]
    }));
    write_json(path, &v)
}

pub(super) fn remove_hook(path: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    if let Some(arr) = v
        .get_mut("hooks")
        .and_then(|h| h.get_mut(HOOK_KEY))
        .and_then(|a| a.as_array_mut())
    {
        // 移除所有 nested.hooks[*].command 指向 memex hooks 目录的项
        arr.retain(|group| {
            let inner = group.get("hooks").and_then(|x| x.as_array());
            !matches!(inner, Some(arr) if has_memex_command(&serde_json::Value::Array(arr.clone())))
        });
    }
    write_json(path, &v)
}

pub(super) fn probe_hook(content: &str) -> bool {
    let v: serde_json::Value = match serde_json::from_str(content) {
        Ok(x) => x,
        Err(_) => return false,
    };
    let Some(arr) = v
        .get("hooks")
        .and_then(|h| h.get(HOOK_KEY))
        .and_then(|a| a.as_array())
    else {
        return false;
    };
    arr.iter().any(|group| {
        group
            .get("hooks")
            .map(|inner| has_memex_command(inner))
            .unwrap_or(false)
    })
}

/// Claude Code wrapper:
///
/// - Claude Code 2.1+ 必须用 `hookSpecificOutput` 信封；2.0 也接受这种。
/// - 同 Cursor wrapper：每次执行都向 `last-run.log` 追加一行；
///   stderr breadcrumb 给 Claude Code 自己的 hook output 频道看。
pub(super) fn wrapper_body(bin: &dyn std::fmt::Display) -> (&'static str, String) {
    let body = format!(
        r#"#!/bin/sh
{banner}
# Claude Code SessionStart hook:
#   docs require this JSON envelope:
#     {{ hookSpecificOutput: {{ hookEventName: "SessionStart",
#                              additionalContext: "<markdown>" }} }}
# memex context exit code is always 0; even if no project matches we
# still emit a banner so AI knows Memex is wired up.

set -e

LOG="$HOME/.memex/hooks/last-run.log"
mkdir -p "$(dirname "$LOG")"
TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

MD="$('{bin}' context --json 2>/dev/null || printf '%s' '{{"markdown":""}}')"
# extract markdown safely with python (no jq dependency)
# use printf '%s' so /bin/sh does NOT interpret backslash escapes (e.g. \n)
# inside the JSON payload — otherwise json.loads sees raw newlines and dies
PY_OUTPUT="$(printf '%s' "$MD" | python3 -c 'import sys,json; d=json.loads(sys.stdin.read() or "{{}}"); print(d.get("markdown",""))' 2>/dev/null || printf '%s' "")"
OUT="$(python3 -c 'import sys,json; md=sys.stdin.read(); print(json.dumps({{"hookSpecificOutput":{{"hookEventName":"SessionStart","additionalContext": md}}}}))' <<EOF
$PY_OUTPUT
EOF
)"

CTX_LEN="$(printf '%s' "$PY_OUTPUT" | wc -c | tr -d ' ')"
OUT_LEN="$(printf '%s' "$OUT" | wc -c | tr -d ' ')"
printf '{{"ts":"%s","ide":"claude-code","cwd":"%s","context_len":%s,"output_len":%s}}\n' \
    "$TS" "$PWD" "$CTX_LEN" "$OUT_LEN" >> "$LOG"
printf 'memex sessionStart fired ide=claude-code ts=%s ctx_len=%s\n' "$TS" "$CTX_LEN" >&2

printf '%s' "$OUT"
"#,
        banner = WRAPPER_BANNER,
        bin = bin
    );
    ("claude-code-session-start.sh", body)
}
