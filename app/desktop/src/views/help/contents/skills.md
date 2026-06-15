# Skill 用法

**Memex 仓库自带 1 份 SKILL.md**——它是给 AI 看的工具用法说明书。装到 IDE 后，AI 在加载会话时会读到这份说明，知道**什么时候**该调用 Memex 的 MCP 工具、传**什么参数**、返回如何解读。

## SKILL 和 MCP 是什么关系

很多人会问：MCP 已经能调用工具了，为什么还需要 SKILL？

| 维度 | MCP | SKILL |
|---|---|---|
| 解决什么 | "AI 能调用什么工具" | "AI 什么时候该调，传什么参数" |
| 形式 | 工具协议 + 函数签名 | 自然语言用法说明 + 触发场景 |
| 加载时机 | 工具注册（startup） | system prompt（每次会话加载） |
| 没有它会怎样 | AI 没工具可用 | AI 有工具但不知道用，或用错参数 |

**举个反例**：没装 SKILL 时，用户问「我之前讨论过 retry 策略吗」，AI 看到 `search_memory` 工具但可能传 `query="retry"`（漏了"策略"），或者优先调 `list_recent` 拿前 10 条然后纯文本匹配（慢且不准）。装了 SKILL 后，AI 知道"问'之前讨论过 X 吗'就直接用 `search_memory` 全词组查询"，命中率和响应速度都更高。

## SKILL.md 内容长什么样

打开 `app/SKILL.md` 你会看到类似这样的结构：

```markdown
# Memex MCP Skill

## When to use which tool

| 用户提问关键词 | 用什么工具 | 为什么 |
|---|---|---|
| "我之前讨论过 X" | search_memory | FTS 命中率高 |
| "整理一下当前项目" | get_project_context | 带 L2 摘要，比纯搜索更结构化 |
| "本周做了什么" | list_sessions_by_range | 按时间区间归纳 |

## Examples

User: 「retry 策略上次怎么决定的」
→ Call: search_memory(query="retry 策略", limit=5)
→ Then: get_session(session_id=<top hit>)
```

AI 读完这段后，遇到对应关键词时会自动套用这套调用模式，输出更符合用户预期。

## 怎么安装

**Connect 页 → IDE 集成 → 装 SKILL** 一键搞定，Memex 会按你当前安装的 IDE 把对应版本复制到正确路径：

| IDE | 路径 |
|---|---|
| Cursor | `~/.cursor/skills/memex/SKILL.md` |
| Claude Code | `~/.claude/skills/memex/SKILL.md` |
| Codex | `~/.codex/skills/memex/SKILL.md` |
| OpenCode | `~/.opencode/skills/memex/SKILL.md` |

> 仓库里 `app/skills/{cursor,claude-code,codex,opencode}/SKILL.md` 是按 IDE 适配过的源文件——不同 IDE 对 skill 元信息（YAML frontmatter）的格式要求不同，Memex 安装时会自动选对版本。

也可以手动安装（适合纯 CLI 用户或者 IDE 没在列表里）：

```bash
mkdir -p ~/.cursor/skills/memex
cp /Applications/Memex.app/Contents/Resources/skills/cursor/SKILL.md \
   ~/.cursor/skills/memex/SKILL.md
```

## 触发关键词

不需要刻意调用，AI 在以下场景会自动用 SKILL + MCP：

- 「整理项目记忆」「我之前在这个项目做到哪了」「接着干」「上次"
- 「搜一下关于 X 的对话」「上次怎么解决的」「找一下之前关于 X 的讨论」
- 「本周做了什么」「上个月的工作总结」「日报 / 周报 / 月报」「6 月 1 日到 8 日」
- 「memex 数据库现状」「攒了多少会话」「一共聊了多少消息」
- 「这个项目现在到什么阶段」「回顾一下决策」「继续做之前的功能」

## 不生效？

按这个顺序排查：

1. **路径**：表格里的 SKILL.md 文件确实存在
2. **重启 IDE**：MCP 配置 + skill 加载都需要重启 IDE 进程才生效（开新 tab 不算）
3. **Cursor**：Settings → Skills 里能看到 **memex** = 装好了
4. **Claude Code**：在新会话里直接问 "你能用 memex skill 吗" 看 AI 是否提到 SKILL 内容
5. **看 IDE 日志**：Cursor 的 Developer Tools → Console 会打印 skill 加载错误（多半是 YAML frontmatter 格式问题，重新装一次会修正）
6. **缓存**：Cursor 偶尔会缓存旧版本 skill，删 `~/.cursor/skills/memex/` 整个目录再重新安装能强制刷新
