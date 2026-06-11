//! Rules 安装：把 memex 使用规则（强制 AI 主动调 memex MCP）投递到 4 个 IDE
//! 的全局规则文件。
//!
//! 设计：
//! - **规则正文单源化**：`rules/memex-rules.md` 通过 `include_str!` 嵌入
//!   binary，所有 IDE 共用，避免 4 份近似副本漂移。
//! - **两套部署形态**：
//!   - **Cursor**：mdc 单文件直接写到 `~/.cursor/rules/memex.mdc`（Cursor 自带的
//!     `alwaysApply: true` frontmatter 在 install 时拼上）。
//!   - **Claude Code / Codex / OpenCode**：单文件 `<...>.md` 用 BEGIN/END marker
//!     块追加到用户已有的 `CLAUDE.md` / `AGENTS.md`，**不**覆盖用户已写的其它指令。
//! - 所有操作幂等：install 多次结果一致；uninstall 只移除 memex 自己写的块。
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;

use super::setup::Ide;

const SHARED_RULE_BODY: &str = include_str!("../../../../rules/memex-rules.md");

/// Cursor `.mdc` 的 frontmatter；强制让 Cursor `alwaysApply` 这条规则。
const CURSOR_FRONTMATTER: &str = "---\nalwaysApply: true\n---\n\n";

/// 块开始 marker —— 找到这一行就找到 memex 管理块的入口。
const BLOCK_START: &str = "<!-- MEMEX-RULES:START (managed by `memex rules install`) -->";
/// 块结束 marker —— 与 START 配对；之间的内容由 memex 完全拥有。
const BLOCK_END: &str = "<!-- MEMEX-RULES:END -->";

#[derive(Debug, Clone, Serialize)]
pub struct RuleStatus {
    pub ide: String,
    /// 目标文件全路径（Cursor 是 mdc 文件；其他 IDE 是 IDE 自身的 instructions 文件）。
    pub dest_path: String,
    /// 当前 memex 规则是否已生效：Cursor = mdc 文件存在；其他 IDE = 目标文件含 marker 块。
    pub installed: bool,
    /// 已安装时上报的字节数。Cursor 是整文件；其他 IDE 是 marker 块内容（不含 marker 行）。
    pub size: Option<u64>,
    /// v1.x 起所有 4 个 IDE 都支持；保留字段让未来出现新 IDE 时仍可优雅降级。
    pub supported: bool,
}

/// 该 IDE 是否在当前版本支持 rules install。
fn is_supported(_ide: Ide) -> bool {
    true
}

fn dest_path(ide: Ide) -> PathBuf {
    let home = dirs::home_dir().expect("INVARIANT: home directory must be resolvable");
    match ide {
        Ide::Cursor => home.join(".cursor").join("rules").join("memex.mdc"),
        // Claude Code 的全局 system instructions 入口；不存在时 install 时创建。
        Ide::ClaudeCode => home.join(".claude").join("CLAUDE.md"),
        // Codex CLI 的全局 instructions；与 OpenAI Codex 的 AGENTS.md 协议一致。
        Ide::Codex => home.join(".codex").join("AGENTS.md"),
        // OpenCode 是 sst 的 AI shell，instructions 走 `~/.config/opencode/AGENTS.md`。
        Ide::OpenCode => home.join(".config").join("opencode").join("AGENTS.md"),
    }
}

/// 是否走 Cursor 风格的单文件覆盖；否则走 BEGIN/END marker 块管理。
fn uses_managed_block(ide: Ide) -> bool {
    !matches!(ide, Ide::Cursor)
}

/// 返回最终要写到 Cursor mdc 的完整内容（frontmatter + body）。
fn cursor_full_content() -> String {
    format!("{CURSOR_FRONTMATTER}{SHARED_RULE_BODY}")
}

