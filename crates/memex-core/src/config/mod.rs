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

/// ⚠️  不要派生 `Default`：`#[derive(Default)]` 在每个字段上调用 `T::default()`，
/// 这会把 `bool` 字段变成 `false`，让 `#[serde(default = "default_true")]`
/// 失效（serde 默认值仅在「反序列化时字段缺失」才会触发，**不会**作用于
/// `Default::default()`）。OOB 首启时 `ensure_memex_dir` 写盘的就是
/// `MemexConfig::default()` 的结果，必须保证它本身就是「开箱可用」的。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptersConfig {
    #[serde(default = "default_true")]
    pub claude_code: bool,
    #[serde(default = "default_true")]
    pub cursor: bool,
    #[serde(default = "default_true")]
    pub codex: bool,
    #[serde(default = "default_true")]
    pub opencode: bool,
    #[serde(default = "default_true")]
    pub aider: bool,
    #[serde(default = "default_true")]
    pub continue_dev: bool,
    #[serde(default = "default_true")]
    pub cline: bool,
}

impl Default for AdaptersConfig {
    fn default() -> Self {
        Self {
            claude_code: true,
            cursor: true,
            codex: true,
            opencode: true,
            aider: true,
            continue_dev: true,
            cline: true,
        }
    }
}

/// 同上：不能 `#[derive(Default)]`，否则 String 字段会变成 `""`，
/// 导致 OOB config.toml 写出 `ollama_url = ""` / `ollama_model = ""`，
/// Ollama provider 永远无法被 `select_provider` 选中。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub ollama_enabled: bool,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    #[serde(default)]
    pub deepseek_enabled: bool,
    #[serde(default = "default_deepseek_model")]
    pub deepseek_model: String,
    #[serde(default)]
    pub cloud_fallback: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            ollama_enabled: false,
            ollama_url: default_ollama_url(),
            ollama_model: default_ollama_model(),
            deepseek_enabled: false,
            deepseek_model: default_deepseek_model(),
            cloud_fallback: false,
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

fn default_deepseek_model() -> String {
    "deepseek-chat".to_string()
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
        let default_config = MemexConfig::default();
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
        assert!(!c.llm.cloud_fallback);
    }

    #[test]
    fn default_adapters_all_enabled() {
        let c = MemexConfig::default();
        assert!(c.adapters.claude_code, "OOB adapters 必须默认全开，否则 watcher 没有任何目录可监听");
        assert!(c.adapters.cursor);
        assert!(c.adapters.codex);
        assert!(c.adapters.opencode);
        assert!(c.adapters.aider);
        assert!(c.adapters.continue_dev);
        assert!(c.adapters.cline);
    }

    #[test]
    fn default_privacy_has_redaction_on() {
        let c = MemexConfig::default();
        assert!(c.privacy.redaction_enabled, "默认必须开启脱敏（隐私默认安全）");
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
cloud_fallback = false

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
        assert!(written.contains("ollama_model = \"llama3.2\""), "实际写入:\n{}", written);
    }
}
