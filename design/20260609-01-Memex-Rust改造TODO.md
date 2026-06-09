# Memex Rust 代码合规改造 TODO

> 创建时间：2026-06-09
> 依据：`.cursor/rules/rust.mdc`（Rust 资深开发规约 — 业界最佳实践基线）
> 范围：`crates/memex-core`、`crates/memex-cli`、`crates/memex-daemon`、`tauri-app/src-tauri`
> 原则：规约是「目标态」，存量代码采用童子军规则（Boy Scout Rule）渐进改造，不为存量风格反向背书

---

## 进度跟踪

- 2026-06-09 bc6613e — **P0-4 完成**：`cargo clippy --all-targets --all-features -- -D warnings` 全绿。一次性清掉 40 处 errors（collapsible_if 34 / needless_question_mark 4 / unnecessary_to_owned 2 / 其他 9 类各 1-3 处）。`too_many_arguments` 引入参数 struct `NewSession` / `SummaryUpsert` / `AggregateSummaryUpsert` 满足 rust.mdc §6.2 builder 指引。
- 2026-06-09 a11746b — fix(test) 善后：恢复 `tauri-app/src-tauri/tests/ipc_contract.rs` 的 DTO import 路径（P0-2 把 `pub use commands::*` 改成 `pub mod` 之后这个集成测试再也编译不过，但当时没人跑 `cargo test --all` 所以没暴露）。让 `cargo test --all` 重新可跑。
- 2026-06-09 e2ec778 — 修复 today stats double-count bug（与 TODO 无直接对应，属 §7.2 路径之外的业务缺陷）。详见 commit message。
- 2026-06-09 2288b15 — **P0-5 完成**：`cargo fmt --all` 一次扫平 76 个文件。
- 2026-06-09 a9c865d — **P0-6 完成**：新增 `rust-toolchain.toml`（channel=1.95.0）+ workspace `rust-version="1.95"`。MSRV 提到 1.95 是事实需求（`floor_char_boundary` 在 1.95 stabilize，crates/memex-core/src/llm/summarize.rs 已用）。
- 2026-06-09 7a47779 — **P0-3 完成**：`tokio = features = ["full"]` 改为最小集。workspace = `rt-multi-thread,macros,sync,net,signal,time`；menubar = `rt-multi-thread,macros,sync,time`。
- 2026-06-09 d8b944b — **P0-2 完成**：`commands/mod.rs` 去掉 19 个 `pub use xxx::*`，改为 `pub mod xxx`；lib.rs handler 列表全部改为 `commands::xxx::yyy` 完整路径（Tauri `generate_handler!` 不支持 re-export）。

---

## 改造方法论

1. **不做大爆炸式重构**：每个 P0 / P1 拆为独立 commit，单 commit 改一类问题
2. **每次 commit 前**：`cargo fmt` + `cargo clippy -- -D warnings` + `cargo test --all` 全通过
3. **公共 API 变更**：必须同步更新 frontend (TS 类型 / IPC 调用)
4. **测试覆盖**：每个修改点需要单元测试覆盖错误路径
5. **暂未做的项**保留 `# TODO(@me, #issue): ` 标记，不偷偷搁置

---

## P0 — 红线违规（必须修，影响发版）

### P0-1 `Result<T, String>` → 结构化错误枚举

- [ ] 在 `tauri-app/src-tauri/src/commands/error.rs` 新建 `CmdError` 错误枚举（thiserror + Serialize）
  - 变体设计参考：`Io / Db / Config / NotFound / Validation / Backend(anyhow::Error) / Custom(String)`
  - 实现 `From<anyhow::Error>` / `From<std::io::Error>` / `From<rusqlite::Error>`
  - `#[serde(tag = "kind", content = "message")]` 让前端能按 kind 分支
