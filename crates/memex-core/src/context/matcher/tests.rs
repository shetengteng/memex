use std::path::Path;

use super::*;

fn paths(arr: &[&str]) -> Vec<String> {
    arr.iter().map(|s| s.to_string()).collect()
}

fn counted(arr: &[(&str, i64)]) -> Vec<(String, i64)> {
    arr.iter().map(|(p, n)| (p.to_string(), *n)).collect()
}

#[test]
fn exact_path_takes_precedence() {
    let cands = paths(&["/home/u/foo", "/home/u/bar"]);
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

#[test]
fn tier1_prefers_longest_prefix_over_home_dir() {
    // 回归 bug：候选里同时存在「家目录」和「家目录下的具体项目」时，
    // 在项目目录内的 cwd 必须命中具体项目，而不是被字典序在前的家目录抢断。
    let cands = paths(&[
        "/Users/me",
        "/Users/me/Documents/personal/foo",
        "/Users/me/Documents/personal/bar",
    ]);
    let m =
        search_by_project_in_candidates(Path::new("/Users/me/Documents/personal/foo"), &cands)
            .unwrap();
    assert_eq!(m.tier, MatchTier::ExactPath);
    assert_eq!(
        m.project_path, "/Users/me/Documents/personal/foo",
        "应命中最长前缀，而非家目录"
    );
}

#[test]
fn tier1_longest_prefix_extends_to_subdirectory() {
    // cwd 在项目下的子目录里，候选既有家目录又有具体项目时，仍命中具体项目。
    let cands = paths(&["/Users/me", "/Users/me/work/proj"]);
    let m = search_by_project_in_candidates(Path::new("/Users/me/work/proj/src/utils"), &cands)
        .unwrap();
    assert_eq!(m.tier, MatchTier::ExactPath);
    assert_eq!(m.project_path, "/Users/me/work/proj");
}

#[test]
fn tier1_home_dir_still_matches_when_no_subproject_does() {
    // 当 cwd 不在任何具体项目内时，家目录仍然是合法的 fallback 命中。
    let cands = paths(&["/Users/me", "/Users/me/work/proj"]);
    let m = search_by_project_in_candidates(Path::new("/Users/me/Downloads/random"), &cands)
        .unwrap();
    assert_eq!(m.tier, MatchTier::ExactPath);
    assert_eq!(m.project_path, "/Users/me");
}

#[test]
fn min_sessions_filters_orphan_home_dir() {
    // 回归 bug：用户 `~/.claude/projects/-Users-TerrellShe/<id>.jsonl`
    // 写过一次 `login` 测试会话 → 数据库里出现 project_path = `/Users/me`
    // 但只有 1 个会话。在它之外还有真实子项目（10 个会话）。
    // 期望：在 `/Users/me/Documents` 这种"中间目录" cwd 下，**不**
    // 命中孤儿家目录，而是返回 None（让上层 banner 兜底）。
    let cands = counted(&[
        ("/Users/me", 1),
        ("/Users/me/Documents/work/real-project", 10),
    ]);
    let r = search_by_project_in_counted_candidates(
        Path::new("/Users/me/Documents"),
        &cands,
        MIN_PROJECT_SESSIONS,
    );
    assert!(
        r.is_none(),
        "孤儿家目录（1 个会话）不应抢断真实项目候选，应返回 None"
    );

    // 反之，cwd 落到真实项目内时仍然命中
    let m = search_by_project_in_counted_candidates(
        Path::new("/Users/me/Documents/work/real-project/src"),
        &cands,
        MIN_PROJECT_SESSIONS,
    )
    .unwrap();
    assert_eq!(m.tier, MatchTier::ExactPath);
    assert_eq!(m.project_path, "/Users/me/Documents/work/real-project");
}

#[test]
fn min_sessions_promotes_home_dir_once_it_has_enough_sessions() {
    // 边界：如果用户确实在家目录直接跑 AI 会话，并且累计 >= 阈值，
    // 那家目录就成了合法 fallback —— 跟旧测试同样可用。
    let cands = counted(&[("/Users/me", 5), ("/Users/me/work/proj", 10)]);
    let m = search_by_project_in_counted_candidates(
        Path::new("/Users/me/Downloads/random"),
        &cands,
        MIN_PROJECT_SESSIONS,
    )
    .unwrap();
    assert_eq!(m.tier, MatchTier::ExactPath);
    assert_eq!(m.project_path, "/Users/me");
}
