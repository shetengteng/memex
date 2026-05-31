# tars-ai-butler Adapter 移植对照清单

> 日期：2026-05-31
> 对应执行计划：`20260531-13-Memex-执行计划-Rust.md` Sprint 1 / Sprint 2 任务
> tars 包路径：`/Users/TerrellShe/.tars-ai-butler/tars/adapters/`
> 用途：把 tars Python 实现里**经验证的**路径、字段、边界条件、清洗规则提炼为 Rust 重写参考。tars 代码不能直接复用，但其踩过的坑要全部复用。

---

## 0. 通用参考

tars 中所有 adapter 都继承自 `tars/adapters/base.py::ToolAdapter`，定义了 5 个抽象方法 + 1 个可选：

| 方法 | 用途 | Memex 对应 |
|---|---|---|
| `name()` | adapter 标识 | `Adapter::name()` |
| `is_available()` | 检测本机是否有数据可读 | 加到 trait（当前缺） |
| `get_history_path()` | 数据根目录 | 已有内部 `base_dir` |
| `parse_history()` | 解析所有 session | `Adapter::scan + collect` |
| `resume_command(session_id, project_path)` | 跳回会话的 shell 命令 | v0.3 menubar 用 |
| `launch_command(project_path)` | 新开会话的 shell 命令 | v0.3 menubar 用 |
| `can_summarize()` / `summarize()` | 用工具自身能力做摘要 | Sprint 6 LLM 集成时再考虑 |

tars 的 `WorkContext` 结构对应 Memex 的 `SessionMeta + RawMessage` 组合，**核心信号字段**包括：

- `recap` —— Claude Code 在 idle 时写入的 `away_summary`，格式："状态. Next: 动作"
- `next_step` —— 从 recap 用正则解析出来的下一步
- `native_summary` —— Claude 的 `isCompactSummary=true` 用户消息（Claude 自己生成的高质量摘要）
- `last_assistant_message` —— 最近一条 assistant 的纯文本回复（截断到 1200 字符）
- `files_touched` —— 通过 `tool_use` 中 `Edit/Write/MultiEdit/NotebookEdit` 工具调用的 `file_path` 收集
- `branch` —— 从 entry 的 `gitBranch` / `git_branch` / `message.gitBranch` 字段
- `session_name` —— Claude session JSONL 中的 `slug` 字段

**Memex 处理建议**：这些信号建议放在 `messages.metadata` 或者 `sessions` 表的扩展列里，直接落库，不用现场抽取，方便后续日报/周报合成。

---

## 1. Claude Code Adapter

### 1.1 路径

| 项 | tars 实际路径 |
|---|---|
| 历史索引 | `~/.claude/history.jsonl`（**Memex 当前未使用**） |
| Session 文件 | `~/.claude/projects/<encoded-project-path>/<session-uuid>.jsonl` |
| 配置文件 | `~/.claude/settings.json` |

> tars 用 `tars.config.get_claude_history_path()` 与 `find_session_file(session_id)` 两个 helper 关联两层数据。Memex 当前直接扫 `projects/**/*.jsonl`，没读 `history.jsonl`，相当于**直接拿原始数据，跳过了索引层**。这种做法对 ingest 没问题，但失去了 tars 的"用户提示词去重 + 项目路径直拿"能力。

### 1.2 history.jsonl 字段（每行一个 entry）

```json
{
  "sessionId": "uuid",
  "timestamp": 1717123456000,
  "display": "用户实际输入的提示词",
  "project": "/Users/x/code/some-project"
}
```

注意 `timestamp` 是**毫秒**，Memex 当前 Claude adapter 处理的是 RFC3339 字符串。两层数据时间格式不同，要注意转换。

### 1.3 Session JSONL 字段映射

```text
type=user, message.content=string|array
  → 用户输入；若 isCompactSummary=true 则是 native_summary，单独存

type=assistant, message.content=array
  block.type=text → last_assistant_message
  block.type=tool_use, name in {Edit, Write, NotebookEdit, MultiEdit}
    → input.file_path 或 input.notebook_path → files_touched

type=system, subtype=away_summary, content
  → recap（要清洗末尾 "(disable recaps in /config)" 字样）

顶层字段（任何 entry 都可能有）
  gitBranch / git_branch / message.gitBranch → branch
  slug → session_name
```

### 1.4 边界条件

