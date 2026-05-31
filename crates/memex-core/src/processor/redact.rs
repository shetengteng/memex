use std::fs;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use regex::Regex;
use serde::Deserialize;

struct RedactionRule {
    pattern: Regex,
    label: String,
}

#[derive(Debug, Deserialize)]
struct RedactionsFile {
    #[serde(default)]
    rules: Vec<CustomRule>,
}

#[derive(Debug, Deserialize)]
struct CustomRule {
    pattern: String,
    label: String,
}

static CUSTOM_RULES: LazyLock<Mutex<Vec<RedactionRule>>> = LazyLock::new(|| Mutex::new(Vec::new()));

pub fn load_custom_rules(path: &Path) {
    if !path.exists() {
        return;
    }
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let parsed: RedactionsFile = match serde_yaml_ng::from_str(&content) {
        Ok(p) => p,
        Err(_) => return,
    };
    let mut rules = CUSTOM_RULES.lock().unwrap();
    rules.clear();
    for rule in parsed.rules {
        if let Ok(re) = Regex::new(&rule.pattern) {
            rules.push(RedactionRule {
                pattern: re,
                label: rule.label,
            });
        }
    }
}

static RULES: LazyLock<Vec<RedactionRule>> = LazyLock::new(|| {
    vec![
        RedactionRule {
            pattern: Regex::new(r"sk-[a-zA-Z0-9_-]{20,}").unwrap(),
            label: "api_key".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(
                r#"(?i)(api[_\-]?key|token|secret)[:\s=]+['"]?([a-zA-Z0-9_\-/.]{16,})['"]?"#,
            )
            .unwrap(),
            label: "token".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap(),
            label: "email".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"(\+?86[-\s]?)?1[3-9]\d{9}").unwrap(),
            label: "phone".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
            label: "ip".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"\b[3-6]\d{3}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b").unwrap(),
            label: "bank_card".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"(?i)password\s*(is|=|:)\s*\S+").unwrap(),
            label: "password".to_string(),
        },
    ]
});

#[derive(Debug, Clone)]
pub struct RedactionHit {
    pub redaction_type: String,
    pub original_length: usize,
}

pub fn redact(content: &str) -> String {
    redact_with_hits(content).0
}

pub fn redact_with_hits(content: &str) -> (String, Vec<RedactionHit>) {
    let mut result = content.to_string();
    let mut hits = Vec::new();

    for rule in RULES.iter() {
        for m in rule.pattern.find_iter(&result.clone()) {
            hits.push(RedactionHit {
                redaction_type: rule.label.clone(),
                original_length: m.len(),
            });
        }
        result = rule
            .pattern
            .replace_all(&result, format!("[REDACTED:{}]", rule.label))
            .to_string();
    }
    if let Ok(custom) = CUSTOM_RULES.lock() {
        for rule in custom.iter() {
            for m in rule.pattern.find_iter(&result.clone()) {
                hits.push(RedactionHit {
                    redaction_type: rule.label.clone(),
                    original_length: m.len(),
                });
            }
            result = rule
                .pattern
                .replace_all(&result, format!("[REDACTED:{}]", rule.label))
                .to_string();
        }
    }
    (result, hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key() {
        let input = "The key is sk-ant-api03-abcdefghijklmnopqrst";
        let output = redact(input);
        assert!(output.contains("[REDACTED:api_key]"));
        assert!(!output.contains("sk-ant-api03"));
    }

    #[test]
    fn test_email() {
        let input = "Contact user@example.com for details";
        let output = redact(input);
        assert!(output.contains("[REDACTED:email]"));
        assert!(!output.contains("user@example.com"));
    }

    #[test]
    fn test_phone() {
        let input = "Call me at 13800138000";
        let output = redact(input);
        assert!(output.contains("[REDACTED:phone]"));
    }

    #[test]
    fn test_ip() {
        let input = "Server at 192.168.1.100 is down";
        let output = redact(input);
        assert!(output.contains("[REDACTED:ip]"));
    }

    #[test]
    fn test_bank_card() {
        let input = "Card number: 6222 0200 1234 5678";
        let output = redact(input);
        assert!(output.contains("[REDACTED:bank_card]"));
    }

    #[test]
    fn test_password_context() {
        let input = "password is mysecret123";
        let output = redact(input);
        assert!(output.contains("[REDACTED:password]"));
    }

    #[test]
    fn test_no_sensitive_info() {
        let input = "This is a normal text about programming";
        let output = redact(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_multiple_matches() {
        let input = "Keys: sk-ant-xxxxxxxxxxxxxxxxxxxx and sk-ant-yyyyyyyyyyyyyyyyyyyy";
        let output = redact(input);
        assert_eq!(output.matches("[REDACTED:api_key]").count(), 2);
    }

    #[test]
    fn test_custom_rules() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("memex_test_redact");
        std::fs::create_dir_all(&dir).unwrap();
        let yaml_path = dir.join("redactions.yaml");
        let mut f = std::fs::File::create(&yaml_path).unwrap();
        writeln!(f, "rules:").unwrap();
        writeln!(f, "  - pattern: \"INTERNAL-\\\\d+\"").unwrap();
        writeln!(f, "    label: internal_id").unwrap();
        drop(f);

        load_custom_rules(&yaml_path);

        let output = redact("Ticket INTERNAL-42 opened");
        assert!(output.contains("[REDACTED:internal_id]"));
        assert!(!output.contains("INTERNAL-42"));

        // cleanup
        std::fs::remove_dir_all(&dir).ok();
    }
}
