//! `~/.memex/hooks/<ide>-session-start.sh` 的写盘逻辑。
//!
//! 每个 IDE 各自有自己的 wrapper 字面量（见 `cursor.rs` / `claude.rs` / `codex.rs`），
//! 本模块只负责挑出对应 IDE 的 body、写文件、加可执行位。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::super::setup::Ide;
use super::{claude, codex, cursor};

/// 每个 wrapper 脚本第一行打头的标识，方便升级时识别"这是 memex 装的"。
pub(super) const WRAPPER_BANNER: &str = "# memex hook wrapper — do not edit by hand";

pub(super) const HOOK_DIRNAME: &str = "hooks";

/// 写 `~/.memex/hooks/<ide>-session-start.sh`，加 0755 可执行位。
pub(super) fn ensure_wrapper(ide: Ide, memex_bin: &Path, memex_home: &Path) -> Result<PathBuf> {
    let dir = memex_home.join(HOOK_DIRNAME);
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;

    let (name, body) = wrapper_body(ide, memex_bin);
    let path = dir.join(name);
    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;

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
        Ide::Cursor => cursor::wrapper_body(&bin),
        Ide::ClaudeCode => claude::wrapper_body(&bin),
        Ide::Codex => codex::wrapper_body(&bin),
        // OpenCode 不在自动注入范围内；返回空 body 仅作 fallback，
        // install() 中 `supports(OpenCode)` 已经先返回了，不会真用到。
        Ide::OpenCode => ("opencode-session-start.sh", String::new()),
    }
}