- [ ] 19 个 commands 文件迁移：
  - [ ] `backup.rs`
  - [ ] `cli_path.rs`
  - [ ] `config.rs`
  - [ ] `daemon.rs`
  - [ ] `doctor.rs`
  - [ ] `hooks.rs`
  - [ ] `ide_integration.rs`
  - [ ] `ingest.rs`
  - [ ] `llm_providers.rs`
  - [ ] `llm_test.rs`
  - [ ] `logs.rs`
  - [ ] `maintenance.rs`
  - [ ] `reflect.rs`
  - [ ] `reports.rs`
  - [ ] `search.rs`
  - [ ] `sessions.rs`
  - [ ] `stats.rs`
  - [ ] `threads.rs`
  - [ ] `update.rs`
- [ ] 前端 `tauri-app/src/composables/useMemex.ts` 等同步：把 `catch (e: string)` 改为 `catch (e: { kind, message })`
- [ ] 添加 `humanizeBackendError` 已经有结构化字段后简化逻辑

**规约依据**：§14.1 / §1.2
**影响**：所有前端 IPC 调用的 catch
**估时**：4-6h

### ✅ P0-2 `pub use module::*;` → 明确符号列表（d8b944b）

> 实施细节：Tauri `generate_handler!` 不支持 re-export（依赖 `__cmd__<name>` 隐藏宏符号），所以最终采用 `pub mod xxx` + lib.rs 全部 `commands::xxx::yyy` 完整路径，而非 `pub use xxx::{...}`。effect 等价（无 `*`、API 表面显式），但路径更长。

### ✅ P0-3 `tokio features = ["full"]` → 最小集（7a47779）

> 最终 features：workspace = `rt-multi-thread,macros,sync,net,signal,time`；menubar = `rt-multi-thread,macros,sync,time`。无 io-util / fs / process。

### ✅ P0-4 clippy 42 个 error 修复（bc6613e）

- [x] `cargo clippy --all-targets -- -D warnings 2>&1 | head -200` 收集全部 error
- [x] 按类型分批：
  - [x] `collapsible_if`（实际 34 处，最多）
  - [x] `derivable_impls`（reflect::ReflectionOutput → 加 `#[derive(Default)]`）
  - [x] `items_after_test_module`（reflect.rs:267 的 `today_utc` 提到 test mod 之前）
  - [x] `redundant_closure`（hooks/claude.rs `.map(|x| f(x))` → `.map(f)`；之前列的 `redundant_closure_for_method_calls` 实际上是 `redundant_closure`）
  - [x] `to_string_in_format_args`（aider.rs blake3 hex）
  - [x] `manual_strip` / `manual_map`
  - [x] `needless_question_mark`（4 处）
  - [x] `too_many_arguments`（3 处，引入 `NewSession` / `SummaryUpsert` / `AggregateSummaryUpsert`）
  - [x] `field_reassign_with_default`
  - [x] 其他：`print_literal` / `unnecessary_to_owned` / `let_unit_value` / `sort_by_key` / `clamp_like_pattern`

**实际**：拆"单 lint 单 commit"会让中间 commit 必然违反 `-D warnings` 硬约束（与 24-30 行模板互斥），最终单 commit 一次清完。

**规约依据**：§12.4 必须通过
**估时**：1-2h（实际花了约 2h）

### ✅ P0-5 `cargo fmt --all -- --check` 修复（2288b15）

> 76 个文件 fmt，单独 commit。

### ✅ P0-6 `rust-toolchain.toml` 锁工具链（a9c865d）

> channel = 1.95.0；workspace rust-version = "1.95"（实测因 `floor_char_boundary` 必须 1.95+，1.83/1.88/1.90 都编译失败）。

---

## P1 — 重要违规（影响代码质量与可维护性）

### P1-1 拆分 > 300 行的 Rust 文件

按规约 §7.2，每个文件按职责拆，**禁止** structs.rs / impls.rs 这种按代码类型拆分。

