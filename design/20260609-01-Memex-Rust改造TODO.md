# Memex Rust 代码合规改造 TODO

> 创建时间：2026-06-09
> 依据：`.cursor/rules/rust.mdc`（Rust 资深开发规约 — 业界最佳实践基线）
> 范围：`crates/memex-core`、`crates/memex-cli`、`crates/memex-daemon`、`tauri-app/src-tauri`
> 原则：规约是「目标态」，存量代码采用童子军规则（Boy Scout Rule）渐进改造，不为存量风格反向背书

---

## 进度跟踪

- 2026-06-09 f9a1d7f — **P1-2 follow-up 完成**：privacy.rs 3 处 `PRIVACY_CONFIG.lock().unwrap()` 收尾。生产换 `parking_lot::Mutex` 消除 poison 风险；两个共写测试用 `static TEST_LOCK: Mutex<()>` 串行化（不引入 serial_test crate）。production unwrap/expect 扫描结果：0/0。
- 2026-06-09 32bb2ad — **P1-4 完成**：5 个 crate root 加上 `#![warn(rust_2018_idioms)]` + `#![warn(clippy::all)]` 与 crate-level doc。`missing_docs` 推迟到 P2 boy-scout（避免一次阻塞 40+ 文件）。`cargo clippy --workspace --all-targets -- -D warnings` 一次过。
- 2026-06-09 1cf99f4 — **P1-3 完成**：新增 `crates/memex-cli/src/io.rs` (142 行) 作为 CLI 输出单一控制点（`init`/`out!`/`err!`/`json`），134 处 `println!`/`eprintln!` 跨 15 文件全部迁移，14 处旧 `crate::out!("{}", serde_json::*())` 直接走 `io::json()` —— 顺手修了 `--json` 模式静默 bug。烟测 `memex --json stats` 输出合法可解析 JSON，`memex stats` 保持人类格式。287/287 tests pass。
- 2026-06-09 ec5bd34 — **P1-2 主体完成（13/16）**：精确扫描后真实 production unwrap/expect = 16 处（旧 TODO 估的 200+ 多在 inline `#[cfg(test)] mod tests` 或 `_tests.rs` 文件级测试里）。3 个 commit 处理 13 处：
  * adf659c：redact.rs 7 regex INVARIANT + 1 std Mutex Ok-else-return
  * 9bac3b3：metadata.rs 3 regex INVARIANT + hooks 2 home_dir INVARIANT
  * ec5bd34：sweep 8 home_dir + tray icon + tauri build + SIGTERM signal handler
  剩余 3 处 in privacy.rs（PRIVACY_CONFIG.lock）需 parking_lot 切换 + 测试串行化，作 follow-up。`cargo test --workspace` 全绿（284 tests）。
- 2026-06-09 fa67b9f — **P1-1 全部完成**：第二轮 13 个 303-375 行文件全部拆掉（config/mod.rs、cursor/tests.rs、setup.rs、claude_code/mod.rs、aider.rs、summaries.rs、opencode.rs、threads.rs、providers.rs、matcher.rs、rebuild.rs、retriever/mod.rs、cline.rs），全部以「按职责子模块 + 抽 tests」收口。workspace 现已无 > 300 行 Rust 源文件（最大 summaries/mod.rs 294 行）。`cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` 全绿（284 tests）。
- 2026-06-09 44b3785 — **P0-1 完成 + cursor pre-existing bug**：19 个 commands 文件全部迁到 `CmdError`（`Result<T, String>` 在 commands/ 下零残留）。前端 `humanizeBackendError` 同步加上 `{kind, message}` 解析，向下兼容旧 string 错误，新增 10 条单测。整体 19 commit（1 基础 + 18 文件 + 1 cursor fix）。顺便修了 main 上 silently-failing 的 `test_sqlite_scan_multifolder_workspace_yields_no_project_path`（multi-folder workspace 不再误用 `.code-workspace` 父目录当 cwd，与 sqlite.rs 自带注释契约一致）。
- 2026-06-09 0865a3e — **library Today bug 修复**：`fTime` 过滤未生效 + "Today" 分组以最新 session 日期当锚点 → 抽出 `sessionFilters.ts` 纯函数 composable（filter / group），用绝对 `new Date()` 锚定，库视图从 503 行减到 470 行；新增 16 条单测覆盖时间边界、分组、组合过滤。`LibraryFacets.vue` 强类型化 `TimeFilter` / `SummaryFilter`，emit 前加 runtime guard 防非法值。
- 2026-06-09 — **rust.mdc §2.6 新增**：增加「卫语句优先 / Happy Path 居中」章节 + §15 提交前清单加项「函数体嵌套 ≤ 3 层」。
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

