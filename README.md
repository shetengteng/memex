# Memex

> The Memex you imagined in 1945 — finally built for the AI era.
> 让所有 AI 编辑器共享同一份"你和 AI 的全部历史"。

---

## 为什么

你现在每天在 5 个 AI 工具之间切换：Cursor、Claude Code、Codex、OpenCode、Claude Desktop……
每打开一个新 session，AI 都从零开始。**你和 AI 几万次对话的经验全部在浪费。**

Memex 是一个本地优先的"AI 记忆中枢"：
- 自动采集所有主流 AI CLI / 编辑器的会话历史
- 统一索引、统一查询
- 通过标准化协议暴露给任意 AI 编辑器，让每个新 session 都能 "想起" 你之前说过什么

**致敬 1945 年** — Vannevar Bush 在 *As We May Think* 中提出 Memex 概念：一台能记住一切、随时调取关联的人类记忆扩展机器。AI 时代终于让这个 80 年前的愿景成真。

---

## 现状

🚧 **设计阶段** — v4 架构已收敛，技术路线为**全程 Rust + Tauri 2 + Vue 3 + shadcn-vue**（无 Python 中间态）。

设计文档在 [`design/`](design/) 目录：

**架构与设计**
- [`20260531-03-Memex-v4最终设计文档.md`](design/20260531-03-Memex-v4最终设计文档.md) — v4 最终架构、模块边界、数据模型
- [`20260531-12-Memex-技术栈.md`](design/20260531-12-Memex-技术栈.md) — 技术选型 + 代码复用来源（DiskMind / tokenbar / tars-ai-butler）

**开发计划**
- [`20260531-13-Memex-执行计划-Rust.md`](design/20260531-13-Memex-执行计划-Rust.md) — Sprint 1 ~ 11 详细排期（带 checkbox）
- [`20260531-01-Memex-v4功能点开发清单.md`](design/20260531-01-Memex-v4功能点开发清单.md) — 功能模块视角的全集 checklist

**测试**
- [`20260531-14-Memex-单元测试用例设计.md`](design/20260531-14-Memex-单元测试用例设计.md) — 模块级测试用例与 fixture
- [`20260531-15-Memex-测试TODO.md`](design/20260531-15-Memex-测试TODO.md) — 按 Sprint 拆分的测试 checklist

**UI 原型**
- [`20260531-04-Memex-menubar-ASCII原型与功能设计.md`](design/20260531-04-Memex-menubar-ASCII原型与功能设计.md) — menubar 信息架构与 ASCII 原型
- [`20260531-05-Memex-menubar-交互原型.html`](design/20260531-05-Memex-menubar-交互原型.html) — 浅色 shadcn 完整面板原型（设计参考）
- [`20260531-09-Memex-menubar-交互原型-v3.html`](design/20260531-09-Memex-menubar-交互原型-v3.html) — 单 popup 聚焦版（实际执行方向）
- [`20260531-06-Memex-数据演变可视化.html`](design/20260531-06-Memex-数据演变可视化.html) — 数据从 raw 到 chunk / metadata 的演变可视化

---

## License

MIT
