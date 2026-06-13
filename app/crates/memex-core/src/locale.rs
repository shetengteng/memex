//! Global UI locale used by LLM prompts and user-facing markdown fallbacks.
//!
//! This is intentionally a tiny atomic instead of a `Mutex` / `RwLock`:
//! `PromptLocale::current()` is read on every LLM call (and inside hot
//! `to_markdown` loops), but the value rarely flips — only on app startup
//! and when the user toggles the language picker in Settings. Atomics keep
//! the read path lock-free.
//!
//! Lifecycle:
//! 1. App startup (menubar / daemon / cli) calls
//!    [`PromptLocale::sync_from_kv`] with the persisted `ui.locale` key from
//!    the SQLite kv table. If the key is absent, defaults to [`PromptLocale::Zh`].
//! 2. UI Settings → "界面语言" toggles `set_config("ui.locale", ...)`. The
//!    Tauri command catches that key specifically and calls
//!    [`PromptLocale::set`] so the next LLM call uses the new locale —
//!    without restarting the app.
//! 3. Any prompt module that needs locale-aware output reads
//!    [`PromptLocale::current()`].

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptLocale {
    Zh,
    En,
}

impl Default for PromptLocale {
    fn default() -> Self {
        Self::Zh
    }
}

impl PromptLocale {
    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "english" => Self::En,
            _ => Self::Zh,
        }
    }

    pub fn current() -> Self {
        match CURRENT.load(Ordering::Relaxed) {
            1 => Self::En,
            _ => Self::Zh,
        }
    }

    pub fn set(loc: PromptLocale) {
        let v = match loc {
            Self::Zh => 0,
            Self::En => 1,
        };
        CURRENT.store(v, Ordering::Relaxed);
    }

    /// Convenience: read `ui.locale` from the kv table and apply it.
    pub fn sync_from_kv(db: &crate::storage::db::Db) {
        if let Ok(Some(s)) = db.kv_get("ui.locale") {
            Self::set(Self::from_str_lossy(&s));
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_lossy_recognizes_english_aliases() {
        assert_eq!(PromptLocale::from_str_lossy("en"), PromptLocale::En);
        assert_eq!(PromptLocale::from_str_lossy("EN"), PromptLocale::En);
        assert_eq!(PromptLocale::from_str_lossy(" en-US "), PromptLocale::En);
        assert_eq!(PromptLocale::from_str_lossy("English"), PromptLocale::En);
    }

    #[test]
    fn from_str_lossy_falls_back_to_zh() {
        assert_eq!(PromptLocale::from_str_lossy(""), PromptLocale::Zh);
        assert_eq!(PromptLocale::from_str_lossy("zh"), PromptLocale::Zh);
        assert_eq!(PromptLocale::from_str_lossy("zh-CN"), PromptLocale::Zh);
        assert_eq!(PromptLocale::from_str_lossy("ja"), PromptLocale::Zh);
    }
}