### ✅ P0-1 `Result<T, String>` → `CmdError` 结构化错误枚举（44b3785）

- [x] `tauri-app/src-tauri/src/commands/error.rs` 新建 `CmdError`（thiserror + Serialize）
  - 实际变体：`Io / Db / Config / NotFound / Validation / Backend`
  - 实现 `From<std::io::Error>` / `From<anyhow::Error>` / `From<String>` / `From<&'static str>`
  - 注：未实现 `From<rusqlite::Error>` —— commands/ 下没有直接拿 rusqlite 的代码，db 错误经 anyhow 进入 `Backend`，避免新增 rusqlite 直接依赖
  - `#[serde(tag = "kind", content = "message", rename_all = "snake_case")]` 序列化为 `{kind, message}`
- [x] 19 个 commands 文件全部迁移（每文件一个 commit）：
  - [x] `search.rs`（与 error.rs 同 commit dfc62e0 作为示范）
  - [x] `ingest.rs` (a3e62c5)
  - [x] `reports.rs` (12d0b2b)
  - [x] `hooks.rs` (c3d156c)
  - [x] `update.rs` (8fc0422)
  - [x] `threads.rs` (10793e9)
  - [x] `config.rs` (ac5946c)
  - [x] `doctor.rs` (0ead412)
  - [x] `maintenance.rs` (387448a)
  - [x] `llm_test.rs` (aa839b2)
  - [x] `stats.rs` (3b60462)
  - [x] `reflect.rs` (55dca37)
  - [x] `backup.rs` (73e7791)
  - [x] `logs.rs` (a4a60a1)
  - [x] `cli_path.rs` (3bf4115)
  - [x] `ide_integration.rs` (c68a73b)
  - [x] `sessions.rs` (883e541)
  - [x] `llm_providers.rs` (7d3d9e3)
  - [x] `daemon.rs` (b78abb7)
- [x] 前端 `tauri-app/src/lib/utils.ts` `humanizeBackendError` 同步：新增 `parseBackendError` 同时识别 `{kind, message}` 与旧 string 输入；按 `kind` 分支文案 (`not_found` / `validation` / `backend` / `io` / `db` / `config`)；保留旧 regex 文案为兜底。10 条新单测覆盖结构化解析、kind 分支、不识别 kind 回退。
- [x] `cargo clippy --all-targets --all-features -- -D warnings` 全绿；`cargo test --all` 全绿；`cd tauri-app && npx vitest run` 全绿（140 tests）。

**实际**：未走 useMemex.ts 改 `catch` 类型——前端 `humanizeBackendError` 内部已 narrow，调用方继续按字符串语义用即可。把 IPC catch 全改成 `{kind, message}` 收益小（要改 ~40 处 invoke 调用），后续 P1 阶段再统一。
**实际工时**：~5h（含 cursor multi-folder pre-existing bug 顺手修）。

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

### ✅ P1-1 拆分 > 300 行的 Rust 文件（2026-06-09 全部完成）

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

**已完成**（截至 2026-06-09）：
- `db/sessions.rs` → `sessions/{mod,read,write}.rs`（commit a88a415）
- `mcp/server.rs` → `mcp/server/{mod,transport,dispatch,tools}.rs`（commit 3698ad8）
- `memex-cli/src/main.rs` → `cli/` + `dispatch/`（commit d577939）
- `context/builder.rs` → `builder/{mod,collect,render}.rs`（commit c052a49）
- `context/builder/render.rs` 382 行 → `render/{mod,tests}.rs` + 抽出 `builder/text.rs`（commit 待提交）
- `ingest.rs` 1148 行 → `ingest/{mod,adapter,summarize_levels,project_summaries,reports,period,threads}.rs`（mod ≤ 288 行；待提交）
- `storage/queries.rs` 1144 行 → `queries/{mod,dto,doctor,stats,workload,tests/{mod,general,workload_basic,workload_daily,workload_heatmap}}.rs`（最大 workload.rs 291 行；测试 13 PASS；待提交）
- `llm/summarize.rs` 720 行 → `summarize/{mod,prompts,parse,period,tests}.rs`（最大 tests 245 行；待提交）
- `llm/threads.rs` 608 行 → `threads/{mod,prompts,cluster,query,fallback,tests}.rs`（最大 tests 193 行；待提交）
- `collector/cursor/sqlite.rs` 670 行 → `sqlite/{mod,types,scan,collect,probe,project_path}.rs`（最大 scan 167 行；待提交）
- `storage/db/tests.rs` 832 行 → `tests/{mod,basic,summaries,sessions,schema,threads}.rs`（最大 sessions 249 行；29 PASS；待提交）

