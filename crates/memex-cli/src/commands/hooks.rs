//! `memex setup hooks <install|uninstall|status>` —— 把 `memex context` 命令
//! 绑定到各 IDE 的 SessionStart hook，让 AI 会话启动时自动注入项目工作记忆。
//!
//! 设计要点：
//!
//! - **只用现有的 IDE hook 协议**：Claude Code `SessionStart`、Cursor
//!   `sessionStart`、Codex `SessionStart`。OpenCode 是 TS plugin 体系，
//!   不在本命令的自动注入范围内 —— 列出 `unsupported` 状态即可。
//!
//! - **wrapper 脚本独立可执行**：我们往 `~/.memex/hooks/` 写一个小 sh wrapper，
//!   wrapper 内调用 `memex context --json`，再按 IDE 的 JSON shape 套壳。
//!   IDE 配置只指向这个 wrapper，将来切换 wrapper 内容不需要再改 IDE 配置。
//!
//! - **幂等 / 可卸载**：写入前先检测条目是否存在；卸载用同样的 key 精确移除，
//!   不动用户加的其他 hook。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use super::setup::Ide;

/// 每个 wrapper 脚本第一行打头的标识，方便我们升级时识别"这是 memex 装的"。
const WRAPPER_BANNER: &str = "# memex hook wrapper — do not edit by hand";
const HOOK_DIRNAME: &str = "hooks";

#[derive(Debug, Clone, Serialize)]
pub struct HookStatus {
    pub ide: String,
    pub supported: bool,
    pub installed: bool,
    pub config_path: String,
    pub wrapper_path: Option<String>,
}

/// 装 hook 到指定 IDE。同时（如果不存在）写好 wrapper 脚本。
pub fn install(ide: Ide, memex_bin: &Path, memex_home: &Path) -> Result<HookStatus> {
    if !supports(ide) {
        return Ok(HookStatus {
            ide: ide.as_str().to_string(),
            supported: false,
            installed: false,
            config_path: hook_config_path(ide).to_string_lossy().to_string(),
            wrapper_path: None,
        });
    }

    let wrapper = ensure_wrapper(ide, memex_bin, memex_home)?;
    let cfg = hook_config_path(ide);
    if let Some(parent) = cfg.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match ide {
        Ide::Cursor => upsert_cursor_hook(&cfg, &wrapper)?,
        Ide::ClaudeCode => upsert_claude_hook(&cfg, &wrapper)?,
        Ide::Codex => upsert_codex_hook(&cfg, &wrapper)?,
        Ide::OpenCode => {} // 上面已经 return 了
    }

    Ok(HookStatus {
        ide: ide.as_str().to_string(),
        supported: true,
        installed: true,
        config_path: cfg.to_string_lossy().to_string(),
        wrapper_path: Some(wrapper.to_string_lossy().to_string()),
    })
}

/// 把 hook 配置精确移除（保留同文件里其他 hook）。
/// wrapper 脚本本身**不**删除，避免用户曾经自己手改过；CLI 输出会提示路径。
pub fn uninstall(ide: Ide) -> Result<HookStatus> {
    if !supports(ide) {
        return Ok(HookStatus {
            ide: ide.as_str().to_string(),
            supported: false,
            installed: false,
            config_path: hook_config_path(ide).to_string_lossy().to_string(),
            wrapper_path: None,
        });
    }
    let cfg = hook_config_path(ide);
    if cfg.exists() {
        match ide {
            Ide::Cursor => remove_cursor_hook(&cfg)?,
            Ide::ClaudeCode => remove_claude_hook(&cfg)?,
            Ide::Codex => remove_codex_hook(&cfg)?,
            Ide::OpenCode => {}
        }
    }
    Ok(HookStatus {
        ide: ide.as_str().to_string(),
        supported: true,
        installed: false,
        config_path: cfg.to_string_lossy().to_string(),
        wrapper_path: None,
    })
}

pub fn status(ide: Ide) -> Result<HookStatus> {
    let cfg = hook_config_path(ide);
    if !supports(ide) {
        return Ok(HookStatus {
            ide: ide.as_str().to_string(),
            supported: false,
            installed: false,
            config_path: cfg.to_string_lossy().to_string(),
            wrapper_path: None,
        });
    }
    if !cfg.exists() {
        return Ok(HookStatus {
            ide: ide.as_str().to_string(),
            supported: true,
            installed: false,
            config_path: cfg.to_string_lossy().to_string(),
            wrapper_path: None,
        });
    }
    let content = fs::read_to_string(&cfg)
        .with_context(|| format!("failed to read {}", cfg.display()))?;
    let installed = match ide {
        Ide::Cursor => probe_cursor_hook(&content),
        Ide::ClaudeCode => probe_claude_hook(&content),
        Ide::Codex => probe_codex_hook(&content),
        Ide::OpenCode => false,
    };
    Ok(HookStatus {
        ide: ide.as_str().to_string(),
        supported: true,
        installed,
        config_path: cfg.to_string_lossy().to_string(),
        wrapper_path: None,
    })
}

