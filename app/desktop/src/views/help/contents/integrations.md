# IDE 接入

Memex 通过 3 种机制把"会话记忆"喂给 AI 编辑器，每种机制解决不同问题，可以叠加使用。

## 三种接入机制对比

| 机制 | 解决什么 | 时机 | 开销 |
|---|---|---|---|
| **MCP** | 让 AI 能调用 Memex 的 6 个搜索 / 检索工具 | 用户提问命中关键词时按需调用 | 单次毫秒级 |
| **SKILL** | 让 AI 知道**什么时候**该用 MCP、传什么参数 | IDE 启动时一次性加载到 system prompt | 上下文 ~1-2K token |
| **Hook** | 在每次新会话开始时**主动注入**项目记忆摘要 | 会话第 0 秒（在用户提问前） | 一次 LLM 输入 ~500 token |

理想接入 = 三个都开。MCP 是地基，SKILL 是说明书，Hook 是开场白。

## Cursor · 最常用

支持：**MCP** · **SKILL** · **Hook（自定义）**

接入文件：

| 文件 | 作用 |
|---|---|
| `~/.cursor/mcp.json` | MCP server 注册（被 Memex 一键安装写入） |
| `~/.cursor/skills/memex/SKILL.md` | 让 AI 学会怎么用 |
| `~/.cursor/rules/memex.mdc` | 注入触发规则（"用户问 X 时强制调用 Y"） |

接入后的真实对话样例：

```
你：我前段时间是不是讨论过 retry 策略？

AI：[自动调 search_memory("retry 策略")]
    找到 3 条相关会话：
    - 2026-06-12 在 memex 项目讨论了 exponential backoff
    - 2026-05-30 在 db-svc 项目用 axios 拦截器实现
    - 2026-05-18 在 aws-mig 项目用 SQS 配置 Lambda 失败重试
    需要看哪一条的详细对话吗？
```

**找不到调用？**
- 重启 Cursor（MCP 配置变更需要重启进程）
- Cursor 设置 → MCP → 看 memex 是否亮绿灯
- Connect 页 → MCP 活动 卡片，看有没有调用记录

## Claude Code · 推荐

支持：**MCP** · **SKILL** · **Hook（原生）**

Claude Code 是**唯一原生支持 sessionStart hook** 的 IDE——每次开新会话时，hook 把当前项目的最近 5 条会话摘要 + 关键决策注入到 system prompt，AI 第一句回复就具备项目上下文，**不用你重新解释背景**。

接入文件：

| 文件 | 作用 |
|---|---|
| `~/.claude/settings.json` | MCP server 注册 + hook 注册 |
| `~/.claude/skills/memex/SKILL.md` | Skill 用法 |

Hook 触发的 banner 长这样（注入到新会话的 system prompt 顶部）：

```
## Memex 工作记忆 · memex (~/Documents/personal/tt-projects/memex)

最近 3 条会话摘要：
1. [2026-06-15] 标记会话为私有 + Hook 重命名 + 部署
2. [2026-06-14] EBADF 修复：daemon FD leak 在 watcher/scheduler abort
3. [2026-06-13] Memex 1.0.3 发布物料：promo 视频脚本 + landing carousel

关键决策：
- v5 migration 加 sessions.is_private 列；MCP 4 个 tool fail-closed 过滤
- 私有标记入口在 Library 列表行右上角，drawer 不再有 toggle
```

## Codex

支持：**MCP** · **SKILL**。

Codex 没有 hook 接口，但 SKILL 会在 system prompt 里告诉 AI"用户问 X 时主动调 get_project_context"——效果接近 hook 但需要触发关键词。

接入文件：`~/.codex/config.json` + `~/.codex/skills/memex/SKILL.md`

## OpenCode

支持：**MCP**（SKILL / Hook 在路线图里）。

接入文件：`~/.opencode/config.toml` 注册 MCP server。SKILL.md 还没适配 OpenCode 的 skill 加载格式，等 OpenCode 端生态稳定后接入。

## 怎么验证接入成功

最直接的方法 —— 在 IDE 里发一句：

> 你能用 memex 搜一下我最近的会话吗？

AI 会调 `list_recent`，返回前 N 条会话列表。看到结果 = MCP 联通；看到 AI 主动选了 `list_recent` 而不是 `search_memory` = SKILL 也生效。

也可以打开 Memex 主窗口的 **Connect 页 → MCP 活动**，那里实时统计 24h 内所有 MCP 调用，能看到来自哪个 IDE、调了什么 tool、耗时多少、成功失败。
