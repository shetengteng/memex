//! 一次性手动备份。复用 `memex backup` CLI，避免在 menubar 这边重新实现
//! tar/flate2 打包逻辑 —— CLI 那边已经把 memex.db / config.toml / sessions/
//! 三个产物收拢好了。
//!
//! 导出 / 导入流程：
//! - `export_db(target_path)` —— 让用户在 Save 对话框选位置后，把整份数据 dump
//!   到那里（仍然是 `memex backup <path>`，因此后续可用 `memex restore` 反向恢复）。
//! - `import_db(source_path)` —— 让用户挑一个 `.tar.gz`，先停 daemon、调用
//!   `memex restore`、再把 daemon 拉回来。daemon 不停的话 SQLite WAL 会锁住 db。
use std::path::{Path, PathBuf};
use std::process::Command;

use memex_core::memex_dir;
use serde::{Deserialize, Serialize};

use super::daemon::daemon_restart_inner;
use super::error::{CmdError, CmdResult};
use crate::services::daemon::DaemonState;

/// 返回 memex 数据目录（`~/.memex`）的绝对路径，给前端展示用。
/// DataTab 用它显示数据库路径，避免之前为了拿一个路径把 doctor 全跑一遍。
#[tauri::command]
pub fn memex_data_dir() -> String {
    memex_dir().to_string_lossy().to_string()
}

/// 确保 `~/.memex/backups/` 存在，并返回其绝对路径。
/// 前端"打开备份目录"按钮在用户从未备份过的情况下也能用：先 ensure 再
/// reveal，否则 Finder 会跳到上一级 `~/.memex`，跟用户意图不一致。
#[tauri::command]
pub fn ensure_backup_dir() -> CmdResult<String> {
    let dir = memex_dir().join("backups");
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir.to_string_lossy().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub path: String,
    pub files: u64,
    pub size_bytes: u64,
}

/// 找到 memex CLI binary。优先 `<bundle>/Contents/MacOS/memex-cli`（与主进程
/// 同目录的 sidecar），否则退到 PATH 上的 `memex-cli`（cli_install 创建的
/// `~/.local/bin/memex-cli` symlink，或 dev 模式 `cargo install` 装的）。
///
/// 注意 binary 名是 `memex-cli`：bundle 里 GUI 主 binary 叫 `Memex`，CLI 不能
/// 用同名（APFS 大小写不敏感会撞），所以物理名与用户命令名都统一为 `memex-cli`。
fn locate_memex_cli() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let p = parent.join("memex-cli");
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(out) = Command::new("which").arg("memex-cli").output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(PathBuf::from(s));
        }
    }
    None
}

/// 在 `~/.memex/backups/` 下生成一个带时间戳的 `.tar.gz`，返回备份信息。
#[tauri::command]
pub async fn backup_now() -> CmdResult<BackupResult> {
    let backup_dir = memex_dir().join("backups");
    if !backup_dir.exists() {
        std::fs::create_dir_all(&backup_dir)?;
    }
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let output_path = backup_dir.join(format!("memex-{}.tar.gz", ts));
    run_backup_cli(&output_path)
}

/// 让用户在 save dialog 选目标位置后，把整份数据导出到该位置。
/// 与 [`backup_now`] 共用同一份 `memex backup` 流水线，区别仅是落点由用户决定。
#[tauri::command]
pub async fn export_db(target_path: String) -> CmdResult<BackupResult> {
    let target = PathBuf::from(&target_path);
    if target.as_os_str().is_empty() {
        return Err(CmdError::Backend("导出路径为空".into()));
    }
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }
    run_backup_cli(&target)
}

/// 把 `memex backup` 写成的 `.tar.gz` 恢复回 `~/.memex/`。
///
/// 关键顺序：必须先 `state.shutdown()` 释放 SQLite WAL 锁 —— daemon 还
/// hold 着 db handle 时，CLI 解包覆盖文件可能拿到不一致的快照或者直接
/// fail。解包完成后再 `daemon_restart_inner` 重新拉起，否则后台采集 / HTTP
/// 健康会一直挂着。CLI 自己什么都不知道，只管解包；这边补上 daemon 边界。
///
/// 把 `import_db` 入参的路径校验提成独立纯函数，方便单测：
/// 不需要构造 `tauri::State<DaemonState>` 就能覆盖所有 reject 分支。
/// 校验通过返回归一化的 `PathBuf`，否则带 `CmdError::Backend / NotFound`。
fn validate_import_source(source_path: &str) -> CmdResult<PathBuf> {
    if source_path.is_empty() {
        return Err(CmdError::Backend("导入路径为空".into()));
    }
    let source = PathBuf::from(source_path);
    if !source.exists() {
        return Err(CmdError::NotFound(format!(
            "导入文件不存在：{}",
            source.display()
        )));
    }
    if !source.is_file() {
        return Err(CmdError::Backend(format!(
            "导入路径不是文件：{}",
            source.display()
        )));
    }
    Ok(source)
}

