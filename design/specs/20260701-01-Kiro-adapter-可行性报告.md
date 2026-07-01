# Kiro Adapter 可行性报告

> 日期：2026-07-01 · 状态：调研完成，建议实施 · 预估工作量：≤ 1 人日

## TL;DR

**结论：可行，且成本极低。** Kiro 是基于 Continue.dev fork 出来的 IDE，会话持久化
结构和 `ContinueAdapter` 几乎一致，最快路径是复制 `continue_dev.rs` 改 3 处路径逻辑即可
接入。**推荐直接加一个 `KiroAdapter`，不要复用/污染 `ContinueAdapter`**（两者路径规则
不同，且未来会独立演化）。

## 一、本地实证

安装：`/Applications/Kiro.app`（版本 0.12，DMG 在 `~/Downloads/kiro-ide-0.12.333-stable-darwin-arm64.dmg`）
数据根：`~/Library/Application Support/Kiro/`

关键目录：

```
~/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/
├── config.json
├── profile.json
├── dev_data/
└── workspace-sessions/
    └── <base64url(workspace_abs_path)>/
        ├── sessions.json               # 索引（该 workspace 全部 session）
        ├── <uuid>.json                 # 每条 session 的完整历史
        └── ...
```

workspace key 是 workspace 绝对路径的 **base64url** 编码（`+/=` → `-_` padding 用 `_`）：

```
L1VzZXJzL1RlcnJlbGxTaGUvRG9jdW1lbnRzL3Byb2plY3RzL2RhdGEtZ292ZXJuYW5jZS1tZXRhZGF0YQ__
  ↓ base64url decode
/Users/TerrellShe/Documents/projects/data-governance-metadata
```

### sessions.json（索引）

```json
[
  {
    "sessionId": "706c865f-ca7b-4e33-a0bd-d131b322cdcf",
    "title": "帮我基于这个分支的 telemetry enum ...",
    "dateCreated": "1782375182036",
    "workspaceDirectory": "/Users/TerrellShe/Documents/projects/data-governance-metadata"
  }
]
```

- `dateCreated` 是 **毫秒** epoch 字符串（不是 Continue 的秒 / ISO）。
- 每个 workspace 目录一份 `sessions.json`，不同 workspace 会话互不重叠。

### `<uuid>.json`（session 详情）

```json
{
  "sessionId": "...",
  "title": "...",
  "workspacePath": "/Users/.../data-governance-metadata",
  "history": [
    { "message": { "role": "user",      "content": [{"type":"text","text":"..."}], "id": "..." } },
    { "message": { "role": "assistant", "content": "On it.",                        "id": "..." }, "promptLogs": [...] }
  ],
  "config": { "models": [...], "contextProviders": [...] },
  "sessionType": "...",
  "autonomyMode": "..."
}
```

`content` 形状与 Continue 完全一致：

- `user` → `[{type:"text", text:"..."}, ...]`（数组，允许多段）
- `assistant` → 通常是纯 `string`；工具调用/上下文丢在同层的 `promptLogs` / `contextItems` 里

## 二、与现有 `ContinueAdapter` 的差异

| 维度 | Continue | Kiro |
|---|---|---|
| 基础目录 | `~/.continue/sessions/` | `~/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/workspace-sessions/` |
| 目录结构 | 扁平，一个 `sessions.json` + 若干 `<id>.json` | **按 workspace 分目录**，每个 workspace 一份 `sessions.json` |
| workspaceDirectory | `file://...` URI | 纯路径（无 `file://`） |
| dateCreated | 秒 / ISO | **毫秒 epoch string** |
| session 文件 | 同目录 `<id>.json` | 同 workspace 目录内 `<id>.json` |
| history 结构 | 同 | 同 ✅（可复用 `HistoryItem` / `ContinueMessage` / `extract_text`） |

**结论：解析器（`collect`）100% 复用，扫描器（`scan`）需要多一层"遍历 workspace 目录"。**

## 三、实施方案

新建 `app/crates/memex-core/src/collector/kiro.rs`，克隆 `continue_dev.rs` 主体，改动
如下（伪代码）：

