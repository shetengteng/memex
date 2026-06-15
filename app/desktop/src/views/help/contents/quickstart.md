# 快速开始

Memex 是一个**本地优先**的 AI 会话记忆中枢——把你过去在 Cursor / Claude Code / Codex / OpenCode 里聊过的所有内容统一索引到一个 SQLite 数据库，让你能跨 IDE / 跨项目搜索、让 AI 自动检索"我之前说过什么"。整个过程在你电脑上完成，不联网（除非你主动配 LLM 做摘要）。

5 分钟内你能跑通：**装好 → 首次扫描 → 第一次搜索成功**。

## 1. 装好 Memex

从 GitHub Releases 下载对应架构的 DMG（按 macOS 架构选 arm64 / x64），双击拖入 `/Applications`，再跑一行脚本清掉 macOS Gatekeeper 的 quarantine 标记并启动：

```bash
# 1. 下载 DMG
#    https://github.com/shetengteng/memex/releases

# 2. 双击 DMG 拖入 /Applications

# 3. 一键安装（清 quarantine、刷新 LaunchServices、启动）
curl -fsSL https://raw.githubusercontent.com/shetengteng/memex/main/scripts/install-macos.sh | bash
```

> **为什么需要这个脚本？** 当前版本是 ad-hoc 签名（没买 Apple Developer 账号），直接双击 DMG 启动会被 Gatekeeper 拦截、扔到 AppTranslocation 临时目录然后报"已损坏 / 未识别开发者"。`xattr -cr Memex.app` 清一次扩展属性，让 macOS 把它当本地编译产物对待，启动就正常了。后续升级（覆盖 DMG）需要再跑一次同一个脚本。

启动后菜单栏出现 **(M)** 图标，左键弹 Tray Popup（最近会话 + 快速搜索），右键打开主窗口。全局快捷键 <kbd>⌘⇧M</kbd> 在任意位置切到主窗口。

## 2. 看第一次扫描

启动后，Memex 自动扫描下面这些 IDE 的本地数据库，把过往会话**复制并解析**到 `~/.memex/`：

| 来源 | 路径 |
|---|---|
| Cursor | `~/Library/Application Support/Cursor/User/workspaceStorage/*/state.vscdb` |
| Claude Code | `~/.claude/projects/*/conversations.jsonl` |
| Codex | `~/.codex/sessions/*.json` |
| OpenCode | `~/.opencode/storage/*/sessions/*.json` |

> Memex 不修改任何 IDE 的源数据，只是**只读**复制 + 解析；删掉 `~/.memex/` 不会影响 IDE。

打开 Library 页，看到 **共 N 个会话** 就说明扫描成功。首次扫描 1000 条会话约需 30-60 秒，期间状态栏会显示进度。L2 摘要（Library 列表里那段"用户意图"）需要本地 LLM（Ollama）才能生成；没配 LLM 也不影响搜索功能本身。

## 3. 做第一次搜索

按 <kbd>⌘K</kbd> 打开命令面板，输入任何过去聊过的关键词。FTS5 全文检索 + 时间衰减排序，结果按"最近聊过"靠前：

```
⌘K → "retry 策略"
→ 命中 3 条会话：
  · cursor / memex   · 2026-06-12  · "重试逻辑用 exponential backoff..."
  · cursor / db-svc  · 2026-05-30  · "前端的 axios 拦截器加重试..."
  · claude / aws-mig · 2026-05-18  · "Lambda 失败重试 SQS 配置..."
```

每条结果会标注：来自哪个 IDE / 哪个项目 / 何时聊的 / 命中片段。点开就跳到详情。

也可以用 CLI 搜索（写脚本 / pipe 处理时方便）：

```bash
# CLI 自带 sidecar 在 /Applications/Memex.app/Contents/MacOS/memex-cli
memex-cli search "retry 策略" --limit 5
```

---

**下一步** → [接入到 AI 编辑器](#integrations)：让 AI 自动调 Memex 检索历史 · [MCP 工具用法](#mcp)：6 个工具的触发场景