- 文件不存在 → 返回空，不报错
- JSON 解析失败的行 → 跳过 + warn 日志
- `display` 为空 → 跳过该 entry
- 用户消息长度 > 500 字 → tars 视为大段粘贴，不计入 prompts（Memex 不一定要这么做，因为 Memex 是全量入库）
- recap 末尾会有 `(disable recaps in /config)` boilerplate，要正则去掉
- last_assistant_message 截断到 1200 字符（防止长回复污染索引）

### 1.5 recap 解析正则（直接搬到 Rust）

```python
# tars 的实现：
re.search(r"(?:Next action\s*:|Next\s*:|下一步\s*[:：])\s*(.+)$", text, flags=re.DOTALL)
```

Rust 版（`regex` crate）：

```rust
let re = regex::Regex::new(r"(?s)(?:Next action\s*:|Next\s*:|下一步\s*[:：])\s*(.+)$").unwrap();
```

返回 `(state, next_action)` 元组。

### 1.6 Memex 当前实现差距（待补）

- [ ] **是否引入 `history.jsonl` 二级索引？** 决策项：
  - 引入：能拿到 `display`（用户文字）+ `project` 路径，省去自己提取
  - 不引入：直接扫 projects/，简单但损失 project 字段
  - **建议**：先引入，因为 `project` 路径是用户视角的关键信号，且 tars 已踩坑
- [ ] 解析 `system, subtype=away_summary` 抓 recap，存到 `messages.metadata.recap` 或 `sessions.recap`
- [ ] 解析 `user, isCompactSummary=true` 抓 native_summary
- [ ] 解析 `assistant, content[type=tool_use, name in 4 names]` 抓 files_touched
- [ ] 解析 gitBranch / slug 存到 `sessions` 表
- [ ] last_assistant_message 截断 1200 字符并存到 `sessions.metadata`
- [ ] recap 末尾 boilerplate 清洗

---

## 2. Cursor Adapter

### 2.1 路径（**与 Memex 设计文档当前描述不一致，需修正**）

| 平台 | 路径 |
|---|---|
| macOS | `~/Library/Application Support/Cursor/User/workspaceStorage/<hash>/state.vscdb` |
| Windows | `~/AppData/Roaming/Cursor/User/workspaceStorage/<hash>/state.vscdb` |
| Linux | `~/.config/Cursor/User/workspaceStorage/<hash>/state.vscdb` |

> ⚠️ Memex 设计文档里写的是 `globalStorage/state.vscdb`，**这是错的**。tars 实测 Cursor 的会话数据存在每个 workspace 自己的 `workspaceStorage/<hash>/state.vscdb`，每个 workspace 一份。

### 2.2 数据结构

```sql
SELECT value FROM ItemTable WHERE key='composer.composerData' LIMIT 1
```

返回的 `value` 是 JSON 字符串，结构：

```json
{
  "allComposers": [
    {
      "composerId": "uuid",
      "name": "user-given-or-auto-title",
      "subtitle": "additional context",
      "createdAt": 1717123456000,
      "lastUpdatedAt": 1717123999000,
      "activeBranch": "main" 
        | { "branchName": "main", ... },
      "createdOnBranch": "main",
      "isArchived": false,
      "isDraft": false
    }
  ]
}
```

⚠️ **`activeBranch` / `createdOnBranch` 可能是 string 或 dict**（dict 含 `branchName`）。Rust 用 `serde_json::Value` 接收，再做类型分支处理。

### 2.3 workspace.json（vscdb 同级文件）

```json
{
  "folder": "file:///Users/x/code/some-project"
}
```

要去掉 `file://` 前缀和末尾斜杠。

### 2.4 边界条件

- vscdb 用只读 + URI 模式打开：`sqlite3.connect(f"file:{vscdb}?mode=ro", uri=True, timeout=3)`，**Rust 用 rusqlite 时也要 `OpenFlags::SQLITE_OPEN_READ_ONLY`**
- 缺 Full Disk Access 权限时 `OperationalError` → 友好报错
- 跳过 `isArchived=true` 和 `isDraft=true`
- 跳过 `lastUpdatedAt` < `cutoff_ms`（180 天前的）— Memex 是否要做年龄过滤？建议：默认不过滤，留给配置控制
- workspace.json 缺失或解析失败 → 跳过该 workspace（不致命）

