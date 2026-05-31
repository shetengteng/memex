//! `~/.memex/credentials.toml` — local-only secret bag for cloud LLM keys.
//!
//! Goals:
//!   * Keep API keys out of `config.toml` (which is meant to be committable / shareable).
//!   * Always set 0600 permissions when we write the file ourselves.
//!   * Allow environment variables (`ANTHROPIC_API_KEY`) to override the file —
//!     this lets CI / temporary shells inject a key without touching disk.
//!
//! File shape:
//! ```toml
//! [anthropic]
//! api_key = "sk-ant-..."
//! model   = "claude-sonnet-4-20250514"   # optional
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "credentials.toml";
const ENV_ANTHROPIC_KEY: &str = "ANTHROPIC_API_KEY";
const ENV_ANTHROPIC_MODEL: &str = "ANTHROPIC_MODEL";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(default)]
    pub anthropic: Option<AnthropicCredentials>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnthropicCredentials {
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

pub fn credentials_path(memex_dir: &Path) -> PathBuf {
    memex_dir.join(FILE_NAME)
}

impl Credentials {
    /// Load `credentials.toml` if it exists. Returns `Default::default()`
    /// when the file is absent — the caller decides whether to fall back to
    /// environment variables.
    pub fn load(memex_dir: &Path) -> Result<Self> {
        let path = credentials_path(memex_dir);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let creds: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(creds)
    }

    /// Persist credentials and chmod the file to 0600 on Unix-like systems.
    /// On other platforms we still write the file but skip the permission step.
    pub fn save(&self, memex_dir: &Path) -> Result<()> {
        fs::create_dir_all(memex_dir)
            .with_context(|| format!("failed to create {}", memex_dir.display()))?;
        let path = credentials_path(memex_dir);
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)
            .with_context(|| format!("failed to write {}", path.display()))?;
        set_owner_only_permissions(&path)?;
        Ok(())
    }

    /// Resolve the effective Anthropic API key:
    ///   1. `credentials.toml` `[anthropic].api_key`
    ///   2. `ANTHROPIC_API_KEY` environment variable
    ///   3. `None`
    pub fn resolve_anthropic_key(&self) -> Option<String> {
        if let Some(c) = &self.anthropic
            && !c.api_key.trim().is_empty()
        {
            return Some(c.api_key.clone());
        }
        std::env::var(ENV_ANTHROPIC_KEY)
            .ok()
            .filter(|k| !k.trim().is_empty())
    }

    pub fn resolve_anthropic_model(&self) -> Option<String> {
        if let Some(c) = &self.anthropic
            && let Some(m) = &c.model
            && !m.trim().is_empty()
        {
            return Some(m.clone());
        }
        std::env::var(ENV_ANTHROPIC_MODEL)
            .ok()
            .filter(|k| !k.trim().is_empty())
    }
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)
        .with_context(|| format!("failed to chmod 0600 on {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let creds = Credentials::load(tmp.path()).unwrap();
        assert!(creds.anthropic.is_none());
        assert!(creds.resolve_anthropic_key().is_none());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let creds = Credentials {
            anthropic: Some(AnthropicCredentials {
                api_key: "sk-ant-example".into(),
                model: Some("claude-3-haiku-20240307".into()),
            }),
        };
        creds.save(tmp.path()).unwrap();
        let loaded = Credentials::load(tmp.path()).unwrap();
        assert_eq!(
            loaded.resolve_anthropic_key().as_deref(),
            Some("sk-ant-example")
        );
        assert_eq!(
            loaded.resolve_anthropic_model().as_deref(),
            Some("claude-3-haiku-20240307")
        );
    }

    #[cfg(unix)]
    #[test]
    fn save_sets_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        let creds = Credentials {
            anthropic: Some(AnthropicCredentials {
                api_key: "sk-ant-x".into(),
                model: None,
            }),
        };
        creds.save(tmp.path()).unwrap();
        let mode = fs::metadata(credentials_path(tmp.path()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "credentials.toml must be 0600");
    }

    #[test]
    fn whitespace_key_is_treated_as_missing() {
        let creds = Credentials {
            anthropic: Some(AnthropicCredentials {
                api_key: "   ".into(),
                model: None,
            }),
        };
        assert!(creds.resolve_anthropic_key().is_none());
    }
}
