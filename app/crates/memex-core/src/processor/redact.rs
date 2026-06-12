use std::fs;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use regex::Regex;
use serde::Deserialize;

struct RedactionRule {
    pattern: Regex,
    label: String,
}

// The on-disk `redactions.yaml` is a *shared* file that is read independently
// by both `processor::redact` (rules) and `processor::privacy` (private_paths /
// private_keywords). Listing the sibling keys here as ignored placeholders
// allows `deny_unknown_fields` to still flag genuine typos in `rules:` /
// `pattern:` / `label:` without rejecting the legitimate privacy keys this
// module doesn't care about.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RedactionsFile {
    #[serde(default)]
    rules: Vec<CustomRule>,
    #[allow(dead_code)]
    #[serde(default)]
    private_paths: Vec<serde::de::IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    private_keywords: Vec<serde::de::IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
    let Ok(mut rules) = CUSTOM_RULES.lock() else {
        // poisoned mutex (another thread panicked while holding it) — drop the
        // new rules silently rather than propagating the panic. redact() also
        // tolerates a poisoned lock and just skips custom rules.
        return;
    };
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

// INVARIANT: every regex below is a hardcoded literal that must compile
// successfully at startup. A failure here is a programmer error, not a runtime
// condition — `.expect` makes that intent explicit and identifies which rule
// broke if the compile ever does fail.
static RULES: LazyLock<Vec<RedactionRule>> = LazyLock::new(|| {
    vec![
        RedactionRule {
            pattern: Regex::new(r"sk-[a-zA-Z0-9_-]{20,}")
                .expect("INVARIANT: api_key regex must compile"),
            label: "api_key".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(
                r#"(?i)(api[_\-]?key|token|secret)[:\s=]+['"]?([a-zA-Z0-9_\-/.]{16,})['"]?"#,
            )
            .expect("INVARIANT: token regex must compile"),
            label: "token".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
                .expect("INVARIANT: email regex must compile"),
            label: "email".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"(\+?86[-\s]?)?1[3-9]\d{9}")
                .expect("INVARIANT: phone regex must compile"),
            label: "phone".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b")
                .expect("INVARIANT: ip regex must compile"),
            label: "ip".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"\b[3-6]\d{3}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b")
                .expect("INVARIANT: bank_card regex must compile"),
            label: "bank_card".to_string(),
        },
        RedactionRule {
            pattern: Regex::new(r"(?i)password\s*(is|=|:)\s*\S+")
                .expect("INVARIANT: password regex must compile"),
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

    apply_rules(&RULES, &mut result, &mut hits);
    if let Ok(custom) = CUSTOM_RULES.lock() {
        apply_rules(&custom, &mut result, &mut hits);
    }
    (result, hits)
}

fn apply_rules(rules: &[RedactionRule], result: &mut String, hits: &mut Vec<RedactionHit>) {
    for rule in rules {
        let match_count = rule.pattern.find_iter(result).count();
        if match_count == 0 {
            continue;
        }
        for m in rule.pattern.find_iter(result.as_str()) {
            hits.push(RedactionHit {
                redaction_type: rule.label.clone(),
                original_length: m.len(),
            });
        }
        let replacement = format!("[REDACTED:{}]", rule.label);
        *result = rule
            .pattern
            .replace_all(result, replacement.as_str())
            .into_owned();
    }
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
    fn redactions_file_rejects_unknown_fields() {
        // Regression guard for #[serde(deny_unknown_fields)] on RedactionsFile:
        // sibling keys consumed by `processor::privacy` (`private_paths`,
        // `private_keywords`) must still parse, but a typo anywhere else must
        // surface immediately instead of being silently dropped.
        let yaml = "rules: []\nprivate_paths: []\nprivate_keywords: []\nrulesss: []\n";
        let err = serde_yaml_ng::from_str::<RedactionsFile>(yaml)
            .expect_err("typo `rulesss` must be rejected");
        assert!(
            err.to_string().contains("rulesss"),
            "error should mention the offending field, got: {err}"
        );
    }

    #[test]
    fn custom_rule_rejects_unknown_fields() {
        // Regression guard for #[serde(deny_unknown_fields)] on CustomRule.
        let yaml = "rules:\n  - pattern: foo\n    label: bar\n    extra: baz\n";
        let err = serde_yaml_ng::from_str::<RedactionsFile>(yaml)
            .expect_err("typo `extra` inside a rule must be rejected");
        assert!(
            err.to_string().contains("extra"),
            "error should mention the offending field, got: {err}"
        );
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

        // 清理临时文件
        std::fs::remove_dir_all(&dir).ok();
    }
}