pub fn list_status() -> Vec<HookStatus> {
    Ide::all()
        .iter()
        .map(|ide| {
            status(*ide).unwrap_or_else(|_| HookStatus {
                ide: ide.as_str().to_string(),
                supported: supports(*ide),
                installed: false,
                config_path: hook_config_path(*ide).to_string_lossy().to_string(),
                wrapper_path: None,
            })
        })
        .collect()
}

fn supports(ide: Ide) -> bool {
    !matches!(ide, Ide::OpenCode)
}

/// 每个 IDE 的 hook 配置文件路径。
///
/// 注意 Claude Code 这里**不复用** `Ide::primary_config()`（那个返回的是
/// `~/.claude.json`，给 MCP servers 用）。Hook 走 `~/.claude/settings.json`。
fn hook_config_path(ide: Ide) -> PathBuf {
    let home = dirs::home_dir().expect("cannot determine home directory");
    match ide {
        Ide::Cursor => home.join(".cursor").join("hooks.json"),
        Ide::ClaudeCode => home.join(".claude").join("settings.json"),
        Ide::Codex => home.join(".codex").join("hooks.json"),
        Ide::OpenCode => home
            .join(".config")
            .join("opencode")
            .join("opencode.json"), // 仅作占位，install 不会用
    }
}

// ----------- wrapper 生成 -----------

/// 写 `~/.memex/hooks/<ide>-session-start.sh`。脚本职责：调 `memex context --json`
/// 拿到 `{ project_path, markdown }`，按各 IDE 的协议输出。
///
/// 不同 IDE 字段名不同（不能一份脚本通用），所以按 ide 分文件。
fn ensure_wrapper(ide: Ide, memex_bin: &Path, memex_home: &Path) -> Result<PathBuf> {
    let dir = memex_home.join(HOOK_DIRNAME);
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create {}", dir.display()))?;

    let (name, body) = wrapper_body(ide, memex_bin);
    let path = dir.join(name);
    fs::write(&path, body)
        .with_context(|| format!("failed to write {}", path.display()))?;

    // 加可执行位
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)?;
    }
    Ok(path)
}

fn wrapper_body(ide: Ide, memex_bin: &Path) -> (&'static str, String) {
    let bin = memex_bin.display();
    match ide {
        Ide::ClaudeCode => {
            // Claude Code 2.1+ 必须用 hookSpecificOutput 信封；2.0 也接受这种。
            // jq 接管 JSON 装配；机器上必装 jq —— Claude Code 用户通常已有。
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
MD="$('{bin}' context --json 2>/dev/null || printf '%s' '{{"markdown":""}}')"
# extract markdown safely with python (no jq dependency)
# use printf '%s' so /bin/sh does NOT interpret backslash escapes (e.g. \n)
# inside the JSON payload — otherwise json.loads sees raw newlines and dies
PY_OUTPUT="$(printf '%s' "$MD" | python3 -c 'import sys,json; d=json.loads(sys.stdin.read() or "{{}}"); print(d.get("markdown",""))' 2>/dev/null || printf '%s' "")"
python3 -c 'import sys,json; md=sys.stdin.read(); print(json.dumps({{"hookSpecificOutput":{{"hookEventName":"SessionStart","additionalContext": md}}}}))' <<EOF
$PY_OUTPUT
EOF
"#,
                banner = WRAPPER_BANNER,
                bin = bin
            );
            ("claude-code-session-start.sh", body)
        }
        Ide::Cursor => {
            // Cursor sessionStart: { env, additional_context } (snake_case)
            //
            // 诊断日志：每次执行都向 ~/.memex/hooks/last-run.log 追加一行 JSON。
            // 这是确认"Cursor 到底有没有调到这个脚本"的唯一靠谱手段——Cursor 文档
            // 明确说 sessionStart 是 fire-and-forget，注入有竞态，没有日志就盲。
            // stderr 也写一份，Cursor IDE 的 Output → Hooks 频道能直接看到。
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
        Ide::Codex => {
            // Codex SessionStart 接受 stdout 纯文本作为 developer context；
            // 直接把 markdown 打出来即可。
            let body = format!(
                r#"#!/bin/sh
{banner}
# Codex SessionStart hook: stdout plain text → developer context
set -e
'{bin}' context 2>/dev/null || true
"#,
                banner = WRAPPER_BANNER,
                bin = bin
            );
            ("codex-session-start.sh", body)
        }
        Ide::OpenCode => ("opencode-session-start.sh", String::new()),
    }
}

