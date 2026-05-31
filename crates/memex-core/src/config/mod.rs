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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdaptersConfig {
    #[serde(default = "default_true")]
    pub claude_code: bool,
    #[serde(default = "default_true")]
    pub cursor: bool,
    #[serde(default = "default_true")]
    pub codex: bool,
    #[serde(default = "default_true")]
    pub opencode: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub ollama_enabled: bool,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    #[serde(default)]
    pub cloud_fallback: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrivacyConfig {
    #[serde(default = "default_true")]
    pub redaction_enabled: bool,
    #[serde(default)]
    pub skip_private_sessions: bool,
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
            let config: Self = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
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
            "# Custom redaction rules\n# Format: list of regex patterns\nrules: []\n",
        )?;
    }

    fs::create_dir_all(memex_dir.join("sessions"))?;
    Ok(())
}

fn shellexpand(s: &str) -> String {
    if s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), &s[1..]);
        }
    }
    s.to_string()
}
