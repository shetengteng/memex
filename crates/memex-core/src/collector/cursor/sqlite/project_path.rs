//! 从 raw composerData JSON 里推断项目根路径。
//!
//! Cursor 的 composerData 里嵌着 `codeBlockData` 的 `file:///` URI 列表，
//! 通过收集前 10 个文件路径再求公共父目录，可以在 `composerHeaders`
//! enrichment 缺失（老 fixture / 旧版 Cursor）时仍然给出 project_path。
//!
//! 仅 LCA 不够：若用户本次会话只改了 `src/views/chat` 下的几个文件，
//! LCA 就会落到 `/repo/src/views/chat`，让同一个项目在 UI 上被切成
//! 多个 facet 行（`tt-demo`、`tt-demo/src`、`tt-demo/src/views`…）。
//! 解决办法：拿到 LCA 后向上爬，第一个含 `.git` / `Cargo.toml` /
//! `package.json` / `pyproject.toml` 等项目根 marker 的目录即视为真实
//! 项目根，否则退回原 LCA。这一步是 FS-touching 的，但每个会话只跑
//! 几次 stat，开销可忽略。

use std::path::{Path, PathBuf};

/// 走"路径深度阈值"过滤，避免把 `/Users/foo` 这种伪根路径误判。
/// `/Users/foo` = 2 components，要求至少 4 个就能把 `/Users/foo/proj` 留下。
const MIN_ANCESTOR_DEPTH: usize = 4;

/// 项目根 marker：第一个命中的目录即视为真实项目根。
/// 顺序按"出现频率 + 信号强度"排，常见的工具链先匹配减少 stat 次数。
const PROJECT_ROOT_MARKERS: &[&str] = &[
    ".git",
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "WORKSPACE",
    "BUILD.bazel",
];

/// 安全上限：不允许从子目录一直爬到 `/`。`/repo/src/views/chat/components`
/// 这种深度顶天也就 6~7 层，给 8 步预算即可；爬不到就退回原 LCA。
const WALKUP_MAX_DEPTH: usize = 8;

pub(super) fn infer_project_from_raw_json(raw: &str) -> Option<String> {
    let pattern = "\"file:///";
    let mut dirs: Vec<String> = Vec::new();
    for (i, _) in raw.match_indices(pattern) {
        let uri_start = i + 1;
        let uri_rest = &raw[uri_start..];
        if let Some(end) = uri_rest.find('"') {
            let uri = &uri_rest[..end];
            if let Some(path) = uri.strip_prefix("file://") {
                let decoded = percent_decode(path);
                let p = Path::new(&decoded);
                let dir = p
                    .parent()
                    .map(|d| d.to_string_lossy().to_string())
                    .unwrap_or_default();
                if dir.is_empty() {
                    continue;
                }
                if dir.contains("/.cursor/") || dir.contains("\\.cursor\\") {
                    continue;
                }
                if !dirs.contains(&dir) {
                    dirs.push(dir);
                    if dirs.len() >= 10 {
                        break;
                    }
                }
            }
        }
    }
    if dirs.is_empty() {
        return None;
    }
    find_common_ancestor(&dirs)
}

fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn find_common_ancestor(dirs: &[String]) -> Option<String> {
    if dirs.is_empty() {
        return None;
    }
    if dirs.len() == 1 {
        let depth = Path::new(&dirs[0]).components().count();
        return if depth >= MIN_ANCESTOR_DEPTH {
            Some(dirs[0].clone())
        } else {
            None
        };
    }
    let first = Path::new(&dirs[0]);
    let components: Vec<_> = first.components().collect();
    let mut common_len = components.len();
    for dir in &dirs[1..] {
        let p = Path::new(dir);
        let p_comps: Vec<_> = p.components().collect();
        let matching = components
            .iter()
            .zip(p_comps.iter())
            .take_while(|(a, b)| a == b)
            .count();
        common_len = common_len.min(matching);
    }
    if common_len < MIN_ANCESTOR_DEPTH {
        return None;
    }
    let ancestor: PathBuf = components[..common_len].iter().collect();
    let s = ancestor.to_string_lossy().to_string();
    if s.is_empty() || s == "/" {
        None
    } else {
        Some(s)
    }
}

/// 从 `start` 向上爬最多 [`WALKUP_MAX_DEPTH`] 层，第一个含
/// [`PROJECT_ROOT_MARKERS`] 任一 marker 的目录即视为项目根。
/// 失败（路径不存在 / 爬完没找到 / 已到文件系统顶）退回 `start`。
///
/// 调用方：cursor sqlite scan 在 enrichment 缺失走 LCA fallback 后调用，
/// 把 `/repo/src/views/chat` 这种"漂移到子目录"的 LCA 修正回 `/repo`。
pub(super) fn walkup_to_project_root(start: &Path) -> PathBuf {
    walkup_with(start, |p| {
        PROJECT_ROOT_MARKERS
            .iter()
            .any(|marker| p.join(marker).exists())
    })
}

/// `walkup_to_project_root` 的可注入版本，方便单测脱离真实 FS。
fn walkup_with<F>(start: &Path, has_marker: F) -> PathBuf
where
    F: Fn(&Path) -> bool,
{
    let mut current = start.to_path_buf();
    for _ in 0..WALKUP_MAX_DEPTH {
        if has_marker(&current) {
            return current;
        }
        let Some(parent) = current.parent() else {
            break;
        };
        if parent == current.as_path() {
            break;
        }
        current = parent.to_path_buf();
    }
    start.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn marker_set(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<PathBuf> = paths.iter().map(PathBuf::from).collect();
        move |p: &Path| set.contains(p)
    }

    #[test]
    fn walkup_returns_start_when_start_itself_has_marker() {
        let has = marker_set(&["/repo"]);
        assert_eq!(walkup_with(Path::new("/repo"), has), PathBuf::from("/repo"));
    }

    #[test]
    fn walkup_climbs_until_first_marker_hit() {
        let has = marker_set(&["/repo"]);
        assert_eq!(
            walkup_with(Path::new("/repo/src/views/chat"), has),
            PathBuf::from("/repo")
        );
    }

    #[test]
    fn walkup_stops_at_nearest_marker_not_deepest() {
        // 嵌套项目：monorepo 根 + 子包都有 marker，应该停在最先命中的
        //（也就是更靠近 start 的那个，即子包），不要爬到 monorepo 根。
        let has = marker_set(&["/repo", "/repo/packages/ui"]);
        assert_eq!(
            walkup_with(Path::new("/repo/packages/ui/src/views"), has),
            PathBuf::from("/repo/packages/ui")
        );
    }

    #[test]
    fn walkup_falls_back_to_start_when_no_marker_found() {
        let has = marker_set(&[]);
        let start = Path::new("/Users/me/orphan/dir");
        assert_eq!(walkup_with(start, has), start.to_path_buf());
    }

    #[test]
    fn walkup_respects_max_depth_and_does_not_run_forever() {
        // 即便 marker 在更深的祖先（理论上不可能，但要确保 depth 限制有效），
        // 也应在 WALKUP_MAX_DEPTH 步内停下。这里 marker 设在 11 层之外，
        // 函数应当退回 start 而不是无限循环或返回 marker。
        let has = marker_set(&["/a"]);
        let start = Path::new("/a/b/c/d/e/f/g/h/i/j/k/l");
        assert_eq!(walkup_with(start, has), start.to_path_buf());
    }
}