// ----------- Cursor: ~/.cursor/hooks.json -----------
//
// shape:
//   { "version": 1, "hooks": { "sessionStart": [ { "command": "<path>" } ] } }

const CURSOR_HOOK_KEY: &str = "sessionStart";

fn upsert_cursor_hook(path: &Path, wrapper: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    let obj = v.as_object_mut().context("hooks.json is not a JSON object")?;
    obj.entry("version".to_string())
        .or_insert(serde_json::json!(1));
    let hooks = obj
        .entry("hooks".to_string())
        .or_insert(serde_json::json!({}))
        .as_object_mut()
        .context("hooks field is not an object")?;
    upsert_cmd_array(hooks, CURSOR_HOOK_KEY, wrapper);
    write_json(path, &v)
}

fn remove_cursor_hook(path: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    if let Some(hooks) = v
        .get_mut("hooks")
        .and_then(|h| h.as_object_mut())
    {
        remove_memex_entries(hooks, CURSOR_HOOK_KEY);
    }
    write_json(path, &v)
}

fn probe_cursor_hook(content: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .and_then(|v| v.get("hooks").cloned())
        .and_then(|h| h.get(CURSOR_HOOK_KEY).cloned())
        .map(|arr| has_memex_command(&arr))
        .unwrap_or(false)
}

// ----------- Claude Code: ~/.claude/settings.json -----------
//
// shape:
//   { "hooks": { "SessionStart": [ { "matcher": "", "hooks": [ { "type":"command","command":"..." } ] } ] } }

const CLAUDE_HOOK_KEY: &str = "SessionStart";

fn upsert_claude_hook(path: &Path, wrapper: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    let obj = v.as_object_mut().context("settings.json is not a JSON object")?;
    let hooks = obj
        .entry("hooks".to_string())
        .or_insert(serde_json::json!({}))
        .as_object_mut()
        .context("hooks field is not an object")?;

    let entries = hooks
        .entry(CLAUDE_HOOK_KEY.to_string())
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .context("SessionStart is not an array")?;
    purge_memex_grouped(entries, wrapper);
    entries.push(serde_json::json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": wrapper.to_string_lossy()
        }]
    }));
    write_json(path, &v)
}