**2026-06-09 第二轮 — 13 个 303-375 行剩余文件全部拆完**：

| 文件 | 旧行数 | 拆分结果 | commit |
|---|---|---|---|
| `config/mod.rs` | 375 | `config/{mod, types, io, tests}.rs`（最大 io 151） | (cf README) |
| `collector/cursor/tests.rs` | 375 | `tests/{mod, jsonl, sqlite_backend}.rs`（最大 sqlite_backend 279） | — |
| `collector/aider.rs` | 355 | `aider/{mod, parser, tests}.rs`（mod 157） | — |
| `memex-cli/commands/setup.rs` | 351 | `setup/{mod, io, json_backends, codex_backend}.rs`（mod 181） | — |
| `collector/claude_code/mod.rs` | 350 | `claude_code/{mod, tests}.rs`（mod 225） | — |
| `storage/db/summaries.rs` | 345 | `summaries/{mod, dto}.rs`（mod 294） | — |
| `collector/opencode.rs` | 341 | `opencode/{mod, tests}.rs`（mod 228） | — |
| `storage/db/threads.rs` | 327 | `threads/{mod, dto}.rs`（mod 286） | — |
| `context/matcher.rs` | 311 | `matcher/{mod, tests}.rs`（mod 145） | — |
| `storage/db/providers.rs` | 309 | `providers/{mod, tests}.rs`（mod 173） | — |
| `storage/rebuild.rs` | 308 | `rebuild/{mod, tests}.rs`（mod 241） | — |
| `retriever/mod.rs` | 305 | `retriever/{mod, tests}.rs`（mod 206） | 8e506b2 |
| `collector/cline.rs` | 303 | `cline/{mod, tests}.rs`（mod 255） | a838a43 |

**第二轮所有 commit 都附 `cargo check --tests` 验证**。最终一次 `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` 全绿（284 tests pass）→ commit fa67b9f。

**全 workspace 现已无 > 300 行的 Rust 源文件**（最大为 `summaries/mod.rs` 294 行，`storage/queries/workload.rs` 291 行）。

**规约依据**：§7.2
**实际成本**：第二轮 ~3h（13 个 file × 平均 13 min 拆分 + verify + commit）
**总计 P1-1 commit 数**：~25 个（含第一轮 5 个 + 第二轮 13 个 + 各类 README/TODO/fmt 维护 commit）

### P1-6 把 `mcp` 模块独立为 `memex-mcp` crate

> **背景**：用户提出「看下 memex mcp 是否可以单独的一个模块从 memex-core 中独立出来」。本节给出评估结论与执行清单。

#### 评估

| 维度 | 现状 | 独立后 |
|---|---|---|
| 依赖方向 | `mcp → core`（Db / Retriever / storage::models），core 无反向引用 | 单向稳定 |
| Caller | 仅 `memex-cli/src/commands/mcp.rs` 一处 14 行 | 改 `use memex_mcp::server` |
| 行数 | 4 个文件 + 1 个 tests + protocol = 781 行 | 已经全部 < 300 行（最大 tools.rs 283） |
| 测试隔离 | tests.rs 用 in-memory Db 做集成测试 | 同样可用；core 把 `Db / Chunk / Retriever` 等导出即可 |
| 编译并行 | mcp 改动重编 memex-core 测试 | mcp 改动只影响 memex-mcp + cli |

**结论**：值得做，风险低。

#### 实施清单

