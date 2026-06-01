# Memex Web UI — Popup 内嵌方案设计

> 日期：2026-06-01
> 状态：设计中
> 背景：参考 tars-ai-butler 的 `tars web` 实现，为 Memex Tauri menubar popup 增加 Web UI 内嵌能力。

---

## 1. 现状分析

### 1.1 TARS Web UI 实现总结

| 层面 | TARS 做法 |
|------|-----------|
| 后端 | Python stdlib `http.server`，零依赖 |
| 前端 | 单 `index.html`（~1000行），vanilla JS + 内联 CSS |
| 架构 | 同一端口 serve 静态文件 + JSON API（SPA fallback） |
| 生命周期 | `tars web` 前台 / `--daemon` 后台 / LaunchAgent 自启动 |
| UI 能力 | Dashboard / Sessions / Projects / Report / Focus 五个视图 |

### 1.2 Memex 现有架构

| 组件 | 说明 |
|------|------|
| `memex-daemon` (axum) | 已有 HTTP API: `/health` `/search` `/sessions` `/sessions/{id}` `/stats` `/config` |
| Tauri menubar | Vue 3 + shadcn-vue，4 个视图：Home / Search / Session / Settings |
| 端口 | daemon 监听 `127.0.0.1:9999` |
| 前端复用 | Tauri IPC → composables → 已有 UI 组件 |

### 1.3 差距与机会

- Memex daemon 已有完整 API，**不需要像 TARS 那样额外写 HTTP server**
- 只需在 daemon 增加静态文件 serve + 少量补充 API
- Tauri popup 可以直接加载 `http://127.0.0.1:9999/` 作为 WebView（内嵌 iframe 或 window）

---

## 2. 设计目标

1. **Zero Extra Dependency**：复用已有 daemon + axum，不引入新进程
2. **Popup 内嵌**：Tauri menubar popup 中新增 "Web Dashboard" 入口，打开内嵌 WebView
3. **独立可访问**：浏览器直接打开 `http://127.0.0.1:9999/` 也能使用（daemon 运行时）
4. **渐进增强**：初期单 HTML 文件（TARS 风格），后续可升级为 Vite 构建产物

---

## 3. 方案概览

```
┌─────────────────────────────────────────────────────┐
│                  Tauri Menubar                        │
│  ┌─────────────────────────────────────────────────┐ │
│  │  Popup Window (320×480, existing Vue app)       │ │
│  │  ┌───┐ ┌────────┐ ┌────────┐ ┌────────────┐   │ │
│  │  │ 🏠│ │ Search │ │Settings│ │ Dashboard↗ │   │ │
│  │  └───┘ └────────┘ └────────┘ └────────────┘   │ │
│  └─────────────────────────────────────────────────┘ │
│                        │ click "Dashboard"            │
│                        ▼                             │
│  ┌─────────────────────────────────────────────────┐ │
│  │  New Window (800×600, WebView)                  │ │
│  │  src = http://127.0.0.1:9999/                   │ │
│  │  ┌─ Sidebar ──┐ ┌─ Main ────────────────────┐  │ │
│  │  │ Dashboard  │ │ Sessions / Search / Stats  │  │ │
│  │  │ Sessions   │ │                            │  │ │
│  │  │ Projects   │ │                            │  │ │
│  │  │ Timeline   │ │                            │  │ │
│  │  └────────────┘ └────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘

Browser 也可直接访问 http://127.0.0.1:9999/
```

---

## 4. 实现步骤

### Phase A：Daemon 端增加静态文件 Serve

**改动文件**：`crates/memex-daemon/src/lib.rs` + `crates/memex-daemon/src/routes.rs`

1. 在 `memex-daemon` 增加 `tower-http` 的 `ServeDir` fallback（已有 `tower-http` 依赖）
2. 静态资源目录：`~/.memex/web/` 或编译时 `include_str!` 嵌入单个 HTML
3. 路由优先级：API routes 优先 → 静态文件 fallback → index.html（SPA）

```rust
// lib.rs 中 build_router 改动示意
pub fn build_router(db: Arc<Db>) -> Router {
    let api = Router::new()
        .route("/health", get(routes::health))
        .route("/search", get(routes::search))
        .route("/sessions", get(routes::list_sessions))
        .route("/sessions/{id}", get(routes::get_session))
        .route("/stats", get(routes::stats))
        .route("/config", get(routes::get_config).post(routes::set_config))
        .with_state(db);

    let static_service = ServeDir::new(web_static_dir())
        .not_found_service(ServeFile::new(web_static_dir().join("index.html")));

    api.fallback_service(static_service)
}
```

### Phase B：Web UI 前端（单 HTML，TARS 风格）

**新增文件**：`web-ui/index.html`（开发时用，编译时嵌入或部署到 `~/.memex/web/`）

初期功能：
- Dashboard（sessions 数/messages 数/chunks 数 + 每日活动柱状图）
- Sessions 列表（搜索 + 过滤器 + 分页）
- Session 详情（messages + metadata）
- 统计图表（adapter 分布饼图 + 时间线）

