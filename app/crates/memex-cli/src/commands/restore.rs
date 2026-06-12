//! `memex restore <path>` —— 从 `memex backup` 产出的 tar.gz 归档恢复
//! `~/.memex` 目录下的 memex.db / config.toml / redactions.yaml / sessions/。
//!
//! 安全网：解包前会把当前数据原子移动到
//! `~/.memex/.before-restore-YYYYMMDD-HHMMSS/`（同盘 rename），用户对结果不满
//! 意时可以手动恢复。该备份目录由用户自己清理。
//!
//! 注意：调用方负责在 restore 前停掉 daemon（否则 SQLite WAL 文件被锁）。
//! CLI 自己不做 daemon 管控 —— 不引入对 daemon 的硬依赖。Tauri 那边的
//! `import_db` IPC 会包好 stop → restore → start 的事务边界。

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use memex_core::memex_dir;
use serde::Serialize;

/// 允许解包到 memex 目录的顶层条目白名单。
/// 任何其他路径（含绝对路径、`..` 父级跳转）都会被拒绝，防止恶意 tar 解包逃逸。
const ALLOWED_ROOTS: &[&str] = &["memex.db", "config.toml", "redactions.yaml", "sessions"];

#[derive(Debug, Serialize)]
pub struct RestoreResult {
    pub source: String,
    pub before_path: String,
    pub files: u64,
}

pub fn run(input_path: &str, json: bool) -> Result<()> {
    let memex = memex_dir();
    let result = restore_to(&memex, Path::new(input_path))?;

    if json {
        crate::io::json(&result)?;
    } else {
        crate::out!(
            "Restore complete from {} ({} files). Previous data moved to {}",
            result.source,
            result.files,
            result.before_path,
        );
    }
    Ok(())
}

/// 把 `archive` 里的内容恢复到 `target_dir`。
/// 返回值的 `before_path` 是被移动走的旧数据目录，可用于回滚或人工 diff。
///
/// 失败时尽力把旧数据回滚到 target_dir，避免半途崩溃后用户既丢了旧数据又没有新数据。
pub fn restore_to(target_dir: &Path, archive: &Path) -> Result<RestoreResult> {
    if !archive.exists() {
        bail!("archive not found: {}", archive.display());
    }
    if !archive.is_file() {
        bail!("not a file: {}", archive.display());
    }

    fs::create_dir_all(target_dir)
        .with_context(|| format!("failed to ensure target dir {}", target_dir.display()))?;

    let before_dir = before_dir_for(target_dir);
    move_existing_aside(target_dir, &before_dir).with_context(|| {
        format!(
            "failed to move existing data aside to {}",
            before_dir.display()
        )
    })?;

    // 从这里开始如果失败必须回滚
    let result = (|| -> Result<u64> {
        let tar_gz = fs::File::open(archive)
            .with_context(|| format!("failed to open archive {}", archive.display()))?;
        let dec = flate2::read::GzDecoder::new(tar_gz);
        let mut ar = tar::Archive::new(dec);
        let mut file_count = 0u64;
        for entry in ar.entries().context("failed to read tar entries")? {
            let mut entry = entry?;
            let path_in_archive = entry.path()?.into_owned();
            let safe_relative = sanitize_entry_path(&path_in_archive)?;
            let dest = target_dir.join(&safe_relative);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to mkdir parent {}", parent.display()))?;
            }
            entry.unpack(&dest).with_context(|| {
                format!(
                    "failed to unpack {} -> {}",
                    path_in_archive.display(),
                    dest.display()
                )
            })?;
            if dest.is_file() {
                file_count += 1;
            }
        }
        Ok(file_count)
    })();

    let files = match result {
        Ok(n) => n,
        Err(e) => {
            // 失败 → 回滚：把 before_dir 里的内容搬回 target_dir
            if let Err(rollback_err) = rollback(&before_dir, target_dir) {
                return Err(anyhow!(
                    "restore failed: {e}; rollback also failed: {rollback_err}"
                ));
            }
            return Err(e.context("restore failed; previous data rolled back"));
        }
    };

    Ok(RestoreResult {
        source: archive.display().to_string(),
        before_path: before_dir.display().to_string(),
        files,
    })
}