- [ ] 新建 `crates/memex-mcp/{Cargo.toml, src/lib.rs}`，依赖 `memex-core` + `serde_json` + `anyhow`
- [ ] `git mv crates/memex-core/src/mcp/* crates/memex-mcp/src/`（保留 commit 历史）
- [ ] 把内部 `use crate::storage::db::Db` 等改成 `use memex_core::storage::db::Db`
- [ ] 把 memex-core 内 `storage::models::{Chunk, ChunkMetadata, ChunkType}` / `retriever::{Retriever, SearchFilter}` 确认为 `pub` 导出
- [ ] 删除 `memex-core/src/mcp/` 与 `lib.rs` 里的 `pub mod mcp;`
- [ ] `memex-cli` Cargo.toml 加 `memex-mcp = { path = "../memex-mcp" }`；`commands/mcp.rs` 改 import
- [ ] root `Cargo.toml` `[workspace.members]` 加 `crates/memex-mcp`
- [ ] cargo check / clippy / test / fmt / `cargo test --doc` 全过
- [ ] commit `refactor(mcp): extract memex-core::mcp into memex-mcp crate`

**估时**：1.5-2h（机械化重构 + 全量 quality gate）
**风险**：低；唯一注意是确保 core 把 mcp 需要的 internal API 提升为 pub（无需破坏现有 ergonomics，只是让模块边界更显式）

### P1-7 数据库升级策略（封板后启用）

> **当前阶段**：开发未封板，schema 变更走「DROP + 重建」单条 baseline（见 P1-5 step 4）。一旦正式发布给外部用户，必须切到 backup-and-migrate 模式。

#### 触发条件

- 任一二进制（menubar / daemon / cli）公开发布到 GitHub Release
- `Cargo.toml` workspace.version 升到 `>= 1.0.0`（或团队约定的「封板」节点）

#### 升级流程（每次 schema 变更）

1. `Db::open` 前先把当前 `memex.db` 备份为 `memex.db.bak-{YYYYMMDD-HHMMSS}-v{user_version}`
2. 跑增量 migration：`M::up("ALTER TABLE ... / CREATE INDEX IF NOT EXISTS ...")`，用 `rusqlite_migration` 接力 v2..=vN
3. 失败回滚：如 to_latest 返回 Err，删除当前 db，从最新备份 copy 回来
4. 成功后保留最近 N 份备份（建议 N=3），更老的自动清理

#### 实施清单

- [ ] `db/migrations.rs` 增加 `pub fn migrations() -> Migrations<'static>`：v1 baseline (复用 SCHEMA_SQL) + v2..=vN 增量
- [ ] `Db::open` 内加 backup-then-migrate 流程
- [ ] backup 文件命名规则 + 旋转策略 unit test
- [ ] doctor / Settings UI 显示「最近一次成功 backup」位置，方便用户手动 rollback
- [ ] 文档：`docs/db-upgrade.md`（升级路径、回滚指引、数据保留策略）

**估时**：4-6h
**风险**：中（用户数据安全相关，需要充分手测）
**规约依据**：§14.2（向后兼容）、§16（运维 / 数据保护）

### ✅ P1-2 生产 `unwrap()` / `expect()` → INVARIANT 标注（2026-06-09 完成 16/16）

> 旧 TODO 给的扫描数（200+）多数在 `#[cfg(test)] mod tests` / `mod test_support`
> / `_tests.rs` 文件里。精确扫描脚本（剥离 cfg(test) 整块 + 文件名后缀过滤）后，
> **真实 production unwrap/expect = 16 处 / 14 文件**。

#### 已完成（commit adf659c / 9bac3b3 / ec5bd34）

| 类别 | 处 | 处理方式 |
|---|---|---|
| `Regex::new(literal)` 在 `LazyLock` 里（redact 7 + metadata 3） | 10 | `.expect("INVARIANT: ... regex must compile")` + 块级注释 |
| `std::sync::Mutex::lock().unwrap()`（redact CUSTOM_RULES write） | 1 | `let Ok else { return }` 兜底 poison |
| `dirs::home_dir().expect(...)`（10 个 adapter / cli / daemon 入口） | 10 | 统一 `expect("INVARIANT: home directory must be resolvable")` |
| `tray.rs Image::from_bytes(include_bytes!())` | 1 | INVARIANT —— 编译期嵌入字节，失败 = 程序员错误 |
| `tauri Builder::build().expect(...)` | 1 | INVARIANT —— app 不可启动 |
| `signal::unix::signal(SIGTERM).expect(...)` | 1 | INVARIANT —— Unix 启动期系统调用 |

**核心原则**：rust.mdc §1.1 接受 expect + INVARIANT 注释的启动期不变式。所有保留的 expect 都满足：
1. 失败 = 不可恢复（HOME 没了 / signal 子系统坏了 / 编译期资源损坏）
2. message 字符串以 `INVARIANT:` 开头便于 grep
3. 关键处块级 `// INVARIANT:` 注释解释为何不会失败

