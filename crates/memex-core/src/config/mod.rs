//! `MemexConfig` —— 用户级配置入口。
//!
//! 模块切分：
//! - `types`：所有 `#[derive(Serialize, Deserialize)]` 的 struct + `Default` impls +
//!   字段级默认值。
//! - `io`：磁盘 load / 首次安装写默认值 / 探测已安装 IDE。
//! - `tests`：序列化往返 + 存量空串回填 + 默认值断言。

mod io;
#[cfg(test)]
mod tests;
mod types;

pub use io::{detect_installed_adapters, ensure_memex_dir};
pub use types::{AdaptersConfig, LlmConfig, MemexConfig, PrivacyConfig};
