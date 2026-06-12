//! Tauri IPC 错误类型 — 跨 Rust ↔ TS 的序列化错误契约。
//!
//! 设计依据 `rust.mdc §1.2`：FFI / 序列化边界使用实现 `Serialize` 的具名错误枚举，
//! 这样前端可以按 `kind` 字段精确匹配错误类别，而不是猜字符串前缀。
//!
//! ## 序列化形态（前端见到的 catch 值）
//!
//! ```jsonc
//! { "kind": "io",          "message": "..." }
//! { "kind": "db",          "message": "..." }
//! { "kind": "config",      "message": "..." }
//! { "kind": "not_found",   "message": "..." }
//! { "kind": "validation",  "message": "..." }
//! { "kind": "backend",     "message": "..." }
//! ```
//!
//! `serde(tag = "kind", content = "message", rename_all = "snake_case")` 把
//! 内部表示 `Io(String)` 序列化成 `{kind:"io", message:"..."}`。
//!
//! ## 转换规则
//!
//! - `std::io::Error`  → `Io`
//! - `anyhow::Error`   → `Backend`（保留完整错误链 `{e:#}`）
//! - `String`          → `Backend`（兼容历史 `Result<T, String>` 的迁移期）
//! - `&'static str`    → `Backend`
//!
//! 业务代码若需要更精确的语义，应**显式构造**对应 variant：
//! `Err(CmdError::NotFound("session 不存在".into()))`、
//! `Err(CmdError::Db("upsert failed".into()))`，而不是依赖 `From`。
//! Memex commands 不直接调用 rusqlite，DB 错误由 memex-core 包成 `anyhow::Result`
//! 上抛，到 Tauri command 边界自动落到 `Backend`，需要细分时再显式 map 到 `Db`。
//!
//! ## 与 anyhow::Error 的关系
//!
//! `memex-core` 库内部全用 `anyhow::Result` 表达 backend 错误，到达 Tauri command
//! 边界时通过 `?` 自动转 `CmdError::Backend`。这与 §1.2 "库 crate 用 thiserror，
//! 应用层用 anyhow，FFI 边界用 Serialize 错误枚举" 的三层模型完全一致。

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum CmdError {
    #[error("io: {0}")]
    Io(String),
    #[error("db: {0}")]
    Db(String),
    #[error("config: {0}")]
    Config(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("backend: {0}")]
    Backend(String),
}

impl From<std::io::Error> for CmdError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<anyhow::Error> for CmdError {
    fn from(e: anyhow::Error) -> Self {
        // `{e:#}` 展开 anyhow context chain，避免丢失上下文。
        Self::Backend(format!("{e:#}"))
    }
}

impl From<String> for CmdError {
    fn from(s: String) -> Self {
        Self::Backend(s)
    }
}

impl From<&'static str> for CmdError {
    fn from(s: &'static str) -> Self {
        Self::Backend(s.to_string())
    }
}

/// Tauri command 函数体内常用的 `Result` 别名。
///
/// 写法：`pub async fn my_cmd() -> CmdResult<MyDto> { ... }`
pub type CmdResult<T> = Result<T, CmdError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_kind_message_object() {
        let json = serde_json::to_value(CmdError::NotFound("session abc".into())).unwrap();
        assert_eq!(json["kind"], "not_found");
        assert_eq!(json["message"], "session abc");
    }

    #[test]
    fn all_variants_serialize_with_snake_case_kind() {
        let cases = [
            (CmdError::Io("a".into()), "io"),
            (CmdError::Db("b".into()), "db"),
            (CmdError::Config("c".into()), "config"),
            (CmdError::NotFound("d".into()), "not_found"),
            (CmdError::Validation("e".into()), "validation"),
            (CmdError::Backend("f".into()), "backend"),
        ];
        for (err, expected_kind) in cases {
            let v = serde_json::to_value(&err).unwrap();
            assert_eq!(v["kind"], expected_kind, "variant {err:?}");
        }
    }

    #[test]
    fn from_io_error_yields_io_variant() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let cmd: CmdError = io.into();
        assert!(matches!(cmd, CmdError::Io(_)));
        let v = serde_json::to_value(&cmd).unwrap();
        assert_eq!(v["kind"], "io");
        assert_eq!(v["message"], "missing");
    }

    #[test]
    fn from_anyhow_preserves_context_chain() {
        let wrapped = anyhow::anyhow!("root cause")
            .context("middle layer")
            .context("outer layer");
        let cmd: CmdError = wrapped.into();
        let CmdError::Backend(msg) = cmd else {
            panic!("expected Backend variant");
        };
        assert!(msg.contains("outer layer"), "msg = {msg}");
        assert!(msg.contains("middle layer"), "msg = {msg}");
        assert!(msg.contains("root cause"), "msg = {msg}");
    }

    #[test]
    fn from_string_yields_backend_variant() {
        let cmd: CmdError = String::from("legacy error").into();
        assert!(matches!(cmd, CmdError::Backend(ref s) if s == "legacy error"));
    }

    #[test]
    fn display_includes_kind_prefix() {
        assert_eq!(
            format!("{}", CmdError::NotFound("x".into())),
            "not found: x"
        );
        assert_eq!(format!("{}", CmdError::Io("y".into())), "io: y");
    }
}