#### Follow-up 完成（commit f9a1d7f）

`processor/privacy.rs` 3 处 `PRIVACY_CONFIG.lock().unwrap()`：
1. 生产：`std::sync::Mutex` → `parking_lot::Mutex`（不会 poison，3 处 `.lock()` 直接拿 guard）
2. 测试：两个共写 PRIVACY_CONFIG 的 test 通过 `static TEST_LOCK: std::sync::Mutex<()>` 显式串行（不引入 `serial_test` 依赖）

3 次重跑 privacy 测试全 PASS（之前 flaky）。production unwrap/expect 扫描结果：**0 处**。

**规约依据**：§1.1
**实际耗时**：~2h（含 follow-up；远低于估时 1-2 天 —— 旧 TODO 估的 200+ 处多在 inline test blocks 里）

### ✅ P1-3 CLI `println!` 抽 io 模块（2026-06-09 完成，~1h）

#### 实施（commit d8ce656 + 1cf99f4）

- `crates/memex-cli/src/io.rs` 142 行，提供 4 个公开 API：
  - `init(json: bool)` —— `dispatch::run` 入口调用一次，把顶层 `--json` flag 注入 `OnceLock<IoFlags>`
  - `out!(...)` macro —— `println!` 形态，受 `--json` 抑制（避免污染 JSON 流）
  - `err!(...)` macro —— `eprintln!` 形态，不受 `--json` 影响（诊断/进度/警告永远输出到 stderr）
  - `json(value: &T)` —— `Serialize` 写到 stdout，`--json` 模式紧凑、人类模式 pretty
- 134 处 `println!`/`eprintln!` 跨 15 文件全部迁移
- 14 处 `crate::out!("{}", serde_json::*(&x))` → `crate::io::json(&x)` 修正了 `--json` 模式静默 bug（如果没改这部分，迁移后 `--json` 反而不出 JSON）

#### 未实施

- 不引入 `--quiet` flag：TODO 提了但用户没明确要求；shell 重定向已能达成 quiet 效果，引入 flag 是 API 表面扩张。
- 14 个 `commands::*::run()` 函数签名里的 `json: bool` 参数没移除：移除会牵涉所有 caller。`io::init()` 已经吃掉 flag，但 commands 内部还是判断局部 `json` 参数。下个 boy-scout commit 可以清理。

#### 验证

- `cargo clippy -p memex-cli --tests --all-targets -- -D warnings` ✓
- `cargo test --workspace`：287 tests 全过
- 烟测：`memex stats` 输出人类格式；`memex --json stats` 输出合法 JSON（python `json.tool` round-trip pass）

**规约依据**：§9.1
**实际工时**：~1h（远低于估时 3-4h —— 大头是 sed 机械替换）

### ✅ P1-4 启用 crate 级 lint（2026-06-09 32bb2ad，~30 min）

- [x] `crates/memex-core/src/lib.rs`：`#![warn(rust_2018_idioms)]` + `#![warn(clippy::all)]` + crate-level doc
- [x] `crates/memex-cli/src/main.rs`：同上
- [x] `crates/memex-daemon/{lib.rs, main.rs}`：同上
- [x] `tauri-app/src-tauri/src/lib.rs`：只 `clippy::all`（webview / Tauri macro 偶尔触发 idiom false-positive），main.rs 几乎为空保持现状
- [ ] **deferred `missing_docs`**：40+ 模块尚无 doc，硬启会阻塞所有 commit，违反「最小化变更」。boy-scout 渐进补齐，记为 P2 任务

#### 验证

- `cargo clippy --workspace --all-targets -- -D warnings`：全绿（一次就过，说明 P0-4 阶段已经做掉大部分 lint 债）
- `cargo test --workspace`：287/287

**规约依据**：§11.4
**实际工时**：~30 min

### P1-5 数据库框架评估：SQLx vs 现状

