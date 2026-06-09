use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn default_data_dir() -> String {
    "~/.memex".to_string()
}

fn default_true() -> bool {
    true
}

fn default_ollama_url() -> String {
    "http://127.0.0.1:11434".to_string()
}

fn default_ollama_model() -> String {
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

impl MemexConfig {
    pub fn load(memex_dir: &Path) -> Result<Self> {
        let config_path = memex_dir.join("config.toml");
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))?;
            let mut config: Self = toml::from_str(&content)?;
            config.repair_empty_string_fields();
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// 存量用户从更早的版本升级上来时，可能写过 `ollama_url = ""` /
    /// `ollama_model = ""`（旧版 `#[derive(Default)]` 的副作用，见 LlmConfig 的
    /// 注释）。TOML 反序列化在「值存在但为空串」时不会触发 `#[serde(default)]`，
    /// 所以这里手动把空串当作未设置，回填代码层默认。
    /// 不持久化，只在内存里修复，避免误覆盖用户故意清空的字段。
    fn repair_empty_string_fields(&mut self) {
        if self.data_dir.trim().is_empty() {
            self.data_dir = default_data_dir();
        }
        if self.llm.ollama_url.trim().is_empty() {
            self.llm.ollama_url = default_ollama_url();
        }
        if self.llm.ollama_model.trim().is_empty() {
            self.llm.ollama_model = default_ollama_model();
        }
    }

    pub fn resolved_data_dir(&self) -> PathBuf {
        let expanded = shellexpand(&self.data_dir);
        PathBuf::from(expanded)
    }
}

pub fn ensure_memex_dir(memex_dir: &Path) -> Result<()> {
    fs::create_dir_all(memex_dir)
        .with_context(|| format!("failed to create {}", memex_dir.display()))?;

    let config_path = memex_dir.join("config.toml");
    if !config_path.exists() {
        let default_config = MemexConfig {
            adapters: detect_installed_adapters(),
            ..MemexConfig::default()
        };
        let content = toml::to_string_pretty(&default_config)?;
        fs::write(&config_path, content)?;
    }

    let redactions_path = memex_dir.join("redactions.yaml");
    if !redactions_path.exists() {
        fs::write(
            &redactions_path,
            concat!(
                "# Custom redaction rules (regex patterns)\nrules: []\n\n",
                "# Private session filtering\n",
                "# Sessions matching these paths will be skipped during ingest\n",
                "# and excluded from MCP search results\n",
                "private_paths: []\n",
                "# Example: private_paths: [\"/secret-project\", \"personal-diary\"]\n\n",
                "# Sessions containing these keywords will be marked private\n",
                "private_keywords: []\n",
                "# Example: private_keywords: [\"confidential\", \"internal-only\"]\n",
            ),
        )?;
    }

    fs::create_dir_all(memex_dir.join("sessions"))?;
    Ok(())
}

/// Auto-detect which IDE tools are installed and return an AdaptersConfig
/// with the corresponding flags set to true.
pub fn detect_installed_adapters() -> AdaptersConfig {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return AdaptersConfig::default(),
    };

    let claude_code = home.join(".claude/projects").exists();
    let cursor = {
        #[cfg(target_os = "macos")]
        {
            home.join("Library/Application Support/Cursor/User/globalStorage/state.vscdb")
                .exists()
        }
        #[cfg(not(target_os = "macos"))]
        {
            home.join(".config/Cursor/User/globalStorage/state.vscdb")
                .exists()
        }
    };
    let codex = home.join(".codex").exists();
    let opencode = home.join(".local/share/opencode/opencode.db").exists();
    let continue_dev = home.join(".continue/sessions").exists();
    let cline = {
        #[cfg(target_os = "macos")]
        {
            let data = dirs::data_dir().unwrap_or_else(|| home.join("Library/Application Support"));
            data.join("Code/User/globalStorage/saoudrizwan.claude-dev/tasks")
                .exists()
                || data
                    .join("Cursor/User/globalStorage/saoudrizwan.claude-dev/tasks")
                    .exists()
        }
        #[cfg(not(target_os = "macos"))]
        {
            let config = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
            config
                .join("Code/User/globalStorage/saoudrizwan.claude-dev/tasks")
                .exists()
        }
    };
    let aider = home.join(".aider.chat.history.md").exists();

    AdaptersConfig {
        claude_code,
        cursor,
        codex,
        opencode,
        aider,
        continue_dev,
        cline,
    }
}

