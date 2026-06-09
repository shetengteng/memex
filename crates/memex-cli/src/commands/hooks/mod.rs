//! `memex setup hooks <install|uninstall|status>` —— 把 `memex context` 命令
//! 绑定到各 IDE 的 SessionStart hook，让 AI 会话启动时自动注入项目工作记忆。
//!
//! 设计要点：
//!
//! - **只用现有的 IDE hook 协议**：Claude Code `SessionStart`、Cursor
//!   `sessionStart`、Codex `SessionStart`。OpenCode 是 TS plugin 体系，
//!   不在本命令的自动注入范围内 —— 列出 `unsupported` 状态即可。
//!
//! - **wrapper 脚本独立可执行**：往 `~/.memex/hooks/` 写一个小 sh wrapper，
//!   wrapper 内调用 `memex context`，再按 IDE 的协议输出 stdout。
//!   IDE 配置只指向这个 wrapper，将来切换 wrapper 内容不需要再改 IDE 配置。
//!
//! - **幂等 / 可卸载**：写入前先检测条目是否存在；卸载用同样的 key 精确移除，
//!   不动用户加的其他 hook。
//!
//! 拆分布局：
//! - `mod.rs`           —— pub API + `HookStatus` + `supports` + `hook_config_path`
//! - `wrapper.rs`       —— `ensure_wrapper`：写 wrapper 脚本到 `~/.memex/hooks/`
//! - `cursor.rs` / `claude.rs` / `codex.rs` —— 各 IDE 的协议处理 + wrapper 模板
//! - `json_helpers.rs`  —— `~/.cursor/hooks.json` 等配置文件的 JSON 读写
//! - `tests.rs`         —— 4 个集成测试（install/uninstall 幂等、协议形态、unsupported）

mod claude;
mod codex;
mod cursor;
mod json_helpers;
mod wrapper;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use super::setup::Ide;
use wrapper::ensure_wrapper;

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
        Ide::Cursor => cursor::upsert_hook(&cfg, &wrapper)?,
        Ide::ClaudeCode => claude::upsert_hook(&cfg, &wrapper)?,
        Ide::Codex => codex::upsert_hook(&cfg, &wrapper)?,
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

/// 把 hook 配置精确移除（保留同文件里其他 hook），同时删除 wrapper 脚本。
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
            Ide::Cursor => cursor::remove_hook(&cfg)?,
            Ide::ClaudeCode => claude::remove_hook(&cfg)?,
            Ide::Codex => codex::remove_hook(&cfg)?,
            Ide::OpenCode => {}
        }
    }
    let wrapper = wrapper_path_for(ide);
    if wrapper.exists() {
        let _ = fs::remove_file(&wrapper);
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
    let content =
        fs::read_to_string(&cfg).with_context(|| format!("failed to read {}", cfg.display()))?;
    let config_has_entry = match ide {
        Ide::Cursor => cursor::probe_hook(&content),
        Ide::ClaudeCode => claude::probe_hook(&content),
        Ide::Codex => codex::probe_hook(&content),
        Ide::OpenCode => false,
    };
    let wrapper = wrapper_path_for(ide);
    let wrapper_exists = wrapper.exists();
    let installed = config_has_entry && wrapper_exists;
    Ok(HookStatus {
        ide: ide.as_str().to_string(),
        supported: true,
        installed,
        config_path: cfg.to_string_lossy().to_string(),
        wrapper_path: if wrapper_exists {
            Some(wrapper.to_string_lossy().to_string())
        } else {
            None
        },
    })
}

fn wrapper_path_for(ide: Ide) -> PathBuf {
    // INVARIANT: HOME (or USERPROFILE on Windows) is set on every OS we ship
    // on. memex CLI never runs in a sandboxed environment that strips $HOME
    // (no Docker / no systemd-nspawn), and dirs crate falls back to passwd
    // entries on POSIX. If this ever fails, the whole CLI is unusable —
    // panic'ing here is honest about that.
    let home =
        dirs::home_dir().expect("INVARIANT: home directory must be resolvable for memex CLI");
    let hooks_dir = home.join(".memex").join(wrapper::HOOK_DIRNAME);
    let name = match ide {
        Ide::Cursor => "cursor-session-start.sh",
        Ide::ClaudeCode => "claude-code-session-start.sh",
        Ide::Codex => "codex-session-start.sh",
        Ide::OpenCode => "opencode-session-start.sh",
    };
    hooks_dir.join(name)
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
    // INVARIANT: see `wrapper_path_for` — HOME must be resolvable for CLI.
    let home =
        dirs::home_dir().expect("INVARIANT: home directory must be resolvable for memex CLI");
    match ide {
        Ide::Cursor => home.join(".cursor").join("hooks.json"),
        Ide::ClaudeCode => home.join(".claude").join("settings.json"),
        Ide::Codex => home.join(".codex").join("hooks.json"),
        Ide::OpenCode => home.join(".config").join("opencode").join("opencode.json"), // 仅作占位，install 不会用
    }
}

#[cfg(test)]
mod tests;
