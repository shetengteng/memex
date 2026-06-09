//! Cursor `~/.cursor/hooks.json` 协议处理。
//!
//! shape:
//! ```json
//! { "version": 1, "hooks": { "sessionStart": [ { "command": "<path>" } ] } }
//! ```
//!
//! 同时也包含 Cursor 的 wrapper 脚本模板字面量 —— Cursor 协议要求 stdout 返回
//! JSON `{ "additional_context": "<markdown>" }`，所以脚本里要做 markdown 包裹。

use std::path::Path;

use anyhow::{Context, Result};

use super::json_helpers::{
    has_memex_command, read_json_or_empty, remove_memex_entries, upsert_cmd_array, write_json,
};
use super::wrapper::WRAPPER_BANNER;

pub(super) const HOOK_KEY: &str = "sessionStart";

pub(super) fn upsert_hook(path: &Path, wrapper: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    let obj = v
        .as_object_mut()
        .context("hooks.json is not a JSON object")?;
    obj.entry("version".to_string())
        .or_insert(serde_json::json!(1));
    let hooks = obj
        .entry("hooks".to_string())
        .or_insert(serde_json::json!({}))
        .as_object_mut()
        .context("hooks field is not an object")?;
    upsert_cmd_array(hooks, HOOK_KEY, wrapper);
    write_json(path, &v)
}

pub(super) fn remove_hook(path: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        remove_memex_entries(hooks, HOOK_KEY);
    }
    write_json(path, &v)
}

pub(super) fn probe_hook(content: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .and_then(|v| v.get("hooks").cloned())
        .and_then(|h| h.get(HOOK_KEY).cloned())
        .map(|arr| has_memex_command(&arr))
        .unwrap_or(false)
}

/// Cursor wrapper:
///
/// - Cursor 文档明确说 sessionStart 是 fire-and-forget，注入有竞态。没有日志就盲。
/// - 每次执行都向 `~/.memex/hooks/last-run.log` 追加一行 JSON。
/// - stderr 也写一份，Cursor IDE 的 Output → Hooks 频道能直接看到。
pub(super) fn wrapper_body(bin: &dyn std::fmt::Display) -> (&'static str, String) {
    let body = format!(
        r#"#!/bin/sh
{banner}
# Cursor sessionStart hook:
#   must return JSON {{ "additional_context": "<markdown>" }} on stdout

set -e

LOG="$HOME/.memex/hooks/last-run.log"
mkdir -p "$(dirname "$LOG")"
TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

MD="$('{bin}' context --json 2>/dev/null || printf '%s' '{{"markdown":""}}')"
# use printf '%s' so /bin/sh does NOT interpret backslash escapes (e.g. \n)
# inside the JSON payload — otherwise json.loads sees raw newlines and dies
PY_OUTPUT="$(printf '%s' "$MD" | python3 -c 'import sys,json; d=json.loads(sys.stdin.read() or "{{}}"); print(d.get("markdown",""))' 2>/dev/null || printf '%s' "")"
OUT="$(python3 -c 'import sys,json; md=sys.stdin.read(); print(json.dumps({{"additional_context": md}}))' <<EOF
$PY_OUTPUT
EOF
)"

# log: ts / ide / cwd / context_len / output_len
CTX_LEN="$(printf '%s' "$PY_OUTPUT" | wc -c | tr -d ' ')"
OUT_LEN="$(printf '%s' "$OUT" | wc -c | tr -d ' ')"
printf '{{"ts":"%s","ide":"cursor","cwd":"%s","context_len":%s,"output_len":%s}}\n' \
    "$TS" "$PWD" "$CTX_LEN" "$OUT_LEN" >> "$LOG"
printf 'memex sessionStart fired ts=%s ctx_len=%s\n' "$TS" "$CTX_LEN" >&2

printf '%s' "$OUT"
"#,
        banner = WRAPPER_BANNER,
        bin = bin
    );
    ("cursor-session-start.sh", body)
}
