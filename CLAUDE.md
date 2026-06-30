# Memex — Claude Code Project Instructions

> 给 Claude Code 看的项目级指南。约束等价于 `.cursor/rules/*.mdc`，并按 Claude Code
> 的 `@import` 语义把详细规约拉进来。新增/修改 Rust 代码以本规约为目标态，存量
> 渐进重构。

## 项目快照

- 多 IDE 会话记忆中枢（Claude Code / Cursor / Codex / OpenCode / Continue Dev 等），
  本地 SQLite + FTS5 全文索引，跨 LLM 共享会话上下文。
- 工作区根：`app/`（Rust workspace，含 `memex-core` / `memex-cli` 两个 crate；
  桌面端在 `app/desktop/`）。
- 数据目录：`~/.memex/`（可用 `MEMEX_HOME` 重定向）。会话归一化为
  `~/.memex/sessions/<adapter>/<session_id>.md`，是所有 raw 工具的沙箱根。

## 项目通用约束（同 `.cursor/rules/project.mdc`）

- 前端组件必须用 **shadcn-vue**（位于 `app/desktop/src/components/ui/`）。
- 单文件代码超过 **300 行**按模块拆分；不要无脑塞进一个大文件。
- 每个功能模块完成后**必须有单元测试**；测试与实现同 PR 提交。
- 开发完成后用最新版本替换本地老版本进行展示（macOS Cask: `app/Casks/memex.rb`）。
- 多用**卫语句**：早 return、扁平化分支，避免深嵌套。

### Rust target 目录体积管理

`target/` 易膨胀到 10–30GB。长期不动的项目 `cargo clean`；活跃项目用
[cargo-cache](https://crates.io/crates/cargo-cache)（`cargo cache --autoclean`）。
跨项目共享编译缓存推荐 [sccache](https://github.com/mozilla/sccache)。这是
开发体验优化，**不影响生产构建**。

## Rust 开发规约

完整规约见 `@.cursor/rules/rust.mdc`（741 行），下面是 Claude Code 最常踩的 8 条红线，
**违反任何一条都应在 PR review 时打回**：

1. **禁止 `.unwrap()` / `.expect()` / `panic!()` / `todo!()` 进入主干**。例外：编译期常量上的
   不变式（加 `// INVARIANT:` 注释）；`#[cfg(test)]` 测试代码不受此限。
2. **错误类型两层模型**：库 crate（`memex-core`）用 `thiserror` 具名枚举；
   应用层 / 二进制（`memex-cli`、daemon、menubar）用 `anyhow::Result` +
   `.context(...)`。禁止 `.map_err(|e| e.to_string())` 丢错误链。
3. **整数算术涉及不可信输入**必须显式选 `checked_*` / `saturating_*` / `wrapping_*`，
   不能默认 `+ - *`。
4. **入口解析为强类型**（Parse, don't validate）：CLI 参数 / 配置 / 外部输入在边界
   处转为业务类型，内部代码不再重复校验。
5. **并发安全靠类型系统**：`Send` / `Sync` 标注准确；共享状态用 `parking_lot::Mutex` /
   `Arc`；禁止裸 `static mut`。
6. **不要用 `Box<dyn>` 替代泛型**，除非真有动态分发需求。
7. **公开 API 必须有 doc comment**（`///`），并用 `# Examples` / `# Errors` /
   `# Panics` 三节标准格式。
8. **测试覆盖**：新增 pub 函数 / 模块至少一个单测；涉及文件/网络的用 `tempfile` +
   依赖注入，禁止把 fixtures 硬编码到 `$HOME`。

完整章节包括：错误处理 / 类型设计 / 所有权与借用 / 并发 / 异步 (tokio) /
模块组织 / 依赖管理 / 测试策略 / 性能 / 安全。展开查 `.cursor/rules/rust.mdc`。

## Memex MCP 自检

本项目本身就是 Memex 实现。**调试或写功能前，先用 MCP 查一遍之前的设计讨论**：

- 工作记忆：`get_project_context(project="/Users/TerrellShe/Documents/personal/tt-projects/memex")`
- 跨会话检索：`search_memory(query="…", adapter="claude_code")`
- FTS5 兜不住时：`raw_grep` / `raw_find` / `raw_read`（设计见
  `design/specs/20260630-01-Memex-原始文件兜底检索设计.md`）

不要用 `git log` / `Read README.md` 替代 memex MCP —— git 反映 commit，反映不了
讨论与决策路径。

## 常用命令

```bash
# 构建 + 跑全部测试
cd app && cargo build && cargo test

# 只跑 memex-core 的某个模块（如新加的 raw 检索）
cargo test -p memex-core --lib retriever::raw -- --test-threads=1

# 启 MCP（Claude Code 自动调，手动调试时用）
./target/debug/memex-cli mcp

# 一次性安装 + 索引
./target/debug/memex-cli setup claude-code
./target/debug/memex-cli ingest

# 桌面端开发
cd app/desktop && pnpm dev
```

## 提交前 checklist

- [ ] `cargo build` clean，无 warning（`#![warn(clippy::all)]` 已开）
- [ ] 新增模块有单测，`cargo test -p <crate>` 全绿
- [ ] 超 300 行的文件已拆分模块
- [ ] 错误处理符合两层模型（库 thiserror / 应用 anyhow）
- [ ] 没有 `.unwrap()` / `panic!()` 漏进主干
- [ ] 设计变更已写入 `design/specs/YYYYMMDD-NN-...md`