fn before_dir_for(target_dir: &Path) -> PathBuf {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    target_dir.join(format!(".before-restore-{ts}"))
}

/// 把 target_dir 下的 ALLOWED_ROOTS 条目移到 before_dir。
/// 不在白名单内的条目原样保留（用户自定义文件、日志、备份目录等不动）。
fn move_existing_aside(target_dir: &Path, before_dir: &Path) -> Result<()> {
    let mut moved_any = false;
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    for name in ALLOWED_ROOTS {
        let src = target_dir.join(name);
        if !src.exists() {
            continue;
        }
        if !moved_any {
            fs::create_dir_all(before_dir)
                .with_context(|| format!("failed to create {}", before_dir.display()))?;
            moved_any = true;
        }
        let dst = before_dir.join(name);
        match fs::rename(&src, &dst) {
            Ok(()) => staged.push((dst, src)),
            Err(e) => {
                // 已经搬过的回滚回去，避免半途留下空洞
                for (moved_to, original) in staged.into_iter().rev() {
                    let _ = fs::rename(&moved_to, &original);
                }
                return Err(anyhow!("failed to move {} aside: {}", name, e));
            }
        }
    }
    Ok(())
}

fn rollback(before_dir: &Path, target_dir: &Path) -> Result<()> {
    if !before_dir.exists() {
        return Ok(());
    }
    for name in ALLOWED_ROOTS {
        let from = before_dir.join(name);
        if !from.exists() {
            continue;
        }
        let to = target_dir.join(name);
        if to.exists() {
            // 解包过程中可能已经写出了一部分新数据，先清掉
            if to.is_dir() {
                let _ = fs::remove_dir_all(&to);
            } else {
                let _ = fs::remove_file(&to);
            }
        }
        fs::rename(&from, &to)
            .with_context(|| format!("rollback rename {} -> {}", from.display(), to.display()))?;
    }
    let _ = fs::remove_dir(before_dir);
    Ok(())
}