### 2.5 注意：Cursor 不存完整消息，只存 composer 元数据

tars Cursor adapter 的 `turns` 永远 = 1，因为 Cursor 没在 vscdb 里存完整 message stream。**Memex 也只能拿到 composer 级别的元信息**（name / subtitle / createdAt / lastUpdatedAt / branch）。要拿完整对话需要逆向 Cursor 内部缓存（tars 也没做）。

### 2.6 Memex 实现指引

```rust
// crates/memex-core/src/collector/cursor.rs（待实现）
fn workspace_storage_root() -> PathBuf {
    if cfg!(target_os = "macos") {
        dirs::home_dir().unwrap()
            .join("Library/Application Support/Cursor/User/workspaceStorage")
    } else if cfg!(target_os = "windows") {
        dirs::data_dir().unwrap().join("Cursor/User/workspaceStorage")
    } else {
        dirs::config_dir().unwrap().join("Cursor/User/workspaceStorage")
    }
}

fn read_vscdb_composers(vscdb: &Path) -> Result<Vec<Composer>> {
    let conn = rusqlite::Connection::open_with_flags(
        vscdb,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    let value: String = conn.query_row(
        "SELECT value FROM ItemTable WHERE key='composer.composerData' LIMIT 1",
        [],
        |r| r.get(0),
    )?;
    let data: ComposerData = serde_json::from_str(&value)?;
    Ok(data.all_composers)
}
```

---

## 3. Codex Adapter

### 3.1 路径

| 项 | 路径 |
|---|---|
| 索引文件 | `~/.codex/session_index.jsonl`（每行一个 session 元信息） |
| Session 文件 | `~/.codex/sessions/YYYY/MM/DD/rollout-<datetime>-<session-id>.jsonl` |

### 3.2 索引文件字段

```json
{
  "id": "session-uuid",
  "updated_at": "2026-05-30T12:34:56Z",
  "thread_name": "user-given-name"
}
```

通过 `id` + `updated_at` 定位完整 session 文件路径。

### 3.3 Session JSONL 字段

每行一个 entry，按 `type` 区分：

| type | 含义 | 关键字段 |
|---|---|---|
| `session_meta` | 会话元信息 | `payload.cwd`（项目路径） |
| `response_item` | 用户/助手消息 | `payload.role`（user/assistant），`payload.content[*]` |
| `event_msg` | Codex 事件 | `payload.type`，特别是 `last_agent_message` |

`response_item` 内容结构：

```json
{
  "type": "response_item",
  "timestamp": "2026-05-30T12:34:56Z",
  "payload": {
    "role": "user",
    "content": [
      {"type": "input_text", "text": "用户输入"},
      {"type": "input_text", "text": "<environment_context>...</environment_context>"}
    ]
  }
}
```

```json
{
  "type": "response_item",
  "timestamp": "...",
  "payload": {
    "role": "assistant",
    "content": [
      {"type": "output_text", "text": "助手回复"}
    ]
  }
}
```

`event_msg` 中 `last_agent_message` 是 Codex 原生 next_step 信号：

```json
{
  "type": "event_msg",
  "payload": {
    "type": "last_agent_message",
    "last_agent_message": "Done. Next: ..."
  }
}
```

### 3.4 边界条件

- **`<environment_context>...</environment_context>` 块要过滤**：以 `<environment_context>` 开头的 input_text 项跳过（Codex 自动注入的环境信息，不是用户输入）
- 时间戳 `Z` 后缀替换为 `+00:00` 再用 `chrono::DateTime::parse_from_rfc3339`
- session 文件按日期目录分布，先按 `updated_at` 解出年月日定位文件夹再 glob，找不到再 rglob 整个 sessions 目录（fallback）
- 没有 user 消息但有 thread_name + project_path → 用 thread_name 作为唯一 prompt（保留索引价值）

### 3.5 Memex 实现指引

Memex 已有 Codex adapter spike？暂未在 commit 里看到。Sprint 2 直接按 tars 模式新增 `crates/memex-core/src/collector/codex.rs`：

- adapter `name() = "codex"`
- 读 `session_index.jsonl` 拿到 (session_id, updated_at, thread_name) 列表
- 对每个 session_id 用 date-based glob 定位 session 文件（fallback rglob）
- 解析 session 文件，按 `type` 字段分流处理
- 提取 cwd / user 消息 / assistant 消息 / last_agent_message 作为 RawMessage 与 metadata

