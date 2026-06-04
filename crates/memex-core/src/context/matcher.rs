//! 把当前工作目录映射到 Memex 已知的 project_path —— 完全照搬 TARS 的
//! 三级匹配策略（Tier 1 精确 → Tier 2 项目名 → Tier 3 子串），但实现
//! 形式是 SQLite 查询，不依赖外部脚本。
//!
//! 为什么要分级：用户在不同会话中可能从 `/Users/me/work/foo` 或
//! `/Users/me/work/foo/sub` 启动 AI，project_path 写下来的可能是
//! `/Users/me/work/foo`（git 根）。三级匹配让 hook 在任何子目录跑
//! 都能找回正确的项目。

use std::path::{Path, PathBuf};

use crate::storage::db::Db;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchTier {
    /// `cwd` 等于或是 `project_path` 的父级（最常见、最可靠）。
    ExactPath,
    /// `cwd` 的最后一级 dirname 等于某个 project 的 basename。
    ProjectName,
    /// `cwd` 的 basename 是某 project basename 的子串，或反之。
    /// 仅作为兜底，准确度最低。
    FuzzySubstring,
}

#[derive(Debug, Clone)]
pub struct ProjectMatch {
    pub project_path: String,
    pub tier: MatchTier,
}

/// 按三级优先级解析 `cwd → project_path`。返回首个命中（高优先级独占）。
///
/// `cwd` 既可以是当前工作目录，也可以是用户用 `--project` 显式指定的
/// 路径。我们对它做规范化（去掉尾斜杠 + 解析符号链接 best-effort）。
pub fn search_by_project(db: &Db, cwd: &Path) -> anyhow::Result<Option<ProjectMatch>> {
    let candidates = db.distinct_projects()?;
    Ok(search_by_project_in_candidates(cwd, &candidates))
}

/// 把数据库依赖剥离的纯函数版本，方便单测：传入候选 project_path 列表。
pub fn search_by_project_in_candidates(
    cwd: &Path,
    candidates: &[String],
) -> Option<ProjectMatch> {
    let cwd_norm = normalize(cwd);

    // Tier 1: exact path  —  cwd 等于 project_path，或 cwd 是它的子目录。
    //   这是最强信号：会话当时的工作目录就在这里 / 它的子级。
    if let Some(p) = candidates.iter().find(|p| {
        let cand = normalize(Path::new(p));
        cwd_norm == cand || cwd_norm.starts_with(&cand)
    }) {
        return Some(ProjectMatch {
            project_path: p.clone(),
            tier: MatchTier::ExactPath,
        });
    }

    // Tier 2: project name —— cwd 的 basename 等于某 project 的 basename。
    //   场景：项目仓库在不同机器上被 clone 到不同位置，但都叫 `memex`。
    let cwd_base = basename(&cwd_norm);
    if !cwd_base.is_empty() {
        if let Some(p) = candidates.iter().find(|p| {
            let cand = normalize(Path::new(p));
            basename(&cand) == cwd_base
        }) {
            return Some(ProjectMatch {
                project_path: p.clone(),
                tier: MatchTier::ProjectName,
            });
        }
    }

    // Tier 3: 子串模糊匹配 —— 仅对 ≥ 4 字符的 basename 启用，避免
    //   "a"、"src" 这种太短的目录名误命中一堆项目。
    if cwd_base.len() >= 4 {
        if let Some(p) = candidates.iter().find(|p| {
            let cand_base = basename(&normalize(Path::new(p)));
            cand_base.contains(&cwd_base) || cwd_base.contains(&cand_base)
        }) {
            return Some(ProjectMatch {
                project_path: p.clone(),
                tier: MatchTier::FuzzySubstring,
            });
        }
    }

    None
}

fn normalize(p: &Path) -> PathBuf {
    // 不调用 canonicalize() —— 它会要求路径必须存在，hook 命令中
    // 用户可能传一个临时路径做演示。只做"去掉尾随分隔符"这种轻规范化。
    let s = p.to_string_lossy();
    let trimmed = s.trim_end_matches('/');
    PathBuf::from(trimmed)
}

fn basename(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(arr: &[&str]) -> Vec<String> {
        arr.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_path_takes_precedence() {
        let cands = paths(&[
            "/home/u/foo",
            "/home/u/bar",
        ]);
        let m = search_by_project_in_candidates(Path::new("/home/u/foo"), &cands).unwrap();
        assert_eq!(m.project_path, "/home/u/foo");
        assert_eq!(m.tier, MatchTier::ExactPath);
    }

    #[test]
    fn exact_path_matches_subdirectory() {
        let cands = paths(&["/home/u/foo"]);
        let m = search_by_project_in_candidates(Path::new("/home/u/foo/sub/deeper"), &cands).unwrap();
        assert_eq!(m.tier, MatchTier::ExactPath);
        assert_eq!(m.project_path, "/home/u/foo");
    }

    #[test]
    fn trailing_slash_does_not_break_match() {
        let cands = paths(&["/home/u/foo/"]);
        let m = search_by_project_in_candidates(Path::new("/home/u/foo"), &cands).unwrap();
        assert_eq!(m.tier, MatchTier::ExactPath);
    }

    #[test]
    fn project_name_matches_across_clones() {
        let cands = paths(&["/Users/me/work/memex"]);
        // 在另一台机器上 clone 在不同位置
        let m = search_by_project_in_candidates(Path::new("/home/other/repos/memex"), &cands).unwrap();
        assert_eq!(m.tier, MatchTier::ProjectName);
        assert_eq!(m.project_path, "/Users/me/work/memex");
    }

    #[test]
    fn project_name_does_not_match_when_exact_path_wins() {
        // 项目名同时是「foo」的两条记录，应优先选 exact path 那条
        let cands = paths(&["/x/foo", "/y/foo"]);
        let m = search_by_project_in_candidates(Path::new("/x/foo/sub"), &cands).unwrap();
        assert_eq!(m.tier, MatchTier::ExactPath);
        assert_eq!(m.project_path, "/x/foo");
    }

    #[test]
    fn fuzzy_substring_matches_long_basenames() {
        let cands = paths(&["/work/memex-prototype"]);
        // cwd 的 basename 是 "memex"（>= 4 字符）→ "memex-prototype" 包含它 → Tier 3 命中
        let m = search_by_project_in_candidates(Path::new("/Users/me/memex"), &cands).unwrap();
        assert_eq!(m.tier, MatchTier::FuzzySubstring);
    }

    #[test]
    fn fuzzy_substring_skipped_for_short_basenames() {
        let cands = paths(&["/work/abc"]);
        // basename "a" 只有 1 字符 < 4，Tier 3 不启用
        let r = search_by_project_in_candidates(Path::new("/x/a"), &cands);
        assert!(r.is_none(), "短 basename 不该触发模糊匹配");
    }

    #[test]
    fn no_candidates_returns_none() {
        let r = search_by_project_in_candidates(Path::new("/anywhere"), &[]);
        assert!(r.is_none());
    }

    #[test]
    fn unrelated_cwd_returns_none() {
        let cands = paths(&["/home/u/foo"]);
        let r = search_by_project_in_candidates(Path::new("/tmp/something-else"), &cands);
        assert!(r.is_none());
    }
}
