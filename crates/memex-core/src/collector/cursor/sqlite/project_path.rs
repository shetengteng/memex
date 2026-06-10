//! 从 raw composerData JSON 里推断项目根路径。
//!
//! Cursor 的 composerData 里嵌着 `codeBlockData` 的 `file:///` URI 列表，
//! 通过收集前 10 个文件路径再求公共父目录，可以在 `composerHeaders`
//! enrichment 缺失（老 fixture / 旧版 Cursor）时仍然给出 project_path。
//!
//! 仅 LCA 不够：若用户本次会话只改了 `src/views/chat` 下的几个文件，
//! LCA 就会落到 `/repo/src/views/chat`，让同一个项目在 UI 上被切成
//! 多个 facet 行（`tt-demo`、`tt-demo/src`、`tt-demo/src/views`…）。
//!
//! 三道防线：
//! 1. enrichment.project_path（composerHeaders.workspaceIdentifier）：最可信，
//!    直接来自 Cursor workspace metadata，但可能命中 cursor 内部 pseudo 路径
//!    （`~/Library/.../User/workspaceStorage/<hash>`）—— 由
//!    [`is_cursor_internal_storage`] 过滤。
//! 2. LCA + [`walkup_to_project_root`]：FS-touching，命中 `.git` /
//!    `Cargo.toml` 等 marker 即视为真实项目根。**项目还在磁盘上时**有效。
//! 3. [`normalize_drifted_subdir`]：纯字符串启发式，给 walkup
//!    走不通（项目已被删 / 已搬走）的兜底——剥掉 `src` / `components` /
//!    `views/chat` 等显式子目录段，把 `tt-demo/src/views/chat` 还原成
//!    `tt-demo`。这一步必然有过修的风险（同名子目录 ≠ 一定是源码组织），
//!    但相比"按子目录切碎项目"，把同项目的多次对话合并到一个 facet
//!    上对用户更有价值。

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

/// [`normalize_drifted_subdir`] 用的"激进版"子目录段表。命中即剥掉。
///
/// 选材原则：
/// - 必须是"几乎只能出现在源码组织、不会出现在 repo 根名"的段；
/// - `src` / `lib` / `app` 等通用源码根；
/// - `components` / `views` / `pages` / `utils` 等前端常见；
/// - `service` / `controller` / `manager` / `dao` / `model` 等后端常见；
/// - 业务子域（`chat` / `knowledge`）—— 同一项目内多分支共用，剥掉合并 facet
///   更符合用户预期，即便偶有 repo 直接叫 `chat`，那种 repo 极少。
///
/// 顺序无关（用 HashSet 包装），全用小写比对。
const SUBDIR_SEGMENTS: &[&str] = &[
    // 通用源码根
    "src",
    "lib",
    "app",
    "apps",
    "packages",
    "crates",
    "modules",
    "internal",
    "pkg",
    "cmd",
    // 端 / 角色分包（独立条目）
    "frontend",
    "backend",
    "server",
    "client",
    "web",
    "mobile",
    "desktop",
    "admin",
    // 前端组织
    "components",
    "component",
    "views",
    "view",
    "pages",
    "page",
    "layouts",
    "layout",
    "utils",
    "util",
    "helpers",
    "helper",
    "hooks",
    "composables",
    "stores",
    "store",
    "router",
    "routes",
    "directives",
    "filters",
    "mixins",
    "plugins",
    "styles",
    "assets",
    "public",
    "static",
    // 后端组织
    "service",
    "services",
    "controller",
    "controllers",
    "manager",
    "managers",
    "dao",
    "daos",
    "repository",
    "repositories",
    "repo",
    "repos",
    "model",
    "models",
    "entity",
    "entities",
    "domain",
    "domains",
    "handler",
    "handlers",
    "middleware",
    "middlewares",
    "api",
    "apis",
    "rest",
    "rpc",
    "grpc",
    "graphql",
    "db",
    "database",
    "databases",
    "migration",
    "migrations",
    "schema",
    "schemas",
    "config",
    "configs",
    "configuration",
    "configurations",
    // 测试 / 文档
    "test",
    "tests",
    "spec",
    "specs",
    "__tests__",
    "e2e",
    "integration",
    "doc",
    "docs",
    "documentation",
    // 业务子域（保守列，避免误伤通用 repo 名）
    "chat",
    "knowledge",
    "feature",
    "features",
    "common",
    "shared",
    "core",
    "types",
    "interfaces",
    "constants",
];