pub fn install(ide: Ide) -> Result<RuleStatus> {
    if !is_supported(ide) {
        return Err(anyhow!(
            "rules install for {} is not supported in this build",
            ide.as_str()
        ));
    }
    let path = dest_path(ide);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if uses_managed_block(ide) {
        upsert_managed_block(&path, SHARED_RULE_BODY)
            .with_context(|| format!("failed to upsert managed block into {}", path.display()))?;
    } else {
        // Cursor：直接覆盖（mdc 是 memex 独占文件，不与用户其他规则共存）。
        fs::write(&path, cursor_full_content())
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    crate::out!("{} rule installed at {}", ide.as_str(), path.display());
    status(ide)
}

pub fn uninstall(ide: Ide) -> Result<RuleStatus> {
    let path = dest_path(ide);
    if !path.exists() {
        crate::out!(
            "{} rule not installed, nothing to remove ({})",
            ide.as_str(),
            path.display()
        );
        return status(ide);
    }

    if uses_managed_block(ide) {
        let removed = remove_managed_block(&path)
            .with_context(|| format!("failed to remove managed block from {}", path.display()))?;
        if removed {
            crate::out!(
                "{} rule block removed from {} (file kept for other instructions)",
                ide.as_str(),
                path.display()
            );
        } else {
            crate::out!(
                "{} rule block not found in {}, nothing to remove",
                ide.as_str(),
                path.display()
            );
        }
    } else {
        fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
        crate::out!("{} rule removed from {}", ide.as_str(), path.display());
    }
    status(ide)
}

pub fn status(ide: Ide) -> Result<RuleStatus> {
    let path = dest_path(ide);
    let (installed, size) = current_block_state(ide, &path);
    Ok(RuleStatus {
        ide: ide.as_str().to_string(),
        dest_path: path.to_string_lossy().to_string(),
        installed,
        size,
        supported: is_supported(ide),
    })
}

pub fn list_status() -> Vec<RuleStatus> {
    Ide::all()
        .iter()
        .map(|ide| {
            status(*ide).unwrap_or_else(|_| RuleStatus {
                ide: ide.as_str().to_string(),
                dest_path: dest_path(*ide).to_string_lossy().to_string(),
                installed: false,
                size: None,
                supported: is_supported(*ide),
            })
        })
        .collect()
}

/// 上报当前安装状态。Cursor 看文件存在；其他 IDE 看 marker 块字节数。
fn current_block_state(ide: Ide, path: &Path) -> (bool, Option<u64>) {
    if !path.exists() {
        return (false, None);
    }
    if !uses_managed_block(ide) {
        let size = fs::metadata(path).ok().map(|m| m.len());
        return (true, size);
    }
    let Ok(text) = fs::read_to_string(path) else {
        return (false, None);
    };
    match extract_managed_block(&text) {
        Some(block) => (true, Some(block.len() as u64)),
        None => (false, None),
    }
}

// ---- managed block helpers（暴露 pub(crate) 仅为单测可见；外部不使用） ----

/// 从 `text` 中提取 memex marker 包围的块内容（不含 marker 行本身）。
/// 找不到时返回 `None`。
pub(crate) fn extract_managed_block(text: &str) -> Option<String> {
    let start_idx = text.find(BLOCK_START)?;
    let after_start = start_idx + BLOCK_START.len();
    let rest = &text[after_start..];
    let end_rel = rest.find(BLOCK_END)?;
    // 去掉 START marker 之后到 END marker 之前的内容，剥前后换行让块体干净。
    let body = &rest[..end_rel];
    Some(body.trim_matches('\n').to_string())
}

/// 把 `body` 用 marker 块的形式写入 `path`：
/// - 文件不存在 → 创建，内容只含 marker 块
/// - 已有 marker 块 → 用新 `body` 替换块内容（块外内容原样保留）
/// - 有内容但无 marker → 在末尾追加一段空行 + marker 块
///
/// 幂等：相同 `body` 多次调用结果一致。
pub(crate) fn upsert_managed_block(path: &Path, body: &str) -> Result<()> {
    let body_trimmed = body.trim_end_matches('\n');
    let new_block = format!("{BLOCK_START}\n{body_trimmed}\n{BLOCK_END}");

    if !path.exists() {
        fs::write(path, format!("{new_block}\n"))?;
        return Ok(());
    }

    let mut text = fs::read_to_string(path)?;

    // 已有 marker 块：替换。
    if let Some(start_idx) = text.find(BLOCK_START) {
        let after_start = start_idx + BLOCK_START.len();
        if let Some(end_rel) = text[after_start..].find(BLOCK_END) {
            let end_idx = after_start + end_rel + BLOCK_END.len();
            text.replace_range(start_idx..end_idx, &new_block);
            fs::write(path, text)?;
            return Ok(());
        }
        // START 存在但 END 缺失：说明历史写入异常，从 START 起重写整个块。
        text.truncate(start_idx);
    }

    // 文件有内容但无 marker：在末尾追加（用空行隔开避免与上一段粘连）。
    let separator = if text.ends_with("\n\n") || text.is_empty() {
        ""
    } else if text.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    text.push_str(separator);
    text.push_str(&new_block);
    text.push('\n');
    fs::write(path, text)?;
    Ok(())
}

/// 从 `path` 中移除 memex marker 包围的块（含 marker 行）。返回是否真的移除过。
/// 块外内容原样保留。文件最终为空时**保留为空文件**（不删除文件）。
pub(crate) fn remove_managed_block(path: &Path) -> Result<bool> {
    let mut text = fs::read_to_string(path)?;
    let Some(start_idx) = text.find(BLOCK_START) else {
        return Ok(false);
    };
    let after_start = start_idx + BLOCK_START.len();
    let Some(end_rel) = text[after_start..].find(BLOCK_END) else {
        return Ok(false);
    };
    let end_idx = after_start + end_rel + BLOCK_END.len();
    // 一并吞掉块尾紧贴的换行，避免留下多余空行。
    let mut drain_end = end_idx;
    if text.get(drain_end..=drain_end) == Some("\n") {
        drain_end += 1;
    }
    // 同样吞掉块前面 upsert 写入时的「分隔空行」（最多 1 个 `\n`），
    // 让 uninstall 在用户原文件以单 `\n` 结尾的常见场景下完全可逆。
    let mut drain_start = start_idx;
    if drain_start > 0 && text.as_bytes()[drain_start - 1] == b'\n' {
        // 仅在前面已经有 `\n` 隔开内容时再吞，避免把用户那一行结尾的 `\n` 也吞掉。
        // 判定方式：前 2 个字符是 `\n\n`，说明这是 install 时插入的分隔空行。
        if drain_start >= 2 && text.as_bytes()[drain_start - 2] == b'\n' {
            drain_start -= 1;
        }
    }
    text.replace_range(drain_start..drain_end, "");
    // 兜底：把任何 3 连换行收敛到 2 个，避免出现成段空行。
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    fs::write(path, text)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_file(name: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join(name);
        (dir, p)
    }

    // ---- 静态白名单 / dest_path 形态 ----

    #[test]
    fn all_four_ides_supported_in_v2() {
        for ide in Ide::all() {
            assert!(is_supported(*ide), "expected {} supported", ide.as_str());
        }
    }

    #[test]
    fn cursor_dest_path_is_mdc_under_dotcursor_rules() {
        let p = dest_path(Ide::Cursor);
        let s = p.to_string_lossy().to_string();
        assert!(s.ends_with("/.cursor/rules/memex.mdc"), "actual: {s}");
    }

    #[test]
    fn claude_code_dest_path_is_claude_md_under_dotclaude() {
        let p = dest_path(Ide::ClaudeCode);
        let s = p.to_string_lossy().to_string();
        assert!(s.ends_with("/.claude/CLAUDE.md"), "actual: {s}");
    }

    #[test]
    fn codex_dest_path_is_agents_md_under_dotcodex() {
        let p = dest_path(Ide::Codex);
        let s = p.to_string_lossy().to_string();
        assert!(s.ends_with("/.codex/AGENTS.md"), "actual: {s}");
    }

    #[test]
    fn opencode_dest_path_is_agents_md_under_config_opencode() {
        let p = dest_path(Ide::OpenCode);
        let s = p.to_string_lossy().to_string();
        assert!(s.ends_with("/.config/opencode/AGENTS.md"), "actual: {s}");
    }

    #[test]
    fn cursor_uses_single_file_others_use_managed_block() {
        assert!(!uses_managed_block(Ide::Cursor));
        assert!(uses_managed_block(Ide::ClaudeCode));
        assert!(uses_managed_block(Ide::Codex));
        assert!(uses_managed_block(Ide::OpenCode));
    }

    // ---- Cursor full content 拼接 ----

    #[test]
    fn cursor_full_content_has_frontmatter_then_shared_body() {
        let s = cursor_full_content();
        assert!(s.starts_with("---\nalwaysApply: true\n---\n"));
        assert!(s.contains("get_project_context"));
        assert!(s.contains("Memex MCP 使用规则"));
    }

    // ---- upsert_managed_block helper：5 个场景 ----

    #[test]
    fn upsert_creates_file_when_missing() {
        let (_d, p) = temp_file("AGENTS.md");
        upsert_managed_block(&p, "hello memex").unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains(BLOCK_START));
        assert!(text.contains("hello memex"));
        assert!(text.contains(BLOCK_END));
    }

    #[test]
    fn upsert_appends_when_file_has_other_content_without_marker() {
        let (_d, p) = temp_file("AGENTS.md");
        fs::write(&p, "# user own instructions\n\nbe nice.\n").unwrap();
        upsert_managed_block(&p, "memex body").unwrap();
        let text = fs::read_to_string(&p).unwrap();
        // 用户原内容保留
        assert!(text.contains("# user own instructions"));
        assert!(text.contains("be nice."));
        // memex 块追加在末尾
        assert!(text.contains("memex body"));
        let user_idx = text.find("be nice.").unwrap();
        let memex_idx = text.find(BLOCK_START).unwrap();
        assert!(
            user_idx < memex_idx,
            "memex block must come after user content"
        );
    }

    #[test]
    fn upsert_replaces_existing_block_in_place() {
        let (_d, p) = temp_file("AGENTS.md");
        // 已经有 memex 块 + 用户尾部内容
        fs::write(
            &p,
            format!(
                "# top\n\n{}\nold body\n{}\n\n# bottom user notes\n",
                BLOCK_START, BLOCK_END
            ),
        )
        .unwrap();
        upsert_managed_block(&p, "new body v2").unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("# top"));
        assert!(text.contains("# bottom user notes"));
        assert!(text.contains("new body v2"));
        assert!(!text.contains("old body"));
    }

    #[test]
    fn upsert_is_idempotent_when_called_twice_with_same_body() {
        let (_d, p) = temp_file("AGENTS.md");
        upsert_managed_block(&p, "stable body").unwrap();
        let first = fs::read_to_string(&p).unwrap();
        upsert_managed_block(&p, "stable body").unwrap();
        let second = fs::read_to_string(&p).unwrap();
        assert_eq!(first, second, "second install must be byte-identical");
    }

    #[test]
    fn upsert_repairs_orphan_start_marker_without_end() {
        let (_d, p) = temp_file("AGENTS.md");
        // 历史写入异常：只有 START 没有 END
        fs::write(
            &p,
            format!("# pre\n\n{}\nbroken half-block...\n", BLOCK_START),
        )
        .unwrap();
        upsert_managed_block(&p, "fresh body").unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("# pre"));
        assert!(text.contains("fresh body"));
        assert!(!text.contains("broken half-block"));
        // 现在应该有完整 marker pair
        assert!(text.matches(BLOCK_START).count() == 1);
        assert!(text.matches(BLOCK_END).count() == 1);
    }

    // ---- remove_managed_block helper：4 个场景 ----

    #[test]
    fn remove_returns_false_when_no_marker() {
        let (_d, p) = temp_file("AGENTS.md");
        fs::write(&p, "# user only\n").unwrap();
        let removed = remove_managed_block(&p).unwrap();
        assert!(!removed);
        // 用户内容保留
        let text = fs::read_to_string(&p).unwrap();
        assert_eq!(text, "# user only\n");
    }

    #[test]
    fn remove_strips_only_memex_block_keeps_other_content() {
        let (_d, p) = temp_file("AGENTS.md");
        fs::write(
            &p,
            format!(
                "# user top\n\n{}\nmemex body\n{}\n\n# user bottom\n",
                BLOCK_START, BLOCK_END
            ),
        )
        .unwrap();
        let removed = remove_managed_block(&p).unwrap();
        assert!(removed);
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("# user top"));
        assert!(text.contains("# user bottom"));
        assert!(!text.contains(BLOCK_START));
        assert!(!text.contains(BLOCK_END));
        assert!(!text.contains("memex body"));
    }

    #[test]
    fn remove_handles_block_only_file() {
        let (_d, p) = temp_file("AGENTS.md");
        fs::write(&p, format!("{}\nsolo body\n{}\n", BLOCK_START, BLOCK_END)).unwrap();
        let removed = remove_managed_block(&p).unwrap();
        assert!(removed);
        let text = fs::read_to_string(&p).unwrap();
        assert!(
            text.trim().is_empty(),
            "block-only file should be left effectively empty, got: {text:?}"
        );
    }

    #[test]
    fn install_then_uninstall_is_byte_reversible_on_typical_file() {
        let (_d, p) = temp_file("AGENTS.md");
        let original = "# user own instructions\n\nbe nice.\n";
        fs::write(&p, original).unwrap();
        upsert_managed_block(&p, "memex body v2").unwrap();
        let removed = remove_managed_block(&p).unwrap();
        assert!(removed);
        let after = fs::read_to_string(&p).unwrap();
        assert_eq!(
            after, original,
            "install + uninstall should restore the file byte-for-byte"
        );
    }

    #[test]
    fn remove_orphan_start_only_returns_false() {
        let (_d, p) = temp_file("AGENTS.md");
        fs::write(&p, format!("# pre\n{}\nincomplete\n", BLOCK_START)).unwrap();
        let removed = remove_managed_block(&p).unwrap();
        assert!(!removed);
    }

    // ---- extract_managed_block：基本读取 ----

    #[test]
    fn extract_returns_body_between_markers() {
        let text = format!(
            "# user\n\n{}\nmemex says hi\nmulti line\n{}\n# tail\n",
            BLOCK_START, BLOCK_END
        );
        let body = extract_managed_block(&text).unwrap();
        assert_eq!(body, "memex says hi\nmulti line");
    }

    #[test]
    fn extract_returns_none_when_no_marker() {
        assert!(extract_managed_block("hello world").is_none());
    }

    // ---- 端到端 install → status → uninstall 在 TempDir ----
    //
    // 注：不能直接调用 install(ide)，因为它走的是真实 HOME 路径。这里
    // 用 helper 函数（upsert / remove）已覆盖所有写入与清理路径。
}