---

## 4. OpenCode Adapter（tars 不支持，自行调研）

tars **没有** OpenCode adapter，因此该 adapter 完全由 Memex 自己调研。Sprint 2 末尾再实现。

---

## 5. 跨 adapter 通用规则（从 tars 抽象出来）

### 5.1 sort 顺序
所有 adapter `parse_history()` 返回的 list 都按 `date desc` 排序。

### 5.2 字段截断
- `last_assistant_message` 截断到 1200 字符
- `key_prompts` 保留最近 10 条（`_MAX_PROMPTS = 10`）

> Memex 不需要 key_prompts 概念（我们存全量 chunks），但截断 last_assistant_message 是合理的。

### 5.3 时间格式
- Claude history.jsonl：**毫秒 epoch**
- Claude session JSONL：RFC3339（"...Z" 后缀）
- Cursor lastUpdatedAt / createdAt：**毫秒 epoch**
- Codex session_index updated_at：RFC3339

Memex `chrono` 处理：

```rust
// epoch ms → DateTime<Utc>
DateTime::<Utc>::from_timestamp_millis(ms)

// RFC3339 → DateTime<Utc>
DateTime::parse_from_rfc3339(&s.replace('Z', "+00:00"))
    .map(|dt| dt.with_timezone(&Utc))
```

### 5.4 Path quoting（resume_command）
v0.3 menubar 实装"在 IDE 中打开"时复制 tars 的 quoting：

```rust
fn shell_quote(path: &str) -> String {
    if path.contains(' ') || path.contains('(') || path.contains(')') {
        format!("\"{}\"", path)
    } else {
        path.to_string()
    }
}
```

### 5.5 Junk prompt 过滤
tars 在 `tars/indexer.py::_is_junk_prompt` 中过滤了一些 noise 输入（如全空白、单字符、模板默认）。Memex 暂时不引入此过滤——我们想全量入库，让用户在搜索时通过 chunk_type / metadata 过滤。

---

## 6. 待 Memex Rust 化的工作量估算

| 任务 | 工时 | Sprint |
|---|---|---|
| Claude adapter 补强（history.jsonl 二级索引 + recap/next_step/native_summary/files_touched） | 1.5 天 | Sprint 1 收尾 |
| Cursor adapter（参考本笔记，零未知） | 1 天 | Sprint 2 |
| Codex adapter（参考本笔记，零未知） | 1 天 | Sprint 2 |
| OpenCode adapter（无 tars 参考） | 1.5 天 | Sprint 2 |
| `recap` 正则解析 + path quoting helpers | 0.5 天 | Sprint 1 收尾 |

**合计：~5.5 天**，整体在 13 执行计划 Sprint 1 ~ Sprint 2 的 4 周内完成不紧张。

---

## 7. 不能直接复用的部分（需要重新设计）

| tars 设计 | Memex 不复用的原因 | Memex 替代方案 |
|---|---|---|
| `WorkContext` 单结构（含全部信号） | Memex 需要 `Session + Message + Chunk` 三层模型 | 已设计的 `models.rs` |
| `parse_history()` 一次性返回全部 session | Memex 是流式增量入库 | adapter trait 已设计 `scan + collect` 分离 |
| `_MAX_PROMPTS = 10` 截断 | Memex 全量入库做 chunks | 不截断 |
| `can_summarize()` 用 Claude API 现场摘要 | Memex 摘要在 Processor.summarize 阶段统一处理 | 留到 Sprint 6 |
| `find_session_file` global helper | Memex 模块化封装 | 放在 `crates/memex-core/src/collector/claude_code.rs` 内部 |

---

## 8. 总结

tars 已经踩了的坑全部复用：
- ✅ Cursor 真实路径（workspaceStorage 而不是 globalStorage）
- ✅ Codex 双层索引 + date-based glob fallback
- ✅ Claude away_summary recap + isCompactSummary 信号
- ✅ files_touched 通过 4 个 edit-style tool name 抽取
- ✅ activeBranch 字段可能是 string 或 dict 的兼容
- ✅ environment_context 自动注入要过滤
- ✅ 时间格式三种混用（ms epoch + RFC3339 + Z 后缀）

不能复用的：tars 的 Python 代码本身（包括正则、路径常量）需要在 Rust 重新写一遍，但可以**逐字翻译**。