#[tauri::command]
pub async fn import_db(
    state: tauri::State<'_, DaemonState>,
    source_path: String,
) -> CmdResult<ImportResult> {
    let source = validate_import_source(&source_path)?;

    // 必须在 restore 之前 shutdown daemon：SQLite WAL 把 memex.db 锁着的时候，
    // CLI 解包覆盖文件可能拿到不一致的快照或者直接 fail。shutdown 内部会触发
    // oneshot + await task join + 清 lock，等返回时 db handle 已完全释放。
    state.shutdown().await;

    let restore = run_restore_cli(&source)?;

    // 数据替换后立刻拉起 daemon，避免 UI 那边接下来的查询都打到一个空的 db state。
    let _ = daemon_restart_inner(&state).await;
    Ok(ImportResult {
        source: restore.source,
        before_path: restore.before_path,
        files: restore.files,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub source: String,
    pub before_path: String,
    pub files: u64,
}

/// CLI `memex backup <output>` 的 JSON 输出契约，与 `commands::backup::run` 中
/// 的 `serde_json::json!({"path","files","size_bytes"})` 对齐。
fn run_backup_cli(output_path: &Path) -> CmdResult<BackupResult> {
    let bin = locate_memex_cli().ok_or_else(|| {
        CmdError::NotFound("找不到 memex CLI（既不在 app 同目录，也不在 PATH）".into())
    })?;
    let output_str = output_path.to_string_lossy().to_string();
    let output = Command::new(&bin)
        .args(["--json", "backup", &output_str])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CmdError::Backend(format!(
            "memex backup 返回非零（{}）：{}",
            output.status, stderr
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .map_err(|e| CmdError::Backend(format!("无法解析 CLI 输出（{}）：{}", e, stdout)))
}

#[derive(Debug, Deserialize)]
struct RestoreCliOutput {
    source: String,
    before_path: String,
    files: u64,
}

/// CLI `memex restore <input>` 的 JSON 输出契约，与 `commands::restore::RestoreResult`
/// 对齐。
///
/// **前置条件**：caller 必须已经 shutdown daemon（释放 SQLite WAL 锁），
/// 否则 restore 解包可能跟 daemon 写 db 撞。
fn run_restore_cli(source_path: &Path) -> CmdResult<RestoreCliOutput> {
    let bin = locate_memex_cli().ok_or_else(|| {
        CmdError::NotFound("找不到 memex CLI（既不在 app 同目录，也不在 PATH）".into())
    })?;
    let source_str = source_path.to_string_lossy().to_string();
    let output = Command::new(&bin)
        .args(["--json", "restore", &source_str])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CmdError::Backend(format!(
            "memex restore 返回非零（{}）：{}",
            output.status, stderr
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .map_err(|e| CmdError::Backend(format!("无法解析 restore CLI 输出（{}）：{}", e, stdout)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn with_temp_memex<F: FnOnce(&std::path::Path)>(f: F) {
        let tmp = tempfile::tempdir().expect("temp dir");
        let prev = std::env::var("MEMEX_HOME").ok();
        // SAFETY: 由 #[serial(memex_home)] 串行化。
        unsafe { std::env::set_var("MEMEX_HOME", tmp.path()) };
        f(tmp.path());
        match prev {
            Some(v) => unsafe { std::env::set_var("MEMEX_HOME", v) },
            None => unsafe { std::env::remove_var("MEMEX_HOME") },
        }
    }

    /// `ensure_backup_dir` 在文件夹不存在时应该把它建出来，并返回完整路径。
    #[test]
    #[serial(memex_home)]
    fn ensure_backup_dir_creates_and_returns_path() {
        with_temp_memex(|_| {
            let path_str = ensure_backup_dir().expect("ensure_backup_dir ok");
            let p = std::path::Path::new(&path_str);
            assert!(p.exists(), "backup dir should exist after call");
            assert!(p.is_dir(), "should be a directory");
            assert!(
                p.ends_with("backups"),
                "must end with `backups`, got {}",
                path_str,
            );

            let path_str_2 = ensure_backup_dir().expect("idempotent");
            assert_eq!(path_str, path_str_2);
        });
    }

    /// `memex_data_dir` 应返回 memex_dir 的绝对路径字符串，不带后缀。
    #[test]
    #[serial(memex_home)]
    fn memex_data_dir_returns_root() {
        with_temp_memex(|dir| {
            let got = memex_data_dir();
            assert_eq!(got, dir.to_string_lossy());
        });
    }

    /// 导出空路径应被前置校验拒绝，不会走到 CLI。
    #[tokio::test]
    async fn export_db_rejects_empty_path() {
        let err = export_db(String::new()).await.unwrap_err();
        assert!(
            matches!(err, CmdError::Backend(_)),
            "expected Backend error, got {:?}",
            err
        );
    }

    /// 导入空路径应被前置校验拒绝。
    /// 注：单测改测 `validate_import_source`，因为顶层 `import_db` 现在依赖
    /// `tauri::State<DaemonState>`，在测试环境里没法零成本构造；校验逻辑已
    /// 单独抽出来，等价覆盖了 reject 分支。
    #[test]
    fn import_db_rejects_empty_path() {
        let err = validate_import_source("").unwrap_err();
        assert!(
            matches!(err, CmdError::Backend(_)),
            "expected Backend error, got {:?}",
            err
        );
    }

    /// 导入不存在的归档路径应返回 NotFound，避免静默把 daemon 停了之后什么都不做。
    #[test]
    fn import_db_rejects_missing_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let missing = tmp.path().join("nope.tar.gz");
        let err = validate_import_source(&missing.to_string_lossy()).unwrap_err();
        assert!(
            matches!(err, CmdError::NotFound(_)),
            "expected NotFound, got {:?}",
            err
        );
    }

    /// 给一个目录路径（不是 file）应被拒绝。否则后端会傻乎乎地把目录传给 `memex restore`，
    /// CLI 那边再 fail 一次，错误信息很模糊。
    #[test]
    fn import_db_rejects_directory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir_path = tmp.path().join("subdir");
        std::fs::create_dir_all(&dir_path).unwrap();
        let err = validate_import_source(&dir_path.to_string_lossy()).unwrap_err();
        assert!(
            matches!(err, CmdError::Backend(_)),
            "expected Backend error, got {:?}",
            err
        );
    }
}
