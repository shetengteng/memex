//! 把当前工作目录映射到 Memex 已知的 project_path —— 完全照搬 TARS 的
//! 三级匹配策略（Tier 1 精确 → Tier 2 项目名 → Tier 3 子串），但实现
//! 形式是 SQLite 查询，不依赖外部脚本。
//!
//! 为什么要分级：用户在不同会话中可能从 `/Users/me/work/foo` 或
//! `/Users/me/work/foo/sub` 启动 AI，project_path 写下来的可能是
//! `/Users/me/work/foo`（git 根）。三级匹配让 hook 在任何子目录跑
//! 都能找回正确的项目。
//!
//! 拆分：tests 移到 sibling `tests.rs`，本文件只承载算法 + 路径规范化。

#[cfg(test)]
mod tests;

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

/// 单条孤儿会话不应被当作"项目"参与匹配 —— 它常常来自家目录或临时目录
/// 下偶然写入的一次测试会话，但因为 Tier 1 是 `starts_with` 前缀匹配，
/// 会反过来"吞掉"真实子项目的 cwd。
pub const MIN_PROJECT_SESSIONS: i64 = 2;

/// 按三级优先级解析 `cwd → project_path`。返回首个命中（高优先级独占）。
///
/// `cwd` 既可以是当前工作目录，也可以是用户用 `--project` 显式指定的
/// 路径。我们对它做规范化（去掉尾斜杠 + 解析符号链接 best-effort）。
pub fn search_by_project(db: &Db, cwd: &Path) -> anyhow::Result<Option<ProjectMatch>> {
    let candidates = db.distinct_projects_with_counts()?;
    Ok(search_by_project_in_counted_candidates(
        cwd,
        &candidates,
        MIN_PROJECT_SESSIONS,
    ))
}

/// 测试便利版本：候选只有路径、没有会话计数。等价于把每条候选都视为
/// 计数 = 1，并把 `min_sessions = 1` 关掉过滤。保留这个签名是为了让
/// 历史单测继续起到回归保护作用，不重写。
pub fn search_by_project_in_candidates(cwd: &Path, candidates: &[String]) -> Option<ProjectMatch> {
    let with_counts: Vec<(String, i64)> = candidates.iter().map(|p| (p.clone(), 1)).collect();
    search_by_project_in_counted_candidates(cwd, &with_counts, 1)
}

/// 把数据库依赖剥离的纯函数版本，方便单测：传入候选 `(project_path, session_count)`
/// 列表与最小会话数阈值。会话数低于阈值的候选会被过滤，避免单条孤儿
/// 会话以"伪项目"身份在 Tier 1 starts_with 阶段抢断真实匹配。
pub fn search_by_project_in_counted_candidates(
    cwd: &Path,
    candidates: &[(String, i64)],
    min_sessions: i64,
) -> Option<ProjectMatch> {
    let cwd_norm = normalize(cwd);

    let eligible: Vec<&String> = candidates
        .iter()
        .filter(|(_, n)| *n >= min_sessions)
        .map(|(p, _)| p)
        .collect();

    // Tier 1: exact path  —  cwd 等于 project_path，或 cwd 是它的子目录。
    //   这是最强信号：会话当时的工作目录就在这里 / 它的子级。
    //
    // 多个候选都满足 starts_with 时（典型场景：候选既有 `/Users/me` 又有
    // `/Users/me/Documents/foo`），必须选**最长**的那个 —— 否则
    // 像 `/Users/me` 这种家目录会抢断所有子项目，hook 会把不相关的家目录
    // 摘要塞给 AI。`candidates` 来自 `ORDER BY project_path` 的字典序，
    // 不能依赖原始顺序，所以这里显式做最长前缀挑选。
    if let Some(p) = eligible
        .iter()
        .copied()
        .filter(|p| {
            let cand = normalize(Path::new(p));
            cwd_norm == cand || cwd_norm.starts_with(&cand)
        })
        .max_by_key(|p| normalize(Path::new(p)).as_os_str().len())
    {
        return Some(ProjectMatch {
            project_path: p.clone(),
            tier: MatchTier::ExactPath,
        });
    }

    // Tier 2: project name —— cwd 的 basename 等于某 project 的 basename。
    //   场景：项目仓库在不同机器上被 clone 到不同位置，但都叫 `memex`。
    let cwd_base = basename(&cwd_norm);
    if !cwd_base.is_empty()
        && let Some(p) = eligible.iter().copied().find(|p| {
            let cand = normalize(Path::new(p));
            basename(&cand) == cwd_base
        })
    {
        return Some(ProjectMatch {
            project_path: p.clone(),
            tier: MatchTier::ProjectName,
        });
    }

    // Tier 3: 子串模糊匹配 —— 仅对 ≥ 4 字符的 basename 启用，避免
    //   "a"、"src" 这种太短的目录名误命中一堆项目。
    if cwd_base.len() >= 4
        && let Some(p) = eligible.iter().copied().find(|p| {
            let cand_base = basename(&normalize(Path::new(p)));
            cand_base.contains(&cwd_base) || cwd_base.contains(&cand_base)
        })
    {
        return Some(ProjectMatch {
            project_path: p.clone(),
            tier: MatchTier::FuzzySubstring,
        });
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
