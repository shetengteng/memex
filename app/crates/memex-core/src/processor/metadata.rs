use crate::storage::models::ChunkMetadata;
use regex::Regex;
use std::sync::LazyLock;

// INVARIANT: all three regex strings are compile-time const literals; failure
// to compile is a programmer error caught immediately in test, not a runtime
// condition. `.expect` makes that contract explicit.
static CODE_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"```(\w+)").expect("INVARIANT: CODE_LANG_RE must compile"));

static ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(error|panic|exception|failed|traceback|fatal)\b")
        .expect("INVARIANT: ERROR_RE must compile")
});

static TOOL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(Read|Write|Shell|Grep|Search|Delete|StrReplace|Glob)\b")
        .expect("INVARIANT: TOOL_RE must compile")
});

pub fn extract(content: &str) -> ChunkMetadata {
    ChunkMetadata {
        topics: extract_topics(content),
        languages: extract_languages(content),
        has_code: content.contains("```"),
        tools_used: extract_tools(content),
        error_keywords: extract_errors(content),
    }
}

fn extract_languages(content: &str) -> Vec<String> {
    let mut langs: Vec<String> = CODE_LANG_RE
        .captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_lowercase()))
        .filter(|l| !l.is_empty() && l != "text" && l != "plain")
        .collect();
    langs.sort();
    langs.dedup();
    langs
}

fn extract_errors(content: &str) -> Vec<String> {
    let mut errors: Vec<String> = ERROR_RE
        .find_iter(content)
        .map(|m| m.as_str().to_lowercase())
        .collect();
    errors.sort();
    errors.dedup();
    errors
}

fn extract_tools(content: &str) -> Vec<String> {
    let mut tools: Vec<String> = TOOL_RE
        .find_iter(content)
        .map(|m| m.as_str().to_string())
        .collect();
    tools.sort();
    tools.dedup();
    tools
}

fn extract_topics(content: &str) -> Vec<String> {
    static TOPIC_WORDS: LazyLock<Vec<&str>> = LazyLock::new(|| {
        vec![
            "redis",
            "database",
            "sql",
            "api",
            "auth",
            "docker",
            "kubernetes",
            "react",
            "vue",
            "rust",
            "python",
            "typescript",
            "javascript",
            "git",
            "ci",
            "cd",
            "deploy",
            "test",
            "debug",
            "performance",
            "cache",
            "queue",
            "kafka",
            "grpc",
            "http",
            "websocket",
            "oauth",
            "jwt",
            "encryption",
            "migration",
            "refactor",
        ]
    });

    let lower = content.to_lowercase();
    TOPIC_WORDS
        .iter()
        .filter(|&&topic| lower.contains(topic))
        .map(|&s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_languages() {
        let content = "Here is code:\n```python\nprint('hi')\n```\nAnd:\n```rust\nfn main(){}\n```";
        let meta = extract(content);
        assert!(meta.languages.contains(&"python".to_string()));
        assert!(meta.languages.contains(&"rust".to_string()));
    }

    #[test]
    fn test_has_code() {
        assert!(extract("```python\nx\n```").has_code);
        assert!(!extract("no code here").has_code);
    }

    #[test]
    fn test_extract_topics() {
        let meta = extract("Let's use redis for cache layer and deploy with docker");
        assert!(meta.topics.contains(&"redis".to_string()));
        assert!(meta.topics.contains(&"docker".to_string()));
        assert!(meta.topics.contains(&"cache".to_string()));
        assert!(meta.topics.contains(&"deploy".to_string()));
    }

    #[test]
    fn test_extract_errors() {
        let meta = extract("Got an Error: connection failed and a panic occurred");
        assert!(meta.error_keywords.contains(&"error".to_string()));
        assert!(meta.error_keywords.contains(&"failed".to_string()));
        assert!(meta.error_keywords.contains(&"panic".to_string()));
    }

    #[test]
    fn test_extract_tools() {
        let meta = extract("I used Read tool and then Shell to run the command");
        assert!(meta.tools_used.contains(&"Read".to_string()));
        assert!(meta.tools_used.contains(&"Shell".to_string()));
    }
}