API 对接：
| Web UI 功能 | 已有 API | 需新增 |
|-------------|----------|--------|
| Dashboard stats | `GET /stats` | ✓ 已有 |
| Sessions 列表 | `GET /sessions?limit=N` | 需补充分页 + 过滤参数 |
| Session 详情 | `GET /sessions/{id}` | ✓ 已有 |
| 搜索 | `GET /search?q=...` | ✓ 已有 |
| Timeline 每日统计 | — | **新增** `GET /timeline?days=30` |
| 按 adapter 分组统计 | — | **新增** `GET /stats/breakdown` |

### Phase C：Tauri Popup 内嵌入口

**改动文件**：`tauri-app/src/views/HomeView.vue` + `tauri-app/src-tauri/src/lib.rs`

两种方案选择：

| 方案 | 实现 | 优点 | 缺点 |
|------|------|------|------|
| A: 新 Tauri Window | `WebviewWindow::builder(...).url("http://127.0.0.1:9999/")` | 独立窗口，全屏体验好 | 多窗口管理 |
| B: iframe 嵌入 popup | `<iframe src="http://127.0.0.1:9999/" />` | 无额外窗口 | popup 尺寸有限 |

**推荐方案 A**：在 HomeView 添加 "Open Dashboard" 按钮，点击后通过 Tauri API 创建新窗口：

```typescript
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

async function openDashboard() {
  const win = new WebviewWindow('dashboard', {
    url: 'http://127.0.0.1:9999/',
    title: 'Memex Dashboard',
    width: 900,
    height: 650,
    resizable: true,
    center: true,
  })
}
```

---

## 5. 需补充的 Daemon API

### 5.1 `GET /sessions` 增强（分页 + 过滤）

```
GET /sessions?limit=30&offset=0&adapter=cursor&project=memex&q=auth
```

响应保持向后兼容，新增 `total` 字段。

### 5.2 `GET /timeline`（新增）

```
GET /timeline?days=30
```

```json
{
  "timeline": [
    { "date": "2026-05-31", "sessions": 5, "messages": 120, "by_adapter": { "cursor": 3, "claude-code": 2 } },
    ...
  ]
}
```

### 5.3 `GET /stats/breakdown`（新增）

```json
{
  "by_adapter": { "cursor": 150, "claude-code": 80, "codex": 12 },
  "by_project": { "memex": 50, "ai-hub": 30, ... },
  "recent_7d": { "sessions": 42, "messages": 1200 },
  "recent_30d": { "sessions": 180, "messages": 5000 }
}
```

---

## 6. 文件组织

```
memex/
├── crates/
│   ├── memex-daemon/
│   │   ├── src/
│   │   │   ├── lib.rs          # 添加 static fallback
│   │   │   ├── routes.rs       # 添加 timeline / breakdown 路由
│   │   │   └── web.rs          # (新) 内嵌 HTML 资源
│   │   └── Cargo.toml          # tower-http 添加 "fs" feature
│   └── ...
├── web-ui/
│   └── index.html              # 单文件 Web Dashboard
└── tauri-app/
    └── src/views/HomeView.vue  # 添加 "Dashboard" 按钮
```

---

## 7. 与 TARS 的差异

| 对比 | TARS | Memex |
|------|------|-------|
| HTTP 框架 | Python stdlib | Rust axum（已有） |
| 前端框架 | 纯 vanilla JS | 同样单 HTML vanilla（初期），后续可升级 Vite |
| 额外进程 | `tars web` 独立进程 | 复用 `memex daemon`，**零额外进程** |
| UI 打开方式 | 浏览器 | Tauri 新窗口 + 浏览器 fallback |
| Daemon 化 | 独立 LaunchAgent | 复用 memex daemon LaunchAgent |
| 数据缓存 | Python TTL cache | SQLite 直查（axum async） |

---

## 8. 开发优先级

1. **P0**：daemon 添加 static serve + index.html fallback（~30min）
2. **P1**：编写 `web-ui/index.html` Dashboard + Sessions 视图（~2h，参照 TARS HTML）
3. **P2**：补充 `/timeline` + `/stats/breakdown` API（~1h）
4. **P3**：Tauri HomeView 添加 "Open Dashboard" 按钮（~15min）
5. **P4**：Sessions 分页 + 过滤参数增强（~1h）

**总估时**：~5h，可在 1 个 Sprint 内完成。

---

## 9. 安全考量

- HTTP 仅绑定 `127.0.0.1`，不对外暴露
- CORS 头已通过 `tower-http` 限制（现有配置）
- Web UI 不写入任何数据（只读展示）
- private session 在 API 层已过滤，Web UI 不会泄露

---

## 10. 后续演进

- v1: 单 HTML 内联 CSS/JS（本设计）
- v2: Vite 构建 + 生产优化（代码分割、gzip）
- v3: 与 Tauri popup 共用 Vue 组件库（统一设计语言）
- v4: Web UI 也支持 dark/light 跟随系统