> **诉求**：用户提议把 DB 框架从 `rusqlite` 切到 `SQLx`。本节先做完整 trade-off 分析，待决策后再细化执行清单。
>
> **决策（2026-06-09）**：用户选定 **候选 E（rusqlite + 周边工具补齐）**；并明确「不考虑老库迁移，当前数据可丢弃」。
>
> **进度**：4 个 step 全部完成。
>
> - [x] **Step 1**：`std::sync::Mutex` → `parking_lot::Mutex`，干掉 80+ 处生产代码 `conn.lock().unwrap()` + 16 处测试 unwrap（commit `f72644b`）
> - [x] **Step 2**：热路径 `conn.prepare(...)` → `conn.prepare_cached(...)`，共 29 个调用点（commit `ee8f4ae`）；最大收益点是 `queries.rs::list_project_summaries` 的循环内 prepare 与 dashboard breakdown / timeline
> - [x] **Step 3**：引入 `serde_rusqlite = "0.36.0"`（与 rusqlite 0.32 共用 libsqlite3-sys），把 `db/sessions/read.rs::list_sessions_paged` / `list_sessions_by_project` 的手写 `row.get(0..=9)?` 改成 `from_rows::<SessionRow>(rows).collect()`（commit `04b8038`）；其余 row mapping 维持原样（`list_sessions_in_range` 因 SQL 缺列保留手写，作 follow-up 时再调整）
> - [x] **Step 4**：引入 `rusqlite_migration = "1.3"`，把 9 段 inline `if from < N` migration 合并为单条 baseline；`init_schema` 从 197 行 → 6 行；删除 `schema_version` 表 + `SCHEMA_VERSION` 常量；`Db::schema_version()` 改读 `PRAGMA user_version` 保持 DoctorReport IPC 形态不变（commit `45680d4`）。⚠️ 这一步带来一次性的**数据 reset**：v1 baseline 会 DROP 旧 memex.db 内全部业务表 + 表 `schema_version`，老库被打开时数据全部丢失，下一轮 ingest 从 adapter 源文件重建。
>

#### 现状盘点

- DB：SQLite（`~/.memex/memex.db`），WAL 模式，单文件 + 文件锁，无服务端
- 驱动：`rusqlite 0.32 features = ["bundled", "modern_sqlite"]`
- 调用面：`crates/memex-core/src` 共 14 个文件，约 130 处 `conn.execute / query_row / prepare / execute_batch / transaction / query_map`
- 主战场：`storage/db/*.rs`（8 文件）+ `storage/queries.rs`（1138 行，已超规约 §7.2 上限，本身是 P1-1 待拆）+ `storage/metrics.rs`
- 旁路战场：`collector/cursor/sqlite.rs`、`collector/opencode.rs` 直接读 **外部应用** 的 SQLite（Cursor / OpenCode 各自的 db），**这部分不是 memex.db、不属于本节范围**
- 同步语义：整层 `Db { conn: Mutex<Connection> }` 顺序执行，所有 `#[tauri::command]` 已经是 `tokio::async_runtime::spawn_blocking`/`tokio::task::spawn_blocking` 包裹后调用
- Schema：DDL 集中在 `storage/db/schema.rs::SCHEMA_SQL`，10 个 migrations 写在 `db/mod.rs::run_migrations` 里，靠 `schema_version` 表手动推进版本

#### 候选 A：保持 `rusqlite`，补强短板（推荐）

| 改造项 | 估时 |
|---|---|
| 引入 `rusqlite_migration` 把 10 个 inline migration 提到 `migrations/V01__*.sql` 文件 | 2h |
| 给热路径 query 加 `Connection::prepare_cached`（已有部分用了 `prepare`，没缓存） | 1h |
| 抽 `Db::with_read<F>()` / `Db::with_write<F>()` helper，把 130 处 `conn.lock().unwrap()` 收敛 | 2h |
| `DbError` 包装成 `thiserror`，对接 `CmdError::Db` 路径 | 1h |
| 可选：用 `rusqlite::types::FromSql + ToSql` 派生（手写已不少），用 `serde_rusqlite` 统一 | 2h |

