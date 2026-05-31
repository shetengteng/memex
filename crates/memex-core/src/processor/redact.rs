use regex::Regex;
use std::sync::LazyLock;

struct RedactionRule {
    pattern: Regex,
    label: &'static str,
}

static RULES: LazyLock<Vec<RedactionRule>> = LazyLock::new(|| {
    vec![
        RedactionRule {
            pattern: Regex::new(r"sk-[a-zA-Z0-9_-]{20,}").unwrap(),
            label: "api_key",
        },
        RedactionRule {
            pattern: Regex::new(r#"(?i)(api[_\-]?key|token|secret)[:\s=]+['"]?([a-zA-Z0-9_\-/.]{16,})['"]?"#).unwrap(),
            label: "token",
        },
        RedactionRule {
            pattern: Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap(),
            label: "email",
        },
        RedactionRule {
            pattern: Regex::new(r"(\+?86[-\s]?)?1[3-9]\d{9}").unwrap(),
            label: "phone",
        },
        RedactionRule {
            pattern: Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
            label: "ip",
        },
        RedactionRule {
            pattern: Regex::new(r"\b[3-6]\d{3}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b").unwrap(),
            label: "bank_card",
        },
        RedactionRule {
            pattern: Regex::new(r"(?i)password\s*(is|=|:)\s*\S+").unwrap(),
            label: "password",
        },
    ]
});

pub fn redact(content: &str) -> String {
    let mut result = content.to_string();
    for rule in RULES.iter() {
        result = rule
            .pattern
            .replace_all(&result, format!("[REDACTED:{}]", rule.label))
            .to_string();
    }
    result
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
}