/// 拒绝绝对路径、空路径、`..` 父级跳转、以及不在白名单顶层的条目。
/// 防止恶意 tar 包写入 `~/.memex` 之外的位置或覆盖白名单外的文件。
fn sanitize_entry_path(p: &Path) -> Result<PathBuf> {
    if p.as_os_str().is_empty() {
        bail!("empty entry path in archive");
    }
    if p.is_absolute() {
        bail!("absolute path in archive: {}", p.display());
    }
    let mut components: Vec<&std::ffi::OsStr> = Vec::new();
    for c in p.components() {
        match c {
            Component::Normal(s) => components.push(s),
            Component::CurDir => {}
            Component::ParentDir => {
                bail!("parent-dir traversal in archive: {}", p.display())
            }
            Component::Prefix(_) | Component::RootDir => {
                bail!("rooted component in archive: {}", p.display())
            }
        }
    }
    let first = components
        .first()
        .ok_or_else(|| anyhow!("no components after sanitize: {}", p.display()))?;
    let first_str = first
        .to_str()
        .ok_or_else(|| anyhow!("non-utf8 component in archive: {}", p.display()))?;
    if !ALLOWED_ROOTS.contains(&first_str) {
        bail!(
            "archive entry outside allowed roots: {} (allowed: {:?})",
            p.display(),
            ALLOWED_ROOTS
        );
    }
    Ok(components.iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};

    /// 用 tar+flate2 在内存里造一个最小可解包的归档，写入 tmp 文件。
    fn make_test_archive(path: &Path, files: &[(&str, &[u8])]) -> io::Result<()> {
        let f = fs::File::create(path)?;
        let enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut builder = tar::Builder::new(enc);
        for (name, contents) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, name, *contents)?;
        }
        let enc = builder.into_inner()?;
        enc.finish()?;
        Ok(())
    }

    /// 完整恢复：目标目录下原有的白名单条目应被搬到 .before-restore-*，
    /// 解包后的内容应出现在目标目录下，并且 files 计数 == 解包文件数。
    #[test]
    fn restore_replaces_allowed_roots_and_keeps_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path();
        // 准备旧数据
        fs::write(target.join("memex.db"), b"OLD-DB").unwrap();
        fs::write(target.join("config.toml"), b"old = 1").unwrap();
        let old_sessions = target.join("sessions/cursor");
        fs::create_dir_all(&old_sessions).unwrap();
        fs::write(old_sessions.join("a.md"), b"OLD-SESSION").unwrap();
        // 不在白名单内的，应该被保留
        fs::write(target.join("UNRELATED.log"), b"keep me").unwrap();

        // 造一份新数据 archive
        let archive = tmp.path().join("backup.tar.gz");
        make_test_archive(
            &archive,
            &[
                ("memex.db", b"NEW-DB"),
                ("config.toml", b"new = 1"),
                ("sessions/codex/b.md", b"NEW-SESSION"),
            ],
        )
        .unwrap();

        let result = restore_to(target, &archive).unwrap();
        assert_eq!(result.files, 3, "files should reflect tar entries");
        assert!(result.before_path.contains(".before-restore-"));

        assert_eq!(fs::read(target.join("memex.db")).unwrap(), b"NEW-DB");
        assert_eq!(fs::read(target.join("config.toml")).unwrap(), b"new = 1");
        assert_eq!(
            fs::read(target.join("sessions/codex/b.md")).unwrap(),
            b"NEW-SESSION"
        );
        // 白名单外的文件保留
        assert_eq!(fs::read(target.join("UNRELATED.log")).unwrap(), b"keep me");

        // 旧数据被搬走（only 白名单条目）
        let before = Path::new(&result.before_path);
        assert!(before.exists() && before.is_dir());
        assert_eq!(fs::read(before.join("memex.db")).unwrap(), b"OLD-DB");
        assert_eq!(
            fs::read(before.join("sessions/cursor/a.md")).unwrap(),
            b"OLD-SESSION"
        );
    }

    /// 损坏归档：restore_to 应失败，并把旧数据回滚回原位置，目标目录可用。
    #[test]
    fn restore_rolls_back_on_corrupt_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path();
        fs::write(target.join("memex.db"), b"OLD-DB").unwrap();

        let bad = tmp.path().join("bad.tar.gz");
        let mut f = fs::File::create(&bad).unwrap();
        f.write_all(b"not a real gzip stream").unwrap();
        drop(f);

        let err = restore_to(target, &bad).unwrap_err();
        let chain: String = err
            .chain()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(chain.contains("rolled back") || chain.contains("restore failed"));

        // 原数据回到了 target，没有残留 .before-restore-*
        assert_eq!(fs::read(target.join("memex.db")).unwrap(), b"OLD-DB");
        let stragglers: Vec<_> = fs::read_dir(target)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".before-restore-")
            })
            .collect();
        assert!(
            stragglers.is_empty(),
            "rolled-back run should clean up before-restore dir, leftovers: {stragglers:?}"
        );
    }

    /// 路径越权：含 `..` 的归档应在解包阶段被拒绝。
    #[test]
    fn sanitize_rejects_parent_dir() {
        assert!(sanitize_entry_path(Path::new("../etc/passwd")).is_err());
        assert!(sanitize_entry_path(Path::new("sessions/../../escape")).is_err());
    }

    /// 路径越权：绝对路径应被拒绝。
    #[test]
    fn sanitize_rejects_absolute() {
        assert!(sanitize_entry_path(Path::new("/etc/passwd")).is_err());
    }

    /// 白名单外的顶层条目应被拒绝。
    #[test]
    fn sanitize_rejects_unknown_root() {
        assert!(sanitize_entry_path(Path::new("etc/passwd")).is_err());
        assert!(sanitize_entry_path(Path::new("memex.db")).is_ok());
        assert!(sanitize_entry_path(Path::new("sessions/cursor/a.md")).is_ok());
    }

    /// archive 缺失：返回错误，目标目录不变。
    #[test]
    fn restore_missing_archive_fails_cleanly() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path();
        fs::write(target.join("memex.db"), b"OLD").unwrap();

        let missing = tmp.path().join("does-not-exist.tar.gz");
        let err = restore_to(target, &missing).unwrap_err();
        assert!(err.to_string().contains("archive not found"));
        // 没动旧数据
        assert_eq!(fs::read(target.join("memex.db")).unwrap(), b"OLD");
    }
}
