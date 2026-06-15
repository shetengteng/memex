# 隐私与数据

Memex 是**本地优先**的应用——所有索引数据都存在你电脑的 `~/.memex/` 目录，**默认不联网**。下面这条是核心承诺：

> Memex 不主动上传任何数据。LLM 摘要功能（L2）默认调用本地 Ollama；只有你**主动**在 Settings 配 OpenAI / Anthropic / 自建 API 时，才会有摘要请求出网，且仅限你那条 API。索引、搜索、Hook 注入全程纯本地。

## 数据流入与存储位置

```
[ Cursor / Claude Code / Codex / OpenCode 的本地 db / jsonl ]
                  ↓ （只读，不修改源数据）
       Memex daemon / collector 解析
                  ↓
            ┌─────┴─────┐
            ↓           ↓
   ~/.memex/sessions/  ~/.memex/memex.db
   原始会话副本（md）  SQLite 索引 + FTS5
                  ↓
       MCP / Hook / CLI 按 privacy 规则筛选
                  ↓
           暴露给 AI 编辑器
```

`~/.memex/` 完整目录结构：

| 路径 | 内容 | 删了会怎样 |
|---|---|---|
| `~/.memex/memex.db` | SQLite 主索引 + FTS5 全文索引 | 重新启动 Memex 会触发完整 reindex |
| `~/.memex/memex.db-wal` / `-shm` | SQLite WAL + 共享内存 | 删了无影响（自动重建） |
| `~/.memex/sessions/` | 原始会话 markdown 副本 | 已索引数据保留，原始内容看不到了 |
| `~/.memex/redactions.yaml` | 路径 / 关键词隐私规则 | 失去过滤；建议 git 管理这份配置 |
| `~/.memex/config.toml` | 应用配置（LLM / privacy 开关 / port） | 回到默认配置 |
| `~/.memex/daemon.lock` | 进程锁文件 | 仅运行时存在 |
| `~/.memex/logs/` | daemon 日志（最近 7 天） | 仅排障用 |

## 把会话标记为「私有」（细粒度）

Library 列表里 hover 一行，标题右侧 IdeChip 旁边会出现**淡灰色锁图标** → 点一下即标记为私有；再点一次取消。已私有的会话锁图标会**常显且 amber 高亮**，列表里一眼能看到。

标记为私有的会话**不会通过任何途径暴露给 AI**：

| 暴露途径 | 私有会话过滤行为 |
|---|---|
| MCP `search_memory` | retain 过滤掉 |
| MCP `list_recent` | retain 过滤掉 |
| MCP `list_sessions_by_range` | retain 过滤掉 |
| MCP `get_session` | **fail-closed** —— 即便 AI 知道 ID 也返回 `session not found`，不暴露存在性 |
| Hook 注入的 `get_project_context` | 过滤掉 + total_sessions 也按过滤后计数（不暴露"还有 N 条看不到"） |
| CLI `memex-cli context` | 同 hook，私有会话不出现在 banner |

> **设置 → 偏好 → "私有会话不通过 MCP 暴露给 IDE"** 开关控制此功能是否生效（默认开启）。关掉后 AI 能看到所有会话——除非命中下面 redactions.yaml 的路径 / 关键词规则。

## redactions.yaml（批量过滤）

逐条标记不现实时，用这份配置做路径 / 关键词级的批量过滤：

```yaml
# ~/.memex/redactions.yaml

# 路径规则：glob 风格
paths:
  - "~/work/secret-project/**"
  - "~/Documents/private-notes/**"
  - "/tmp/**"

# 关键词规则：会话内容包含任意一条就视同私有
keywords:
  - "API_KEY"
  - "BEGIN PRIVATE KEY"
  - "ANTHROPIC_API_KEY"
  - "passwd"
  - "BEGIN OPENSSH PRIVATE KEY"
```

匹配优先级：

1. 会话的 `is_private = true`（手动标记）→ 私有
2. `project_path` 命中 `paths` 规则 → 私有
3. 会话内任意 message content 命中 `keywords` 规则 → 私有
4. 否则 → 公开

任意一条命中即视同私有。规则改完**实时生效**（下一次 MCP 调用就按新规则过滤）。

## LLM 配置时的隐私边界

Memex 索引、搜索、Hook 注入都是纯本地 SQL 查询。但 **L2 摘要功能**（生成"用户意图"和"会话摘要"）需要 LLM。Settings 里可以选：

| LLM 提供商 | 数据流向 | 隐私级别 |
|---|---|---|
| 本地 Ollama（默认） | 不出本机 | 完全本地 |
| OpenAI / Anthropic API | 摘要原文发到对应 API | 受该 API 隐私政策约束 |
| 自建 API endpoint | 你自己控制 | 看你的部署 |

**摘要内容包含什么**：会话的全部 user / assistant 消息原文。如果会话里聊过敏感内容，又选了云端 LLM，那这些内容就会被发出去。建议：

- 默认装 Ollama，纯本地处理
- 或者：把含敏感会话所在项目加进 `redactions.yaml` 的 paths，根本不走 LLM 摘要

## 怎么彻底清掉一条会话

标记私有只是"不暴露"，**数据本身还在 SQLite 里**。要彻底清掉：

1. 在 Library 找到那条会话，记下 ID
2. 删 SQLite 行：`sqlite3 ~/.memex/memex.db "DELETE FROM sessions WHERE id = '<id>'"`
3. 删 chunks（FTS 索引）：`sqlite3 ~/.memex/memex.db "DELETE FROM chunks WHERE session_id = '<id>'"`
4. 删原始 md 文件：`rm ~/.memex/sessions/<adapter>/<session_id>.md`

要清整个项目：把上面 1-3 的 WHERE 换成 `WHERE project_path = '/abs/path'`。

> Memex UI 还没暴露逐条删除按钮（v1.0.4 路线图），暂时手动 SQL。

## 导出 / 重建 / 彻底重置

- **导出全部为 JSON**：Settings → System → 导出（用于备份或迁移到另一台机器）
- **重建索引**：Settings → System → 重建索引（删 SQLite、保留 sessions/*.md，从源 md 重新 ingest；耗时跟首次扫描接近）
- **彻底重置**：Settings → System → 彻底重置（删 db + sessions + 原始文件，回到首次启动状态。**操作不可逆**）
