//! 从 raw composerData JSON 里推断项目根路径。
//!
//! Cursor 的 composerData 里嵌着 `codeBlockData` 的 `file:///` URI 列表，
//! 通过收集前 10 个文件路径再求公共父目录，可以在 `composerHeaders`
//! enrichment 缺失（老 fixture / 旧版 Cursor）时仍然给出 project_path。

/// 走"路径深度阈值"过滤，避免把 `/Users/foo` 这种伪根路径误判。
/// `/Users/foo` = 2 components，要求至少 4 个就能把 `/Users/foo/proj` 留下。
const MIN_ANCESTOR_DEPTH: usize = 4;

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
                let p = std::path::Path::new(&decoded);
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
        let depth = std::path::Path::new(&dirs[0]).components().count();
        return if depth >= MIN_ANCESTOR_DEPTH {
            Some(dirs[0].clone())
        } else {
            None
        };
    }
    let first = std::path::Path::new(&dirs[0]);
    let components: Vec<_> = first.components().collect();
    let mut common_len = components.len();
    for dir in &dirs[1..] {
        let p = std::path::Path::new(dir);
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
    let ancestor: std::path::PathBuf = components[..common_len].iter().collect();
    let s = ancestor.to_string_lossy().to_string();
    if s.is_empty() || s == "/" {
        None
    } else {
        Some(s)
    }
}
