//! `MemexConfig` 与其 4 个子结构 + 各字段的代码层默认值。
//!
//! 单独抽这一文件的两点考量：
//! 1. 这些 struct 是序列化契约（写到 `config.toml`、被 IPC 跨进程读），改动
//!    需要谨慎 review；
//! 2. 默认值函数的注释篇幅不小（特别是 `summary_cooldown_secs` /
//!    `summarize_interval_ms` 的取值理由），跟 load / detect 逻辑放一起会
//!    噪声过大。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemexConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    #[serde(default)]
    pub adapters: AdaptersConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub privacy: PrivacyConfig,
}

/// 数据源适配器默认全部关闭，用户在 Settings 页面按需开启。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptersConfig {
    #[serde(default)]
    pub claude_code: bool,
    #[serde(default)]
    pub cursor: bool,
    #[serde(default)]
    pub codex: bool,
    #[serde(default)]
    pub opencode: bool,
    #[serde(default)]
    pub aider: bool,
    #[serde(default)]
    pub continue_dev: bool,
    #[serde(default)]
    pub cline: bool,
}

/// 同上：不能 `#[derive(Default)]`，否则 String 字段会变成 `""`，
/// 导致 OOB config.toml 写出 `ollama_url = ""` / `ollama_model = ""`，
/// Ollama provider 永远无法被 `select_provider` 选中。
///
/// 仅保留 Ollama 老配置作为「快捷开关」入口；其他云端 provider
/// （OpenAI 兼容 / Anthropic）一律走 DB 中的 llm_providers。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmConfig {
    #[serde(default)]
    pub ollama_enabled: bool,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,

    /// 会话「冷却」秒数：sessions.updated_at 必须距离现在至少这么久，
    /// 才会被纳入 L2 摘要候选。配合方案 A「过期检测」一起防止 L2 摘要
    /// 过早固化以及高频抖动。默认 600 秒（10 分钟）。
    /// 设为 0 可禁用冷却（每次 ingest 立刻摘要 / 重摘要）。
    #[serde(default = "default_summary_cooldown_secs")]
    pub summary_cooldown_secs: u64,

    /// 批量摘要时，两次 LLM 调用之间的间隔毫秒数。
    /// 用户实测 100 个会话连跑 Ollama 会让本地 GPU/CPU 长时间高负载、UI 卡顿。
    /// 加 throttle 之后会显著降低瞬时压力，代价是总耗时变长。
    /// 默认 2000ms（2 秒）；设为 0 表示不 throttle（保持旧行为）。
    #[serde(default = "default_summarize_interval_ms")]
    pub summarize_interval_ms: u64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            ollama_enabled: false,
            ollama_url: default_ollama_url(),
            ollama_model: default_ollama_model(),
            summary_cooldown_secs: default_summary_cooldown_secs(),
            summarize_interval_ms: default_summarize_interval_ms(),
        }
    }
}

/// 同上：`redaction_enabled` 必须默认 true。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyConfig {
    #[serde(default = "default_true")]
    pub redaction_enabled: bool,
    #[serde(default)]
    pub skip_private_sessions: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            redaction_enabled: true,
            skip_private_sessions: false,
        }
    }
}

impl Default for MemexConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            adapters: AdaptersConfig::default(),
            llm: LlmConfig::default(),
            privacy: PrivacyConfig::default(),
        }
    }
}

pub(super) fn default_data_dir() -> String {
    "~/.memex".to_string()
}

fn default_true() -> bool {
    true
}

pub(super) fn default_ollama_url() -> String {
    "http://127.0.0.1:11434".to_string()
}

pub(super) fn default_ollama_model() -> String {
    "llama3.2".to_string()
}

/// L2 摘要冷却时间默认值（秒）。
/// 选 10 分钟：足以让 Claude Code / Cursor 等「短会话用完就关」的场景在关闭后
/// 一次性拿到全量内容摘要；同时短于绝大多数后台 ingest 频率，长会话也能在
/// 适度延迟内拿到「最新」摘要，而不至于每次 ingest 都重摘要。
fn default_summary_cooldown_secs() -> u64 {
    600
}

fn default_summarize_interval_ms() -> u64 {
    2000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memex_config_rejects_unknown_top_level_fields() {
        // Regression guard for #[serde(deny_unknown_fields)]: typos in the
        // top-level config.toml must error out instead of being silently
        // dropped into defaults.
        let toml = "data_dir = \"~/.memex\"\ntelemetry = true\n";
        let err = toml::from_str::<MemexConfig>(toml)
            .expect_err("unknown key `telemetry` must be rejected");
        assert!(
            err.to_string().contains("telemetry"),
            "error should mention the offending field, got: {err}"
        );
    }

    #[test]
    fn llm_config_rejects_unknown_fields() {
        // Regression guard against #[serde(deny_unknown_fields)] being silently
        // removed from LlmConfig. The common typo is to drop the trailing `s`
        // from `summarize_interval_ms`.
        let toml = "[llm]\nollama_enabled = true\nsummarize_intervall_ms = 1000\n";
        let err = toml::from_str::<MemexConfig>(toml)
            .expect_err("typo `summarize_intervall_ms` must be rejected");
        assert!(
            err.to_string().contains("summarize_intervall_ms"),
            "error should mention the offending field, got: {err}"
        );
    }

    #[test]
    fn memex_config_accepts_known_fields_with_defaults() {
        // Sanity check that #[serde(default)] still works alongside
        // deny_unknown_fields — missing keys must populate from defaults.
        let cfg: MemexConfig = toml::from_str("").expect("empty toml must parse");
        assert_eq!(cfg.data_dir, default_data_dir());
        assert!(cfg.llm.ollama_url.starts_with("http"));
    }
}