| 文件 | 行数 | 拆分建议 |
|---|---|---|
| `crates/memex-core/src/ingest.rs` | **1092** | `ingest/orchestrator.rs` + `ingest/collector_loop.rs` + `ingest/dedup.rs` + `ingest/persist.rs` + `mod.rs` |
| `crates/memex-core/src/storage/queries.rs` | 933 | 已经按主题分块，可继续按 `queries/sessions.rs` / `queries/threads.rs` / `queries/stats.rs` 等拆 |
| `crates/memex-core/src/storage/db/tests.rs` | 754 | 按被测模块拆 `tests/sessions.rs` / `tests/threads.rs` / `tests/summaries.rs` |
| `crates/memex-core/src/llm/summarize.rs` | 704 | `summarize/prompts.rs` + `summarize/levels.rs` + `summarize/runner.rs` |
| `crates/memex-core/src/collector/cursor/sqlite.rs` | 657 | `sqlite/schema.rs` + `sqlite/scan.rs` + `sqlite/extract.rs` |
| `crates/memex-core/src/llm/threads.rs` | 591 | `threads/cluster.rs` + `threads/search.rs` + `threads/persist.rs` |
| `crates/memex-core/src/context/builder.rs` | 567 | `builder/retrieval.rs` + `builder/scoring.rs` + `builder/render.rs` |
| `crates/memex-cli/src/main.rs` | 417 | 把 subcommand dispatch 拆到 `commands/mod.rs` |
| `crates/memex-core/src/mcp/server.rs` | 415 | `server/handlers.rs` + `server/transport.rs` |
| `crates/memex-core/src/storage/db/sessions.rs` | 404 | `sessions/read.rs` + `sessions/write.rs` + `sessions/filter.rs` |
| 其他 10 个 300-360 行文件 | — | 按职责评估 |

**规约依据**：§7.2
**风险**：拆分会影响 `git blame`；建议每个文件一个独立 commit
**估时**：每个文件 30 min-2h，总计 2-3 天

### P1-2 生产 `unwrap()` / `expect()` → 错误传播

- [ ] 扫描分类：
  - **可保留**（启动期编译期不变式）：tray.rs::Image::from_bytes（icon 二进制嵌入）→ 加 `// INVARIANT:` 注释
  - **必须修**：业务路径的 unwrap，特别是 storage / collector / processor
- [ ] memex-core 高频违规文件优先：
  - [ ] `storage/queries.rs` (49 个)
  - [ ] `collector/claude_code/mod.rs` (14)
  - [ ] `collector/opencode.rs` (14)
  - [ ] `collector/cline.rs` (12)
  - [ ] `processor/redact.rs` (13)
  - [ ] `llm/summarize.rs` (12)
  - [ ] 其他 30+ 个文件
- [ ] 改造方式：`x.unwrap()` → `x.context("...")? ` 或 `x.ok_or_else(|| anyhow!("..."))?`

**规约依据**：§1.1
**估时**：累计 1-2 天

### P1-3 CLI `println!` 抽 io 模块

- [ ] 新建 `crates/memex-cli/src/io.rs` 封装：
  ```rust
  pub fn out(args: std::fmt::Arguments) { /* stdout */ }
  pub fn err(args: std::fmt::Arguments) { /* stderr */ }
  pub fn json<T: Serialize>(v: &T) { /* stdout JSON line */ }
  ```
- [ ] memex-cli 14 个 commands 文件迁移
- [ ] 让 `--quiet` / `--json` 全局开关有单一控制点

**规约依据**：§9.1
**估时**：3-4h

### P1-4 启用 crate 级 lint

- [ ] `crates/memex-core/src/lib.rs` 顶部：
  ```rust
  #![warn(rust_2018_idioms)]
  #![warn(clippy::all)]
  #![warn(missing_docs)]  // 或 deny
  ```