/// JVM 系语言的 `src/main/java/<pkg>` 模式：碰到 `src/main/java`
/// 或 `src/main/kotlin` 之类，把它和后面的整段 package 路径一起剥掉。
/// 由 [`normalize_drifted_subdir`] 单独处理（pattern 复杂，不走通用段）。
const JVM_SRC_PREFIXES: &[&[&str]] = &[
    &["src", "main", "java"],
    &["src", "main", "kotlin"],
    &["src", "main", "scala"],
    &["src", "main", "groovy"],
    &["src", "main", "resources"],
    &["src", "test", "java"],
    &["src", "test", "kotlin"],
    &["src", "test", "scala"],
    &["src", "main"],
    &["src", "test"],
];

/// Cursor 把"无 workspace"的临时会话挂在内部 storage 下，路径形如：
/// `~/Library/Application Support/Cursor/User/workspaceStorage/<sha>`
/// 或 `~/.config/Cursor/User/workspaceStorage/<sha>`（Linux）。
/// 这些 pseudo workspace 进了 facet 就是噪音，必须过滤。
const CURSOR_INTERNAL_STORAGE_FRAGMENTS: &[&str] = &[
    "/Library/Application Support/Cursor/",
    "/Library/Application Support/cursor/",
    "/.config/Cursor/",
    "/.config/cursor/",
    "/AppData/Roaming/Cursor/",
    "/AppData/Roaming/cursor/",
    "/workspaceStorage/",
];

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

/// Cursor 内部 workspace storage 路径判定。命中即视为"无真实项目"，
/// 调用方应把对应 session 的 project_path 置 None。
pub(super) fn is_cursor_internal_storage(path: &str) -> bool {
    CURSOR_INTERNAL_STORAGE_FRAGMENTS
        .iter()
        .any(|frag| path.contains(frag))
}

/// 走纯字符串启发式把"漂移到子目录"的路径裁回项目根。
/// 这是 [`walkup_to_project_root`] 找不到 marker 时的兜底（项目已删 /
/// 已搬走 / FS 不可达）。
///
/// 规则（按优先级）：
/// 1. 先识别并整段剥掉 [`JVM_SRC_PREFIXES`] —— `…/src/main/java/com/foo` →
///    剥到 `src/main/java` 边界之前；
/// 2. 然后从前往后扫，**首个 [`SUBDIR_SEGMENTS`] 命中段**及其后所有段一并切除
///    （而不是仅从尾部逐段剥）。这样 `tt-demo/components/circular-time-picker`
///    能正确合并成 `tt-demo` —— 中间的 `circular-time-picker` 不在白名单
///    里也会被一起切掉，因为它一定是 `components` 下的某个具体组件名；
/// 3. 安全护栏：
///    - 至少保留 [`MIN_ANCESTOR_DEPTH`] 个 components（避免裁到 `/Users/foo`）；
///    - 剥到只剩根（`/`）也直接返回原路径，绝不输出无意义路径；
///    - 若原路径根本不含可剥段，返回原值（零开销）。
///
/// 大小写比对：用 `to_ascii_lowercase`，覆盖 `Src` / `Components` 等
/// macOS / Windows 大小写不一致情况。
///
/// 已知风险：若 repo 本身就叫 `components` / `src` 等子目录名（极罕见），
/// 会被裁到上层。权衡——为了让"同项目多子目录"在 UI facet 上合并，
/// 这种边界 case 用户可以通过 AI 摘要阶段的 `corrected_project_path` 兜底。
pub(super) fn normalize_drifted_subdir(path: &str) -> String {
    let original = path.to_string();
    if original.is_empty() {
        return original;
    }

    let p = PathBuf::from(&original);
    let mut comps: Vec<String> = p
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();

    if comps.len() < MIN_ANCESTOR_DEPTH {
        return original;
    }

    if let Some(stripped) = strip_jvm_prefix(&comps) {
        comps = stripped;
    }

    if let Some(cut) = first_subdir_segment_index(&comps)
        && cut >= MIN_ANCESTOR_DEPTH
    {
        comps.truncate(cut);
    }

    let rebuilt: PathBuf = comps.iter().collect();
    let s = rebuilt.to_string_lossy().into_owned();
    if s.is_empty() || s == "/" {
        original
    } else {
        s
    }
}

