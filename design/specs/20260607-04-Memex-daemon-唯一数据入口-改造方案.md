# Memex Daemon 唯一数据入口改造方案

> 2026-06-07 · Plan 文档
>
> **关联**：
> - [20260607-01-Memex-popup替换为桌面应用-重构方案.md](./20260607-01-Memex-popup替换为桌面应用-重构方案.md)
> - [20260607-02-Memex-popup转桌面-TODO.md](./20260607-02-Memex-popup转桌面-TODO.md)
> - [20260607-03-Memex-popup转桌面-测试清单.md](./20260607-03-Memex-popup转桌面-测试清单.md)

---

## 0. 背景与决策

### 0.1 现状（事实）

| 调用方 | 是否依赖 daemon | 当前数据访问路径 |
|---|---|---|
| Tauri GUI（主窗口 + popup） | ❌ 不依赖 | 30+ Tauri command 全部 `Db::open(&path)` **直读 SQLite** |
| `memex mcp`（给 Cursor/Claude Code 用） | ❌ 不依赖 | **直读 SQLite** |
| `memex search …` CLI | ⚠️ 可选 | daemon 在就走 HTTP，不在则 fallback 直读 SQLite |
| File watcher | ✅ 必须 | daemon 独占 |
| Bootstrap ingest | ✅ 必须 | daemon 独占 |

**问题**：三套 client 同时直读 SQLite → 写入路径分散、视图不一致、watcher 与 GUI 可能竞争锁。

### 0.2 目标架构

```text
┌──────────────┐
│  Tauri GUI   │──HTTP+token─┐
└──────────────┘             │
┌──────────────┐             │     ┌─────────────────┐     ┌──────────────┐
│ memex mcp    │──HTTP+token─┼────►│     daemon      │────►│  memex.db    │
└──────────────┘             │     │ 唯一 DB 写入方    │     └──────────────┘
┌──────────────┐             │     │ + HTTP API      │
│ memex CLI    │──HTTP+token─┘     │ + file watcher  │
└──────────────┘                   └─────────────────┘
```

### 0.3 关键决策（已和用户确认）

| # | 决策 | 选择 | 理由 |
|---|---|---|---|
| **D1** | daemon 进程谁来起 | **Tauri spawn** | 无需安装期权限；卸载即清理；行为一致 |
| **D2** | crash 重启 | **Tauri 监控并重启**（指数退避，最多 3 次） | 跟 D1 配套，统一生命周期管理 |
| **D3** | HTTP API auth | **启动生成 random token，写 `~/.memex/daemon.token`** | 防御纵深；为未来 LAN 暴露留余地 |
| **D4** | API 风格 | **复刻现有 Tauri command 名** | 改造工作量最小；可一对一对照测试 |
| **D5** | 直读 DB fallback | **一刀切删除，daemon 是唯一访问方式** | 长期清晰；不留技术债 |
| **D6** | UI 入口 | 设置 → 系统 → Doctor 区域加 daemon 状态卡 + 手动重启按钮；popup footer 也保留状态点 | 用户已明确要求 |

### 0.4 非目标（明确排除）

- ❌ 不做 REST 资源重设计（D4 已定）
- ❌ 不暴露 LAN 端口（D3 token 只是为未来铺路；本次仍 localhost-only）
- ❌ 不动 36 个 Tauri command 里的**本机操作类**（CLI 安装、IDE 注册、hooks、doctor、update 等）—— 它们和 DB 无关
- ❌ 不做性能优化（HTTP 比直读 SQLite 慢几毫秒，可接受）
- ❌ MCP / CLI 端切换作为 Phase 5 独立工作，本次主线先把 Tauri 端打通

---

## 1. 受影响代码盘点

### 1.1 daemon 端（`crates/memex-daemon`）—— **需要扩展**

#### 已有 9 个 HTTP routes（`src/lib.rs`）
```text
GET  /health
GET  /search
GET  /sessions
GET  /sessions/{id}
GET  /stats
GET  /stats/breakdown
GET  /timeline
GET  /config
POST /config
GET  /summaries/stats
GET  /sessions/{id}/summary
```