fn shellexpand(s: &str) -> String {
    if s.starts_with("~/")
        && let Some(home) = dirs::home_dir()
    {
        return format!("{}{}", home.display(), &s[1..]);
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_llm_config_has_nonempty_ollama_url_and_model() {
        let c = MemexConfig::default();
        assert_eq!(
            c.llm.ollama_url, "http://127.0.0.1:11434",
            "MemexConfig::default() 必须给出可用的 ollama_url，否则 OOB 写出来的 config.toml 是空串"
        );
        assert_eq!(c.llm.ollama_model, "llama3.2");
        assert!(!c.llm.ollama_enabled);
        assert_eq!(
            c.llm.summary_cooldown_secs, 600,
            "默认 L2 摘要冷却时间应为 10 分钟（600 秒），与方案 B 的设计一致"
        );
    }

    #[test]
    fn default_adapters_all_disabled() {
        let c = MemexConfig::default();
        assert!(
            !c.adapters.claude_code,
            "OOB adapters 必须默认关闭，由用户按需开启"
        );
        assert!(!c.adapters.cursor);
        assert!(!c.adapters.codex);
        assert!(!c.adapters.opencode);
        assert!(!c.adapters.aider);
        assert!(!c.adapters.continue_dev);
        assert!(!c.adapters.cline);
    }

    #[test]
    fn default_privacy_has_redaction_on() {
        let c = MemexConfig::default();
        assert!(
            c.privacy.redaction_enabled,
            "默认必须开启脱敏（隐私默认安全）"
        );
        assert!(!c.privacy.skip_private_sessions);
    }

    #[test]
    fn load_existing_config_with_empty_ollama_strings_repairs_to_defaults() {
        // 模拟 v0.2.3 存量用户：MemexConfig::default() 在旧版本下把空串写进了
        // config.toml。新版本 load 时必须把空串视为「字段缺失」并回填默认值，
        // 否则升级到新版后行为还是错的（toml 反序列化在「值存在但为空串」时
        // **不会**触发 #[serde(default = "...")]）。
        let tmp = tempfile::tempdir().unwrap();
        let cfg_path = tmp.path().join("config.toml");
        fs::write(
            &cfg_path,
            r#"data_dir = "~/.memex"

[adapters]
claude_code = true
cursor = true
codex = true
opencode = true
aider = true
continue_dev = true
cline = true

[llm]
ollama_enabled = true
ollama_url = ""
ollama_model = ""

[privacy]
redaction_enabled = true
skip_private_sessions = false
"#,
        )
        .unwrap();
        let c = MemexConfig::load(tmp.path()).unwrap();
        assert!(c.llm.ollama_enabled);
        assert_eq!(
            c.llm.ollama_url, "http://127.0.0.1:11434",
            "存量用户配置里 ollama_url='' 必须被当作未设置回填默认值"
        );
        assert_eq!(c.llm.ollama_model, "llama3.2");
    }

    #[test]
    fn ensure_memex_dir_writes_nonempty_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        ensure_memex_dir(tmp.path()).unwrap();
        let written = fs::read_to_string(tmp.path().join("config.toml")).unwrap();
        assert!(
            written.contains("ollama_url = \"http://127.0.0.1:11434\""),
            "OOB config.toml 必须写入完整 ollama_url，否则新用户开关打开后 Ollama provider 选不上。\n实际写入:\n{}",
            written
        );
        assert!(
            written.contains("ollama_model = \"llama3.2\""),
            "实际写入:\n{}",
            written
        );
    }
}
