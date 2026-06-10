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

use super::daemon::{daemon_restart, stop_daemon_blocking};
use super::error::{CmdError, CmdResult};

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

fn locate_memex_cli() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let p = parent.join("memex");
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(out) = Command::new("which").arg("memex").output() {
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
/// 关键顺序：必须先停 daemon —— SQLite WAL 把 memex.db 锁着的时候，
/// 解包覆盖文件可能拿到不一致的快照或者直接 fail，所以前置 stop_daemon_blocking。
/// CLI 自己什么都不知道，只管解包；这边补上 daemon 边界。
///
/// 解包完成后调 `daemon_restart`，否则后台采集 / HTTP 健康都会一直挂着。
#[tauri::command]
pub async fn import_db(source_path: String) -> CmdResult<ImportResult> {
    let source = PathBuf::from(&source_path);
    if source.as_os_str().is_empty() {
        return Err(CmdError::Backend("导入路径为空".into()));
    }
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

    let restore = run_restore_cli(&source)?;
    // 数据替换后立刻拉起 daemon，避免 UI 那边接下来的查询都打到一个空的 db state。
    let _ = daemon_restart().await;
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
/// 对齐。调用前先停 daemon 防 SQLite WAL 锁竞争。
fn run_restore_cli(source_path: &Path) -> CmdResult<RestoreCliOutput> {
    let bin = locate_memex_cli().ok_or_else(|| {
        CmdError::NotFound("找不到 memex CLI（既不在 app 同目录，也不在 PATH）".into())
    })?;
    // 同步停 daemon，等 SIGTERM/KILL 走完才解包，避免覆盖到一半 daemon 还在写。
    stop_daemon_blocking();
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
    #[tokio::test]
    async fn import_db_rejects_empty_path() {
        let err = import_db(String::new()).await.unwrap_err();
        assert!(
            matches!(err, CmdError::Backend(_)),
            "expected Backend error, got {:?}",
            err
        );
    }

    /// 导入不存在的归档路径应返回 NotFound，避免静默把 daemon 停了之后什么都不做。
    #[tokio::test]
    async fn import_db_rejects_missing_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let missing = tmp.path().join("nope.tar.gz");
        let err = import_db(missing.to_string_lossy().to_string())
            .await
            .unwrap_err();
        assert!(
            matches!(err, CmdError::NotFound(_)),
            "expected NotFound, got {:?}",
            err
        );
    }

    /// 给一个目录路径（不是 file）应被拒绝。否则后端会傻乎乎地把目录传给 `memex restore`，
    /// CLI 那边再 fail 一次，错误信息很模糊。
    #[tokio::test]
    async fn import_db_rejects_directory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir_path = tmp.path().join("subdir");
        std::fs::create_dir_all(&dir_path).unwrap();
        let err = import_db(dir_path.to_string_lossy().to_string())
            .await
            .unwrap_err();
        assert!(
            matches!(err, CmdError::Backend(_)),
            "expected Backend error, got {:?}",
            err
        );
    }
}