#### **需新增**的 HTTP routes（覆盖 Tauri 现有 command）

| 新增 endpoint | 对应 Tauri command | 来源文件 |
|---|---|---|
| `POST /sessions/{id}/retry-summary` | `retry_summary` | `commands/sessions.rs` |
| `POST /summaries/batch` | `batch_summarize` | `commands/sessions.rs` |
| `GET  /projects` | `list_projects` | `commands/stats.rs` |
| `GET  /workload` | `get_workload` | `commands/stats.rs` |
| `GET  /reports` | `list_reports` | `commands/reports.rs` |
| `POST /reports/regenerate` | `regenerate_report` | `commands/reports.rs` |
| `GET  /reflect` | `reflect_list` | `commands/reflect.rs` |
| `GET  /reflect/{scope_key}` | `reflect_get` | `commands/reflect.rs` |
| `POST /reflect/run` | `reflect_run` | `commands/reflect.rs` |
| `GET  /llm-providers` | `llm_provider_list` | `commands/llm_providers.rs` |
| `POST /llm-providers` | `llm_provider_upsert` | `commands/llm_providers.rs` |
| `DELETE /llm-providers/{id}` | `llm_provider_delete` | `commands/llm_providers.rs` |
| `POST /llm-providers/{id}/test` | `llm_provider_test` | `commands/llm_providers.rs` |
| `POST /llm-providers/test-draft` | `llm_provider_test_draft` | `commands/llm_providers.rs` |
| `POST /llm-providers/list-models` | `llm_list_models` | `commands/llm_providers.rs` |
| `POST /llm/test-ollama` | `llm_test_ollama` | `commands/llm_test.rs` |
| `POST /ingest` | `trigger_ingest` | `commands/ingest.rs` |
| `POST /maintenance/reset-index` | `system_reset_index` | `commands/maintenance.rs` |
| `POST /maintenance/reset-all` | `system_reset_all` | `commands/maintenance.rs` |
| `POST /config/toggle-adapter` | `toggle_adapter` | `commands/config.rs` |

**新增 endpoint 总数：约 20 个**

#### 不进 daemon 的 Tauri command（本机操作类，保留直接实现）
```text
- CLI:        cli_status / cli_install / cli_uninstall
- IDE:        ide_list_status / ide_install / ide_uninstall
- Skill:      skill_list_status / skill_install / skill_uninstall
- Hook:       hook_list_status / hook_install / hook_uninstall
- Doctor:     doctor_run
- Update:     check_for_updates
- System:     get_system_username
- Daemon 管理: daemon_status / daemon_restart（必须保留，因为 daemon 自己不能管自己）
```

### 1.2 daemon 端 —— **需要新增的支持模块**

- `src/auth.rs`：token 生成、`~/.memex/daemon.token` 写入、Axum middleware 校验 `Authorization: Bearer <token>` header
- `src/routes/` 拆分（当前 `routes.rs` 已经接近 200 行，新增 20 个 endpoint 后必须拆）：
  - `routes/sessions.rs`
  - `routes/stats.rs`
  - `routes/reports.rs`
  - `routes/reflect.rs`
  - `routes/llm.rs`
  - `routes/ingest.rs`
  - `routes/maintenance.rs`
  - `routes/config.rs`

### 1.3 Tauri 端（`tauri-app/src-tauri`）—— **需要新增**

- `src/daemon_supervisor.rs` —— **新文件**
  - `spawn_daemon()`：fork `memex-daemon` 子进程，绑定 stdout/stderr 到 log file
  - `wait_for_ready()`：指数退避轮询 `/health`（200ms → 500ms → 1s → 2s → 4s，最多 8s）
  - `monitor_loop()`：tokio task，每 5s 检查 daemon 进程存活；挂了自动重启（同上限）
  - `restart()`：手动重启入口（被 `daemon_restart` command 调用）
  - `shutdown()`：app exit 时优雅关闭 daemon

- `src/daemon_client.rs` —— **新文件**
  - 读 `~/.memex/daemon.token` + `daemon.lock`
  - 包装 `reqwest::Client`，自动注入 `Authorization` header
  - `get_json<T>()`, `post_json<T, R>()`, `delete()`
  - 错误归一化（network error / 4xx / 5xx → `String`）

