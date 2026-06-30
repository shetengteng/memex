# memex-core 错误类型迁移：anyhow → thiserror

- 日期：2026-06-30
- 状态：草案，待评审
- 作者：terrell.she
- 关联：`CLAUDE.md` Rust 红线 2（错误类型两层模型）

## 1. 背景

`CLAUDE.md` 与 `.cursor/rules/rust.mdc` 规定错误类型的两层模型：

| 场景 | 选择 | 理由 |
|---|---|---|
| **库 crate**（被复用） | `thiserror` 具名错误枚举 | 调用方需要按 variant `match` 处理 |
| 应用 / 二进制 / 集成层 | `anyhow::Result` + `.context(...)` | 关心错误链而非分支 |

仓库现状：

- **`memex-core` 是库 crate**，被三个下游 binary（`memex-cli` / `memex-daemon` / `memex-menubar`）和未来潜在的桌面 / web 端引用。
- 但 `memex-core` 内 **62 个 `.rs` 文件用 `anyhow`，0 个用 `thiserror`** —— 全部偏离规约。
- `pub fn` 返回 `Result` 的有 **98 处**。下游目前都把这些当 `anyhow::Error` 处理，无法 `match` 区分错误来源。

## 2. 目标 & 非目标

### 目标

1. `memex-core` 公开 API 返回的 `Result<T, E>` 中，`E` 是具名枚举（按子模块分），不再是 `anyhow::Error`。
2. 下游 binary（`memex-cli` 等）保留 `anyhow`，通过 `#[from]` 自动把 core 错误链入 `anyhow::Error`，**不需要大范围改动**。
3. 改造期间允许 core 内部模块继续用 `anyhow`，只在跨模块 / 跨 crate 边界露出 thiserror 枚举。
4. 渐进迁移，**禁止一次 PR 全改完** —— 分模块小步切换。

### 非目标

- 不引入 `snafu` / `eyre` / 自己造轮子。
- 不重写错误消息文案（保留原 `.context(...)` 的描述）。
- 不改下游 binary 的内部错误处理风格。
- 不追求 100% `match` 安全 —— 极少数 catch-all 场景（如外部 IO）可以用 `#[from] std::io::Error` 收尾。

## 3. 错误枚举设计

按 `memex-core` 现有模块边界划分（与 `lib.rs` 的 mod 一致）：

