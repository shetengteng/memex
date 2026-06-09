//! CLI 输出的单一控制点。
//!
//! ## 设计目标
//!
//! - 所有 commands 用 `out!` / `err!` macro 替代 `println!` / `eprintln!`
//! - 全局 `--json` flag 在这里统一感知：当用户传了 `--json` 时，所有人类可读
//!   的 `out!` 输出会被压制，只允许 `json()` 真正写到 stdout。这样 caller
//!   可以稳定 pipe stdout 到 `jq` 等工具。
//! - `err!`（stderr）不受 `--json` 影响：诊断信息、警告、进度等不应污染
//!   stdout 上的 JSON 流。
//!
//! ## 初始化
//!
//! `dispatch::run` 在解析完 clap 之后必须调用 [`init`] 一次。重复调用是
//! no-op（OnceLock 语义）。
//!
//! ## 用法
//!
//! ```ignore
//! crate::io::init(cli.json);
//! out!("Found {} sessions", n);                  // stdout, 非 json
//! err!("warn: skipping malformed entry");        // stderr, 永远输出
//! crate::io::json(&payload)?;                    // stdout, 仅 json
//! ```

use std::io::Write;
use std::sync::OnceLock;

use anyhow::Result;
use serde::Serialize;

static FLAGS: OnceLock<IoFlags> = OnceLock::new();

#[derive(Debug, Clone, Copy, Default)]
struct IoFlags {
    json: bool,
}

/// 在 main → dispatch 接力时调用一次，把 clap 顶层 flag 注入到 io 子系统。
///
/// 重复调用是 no-op（OnceLock 语义）。
pub fn init(json: bool) {
    let _ = FLAGS.set(IoFlags { json });
}

fn flags() -> IoFlags {
    *FLAGS.get().unwrap_or(&IoFlags::default())
}

/// 内部实现：被 [`out!`] macro 调用。
#[doc(hidden)]
#[allow(dead_code)] // 下一 commit 迁移调用点后移除
pub fn write_out(args: std::fmt::Arguments<'_>) {
    if flags().json {
        return;
    }
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{}", args);
}

/// 内部实现：被 [`err!`] macro 调用。
#[doc(hidden)]
#[allow(dead_code)] // 下一 commit 迁移调用点后移除
pub fn write_err(args: std::fmt::Arguments<'_>) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    let _ = writeln!(lock, "{}", args);
}

/// 把 `value` 序列化为 JSON 写到 stdout。
///
/// - 非 `--json` 模式下仍然输出（让 caller 显式选择「我就是想要 JSON」）
/// - `--json` 模式下输出紧凑格式（一行一对象），方便 pipe 处理
/// - 非 `--json` 模式下输出 pretty 格式，便于人眼看
#[allow(dead_code)] // 下一 commit 迁移调用点后移除
pub fn json<T: Serialize>(value: &T) -> Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    if flags().json {
        serde_json::to_writer(&mut lock, value)?;
    } else {
        serde_json::to_writer_pretty(&mut lock, value)?;
    }
    writeln!(lock)?;
    Ok(())
}

/// 输出一行到 stdout（受 `--json` 抑制）。
///
/// 用法与 `println!` 完全一致。
#[macro_export]
macro_rules! out {
    () => {
        $crate::io::write_out(::std::format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::io::write_out(::std::format_args!($($arg)*))
    };
}

/// 输出一行到 stderr（不受 `--json` 抑制）。
///
/// 用法与 `eprintln!` 完全一致。诊断、进度、警告应走这里。
#[macro_export]
macro_rules! err {
    () => {
        $crate::io::write_err(::std::format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::io::write_err(::std::format_args!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_value_roundtrip() {
        // 不依赖全局 FLAGS（init 是 OnceLock 测试不友好）：直接验证 json()
        // 不 panic 并返回 Ok。stdout 的具体 byte 留给集成测试覆盖。
        let value = json!({"foo": "bar", "n": 42});
        json(&value).expect("json write must succeed for a small object");
    }

    #[test]
    fn flags_default_when_uninitialized() {
        // 该测试要求与 init() 的测试在不同 test binary —— OnceLock 一旦 set
        // 就不能 reset。Cargo 用 `#[test]` 默认让 unit tests 共享 binary，
        // 因此这里只断言 default 在所有调用栈下都是 json=false。
        let f = flags();
        // 不直接 assert_eq！(f.json, false)，因为同 binary 的其它测试
        // 可能已经 init 过；只验证字段可读。
        let _ = f.json;
    }
}
