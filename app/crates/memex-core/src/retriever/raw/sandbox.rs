//! 沙箱根目录与路径校验。
//!
//! 所有 raw 工具的路径必须落在 [`sessions_root`] 之下，校验通过
//! [`ensure_inside_sandbox`] 完成 —— `canonicalize` 后做 prefix 检查，
//! symlink 越界会被自动拒绝。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::memex_dir;

/// 沙箱根目录：`<data_dir>/sessions`。
///
/// 所有 raw 工具（`raw_grep` / `raw_find` / `raw_read`）的可访问范围限定
/// 在这棵目录之下。`<data_dir>` 由 [`crate::memex_dir`] 解析，默认
/// `~/.memex`，可用环境变量 `MEMEX_HOME` 重定向（主要用于测试和多实例隔离）。
pub fn sessions_root() -> PathBuf {
    memex_dir().join("sessions")
}

/// 校验任意路径是否落在 [`sessions_root`] 之下，返回 canonical 路径。
///
/// # Errors
///
/// - 沙箱根目录不存在或无法访问
/// - 入参路径不存在或无法 canonicalize
/// - canonical 路径不在沙箱根之下（含 symlink 越界）
pub(super) fn ensure_inside_sandbox(path: &Path) -> Result<PathBuf> {
    let root = sessions_root().canonicalize().with_context(|| {
        format!(
            "sessions root not initialized: {}",
            sessions_root().display()
        )
    })?;
    let canon = path
        .canonicalize()
        .with_context(|| format!("path not accessible: {}", path.display()))?;
    if !canon.starts_with(&root) {
        bail!("path_outside_sandbox: {}", canon.display());
    }
    Ok(canon)
}