### 1.4 Tauri 端 —— **需要改造的 command 文件**（15 个）

```text
commands/sessions.rs       ── list_recent / get_session / retry_summary / batch_summarize
commands/stats.rs          ── get_stats / get_breakdown / get_timeline / list_projects / get_workload
commands/search.rs         ── search_memex
commands/reports.rs        ── list_reports / regenerate_report
commands/reflect.rs        ── reflect_list / reflect_get / reflect_run
commands/llm_providers.rs  ── 6 个 LLM 相关 command
commands/llm_test.rs       ── llm_test_ollama
commands/ingest.rs         ── trigger_ingest
commands/maintenance.rs    ── system_reset_index / system_reset_all
commands/config.rs         ── get_config / set_config / toggle_adapter
```

**改造模式**：
```rust
// 改造前
#[tauri::command]
pub async fn list_recent(...) -> Result<Vec<SessionRow>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() { return Ok(vec![]); }
    // ... 直读 DB ...
}

// 改造后
#[tauri::command]
pub async fn list_recent(limit: Option<usize>, offset: Option<usize>) -> Result<Vec<SessionRow>, String> {
    let client = daemon_client::get()?;
    let mut path = format!("/sessions?");
    if let Some(l) = limit { path.push_str(&format!("limit={}&", l)); }
    if let Some(o) = offset { path.push_str(&format!("offset={}", o)); }
    client.get_json(&path).await.map_err(|e| e.to_string())
}
```

**移除的 dependency**：`memex_core::storage::db::Db` 不再被 Tauri commands 直接依赖（只剩 daemon supervisor / 本机操作类 command 用）。

### 1.5 MCP / CLI 端（Phase 5 处理，不在本次主线）

- `crates/memex-mcp`：需要从直读 DB 改为走 daemon HTTP
- `crates/memex-cli/src/commands/search.rs`：移除 `run_direct` fallback，强制走 daemon
- `crates/memex-cli/src/commands/daemon_client.rs`：补充 token 读取

---

## 2. 安全模型（D3 token）

### 2.1 token 生命周期

```text
daemon 启动时：
  1. 检查 ~/.memex/daemon.token 是否存在且最近修改时间 < 24h
  2. 不满足 → 生成新 32-byte random（base64 编码）→ 写入文件（chmod 600）
  3. 满足 → 复用现有 token

daemon 退出时：
  - token 文件保留（不删）—— 下次启动如果 < 24h 内复用，避免 client 反复重读
  - 如果用户手动重启 → 复用同一 token，client 无需重新读

token 文件格式（明文）：
  ~/.memex/daemon.token
  ─────────────────────
  9f3a8b...（base64，44 chars）
```

### 2.2 Axum middleware

```rust
async fn auth_middleware(
    State(expected): State<Arc<String>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // /health 不需要 auth（用于 supervisor 探活）
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }
    let auth = req.headers().get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    match auth {
        Some(t) if t == expected.as_str() => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
```

### 2.3 client 端读 token

- Tauri / MCP / CLI 三处共用同一个工具函数（建议放到 `memex-core::auth::read_token()`）
- 文件不存在 → 错误："daemon 未启动或 token 未生成"
- 文件权限不是 600 → 警告日志但仍读（macOS 上偶尔权限被改）

---

## 3. 进程生命周期（D1 + D2）

### 3.1 启动顺序

