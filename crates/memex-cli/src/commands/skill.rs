//! SKILL.md 安装：把仓库内 skills/<ide>/SKILL.md 投递到对应 IDE 的
//! "skills/commands/prompts" 目录。
//!
//! 设计要点：
//! - SKILL.md 源文件通过 `include_str!` 嵌入二进制，brew/dmg 装的用户也能用。
//! - 4 个 IDE 安装路径差异极大，所以每个 IDE 单独定义 dest path + 内容来源。
//! - install/uninstall/status 全部幂等。
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Serialize;

use super::setup::Ide;

const CURSOR_SKILL: &str = include_str!("../../../../skills/cursor/SKILL.md");
const CLAUDE_CODE_SKILL: &str = include_str!("../../../../skills/claude-code/SKILL.md");
const CODEX_SKILL: &str = include_str!("../../../../skills/codex/SKILL.md");
const OPENCODE_SKILL: &str = include_str!("../../../../skills/opencode/SKILL.md");

#[derive(Debug, Clone, Serialize)]
pub struct SkillStatus {
    pub ide: String,
    pub dest_path: String,
    pub installed: bool,
    /// 若已安装，安装文件的字节数。
    pub size: Option<u64>,
}

fn dest_path(ide: Ide) -> PathBuf {
    let home = dirs::home_dir().expect("cannot determine home directory");
    match ide {
        // Cursor / Claude Code 都用 skills/<name>/SKILL.md 约定。
        Ide::Cursor => home.join(".cursor").join("skills").join("memex").join("SKILL.md"),
        Ide::ClaudeCode => home.join(".claude").join("skills").join("memex").join("SKILL.md"),
        // Codex 没有 skills 目录，用 prompts/<name>.md（通过 `/<name>` 调用）。
        Ide::Codex => home.join(".codex").join("prompts").join("memex.md"),
        // OpenCode 用 commands/<name>.md（也是 slash command）。
        Ide::OpenCode => home
            .join(".config")
            .join("opencode")
            .join("commands")
            .join("memex.md"),
    }
}

fn skill_content(ide: Ide) -> &'static str {
    match ide {
        Ide::Cursor => CURSOR_SKILL,
        Ide::ClaudeCode => CLAUDE_CODE_SKILL,
        Ide::Codex => CODEX_SKILL,
        Ide::OpenCode => OPENCODE_SKILL,
    }
}

pub fn install(ide: Ide) -> Result<SkillStatus> {
    let path = dest_path(ide);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&path, skill_content(ide))
        .with_context(|| format!("failed to write {}", path.display()))?;
    println!("{} skill installed at {}", ide.as_str(), path.display());
    status(ide)
}

pub fn uninstall(ide: Ide) -> Result<SkillStatus> {
    let path = dest_path(ide);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
        println!("{} skill removed from {}", ide.as_str(), path.display());
    } else {
        println!(
            "{} skill not installed, nothing to remove ({})",
            ide.as_str(),
            path.display()
        );
    }
    status(ide)
}

pub fn status(ide: Ide) -> Result<SkillStatus> {
    let path = dest_path(ide);
    let exists = path.exists();
    let size = if exists {
        fs::metadata(&path).ok().map(|m| m.len())
    } else {
        None
    };
    Ok(SkillStatus {
        ide: ide.as_str().to_string(),
        dest_path: path.to_string_lossy().to_string(),
        installed: exists,
        size,
    })
}

pub fn list_status() -> Vec<SkillStatus> {
    Ide::all()
        .iter()
        .map(|ide| status(*ide).unwrap_or_else(|_| SkillStatus {
            ide: ide.as_str().to_string(),
            dest_path: dest_path(*ide).to_string_lossy().to_string(),
            installed: false,
            size: None,
        }))
        .collect()
}
