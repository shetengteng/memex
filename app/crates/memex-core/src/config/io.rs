//! `MemexConfig` 的磁盘 I/O：load / first-run scaffolding / OS adapter 探测。
//!
//! 业务规则放在这里：
//! - `MemexConfig::load` 容忍历史版本的空串字段（详见 `repair_empty_string_fields`）；
//! - `ensure_memex_dir` 是首次安装时的 OOB 模板写入入口；
//! - `detect_installed_adapters` 在写默认 `config.toml` 之前用文件系统嗅探
//!   哪些 IDE 真的装了，让用户少改一次配置。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::{
    AdaptersConfig, MemexConfig, default_data_dir, default_ollama_model, default_ollama_url,
};

impl MemexConfig {
    pub fn load(memex_dir: &Path) -> Result<Self> {
        let config_path = memex_dir.join("config.toml");
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        let mut config: Self = toml::from_str(&content)?;
        config.repair_empty_string_fields();
        Ok(config)
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
    let Some(home) = dirs::home_dir() else {
        return AdaptersConfig::default();
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