```text
Tauri 主进程 main()
  │
  ├─► tauri::Builder::new().setup(|app| { ... })
  │     │
  │     ├─► 1. 读 daemon.lock，检查是否已有活的 daemon 进程（pid alive + /health 200）
  │     │     │
  │     │     ├─ 有活的 → 跳过 spawn，直接进入下一步
  │     │     └─ 没有  → daemon_supervisor::spawn_daemon()
  │     │                │
  │     │                ├─► 找到 memex-daemon binary 路径：
  │     │                │     - dev:    target/debug/memex-daemon
  │     │                │     - bundle: <App.app>/Contents/Resources/memex-daemon
  │     │                ├─► Command::new(binary).spawn()
  │     │                ├─► 写 daemon.lock（pid + port）
  │     │                └─► daemon 端：生成/读 token → 启 HTTP server
  │     │
  │     ├─► 2. wait_for_ready()：轮询 /health，指数退避 8s 内必须 ready
  │     │     ready 失败 → 弹窗 "后台服务启动失败，点击查看日志" + 进入 degraded 模式（GUI 启动但所有数据 page 显示错误）
  │     │
  │     ├─► 3. 启动 supervisor monitor_loop()（tokio task，每 5s 探活）
  │     │
  │     └─► 4. 正常进入 GUI 初始化
  │
  └─► Tauri main loop
```

### 3.2 关闭顺序

```text
用户 ⌘Q 或 托盘 → Quit
  │
  ├─► tauri::RunEvent::ExitRequested
  │     ├─► daemon_supervisor::shutdown()
  │     │     ├─► HTTP POST /shutdown（或 SIGTERM）
  │     │     ├─► wait 2s
  │     │     └─► 还活着 → SIGKILL + 清 daemon.lock
  │     └─► proceed exit
```

**注意**：close 主窗口（B-6 红圆点）**不触发 ExitRequested**，daemon 继续跑。

### 3.3 监控循环

```rust
async fn monitor_loop(supervisor: Arc<Supervisor>) {
    let mut consecutive_failures = 0u32;
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        if supervisor.is_alive_and_healthy().await {
            consecutive_failures = 0;
            continue;
        }
        consecutive_failures += 1;
        warn!("daemon health check failed ({} consecutive)", consecutive_failures);
        if consecutive_failures >= 2 {  // 10s 内 2 次失败才重启，避免抖动
            warn!("attempting to restart daemon");
            if let Err(e) = supervisor.restart().await {
                error!("daemon restart failed: {}", e);
                // emit Tauri event 'daemon-restart-failed' → 前端弹 toast
            }
            consecutive_failures = 0;
        }
    }
}
```

---

## 4. UI 改动（D6）

### 4.1 设置 → 系统 → daemon 状态卡（**新增**）

文件：`tauri-app/src/views/settings/components/SystemTab.vue`

布局（放在现有 CLI / Doctor 卡之间）：

```vue
<Card>
  <CardHeader>
    <CardTitle>后台服务</CardTitle>
    <CardDescription>
      Memex 后台进程负责文件监听与数据查询。所有 GUI / CLI / MCP 客户端均通过它访问。
    </CardDescription>
  </CardHeader>
  <CardContent class="space-y-3">
    <!-- 状态行 -->
    <div class="flex items-center gap-2 text-sm">
      <span :class="['status-dot', daemonRunning ? 'status-dot-ok' : 'status-dot-warn']" />
      <span>{{ daemonRunning ? '运行中' : '已停止' }}</span>
      <span v-if="daemonRunning" class="text-muted-foreground">
        · PID {{ daemon.pid }} · 端口 {{ daemon.port }} · 已运行 {{ uptime }}
      </span>
    </div>
    <!-- 上次重启时间、最近一次 ingest 时间 -->
    <div class="text-xs text-muted-foreground">…</div>
  </CardContent>
  <CardFooter class="gap-2">
    <Button variant="outline" :disabled="restarting" @click="onRestart">
      <RefreshCw :class="['mr-1.5 size-3.5', restarting && 'animate-spin']" />
      {{ restarting ? '重启中…' : '重启后台服务' }}
    </Button>
    <Button variant="ghost" size="sm" @click="onCopyLogPath">复制日志路径</Button>
  </CardFooter>
</Card>
```

调用 `daemon_status` / `daemon_restart` command。

### 4.2 popup footer daemon 点（**已存在，无需改**）

`tauri-app/src/views/tray-popup/index.vue` 中 `useDaemon()` + 状态点 + restart icon 已经实现（之前的 round 已修），用户重启 dev app 即可看到。

### 4.3 启动期 degraded 状态

GUI 端 store 加一个 `daemonReady: Ref<boolean>`：
- ready 之前所有数据 view（Today / Sessions / Reports）显示 skeleton + "正在连接后台服务…"
- ready 超时（>8s）显示 error state + "查看日志" + "手动重启" 按钮

