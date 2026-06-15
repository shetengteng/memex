# 项目记忆 / Hook

Hook 是 IDE 在每次开新会话时自动调用的脚本入口。Memex 的 sessionStart hook 会把当前项目的最近会话摘要 + 关键决策**主动注入**到新会话的 system prompt——AI 第一句回复就具备项目上下文，**不需要你重新解释背景**。

这是 Memex 体验最爽的部分：你切回某个项目开新会话，第一句问"上次的 retry 策略后来怎么了"，AI 直接给出延续上次讨论的回答，不会问"上次是讨论什么"。

## 原理

```
┌─ IDE 新会话开始（Claude Code Cmd-N）
│
├─ Claude Code 触发 sessionStart hook
│       ↓
│       ~/.claude/settings.json 里注册的 hook 命令
│       ↓
│       memex-cli context --project $(pwd) --top 5
│
├─ memex-cli 内部：
│   1. 读 ~/.memex/memex.db
│   2. 按 project_path 过滤会话
│   3. 取最近 5 条，附带 L2 摘要 + 决策
│   4. 套 redactions / privacy 过滤（私有会话剔除）
│   5. 输出一段结构化 markdown
│
├─ stdout 内容被注入到 system prompt 顶部
│       ↓
└─ AI 第一句回复就具备项目上下文 ✓
```

整个过程 100ms 以内（Memex 是本地 SQLite 查询），用户体感完全无感，但 AI 的回复质量肉眼可见的提升。

## banner 实际样例

下面是真实注入到 Claude Code 新会话顶部的内容（你的 system prompt 的一部分）：

```markdown
## Memex 工作记忆 · memex (~/Documents/personal/tt-projects/memex)

### 项目摘要
本地优先的跨 LLM 会话记忆中枢，由 Tauri + Vue + Rust 实现。
6 个 MCP 工具暴露给 AI 编辑器。

### 最近 5 条会话
1. [2026-06-15] 标记会话为私有 + Hook 命名优化 + 部署
2. [2026-06-14] 修复 daemon EBADF：watcher/scheduler abort + tokio process
3. [2026-06-13] Memex 1.0.3 发布物料：promo 视频 + landing carousel
4. [2026-06-11] 架构合并：daemon 折叠进 Tauri 主进程的方案讨论
5. [2026-06-10] MCP 工具列表确认 + 测试覆盖

### 关键决策
- v5 migration 加 sessions.is_private 列
- MCP 4 个 tool fail-closed 过滤私有会话（含 get_session）
- collect_project_context 改纯函数，skip_private 由调用方传

### 项目主题
memex, rust, tauri, vue, sqlite, mcp
```

AI 看到这段后，"上次"是什么、"接着干"做什么，全部都有锚点了。

## 配置位置

只有 **Claude Code** 当前原生支持 sessionStart hook：

```json
// ~/.claude/settings.json（Memex 一键安装后会写入）
{
  "hooks": {
    "sessionStart": "/Applications/Memex.app/Contents/MacOS/memex-cli context --project $WORKSPACE --top 5"
  }
}
```

`$WORKSPACE` 由 Claude Code 在调用时填入当前 workspace 绝对路径。

其他 IDE（Cursor / Codex / OpenCode）没有 hook 接口，但效果可以靠 SKILL.md + 触发关键词模拟——SKILL 在 system prompt 里告诉 AI "用户问 X 时主动调 get_project_context"，命中关键词就主动检索，效果接近 hook 但**需要触发**。

## 调试方法

**1. 直接看 banner**

在 AI 里直接问：

> 你能看到 memex 注入的工作记忆吗？请把 banner 内容原样输出一下。

AI 会从 system prompt 里把 banner 段拷贝出来，看到 = hook 工作中。

**2. 本地跑同一条命令**

```bash
memex-cli context --project $(pwd) --top 5
```

stdout 是什么，注入到 IDE 的就是什么。空？参考下面一条。

**3. banner 显示"暂无关联会话记忆"**

最常见原因：`$PWD` 跟 Memex 索引里的 `project_path` 对不上（IDE 启动 hook 时 cwd 可能是配置目录而不是项目根）。

修复：传**绝对路径**手动测：

```bash
memex-cli context --project /Users/me/work/memex --top 5
```

仍然空 = 该项目还没被索引（Library 页能看到吗），或者所有会话都被 redactions / private 过滤掉了。

**4. hook 没触发**

Claude Code 设置 → 看 `~/.claude/settings.json` 是否真的有 `hooks.sessionStart`。一键安装失败时这条不会写入。Memex 主窗口 Connect 页 → IDE 集成 → Claude Code → 重新装 hook。

**5. Claude Code 没显示 banner**

Claude Code 的对话日志（`~/.claude/projects/<workspace_hash>/conversations.jsonl`）第一条 system 消息会包含 hook 输出。`grep "Memex 工作记忆" ~/.claude/projects/*/*.jsonl` 能确认是不是真的注入了。

## 项目记忆和搜索的区别

很多人混淆——它们是不同层次的工具，配合使用：

| | 主动注入 / 被动检索 | 数据形态 | 适合场景 |
|---|---|---|---|
| Hook（项目记忆） | **主动**注入 | 摘要 + 决策 + 主题（结构化） | "我回到这个项目了，告诉我上次到哪" |
| MCP search_memory | **被动**检索 | 原始对话片段（FTS 命中） | "我之前讨论过 X 吗" |

Hook 是"开场白"，搜索是"翻笔记"。AI 用 Hook 的内容知道**当前坐标**，用 search 工具按需翻**具体细节**。