**总估时**：~6-8h，**风险低**，符合 [Rust ORMs in 2026](https://aarambhdevhub.medium.com/rust-orms-in-2026...) 业界对桌面 + SQLite 场景的共识："If it's SQLite, use Rusqlite"。

#### 候选 B：切换 `SQLx 0.8` + SQLite driver

| 改造项 | 估时 |
|---|---|
| `Cargo.toml`：rusqlite → sqlx 0.8 features=["sqlite", "macros", "migrate", "runtime-tokio-rustls"] | 30min |
| `Db` 重写：`SqlitePool`（max_connections=1 单 writer，+ 可选 reader pool） | 1h |
| schema migrations 拆到 `migrations/<timestamp>__*.sql`，用 `sqlx::migrate!()` | 2h |
| 8 个 `db/*.rs` 文件全部改 `sqlx::query!/query_as!` | 6-8h |
| `queries.rs` 1138 行（同 P1-1 拆分一起做） | 8-10h |
| `metrics.rs` 改 SQLx | 30min |
| **关键决策：异步穿透 vs `block_on` facade**（详见下方分析） | **见下** |
| 编译期 query check：装 `sqlx-cli`，配 `DATABASE_URL`，`cargo sqlx prepare` 生成 `.sqlx/` 入仓 | 1h |
| `db/tests.rs` 全部改 `#[sqlx::test]` 或 `SqlitePool::connect(":memory:")` | 2h |
| `commands/error.rs` 把 `From<rusqlite::Error>` 替换为 `From<sqlx::Error>`（如果引入 Db variant） | 30min |

**总估时**：~22-30h，**风险中-高**。

#### 候选 B 的 async/sync 决策（关键）

SQLx 是 **async-only**（即便 SQLite driver 也走后台 worker thread + channels；[docs.rs/sqlx SqliteConnection](https://docs.rs/sqlx/latest/sqlx/struct.SqliteConnection.html)）。Memex 当前 storage 层 100% 同步，要在 SQLx 上跑出来必须二选一：

- **B1. 异步穿透**：`Db` 全部方法改 `async fn`，`memex-core` 几乎所有调用点（ingest / collector / retriever / context / llm/summarize / llm/threads / mcp/server / memex-cli / memex-daemon）的方法签名都要染上 `async`。**额外估时 8-12h**，影响面极大，commits 难以拆细。
- **B2. `block_on` 同步 facade**：在 `Db` 内部 `tokio::runtime::Handle::current().block_on(pool.execute(...))` 包裹，对外保持同步签名。**问题**：必须保证调用上下文里有 tokio runtime 跑着，否则 panic；在 `tauri::command` 已 `spawn_blocking` 的路径上调 `block_on` 是 [anti-pattern](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)（在 blocking 线程里再次进入 runtime），一旦哪天换 runtime 或 sync 调用从 daemon 后台触发，会出现死锁/panic。

二者都不优雅。SQLx 的 async 优势在 web 服务端（多请求并发等 IO）才显著；桌面单用户场景里 IPC 已经在 `spawn_blocking` 池里跑，再嵌一层 async 收益约等于零。

#### 候选 C：混合（不推荐，但记录）

memex.db 切 SQLx，外部 db（cursor / opencode）继续 rusqlite。同时引入两个 SQLite 绑定，binary 体积 +2-3 MB，dev 心智成本翻倍。

#### 候选 D：`Diesel 2.x` ORM（同步 + 编译期 typed query）

Diesel 是 Rust 生态里**唯一的同步原生 ORM**，且**直接支持 SQLite**（`diesel = "2"` features=`["sqlite"]`，可加 `bundled`）。也提供 `diesel-async`（可选，本节不取）。

| 改造项 | 估时 |
|---|---|
| `Cargo.toml`：rusqlite → diesel 2 features=["sqlite", "r2d2", "chrono"] (+`libsqlite3-sys/bundled`) | 30min |
| `diesel setup` + `diesel print-schema > src/storage/schema.rs` 生成 `table!` 宏 | 1h |
| 把 10 个 inline migration 写成 `migrations/<timestamp>_<name>/{up,down}.sql`（`diesel migration generate`） | 2h |
| 8 个 `db/*.rs` 改 query DSL（`users::table.filter(...).load::<User>(&conn)?`），或继续手写 SQL 用 `sql_query::<T>(...)` 但失去类型安全 | 8-12h |
| `queries.rs` 1138 行（含大量 aggregation / window / FTS5）很多 SQL **diesel DSL 表达不动**，必须 `sql_query` + 手写 typed Row → 实质是把 rusqlite 的 row mapping 用 Diesel 重写一遍 | 8-12h |
| `db/tests.rs`：连接池初始化方式变了（`SqliteConnection::establish(":memory:")`），16 处 in-memory db 全改 | 1.5h |
| 模型派生 `#[derive(Queryable, Insertable, AsChangeset)]`，所有 struct 重新标注列映射 | 2h |
| `commands/error.rs` 加 `From<diesel::result::Error>` | 30min |

**总估时**：~24-31h（与候选 B2 同量级）。

##### Diesel 的真正成本

1. **学习曲线陡**：`table!` 宏 + query DSL（`users::dsl::*`）+ `Queryable` / `AsChangeset` / `Insertable` derive 是一整套 mental model；新人上手 ≥1 周。
2. **复杂 SQL 表达不动**：window functions / CTE / FTS5 MATCH 在 DSL 里都得退回 `sql_query`，DSL 收益打七折。memex 在 `queries.rs` 里有大量这类 SQL（`workload_report` 用了 7 个 CTE，summary 进度条 query 用了 `LEFT JOIN ... GROUP BY`），DSL 化收益 < 30%。
3. **bundled SQLite 编译时间**：Diesel 启动 cargo build 第一次 +60-90s（绑 SQLite + macro 展开）。
4. **macro debug 体验差**：`table!` / `joinable!` 有时 query 编不过却报奇怪错误，比 SQLx 的 `query!` macro 更难调。

**适合场景**：纯 CRUD + 简单 join 业务（用户系统 / 订单系统）。**memex 是分析型 + 全文检索 + 大量 aggregate，不是 Diesel 的甜蜜区。**

#### 候选 E：`rusqlite` + 周边工具补齐 = 候选 A 的具体化

这是把候选 A 写得更具体的版本，所有依赖均为 rusqlite 生态原生组件：

| 依赖 | 版本 | 解决什么 |
|---|---|---|
| `rusqlite_migration` | 1.x | 把 inline migration 提到 `migrations/V01__*.sql` 文件，自动管 `schema_version`，`Migrations::new(&[...])` 一行声明 |
| `serde_rusqlite` | 0.36 | `from_row::<MySession>(row)` 替代手写 `row.get(0)?, row.get(1)?, ...`，DRY |
| 自带 `prepare_cached` | n/a | 热路径 query 自动复用编译后的 stmt，省 parse + plan |
| 自带 `Transaction` | n/a | 显式 `tx.commit()` 替代散落的 batch execute |
| 可选 `rusqlite_from_row` | 0.7 | derive macro 生成 `FromRow`，比 `serde_rusqlite` 少一层 serde 间接 |

**总估时**：6-10h，**风险低**，**API 表面完全不变**（130 处调用点最多调局部参数，不动方法签名）。

#### 候选横向对比

| 候选 | sync/async | 编译期 query check | 学习曲线 | 估时 | 风险 | 适合度 |
|---|---|---|---|---|---|---|
| A/E rusqlite + 补强 | sync | ❌（runtime 报错） | 低 | 6-10h | 低 | ⭐⭐⭐⭐⭐ |
| B1 SQLx 全异步 | async | ✅（macro） | 中 | 30-42h | 高 | ⭐⭐ |
| B2 SQLx + block_on | sync 假象 | ✅（macro） | 中 | 22-30h | 中-高（反模式） | ⭐ |
| C 混合 | mixed | 部分 ✅ | 高 | 25-35h | 高 | ⭐ |
| D Diesel ORM | sync | ✅（DSL + macro） | 高 | 24-31h | 中 | ⭐⭐ |

#### 决策建议

**强烈倾向候选 E**（rusqlite + 周边工具补齐）。判断依据：

1. **业务场景错配**：memex 是 local-first 桌面应用 + 分析型查询为主，async 收益微薄、ORM DSL 表达力不够、编译期 query check 不等于业务正确性
2. **改造成本失衡**：E 6-10h vs B/C/D 22-42h，4-7 倍工时差，但收益不到 1.5 倍
3. **业界共识**：2026 年 Rust ORM 综述明确 "CLI tools, desktop apps, embedded systems → Rusqlite"
4. **API 稳定性**：候选 E 完全不破坏 130 处调用点的方法签名，可与 P1-1 / P1-2 并行推进

**若仍想换框架**，按场景适用度排序应该是：
1. **Diesel**（候选 D）—— 同步 + SQLite + 编译期 check，唯一不引入 async 的 ORM 候选；但 DSL 学习成本 + 复杂 SQL 退化为 `sql_query` 的现实，最终收益打折
2. SQLx + block_on（候选 B2）—— 编译期 check 在；但 block_on 反模式
3. SQLx 全异步（候选 B1）—— 改造面最大，但路径最干净

**已决策**：候选 E（rusqlite + 周边工具补齐），4 个 step 全部完成。

**规约依据**：§3.1 依赖最小化、§7.2 文件大小、§14.1 错误类型
**实际耗时**：约 4h（4 个 step × 1h），低于预估 6-10h 区间

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