---

## 5. Phase 拆解（生产级原则：小步、可回滚、每步可测）

### Phase 1：daemon 端基础设施（**约 1 天**）

**目标**：daemon 能 token-auth、能拆分 routes 模块、能优雅 shutdown。

任务：
1. 新建 `crates/memex-daemon/src/auth.rs`：token 生成 + middleware
2. 拆 `routes.rs` → `routes/mod.rs` + `routes/{sessions,stats,...}.rs`（先只拆，不加新）
3. 加 `POST /shutdown` endpoint（带 token，触发 graceful shutdown）
4. 单元测试：token 校验、shutdown
5. **回归**：现有 9 个 endpoint + CLI search 必须正常

**验收**：`cargo test -p memex-daemon` 全绿；`memex search …` 能用。

### Phase 2：daemon 端补全 20 个 endpoint（**约 2 天**）

**目标**：daemon 端 HTTP API 覆盖 Tauri 所有数据 command。

任务：每个 endpoint 一个 handler，复用 `memex-core` 已有逻辑（不要在 daemon 里写业务）。

**验收**：每个新 endpoint 至少 1 个集成测试（mem DB + axum::test）；覆盖率人工 checklist。

### Phase 3：Tauri 端 supervisor + client 基础设施（**约 1 天**）

任务：
1. `daemon_supervisor.rs`：spawn / wait_for_ready / monitor / shutdown / restart
2. `daemon_client.rs`：reqwest 封装 + token 读取
3. setup hook 集成：启动时 spawn daemon 并等 ready
4. exit hook 集成：退出时 shutdown daemon
5. `daemon_status` / `daemon_restart` command 改走 supervisor

**验收**：手动测试启动/关闭 GUI，进程数正确；强 kill daemon 后能自动重启。

### Phase 4：Tauri 端 15 个 command 改造（**约 2 天**）

**策略**：**逐文件改造、逐文件测试**。每改一个文件，跑一次 `npm run tauri:dev` 手动验证对应 page。

顺序（从读多写少 → 读少写多）：
1. `commands/sessions.rs`（read-only 4 个）
2. `commands/stats.rs`（read-only 5 个）
3. `commands/search.rs`（read-only 1 个）
4. `commands/reports.rs`（mixed 2 个）
5. `commands/reflect.rs`（mixed 3 个）
6. `commands/config.rs`（write 3 个）
7. `commands/llm_providers.rs`（mixed 6 个）
8. `commands/llm_test.rs`（write 1 个）
9. `commands/ingest.rs`（write 1 个）
10. `commands/maintenance.rs`（destructive 2 个）—— 最后做，因为风险最高

**验收**：
- 每改完一个文件 → 跑对应 GUI page 手动验证
- 全部改完 → `cargo test --workspace`（不能 break MCP / CLI 现有的 direct-DB 测试）
- 全部改完 → 删除 Tauri command 里的 `use memex_core::storage::db::Db;` import（CI 卡住）

### Phase 5：UI 完善 + degraded 模式（**约 1 天**）

任务：
1. 实现 4.1 的 daemon 状态卡（SystemTab.vue）
2. 实现 4.3 的 degraded 状态：store 加 `daemonReady`，所有数据 view 加 skeleton/error state
3. 启动超时弹窗
4. 复制日志路径 / 打开日志文件功能

**验收**：手动 kill daemon → GUI 显示 degraded → 自动恢复后 GUI 恢复正常。

### Phase 6：MCP / CLI 切换（**约 1 天**）

任务：
1. `memex-mcp`：直读 DB → 走 daemon HTTP（token 读 `~/.memex/daemon.token`）
2. `memex-cli/commands/search.rs`：删除 `run_direct` fallback
3. 其它 CLI 命令同样改造（如果有的话）

**验收**：daemon 没起来时 `memex search …` 应明确报错 "daemon 未运行，请先打开 Memex 应用"；MCP 同理。

### Phase 7：清理 + 端到端验收（**约 0.5 天**）

