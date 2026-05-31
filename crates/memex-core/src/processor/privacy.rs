use std::fs;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
struct PrivacyConfig {
    #[serde(default)]
    private_paths: Vec<String>,
    #[serde(default)]
    private_keywords: Vec<String>,
}

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
    let mut config = PRIVACY_CONFIG.lock().unwrap();
    *config = parsed;
}

pub fn is_private_path(file_path: &str) -> bool {
    let config = PRIVACY_CONFIG.lock().unwrap();
    let lower = file_path.to_lowercase();
    config
        .private_paths
        .iter()
        .any(|p| lower.contains(&p.to_lowercase()))
}

pub fn is_private_content(content: &str) -> bool {
    let config = PRIVACY_CONFIG.lock().unwrap();
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

    #[test]
    fn test_load_privacy_rules() {
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