fn remove_claude_hook(path: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    if let Some(arr) = v
        .get_mut("hooks")
        .and_then(|h| h.get_mut(CLAUDE_HOOK_KEY))
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

fn probe_claude_hook(content: &str) -> bool {
    let v: serde_json::Value = match serde_json::from_str(content) {
        Ok(x) => x,
        Err(_) => return false,
    };
    let Some(arr) = v
        .get("hooks")
        .and_then(|h| h.get(CLAUDE_HOOK_KEY))
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

// ----------- Codex: ~/.codex/hooks.json -----------
//
// shape:
//   { "hooks": { "SessionStart": [ { "matcher": "startup|resume|clear|compact",
//                                    "hooks": [ { "type":"command","command":"..." } ] } ] } }

const CODEX_HOOK_KEY: &str = "SessionStart";

fn upsert_codex_hook(path: &Path, wrapper: &Path) -> Result<()> {
    let mut v = read_json_or_empty(path)?;
    let obj = v.as_object_mut().context("hooks.json is not a JSON object")?;
    let hooks = obj
        .entry("hooks".to_string())
        .or_insert(serde_json::json!({}))
        .as_object_mut()
        .context("hooks field is not an object")?;
    let entries = hooks
        .entry(CODEX_HOOK_KEY.to_string())
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .context("SessionStart is not an array")?;
    purge_memex_grouped(entries, wrapper);
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

fn remove_codex_hook(path: &Path) -> Result<()> {
    remove_claude_hook(path) // 形态完全一致，复用
}

fn probe_codex_hook(content: &str) -> bool {
    probe_claude_hook(content)
}

// ----------- shared helpers -----------

/// Cursor 风格的简单条目数组：[{ "command": "<wrapper>" }, ...]
fn upsert_cmd_array(parent: &mut serde_json::Map<String, serde_json::Value>, key: &str, wrapper: &Path) {
    let arr = parent
        .entry(key.to_string())
        .or_insert(serde_json::json!([]))
        .as_array_mut();
    if let Some(arr) = arr {
        // 移除任何旧的 memex 条目（identify by command path containing /hooks/）
        arr.retain(|entry| {
            entry.get("command")
                .and_then(|c| c.as_str())
                .map(|s| !is_memex_command(s))
                .unwrap_or(true)
        });
        arr.push(serde_json::json!({
            "command": wrapper.to_string_lossy()
        }));
    }
}

fn remove_memex_entries(parent: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    if let Some(arr) = parent.get_mut(key).and_then(|a| a.as_array_mut()) {
        arr.retain(|entry| {
            entry.get("command")
                .and_then(|c| c.as_str())
                .map(|s| !is_memex_command(s))
                .unwrap_or(true)
        });
    }
}

fn purge_memex_grouped(arr: &mut Vec<serde_json::Value>, _wrapper: &Path) {
    arr.retain(|group| {
        let Some(inner) = group.get("hooks").and_then(|x| x.as_array()) else { return true; };
        !inner.iter().any(|h| {
            h.get("command").and_then(|c| c.as_str())
                .map(is_memex_command).unwrap_or(false)
        })
    });
}

fn has_memex_command(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Array(arr) => arr.iter().any(has_memex_command),
        serde_json::Value::Object(o) => o
            .get("command")
            .and_then(|c| c.as_str())
            .map(is_memex_command)
            .unwrap_or(false),
        _ => false,
    }
}

fn is_memex_command(cmd: &str) -> bool {
    // 我们写入的 wrapper 都在 ~/.memex/hooks/*-session-start.sh，
    // 这是足够稳定的识别签名。
    cmd.contains("/.memex/hooks/")
        || cmd.contains("/memex/hooks/")  // for tests / atypical paths
        || cmd.ends_with("memex-session-start.sh")
}

fn read_json_or_empty(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    Ok(serde_json::from_str(&content).unwrap_or(serde_json::json!({})))
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    let out = serde_json::to_string_pretty(value)?;
    fs::write(path, out)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_tmp_json(path: &Path, v: &serde_json::Value) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_string_pretty(v).unwrap()).unwrap();
    }

    #[test]
    fn cursor_install_then_uninstall_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cursor").join("hooks.json");
        // 模拟已有的用户 hook（项目自己的 audit.sh），install 不应把它干掉
        write_tmp_json(&cfg, &json!({
            "version": 1,
            "hooks": {
                "sessionStart": [{ "command": "./hooks/audit.sh" }]
            }
        }));

        let wrapper = tmp.path().join(".memex").join("hooks").join("cursor-session-start.sh");
        fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
        fs::write(&wrapper, "#!/bin/sh\n").unwrap();

        upsert_cursor_hook(&cfg, &wrapper).unwrap();

        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
        let arr = v["hooks"]["sessionStart"].as_array().unwrap();
        assert_eq!(arr.len(), 2, "memex 写入应保留用户既有 hook，结果:\n{:#}", v);
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
        write_tmp_json(&cfg, &json!({
            "hooks": {
                "SessionStart": [{
                    "matcher": "",
                    "hooks": [{ "type": "command", "command": "/usr/local/bin/their-script.sh" }]
                }]
            }
        }));

        let wrapper = tmp.path().join(".memex").join("hooks").join("claude-code-session-start.sh");
        fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
        fs::write(&wrapper, "#!/bin/sh\n").unwrap();

        upsert_claude_hook(&cfg, &wrapper).unwrap();

        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
        let arr = v["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(arr.len(), 2, "memex 与用户 hook 共存\n{:#}", v);
        // memex 条目应位于末尾
        assert_eq!(arr[1]["hooks"][0]["type"], "command");
        assert!(
            arr[1]["hooks"][0]["command"].as_str().unwrap().contains("/.memex/hooks/"),
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
        assert_eq!(arr[0]["hooks"][0]["command"], "/usr/local/bin/their-script.sh");
    }

    #[test]
    fn codex_install_adds_matcher_for_all_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("codex").join("hooks.json");
        fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        let wrapper = tmp.path().join(".memex").join("hooks").join("codex-session-start.sh");
        fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
        fs::write(&wrapper, "#!/bin/sh\n").unwrap();

        upsert_codex_hook(&cfg, &wrapper).unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
        let group = &v["hooks"]["SessionStart"][0];
        // 确保 startup / resume / clear / compact 都会触发
        assert_eq!(group["matcher"], "startup|resume|clear|compact");
        assert!(group["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("/.memex/hooks/"));
    }

    #[test]
    fn opencode_is_marked_unsupported() {
        let st = status(Ide::OpenCode).unwrap();
        assert!(!st.supported, "OpenCode 应标记为 unsupported（plugin 体系）");
        assert!(!st.installed);
    }
}
