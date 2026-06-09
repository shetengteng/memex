use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use parking_lot::Mutex;
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
struct PrivacyConfig {
    #[serde(default)]
    private_paths: Vec<String>,
    #[serde(default)]
    private_keywords: Vec<String>,
}

// parking_lot::Mutex 不会 poison —— 这里的 lock() 永远不会返回 Err，
// 因此可以放心地直接拿 guard，无需 .unwrap() / fallback。
static PRIVACY_CONFIG: LazyLock<Mutex<PrivacyConfig>> =
    LazyLock::new(|| Mutex::new(PrivacyConfig::default()));

pub fn load_privacy_rules(path: &Path) {
    if !path.exists() {
        return;
    }
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let parsed: PrivacyConfig = match serde_yaml_ng::from_str(&content) {
        Ok(p) => p,
        Err(_) => return,
    };
    *PRIVACY_CONFIG.lock() = parsed;
}

pub fn is_private_path(file_path: &str) -> bool {
    let config = PRIVACY_CONFIG.lock();
    let lower = file_path.to_lowercase();
    config
        .private_paths
        .iter()
        .any(|p| lower.contains(&p.to_lowercase()))
}

pub fn is_private_content(content: &str) -> bool {
    let config = PRIVACY_CONFIG.lock();
    if config.private_keywords.is_empty() {
        return false;
    }
    let lower = content.to_lowercase();
    config
        .private_keywords
        .iter()
        .any(|k| lower.contains(&k.to_lowercase()))
}

pub fn is_private_session(file_path: &str, project_path: Option<&str>) -> bool {
    if is_private_path(file_path) {
        return true;
    }
    if project_path.is_some_and(is_private_path) {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    // 两个测试都写共享的 PRIVACY_CONFIG 全局状态，必须串行化执行。cargo test
    // 默认 --test-threads=N 会并发跑同 binary 的 #[test]，没有这个 gate
    // 任一 test 都可能看到对方写入的状态而 race。
    // 不引入 serial_test crate 是为避免单文件的隔离需求拉一个新依赖。
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_load_privacy_rules() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("redactions.yaml");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(
            f,
            "private_paths:\n  - /secret/project\n  - personal-diary\nprivate_keywords:\n  - confidential\n  - top-secret"
        )
        .unwrap();

        load_privacy_rules(&path);

        assert!(is_private_path("/home/user/secret/project/session.jsonl"));
        assert!(is_private_path("/data/personal-diary/chat.jsonl"));
        assert!(!is_private_path("/normal/project/data.jsonl"));

        assert!(is_private_content("This is CONFIDENTIAL information"));
        assert!(is_private_content("Classified as top-secret"));
        assert!(!is_private_content("Normal conversation about code"));
    }

    #[test]
    fn test_is_private_session() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("redactions.yaml");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "private_paths:\n  - /private-repo").unwrap();

        load_privacy_rules(&path);

        assert!(is_private_session(
            "/data/session.jsonl",
            Some("/private-repo")
        ));
        assert!(!is_private_session(
            "/data/session.jsonl",
            Some("/public-repo")
        ));
    }
}