任务：
1. 删除 `tauri-app/src-tauri` 中所有不再使用的 `Db::open` 调用
2. 删除 CLI/MCP 的 `run_direct` 等 fallback 代码
3. 更新 [测试清单](./20260607-03-Memex-popup转桌面-测试清单.md) 加入 daemon 相关测试 case
4. 更新 README 架构图

**验收**：`rg "memex_core::storage::db::Db" tauri-app/src-tauri/src/commands` 应为空（除了不涉及 DB 的本机操作类 command）。

---

## 6. 总工作量

| Phase | 工作量 |
|---|---|
| 1. daemon 基建（auth + 拆 routes + shutdown） | 1 天 |
| 2. daemon 补 20 endpoints | 2 天 |
| 3. Tauri supervisor + client | 1 天 |
| 4. Tauri 15 command 改造 | 2 天 |
| 5. UI 完善 + degraded | 1 天 |
| 6. MCP / CLI 切换 | 1 天 |
| 7. 清理 + 验收 | 0.5 天 |
| **合计** | **8.5 天** |

---

## 7. 风险登记

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| daemon 启动慢导致首屏白屏 | 高 | 中 | wait_for_ready 指数退避；degraded UI |
| daemon crash 后无法重启（端口被占、binary 找不到） | 中 | 高 | supervisor 最多重启 3 次后弹错；手动重启按钮兜底 |
| HTTP 比直读 SQLite 慢 → 列表页 perceivable lag | 高 | 低 | 加 client 端短期缓存（5s TTL）；daemon 端连接池 |
| 旧的 daemon.lock 文件残留导致 spawn 跳过 | 中 | 中 | startup 时验 pid + /health，不仅看 lock 文件存在 |
| token 文件被其他程序读走 | 低 | 中 | chmod 600；macOS 文件系统沙箱（其他应用默认无权访问 ~/.memex） |
| 修改 36 个 command 引入回归 | 高 | 高 | Phase 4 逐文件改、逐文件验；保留 git commit 粒度 |
| MCP / CLI 切换后老用户报错"daemon 未运行" | 中 | 中 | 错误消息明确 + 文档更新 |

---

## 8. 测试清单（追加到 20260607-03-Memex-popup转桌面-测试清单.md）

### 8.1 daemon 生命周期
- [ ] 全新启动：app 启 → daemon 自动起 → 30s 内 GUI 可查数据
- [ ] 强 kill：`kill -9 <daemon pid>` → 10-15s 内 supervisor 自动重启 → GUI 恢复
- [ ] 设置页手动重启：点"重启后台服务" → daemon 重启 → 重启期 GUI degraded → 恢复
- [ ] 关闭主窗口（红圆点）：daemon 仍在跑（验证 `ps aux | grep memex-daemon`）
- [ ] ⌘Q：daemon 同时退出（验证 `ps aux | grep memex-daemon` 为空、daemon.lock 被清）

### 8.2 token auth
- [ ] 无 token 直接 `curl http://127.0.0.1:<port>/sessions` → 401
- [ ] 错 token → 401
- [ ] 对 token → 200
- [ ] `/health` 无 token 也能访问 → 200

### 8.3 异常路径
- [ ] daemon binary 不存在 → setup 弹窗 + degraded 模式（不应崩溃）
- [ ] daemon 端口被占 → daemon 自己换端口 + 更新 lock → supervisor 重新探测
- [ ] daemon 启动超时（>8s）→ degraded UI + "查看日志" 按钮可点

### 8.4 数据正确性
- [ ] Today / Sessions / Reports / Reflect / LLM Providers / Settings 所有 page 数据和切换前一致
- [ ] 触发摘要 / 重置索引 / 重置全部 → 行为和切换前一致

---

## 9. 待用户最终确认

在动 Phase 1 之前请确认：

- [ ] Phase 拆解和顺序认同？
- [ ] 8.5 天工作量预估在你接受范围？
- [ ] 风险 #6（Phase 4 引入回归）是否需要更保守的策略（例如保留 fallback 半年）？
- [ ] daemon binary 在 macOS bundle 里的打包路径是否需要现在就讨论？（涉及 tauri.conf.json 的 resources）