/// 找首个 [`SUBDIR_SEGMENTS`] 命中段的下标（用于 truncate 切点）。
///
/// 跳过前 `MIN_ANCESTOR_DEPTH + 1` 段以保护 repo 名本身：典型用户路径形如
/// `/Users/<name>/Documents/<repo>/src/...`，前 5 段（含 root）是
/// `/`, `Users`, `<name>`, `Documents`, `<repo>`，repo 即便恰好命中
/// SUBDIR_SEGMENTS（如真有人把 repo 叫 `src` / `lib`）也不应当被裁。
/// 这意味着 normalize 只对深度 ≥ 6 的路径生效——这是合理上限，浅路径
/// 几乎不可能漂到子目录。
fn first_subdir_segment_index(comps: &[String]) -> Option<usize> {
    comps
        .iter()
        .enumerate()
        .skip(MIN_ANCESTOR_DEPTH + 1)
        .find(|(_, seg)| is_subdir_segment(seg))
        .map(|(i, _)| i)
}

fn is_subdir_segment(seg: &str) -> bool {
    let lower = seg.to_ascii_lowercase();
    SUBDIR_SEGMENTS.contains(&lower.as_str())
}

/// 在 `comps` 里寻找 [`JVM_SRC_PREFIXES`] 任一前缀的"连续段"，
/// 命中则把该前缀及其后所有段截掉，返回新 comps。
/// 多个 JVM 前缀重叠时取最长匹配（`src/main/java` 优于 `src/main`）。
fn strip_jvm_prefix(comps: &[String]) -> Option<Vec<String>> {
    let lower: Vec<String> = comps.iter().map(|c| c.to_ascii_lowercase()).collect();

    let mut best_cut: Option<usize> = None;
    for prefix in JVM_SRC_PREFIXES {
        if prefix.len() > lower.len() {
            continue;
        }
        for start in 0..=lower.len() - prefix.len() {
            let window = &lower[start..start + prefix.len()];
            if window.iter().zip(prefix.iter()).all(|(a, b)| a == b) {
                let cut = start;
                if best_cut.is_none_or(|cur| cut < cur) {
                    best_cut = Some(cut);
                }
                break;
            }
        }
    }

    best_cut.map(|cut| comps[..cut].to_vec())
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

    #[test]
    fn normalize_strips_single_subdir_segment() {
        assert_eq!(
            normalize_drifted_subdir("/Users/me/Documents/personal/tt-demo/src"),
            "/Users/me/Documents/personal/tt-demo"
        );
    }

    #[test]
    fn normalize_strips_multiple_nested_subdirs() {
        // tt-demo/src/views/chat/components → tt-demo
        assert_eq!(
            normalize_drifted_subdir(
                "/Users/me/Documents/personal/tt-demo/src/views/chat/components"
            ),
            "/Users/me/Documents/personal/tt-demo"
        );
    }

    #[test]
    fn normalize_strips_business_subdomain_chat() {
        // 同项目内多次会话只动 src/views/chat → 应合并到 tt-demo facet
        assert_eq!(
            normalize_drifted_subdir("/Users/me/Documents/personal/tt-demo/src/views/chat"),
            "/Users/me/Documents/personal/tt-demo"
        );
    }

    #[test]
    fn normalize_keeps_repo_name_intact() {
        // repo 根本身就叫 src（罕见但合法），不能被裁掉成上层目录
        assert_eq!(
            normalize_drifted_subdir("/Users/me/Documents/personal/tt-demo"),
            "/Users/me/Documents/personal/tt-demo"
        );
    }

    #[test]
    fn normalize_handles_jvm_package_path() {
        // src/main/java/com/zoom/foo → 裁到 src/main/java 之前
        assert_eq!(
            normalize_drifted_subdir(
                "/Users/me/Workspace/zoom-svc/src/main/java/com/zoom/foo/service"
            ),
            "/Users/me/Workspace/zoom-svc"
        );
    }

    #[test]
    fn normalize_handles_jvm_test_path() {
        assert_eq!(
            normalize_drifted_subdir(
                "/Users/me/Workspace/zoom-svc/src/test/java/com/zoom/foo/ServiceTest"
            ),
            "/Users/me/Workspace/zoom-svc"
        );
    }

    #[test]
    fn normalize_respects_min_depth_guard() {
        // /a/src 只剩 2 components，低于 MIN_ANCESTOR_DEPTH(4) 不应继续裁
        assert_eq!(normalize_drifted_subdir("/a/src"), "/a/src");
    }

    #[test]
    fn normalize_case_insensitive_segment_match() {
        // macOS 大小写不敏感场景下 Cursor 可能给出 Src / Components
        assert_eq!(
            normalize_drifted_subdir("/Users/me/Documents/personal/tt-demo/Src/Components"),
            "/Users/me/Documents/personal/tt-demo"
        );
    }

    #[test]
    fn normalize_empty_input_returns_empty() {
        assert_eq!(normalize_drifted_subdir(""), "");
    }

    #[test]
    fn normalize_no_subdir_segment_returns_input() {
        let p = "/Users/me/Documents/personal/some-repo";
        assert_eq!(normalize_drifted_subdir(p), p);
    }

    #[test]
    fn normalize_strips_frontend_subdir() {
        // yuan-notes/frontend → yuan-notes（前后端分离 repo 内的子目录）
        assert_eq!(
            normalize_drifted_subdir("/Users/me/Documents/personal/yuan-notes/frontend"),
            "/Users/me/Documents/personal/yuan-notes"
        );
    }

    #[test]
    fn normalize_strips_backend_subdir() {
        assert_eq!(
            normalize_drifted_subdir("/Users/me/Documents/personal/yuan-notes/backend/server/api"),
            "/Users/me/Documents/personal/yuan-notes"
        );
    }

    #[test]
    fn normalize_strips_unknown_segment_after_known_one() {
        // components 后面跟具体组件名（不在白名单），也要整段切掉。
        // 这是之前从尾部剥版本的 BUG fixture。
        assert_eq!(
            normalize_drifted_subdir(
                "/Users/me/Documents/HBuilderProjects/tt-calc/components/sx-svg"
            ),
            "/Users/me/Documents/HBuilderProjects/tt-calc"
        );
    }

    #[test]
    fn normalize_repo_with_subdir_name_protected_by_min_depth() {
        // 罕见但合法：repo 名就叫 src（位置在 MIN_ANCESTOR_DEPTH=4 处）。
        // first_subdir_segment_index 的 skip(4) 保护可以避免裁过头。
        assert_eq!(
            normalize_drifted_subdir("/Users/me/Documents/src"),
            "/Users/me/Documents/src"
        );
    }

    #[test]
    fn is_internal_storage_detects_mac_paths() {
        assert!(is_cursor_internal_storage(
            "/Users/me/Library/Application Support/Cursor/User/workspaceStorage/abc123"
        ));
    }

    #[test]
    fn is_internal_storage_detects_linux_paths() {
        assert!(is_cursor_internal_storage(
            "/home/me/.config/Cursor/User/workspaceStorage/abc"
        ));
    }

    #[test]
    fn is_internal_storage_detects_workspace_storage_anywhere() {
        // workspaceStorage 即便不在标准位置，也属于内部路径
        assert!(is_cursor_internal_storage("/tmp/foo/workspaceStorage/bar"));
    }

    #[test]
    fn is_internal_storage_rejects_normal_project() {
        assert!(!is_cursor_internal_storage(
            "/Users/me/Documents/personal/tt-demo"
        ));
    }
}