```rust
pub struct KiroAdapter { base_dir: PathBuf }

impl KiroAdapter {
    pub fn new() -> Self {
        let base = dirs::home_dir().expect("...")
            // macOS 唯一发行渠道；Linux/Windows 上 Kiro 目前不提供
            .join("Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/workspace-sessions");
        Self { base_dir: base }
    }
}

impl Adapter for KiroAdapter {
    fn name(&self) -> &str { "kiro" }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        if !self.base_dir.exists() { return Ok(vec![]); }
        let mut out = vec![];
        for entry in fs::read_dir(&self.base_dir)? {          // 遍历 base64 workspace 目录
            let ws_dir = entry?.path();
            if !ws_dir.is_dir() { continue; }
            let idx = ws_dir.join("sessions.json");
            if !idx.exists() { continue; }
            let list: Vec<SessionIndex> = parse_or_skip(&idx);
            for e in list {
                let file = ws_dir.join(format!("{}.json", e.session_id));
                if !file.exists() { continue; }
                out.push(SessionMeta {
                    id: e.session_id,
                    source: "kiro".into(),
                    project_path: e.workspace_directory,        // 已经是纯路径
                    file_path: file.to_string_lossy().into(),
                    // dateCreated 是 ms epoch string → 转秒
                    created_secs: parse_ms_to_secs(&e.date_created),
                    mtime: mtime_of(&file),
                    last_offset: 0,
                    title: e.title,
                });
            }
        }
        Ok(out)
    }

    // collect() 直接调用 continue_dev.rs 中已有的 parse 逻辑，
    // 或者把 SessionFile/HistoryItem/extract_text 抽到 collector::mod 或独立小模块
}
```

### 需要抽公共代码吗？

**不着急抽。** 目前只有 2 个 adapter 用这个 schema，先复制粘贴（Rust 里 ~50 行）。
等到出现第 3 个 continue-style fork 再抽 `continue_schema.rs`。这是典型 rule-of-three。

## 四、需要改的其他位置

1. `collector/mod.rs`：`mod kiro;`、`all_adapters()`、`enabled_adapters()` 加入 Kiro
2. `config/types.rs`：`AdaptersConfig` 增加 `pub kiro: bool`
3. `config/io.rs::detect_installed_adapters`：
   ```rust
   let kiro = home
       .join("Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/workspace-sessions")
       .exists();
   ```
4. `commands/config.rs`：`"adapters.kiro"` 分支
5. `config/tests.rs`：补默认值测试
6. `setup <ide>` 命令（如有）：加 `kiro` 分支 —— Kiro 内置 MCP 支持（`~/Library/Application Support/Kiro/logs/.../Kiro - MCP Logs.log` 可证），写入 `~/.kiro/settings/mcp.json` 或 Kiro 的 MCP 配置文件即可（**需再花 5 分钟看下 Kiro 的 MCP 配置路径**，本次未确认写入位置）
7. 桌面端 sidebar 里的 IDE 图标 / 开关（如有硬编码列表）

## 五、风险与已知未确认项

| 项 | 状态 |
|---|---|
| Kiro MCP 配置文件的确切路径与格式（用来做 `memex setup kiro`） | **未确认**，需再看 Kiro 设置 UI 或官方文档 |
| Linux / Windows 上 Kiro 的数据路径 | **未确认**（当前只测了 macOS）。可先只在 macOS 检测 `kiro`，其他平台默认 `false`，与 `cursor`/`cline` 现有做法一致 |
| Kiro 是否会在长会话下切换文件（分片） | 未见 —— 本地最大 31KB，全在单文件内 |
| Kiro `assistant.content` 是否可能变成数组带 tool_use（未来版本） | 复用 `extract_text` 已经能容错，最多丢 tool_use 文本，不会崩 |

## 六、测试计划

- 单测：仿 `test_parse_continue_session` 写 `test_parse_kiro_session`，覆盖：
  - workspace 目录 base64 命名 + 内嵌 `sessions.json` + `<uuid>.json` 全流程
  - `dateCreated` ms 字符串转秒正确
  - `workspaceDirectory` 无 `file://` 前缀
- 集成：本地 `cargo run -p memex-cli -- ingest` 后跑 `stats`，确认 kiro sessions 入库、
  `search_memory query=... adapter=kiro` 命中。

## 七、结论

- 数据结构上 Kiro ≈ Continue，接入几乎零风险
- 建议独立 `KiroAdapter`，不复用 `ContinueAdapter` 类型
- MVP 范围：adapter + config detect + 单测。`memex setup kiro` MCP 写入放二期
- 预估：核心代码 + 测试 半天；`setup kiro` MCP 写入再半天

`ponytail:` 采用最小可行方案（复制 Continue adapter + 一层目录遍历），
不抽公共 schema、不上跨平台探测，等有实证再扩展。