```rust
// crates/memex-core/src/error.rs
use thiserror::Error;

/// memex-core 顶层错误。下游 binary 用 `anyhow::Error` 接收时，
/// `#[from]` 自动保留 source chain。
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("collector error: {0}")]
    Collector(#[from] CollectorError),

    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("ingest error: {0}")]
    Ingest(#[from] IngestError),

    #[error("retrieval error: {0}")]
    Retriever(#[from] RetrieverError),

    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    #[error("llm error: {0}")]
    Llm(#[from] LlmError),

    #[error("context build error: {0}")]
    Context(#[from] ContextError),
}

pub type Result<T> = std::result::Result<T, CoreError>;
```

子枚举按模块定义，每个子模块拥有自己的错误枚举：

```rust
// crates/memex-core/src/collector/error.rs
#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("source path not found: {0}")]
    SourceMissing(PathBuf),
    #[error("malformed session jsonl at {path}:{line}")]
    Malformed { path: PathBuf, line: usize },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
```

```rust
// crates/memex-core/src/storage/error.rs
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("schema migration failed: {0}")]
    Migration(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

`RetrieverError` / `IngestError` / `ConfigError` / `LlmError` / `ContextError` 同理。具体 variant 在迁移每个模块时补全。

### 设计原则

1. **每个 variant 是一个"故障模式"，不是一个 fmt 字符串**。`#[error("foo: {0}")]` 后面接的字段是结构化的，调用方可以 `if let CollectorError::Malformed { path, .. } = e`。
2. **外部错误用 `#[from] + transparent`**：`std::io::Error` / `rusqlite::Error` / `serde_json::Error` 这种没办法预设的错误，用 `#[from]` 自动转换，`#[error(transparent)]` 透传 source。
3. **避免"上帝枚举"**：每个 variant 不超过 ~10 个；过大就拆子模块。
4. **错误消息不重复 variant 名**：`#[error("...")]` 写"为什么 / 上下文"，variant 名写"是什么"。

## 4. 迁移分期

**禁止单 PR 全改**。按模块顺序，每期一个 PR，独立可 review：

| 期 | 范围 | 文件数 | 风险 | 备注 |
|---|---|---|---|---|
| **P2.1** | 新建 `error.rs`，定义 `CoreError` 与各子枚举骨架，先全部留空 variant | 1~8 | 极低 | 不改任何现有代码，下游可继续用 anyhow |
| **P2.2** | `config` 模块迁移 | ~3 | 低 | 错误面窄、依赖少，先吃下来建立模式 |
| **P2.3** | `storage` 模块迁移 | ~12 | 中 | 涉及 db schema / fts5，错误最多元 |
| **P2.4** | `collector` 模块迁移 | ~18 | 中 | 每个 adapter 子模块独立切换 |
| **P2.5** | `ingest` 模块迁移 | ~8 | 中 | 依赖 collector + storage |
| **P2.6** | `retriever` / `context` 模块迁移 | ~10 | 低 | 已经在 raw.rs 里熟悉模式 |
| **P2.7** | `llm` 模块迁移 | ~7 | 低 | 错误面已被 HTTP 客户端封过 |
| **P2.8** | 清理：去掉 core 内残留 `anyhow` 依赖；`Cargo.toml` 移除 `anyhow` | 全局 | 低 | 验收期 |

每期 PR 必须：

- 自包含可编译通过（即下游 binary 不需要同步改）
- 通过 `cargo test -p memex-core`
- 在 commit message 附"为什么这一期边界这么划"

## 5. 迁移单文件的标准动作

以 `config/io.rs` 为例：

**改前**

```rust
use anyhow::{Context, Result};

pub fn load(path: &Path) -> Result<Config> {
    let bytes = fs::read(path)
        .with_context(|| format!("read config at {}", path.display()))?;
    toml::from_slice(&bytes).context("parse config")
}
```

**改后**

```rust
use crate::error::{ConfigError, Result};

pub fn load(path: &Path) -> Result<Config> {
    let bytes = fs::read(path).map_err(|e| ConfigError::Read {
        path: path.to_path_buf(),
        source: e,
    })?;
    toml::from_slice(&bytes).map_err(ConfigError::Parse)
}
```

**子枚举增量补 variant**

```rust
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config at {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config TOML")]
    Parse(#[from] toml::de::Error),
}
```

**下游 binary 不需要改**：

```rust
// memex-cli 内继续用 anyhow
let cfg = memex_core::config::load(&path)?;  // ? 自动 #[from] 把 CoreError 链入 anyhow::Error
```

## 6. 兼容性

`From<CoreError> for anyhow::Error` 由 anyhow 自动提供（任何 `std::error::Error + Send + Sync + 'static` 都满足）。因此下游 `anyhow::Result<()>` + `?` 完全无缝。

公开 API 签名变化（`anyhow::Result<T>` → `core::Result<T>`），属于 **SemVer minor break**。memex-core 暂未发布到 crates.io，无 ABI 约束；如未来发版必须在 changelog 显眼标注。

## 7. 测试策略

- 每个 variant 至少一个单测覆盖 happy + sad path
- 增加一类 **错误链测试**：制造一个深层错误，断言 `e.source().source()` 能拿到原始 `std::io::Error`
- `cargo test -p memex-core` 全绿是合并门槛
- 不引入新的端到端测试 —— 错误类型重构是内部重构

## 8. 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| 子枚举设计跑偏，出现 "AnyhowLite" 反模式（每个 variant 都是 String） | 调用方还是无法 match | review 必须卡 variant 结构化字段 |
| 中途暂停留下半 anyhow 半 thiserror | 心智负担 | 每期独立可发布，可中止；CLAUDE.md 已说明渐进 |
| 下游 binary 误用 `CoreError` 而非 anyhow | 风格漂移 | binary crate 不应直接 `use memex_core::error::CoreError`，只通过 `?` 隐式转换 |
| `#[from]` 自动转换在多个 variant 之间冲突（如多个用 `io::Error`） | 编译报错 | 大多数模块只有一个 `io::Error` 通道；冲突时改用显式 `map_err` |

## 9. 验收清单

- [ ] `crates/memex-core/Cargo.toml` 不再依赖 `anyhow`
- [ ] `grep -r "use anyhow" crates/memex-core/src` 返回空
- [ ] 所有 `pub fn` 返回 `core::Result<T>` 或 `Result<T, SubError>`，无 `anyhow::Result`
- [ ] 下游三个 binary (`memex-cli` / `memex-daemon` / `memex-menubar`) 无修改即可编译
- [ ] `cargo test --workspace` 全绿
- [ ] 至少一个错误链测试断言 source chain 完整

## 10. 后续

- P3：把 `memex-cli` / `memex-daemon` 顶层 main 的错误打印改用 `tracing::error!(error = ?e)` 以自动展开 source chain（目前可能是 `{}` 只打第一层）
- P4：考虑在 MCP / HTTP 边界把 `CoreError` 序列化为结构化错误码暴露给 IDE，方便客户端做差异化 UI（"重试" vs "请联系作者"）