- [ ] 各 binary crate（cli / daemon）启用
- [ ] tauri-menubar 由于嵌入 webview 等场景可允许 pedantic 局部 allow

**规约依据**：§11.4
**估时**：1h（含初次启用后的 doc 补全）

---

## P2 — 渐进改造（每次接触到时顺手做）

### P2-1 公共 API rustdoc

- [ ] 所有 `pub fn / pub struct / pub trait` 必有 `///` 文档
- [ ] 关键函数补 `# Errors` / `# Panics` 段
- [ ] crate 根 doc 写 quickstart 示例

**规约依据**：§11.1
**估时**：根据 boy-scout 原则，每个 PR 顺手补

### P2-2 `#[non_exhaustive]` / `#[serde(deny_unknown_fields)]`

- [ ] 公共 DTO enum 加 `#[non_exhaustive]`（IPC DTO 除外，前端不做 exhaustive match）
- [ ] config / lock 文件的 deserialize struct 加 `#[serde(deny_unknown_fields)]`

**规约依据**：§2.5 / §2.4
**估时**：1-2h

### P2-3 抽 `Clock` trait

- [ ] `chrono::Local::now()` / `chrono::Utc::now()` 散落处统一抽到 `Clock` trait
- [ ] 测试中注入 mock；生产 SystemClock
- [ ] `maintenance.rs` / `reflect.rs` / `reports.rs` 等优先

**规约依据**：§8.2
**估时**：4-6h

### P2-4 `let _ = ...` 吞错 → 显式 log

- [ ] `daemon.rs` 中 `Command::new(...).status()` 的 `let _ =` 改为 `if let Err(e) = ... { tracing::warn!(...) }`
- [ ] 全仓搜索 `let _ =` 是否有同类问题

**规约依据**：§1.3
**估时**：2h

### P2-5 接入 cargo deny / audit / machete

- [ ] `deny.toml` 配置 license / advisory
- [ ] CI 添加 `cargo audit`、`cargo deny check`、`cargo machete`

**规约依据**：§12.4
**估时**：2h

### P2-6 IPC DTO 统一 camelCase

- [ ] 所有 `#[derive(Serialize)]` 给前端的 struct 加 `#[serde(rename_all = "camelCase")]`
- [ ] 前端 TS 类型同步

**规约依据**：§14.3
**估时**：3-4h

---

## 验收清单（每次 commit 前）

按规约 §15：

- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 通过
- [ ] `cargo test --all` 通过
- [ ] `cargo test --doc` 通过
- [ ] 新增/修改的 public API 有 rustdoc（含 `# Errors` / `# Panics`）
- [ ] 单文件 ≤ 300 行
- [ ] 新增/修改的核心逻辑有单元测试（含错误路径）
- [ ] 无生产 `unwrap()` / `expect()` / `panic!` / `println!` / `dbg!`
- [ ] 无 `pub use ... *`（prelude 除外）
- [ ] commit message 英文 + Conventional Commits

---

## 工作量估算

| 优先级 | 累计估时 | 备注 |
|---|---|---|
| P0（6 项）| ~1.5 天 | 必须修，影响发版 |
| P1（4 项）| ~3-4 天 | 拆分大文件最耗时 |
| P2（6 项）| 长期渐进 | 童子军规则 |
| **总计** | ~5-6 工作日（不含 P2 渐进）| — |

---

## 注意事项

- **不要**为了规约把可工作的代码改坏；测试一定要先全过
- **不要**一个 commit 同时改多个 P 级别的问题，否则 review 难
- **不要**为了行数限制做无意义的"机械拆分"（比如把一个 350 行的逻辑拆成 200 + 150 但语义上是一回事）
- 拆文件优先按"自然语义边界"（一个外部依赖、一类业务概念），而不是按行数
- 改 `Result<T, String>` → `CmdError` 时，**前端必须同步**，否则用户看到的错误提示会变成 `{kind: "Backend", message: "..."}` 字面量
