# 20260607-04 · Memex daemon 架构决策

> 决策时间：2026-06-07  
> 范围：daemon 是否保留、谁来起、谁负责重启、API 风格、auth、是否一刀切去掉 direct-DB fallback  
> 状态：草案，待 Implement

---

## 背景

用户在 popup 重构后期反复出现"daemon 卡片消失/重启没用/不知道有没有跑"的体验问题，提出了根本性疑问：

> "daemon 这个服务是否有必要呢？"

并给出 D1-D5 五个决策点，希望我"哪种好用就用哪种"。

本文回答这个根本问题，以及 D1-D5 的具体选择。

---

## 0. daemon 是否有必要？

### 现状（重新梳理）

memex 当前有 **两条独立的数据访问路径**：

| 客户端 | 路径 | 启动方式 |
|---|---|---|
| **menubar app（Tauri）** | 直接 `Db::open()` 打开 sqlite | 用户启动 app 时 |
| **memex CLI**（`memex search ...`） | HTTP GET :9999/search | 终端命令 |
| **memex-daemon**（HTTP server） | sqlite + 文件 watcher + bootstrap ingest | Tauri 端 `Command::spawn` |

daemon 的**真正职责**只有两件：

1. **文件 watcher**：监听 `~/.claude/projects/`、`~/.cursor/projects/` 等目录的 `.jsonl/.json` 变更，触发增量 ingest。这是 sqlite 拿不了的能力。
2. **后台 bootstrap ingest**：app 启动后异步跑一次全量扫描，主要给 Cursor sqlite-KV（`state.vscdb`）兜底（watcher 抓不到 sqlite 文件变化）。

HTTP routes（search/sessions/stats/...）目前**只服务 CLI 客户端**——Tauri 端从来不调它，全部走直连 sqlite。

### 三种架构选项

#### 方案 A：保留 daemon（独立长进程）

- daemon 永远在跑（即使 menubar app 退出，可选）
- watcher + ingest 在 daemon 里
- Tauri 端：ui + ipc-command（直读 sqlite）
- CLI 端：HTTP 调 daemon

**优点**：menubar 关掉后也能继续采集；CLI / 第三方集成有标准 HTTP 入口；watcher 进程独立，不阻塞 UI 线程。

**缺点**：进程治理复杂（谁起、谁监控、port 冲突、auth）；用户感知到"两个进程"对小工具来说重；HTTP/sqlite 双数据路径容易写出 bug（实际上 toggle adapter 就出过此类 bug）。

#### 方案 B：去 daemon，全部走 Tauri

- 把 watcher + bootstrap ingest 内嵌到 Tauri 主进程的后台 task
- HTTP server 也搬到 Tauri 主进程内（仍听 :9999）
- Tauri 端 IPC 直接调内嵌的同步函数
- CLI 端：要么走 HTTP（前提 menubar 在跑），要么也直读 sqlite

**优点**：单进程；少一层；用户看到的就是"打开/关闭 Memex"一件事。

**缺点**：menubar 退出后停止采集（违背"AI 编辑器历史不丢"的核心承诺）；CLI 在 menubar 没开时彻底失能；"关掉主窗口 daemon 仍跑"这个用户已经熟悉的承诺破坏。

#### 方案 C：daemon 是唯一访问方式

- Tauri 端**不再直读** sqlite，所有数据请求都走 HTTP :9999
- daemon 是唯一的"真理之源"
- CLI 端继续走 HTTP

**优点**：架构最干净，单写者（daemon）单读者（多客户端）；写操作天然串行化；删除 Tauri 端 ~9 个 commands 文件的直读代码。

**缺点**：每次 IPC 多一层 HTTP 往返延迟（~2ms）；daemon 必须保证可达，否则 UI 全瘫痪；现有 9 个 commands 文件全部要重写。

### 决策：**方案 A（保留 daemon）+ 长期向方案 C 迁移**

**理由**：

1. **方案 B 不可接受** —— "menubar 关了仍采集"是用户在 prompt 里反复确认的核心需求（"主窗口隐藏（不退出，daemon 仍跑）"），同时是和"AI 编辑器历史长期保存"宣传一致的产品承诺。
2. **方案 C 是终局**，但 ROI 偏低 —— 现在 9 个 Tauri commands 改成 HTTP 调用，约 800 行代码改写 + 测试重写，不是这一期能/应该做的。
3. **方案 A 当前就是事实**，问题只是"用户感知不到 daemon、找不到状态"——这个是 UI 问题，不是架构问题。已经在前一轮 popup 改造中把 daemon hero card 加回来了。

**用户原始问题"daemon 是否有必要"** = "**有必要，daemon 是 menubar 关闭后仍能采集的唯一方式；当前问题不是架构问题，是 UI 让用户没看到 daemon 在跑**"。

---

## 1. D1 决策：daemon 谁起？

候选：
- (a) Tauri 启动时 `Command::spawn` 拉起
- (b) 安装包注册成 launchd agent 开机自启
- (c) 都支持，配置可选

**用户偏好**：保留一个，哪种好用哪种。

### 选择：(a) Tauri 启动时 spawn

理由：
1. **现状已经是 (a)** —— `tauri-app/src-tauri/src/commands/daemon.rs::daemon_restart` 已实现 `find_daemon_binary` + `Command::spawn`，且 `lib.rs::setup` 启动时自动调用一次（前面 612654.txt 日志里能看到 `daemon already running pid=39844 port=9999, skip auto-start`）。
2. **launchd agent 的副作用太重**：用户从来没装过 plist，但要想运行 launchd 得让安装包在用户首次安装时申请权限、写 `~/Library/LaunchAgents/com.memex.daemon.plist`、`launchctl load`。对一个本地开发者工具，这种"开机自启"是过度设计——用户**没打开 menubar 也就用不到 memex**，没必要让 daemon 永远在跑。
3. **(c) 配置可选** = 维护两套代码 = 用户两套问题。
4. 现成的 `crates/memex-daemon/src/launchd.rs` 模块**保留**（CLI `memex daemon install-plist` 仍可用），但 menubar app 不依赖它。

### 行动

- 删除"先用 a，再加 b"的中间态预设；明确文档说"安装 plist 是高级用户可选项，menubar 启动时会拉起 daemon"。
- 保留 `memex daemon install-plist` 命令（CLI 高级用户可手动用）。

---

## 2. D2 决策：daemon crash 后谁负责重启？

候选：
- (a) Tauri 监控并重启
- (b) launchd 监控（`KeepAlive=true`）
- (c) Tauri 端按需重连，不自动 spawn

**用户偏好**：哪种好用哪种。

### 选择：(c) 按需重连 + 用户手动重启

理由：

1. **(a) 实现复杂收益小**：要在 Tauri 里加一个 watchdog 线程定时探活，crash 时 spawn。这个 watchdog 本身又会 crash（极端情况 OOM 等），最终用户还是得手动管。
2. **(b) 我们刚拒了 launchd**（D1），所以也不能用它来做 KeepAlive。
3. **(c) 当前实现就是这个**：
   - `daemon_status` IPC 检查 lock + `kill -0` + `curl /health`，UI 显示状态点；
   - 用户在 popup 看到红色 → 手动点"启动"按钮；
   - 没用户的时候 daemon 真的 crash 了**也无所谓**，下次用户打开 menubar 主进程会重新探测，不健康就 spawn。

### 真实场景

- daemon crash 概率极低（核心代码就是 axum + sqlite，跑了好几个月没听说过 crash）
- crash 一般是 sqlite 写锁竞争 / disk full 等系统级问题，自动重启也救不回来
- 用户感知到 "数据没更新" 时，UI 已经显示了红色状态，他点"重启"即可

### 行动

- 保持当前 `daemon_status` 心跳（5s）+ `daemon_restart` 手动重启的设计
- popup hero card 已加回（前一轮），主窗口 Settings 里也有同样的 daemon 状态 card（`SystemStatusCard.vue` 在 Today 页里也展示）
- **不增加** auto-restart 逻辑

---

## 3. D3 决策：daemon HTTP API 是否要 auth？

**用户选择**：(b) 启动时生成 random token，写入 `~/.memex/daemon.token`，client 读取。

### 现状

daemon HTTP server 当前**完全裸奔** —— 任何能访问 `127.0.0.1:9999` 的本地进程都能读所有会话内容。

风险：
- 同机其他用户（macOS 多用户机器）能读你的私聊数据
- 浏览器里 `fetch('http://127.0.0.1:9999/sessions')` 可绕同源策略（取决于 CORS）
- macOS 沙盒进程也能命中 localhost loopback

虽然概率不高，但既然用户主动选了 (b)，就实施。

### 实施细节

| 项 | 设计 |
|---|---|
| **token 文件** | `~/.memex/daemon.token`（mode 0600，只有 user 可读） |
| **生成时机** | daemon 启动时，如果文件不存在 → 生成 32 字节随机 hex；否则读现有 |
| **token 格式** | 32 字节 hex（256 bit），用 `rand` crate |
| **传输方式** | HTTP `Authorization: Bearer <token>` header |
| **client 读取** | Tauri / CLI 都从 `~/.memex/daemon.token` 读，启动时一次，缓存 |
| **失败行为** | 401 时清缓存重读 token 文件再重试一次；仍失败提示用户"daemon token 不一致，请重启 daemon" |
| **Tauri 端的兼容** | 当前 Tauri 不调 daemon HTTP，所以这一步主要影响 CLI 和未来的 MCP/Hook 集成 |

### 行动

- 新增 `crates/memex-daemon/src/auth.rs` 模块：`load_or_generate_token(memex_dir) -> String`
- daemon `lib.rs::run` 启动时调用，写到 `~/.memex/daemon.token`，附 0600 权限
- axum middleware：检查 `Authorization: Bearer <token>`，不匹配 401
- `health` endpoint **不需要 auth**（`daemon_status` 心跳要能通）—— 加个白名单
- CLI 客户端（`crates/memex-cli`）发请求时读 token 加 header
- 文档：`design/...token.md` 说明 token 文件、轮换策略

### 不在本期做的

- token 轮换（每次 daemon 重启就换新 token，client 自动跟随，无需手动轮换）
- 多用户支持（mode 0600 已经隔离了）

---

## 4. D4 决策：API 风格

候选：
- (a) 复刻现有 Tauri command 名（`list_recent → GET /sessions/recent`）
- (b) 重新设计 REST 资源

**用户偏好**：哪种合适哪种。

### 选择：(b) REST 资源风格

理由：

1. **当前 daemon 已经是 REST 风格**（`/sessions`、`/sessions/{id}`、`/stats`、`/timeline`、`/config`），不需要重新设计
2. Tauri commands 名字（`list_recent`、`get_breakdown`、`trigger_ingest`）是 RPC 风格，不适合 HTTP API
3. CLI 用 HTTP 时 REST 更直观（`memex search foo` → `GET /search?q=foo`）

### 行动

- **保持现有 `crates/memex-daemon/src/routes.rs` REST 路由不变**
- 后续要新增 endpoint 时按资源风格命名（`/reflect/runs`、`/llm-providers/{id}/test`）
- Tauri commands 命名也保持 RPC 风格（不要为了对齐 HTTP 而重命名 Tauri commands）

---

## 5. D5 决策：是否在本次改造里保留 direct-DB fallback？

**用户选择**：一刀切，daemon 是唯一访问方式。

### 解读

"一刀切"在用户的语境里是**目标终态**，不是这一期就要做完的事。让我重新理解：

| 解读 1：本期就要做 | 解读 2：长期方向 |
|---|---|
| 立刻删掉 Tauri 9 个 commands 文件的 sqlite 直读代码，全改 HTTP 调 daemon | 标记 direct-DB fallback 为 deprecated，新功能强制走 daemon HTTP；旧代码下一个版本删 |

这两种解读 ROI 差异巨大。我推荐**解读 2**，理由：

1. 当前 9 个 commands 文件 ~800 行代码，全部改成 HTTP fetch + 错误处理 + token 读取，**至少 3 天工作量**（含测试）
2. 改完之后所有 IPC 加 ~2ms 延迟（HTTP loopback 比直读 sqlite 慢一个量级）
3. 改完之后**功能等价 = 不影响用户体验** —— 用户不会感知到这次重构
4. 有更高优先级的 bug 在排队（B-K 章节的手动测试还没跑完）

但既然用户明确说"一刀切"，我们至少要：

### 行动（本期）

- ✅ **保留 daemon 是唯一写者**（已经是了，watcher / bootstrap ingest 都只在 daemon）
- ✅ **不要**把 watcher / ingest 也搬到 Tauri 主进程（明确拒绝方案 B）
- ⚠️ Tauri 端的 `Db::open` 直读保留，但加注释说明这是"读优化路径"，不是架构上的双数据源
- ⚠️ **不再新增** Tauri command 直接写 sqlite 的代码 —— 写操作必须经 daemon HTTP（本期之前的 toggle_adapter / config 等已经在写，先不动；下次新增时强制走 daemon）
- 📝 在 `crates/memex-core/src/storage/db.rs` 顶部加注释，说明"直读 sqlite 仅供 Tauri 同进程的读优化使用，所有写操作走 daemon HTTP"

### 行动（长期，不在本期）

- 新增 epic：把 9 个 Tauri commands 全部改为 HTTP 调 daemon
- 删除 Tauri 端的 `Db::open` 调用，仅保留 daemon 一个

---

## 6. 设置中手动重启 daemon

**用户原始话**："然后在设置中可以手动重启 daemon"

### 现状

- ✅ **Popup**：本期已加 daemon hero card，含"重启"按钮（前一轮提交）
- ⚠️ **主窗口 Settings**：当前 `SystemTab.vue` 只有"Doctor 系统检查"和"CLI 工具"两个 card，没有 daemon 重启入口
- ⚠️ **主窗口 Today**：`SystemStatusCard.vue` 显示 daemon pid 但没有重启按钮

### 行动

在 `SystemTab.vue`（Settings → 系统）新增第一个 card：

- 标题："后台服务"
- 内容：daemon 状态（绿/橙）+ pid + port + uptime
- 按钮："重启 / 启动" + "查看日志"（点击 reveal `~/.memex/daemon.stdout.log`）
- 数据来源：复用现有 `useDaemon` composable + `daemon_restart` IPC

放在 SystemTab 第一个 card 位置（CLI 工具上方），最显眼。

---

## 7. 实施分期

| 阶段 | 任务 | 预计工时 |
|---|---|---|
| **Phase 1：本期立即做** | 1. SystemTab 新增 daemon 状态 card（带重启按钮）<br>2. crates/memex-core/src/storage/db.rs 顶部加架构注释 | 1h |
| **Phase 2：本期次优先** | 1. daemon auth：加 token 生成 + middleware + CLI 适配 | 4h |
| **Phase 3：下个版本** | 1. 9 个 Tauri commands 改 HTTP 调 daemon<br>2. 删除 Tauri 端的 sqlite 直读 | 3-5d |
| **Phase 4：可选** | 1. Tauri 端的 daemon health watchdog（auto-restart） | 2h |
| **不做** | 1. launchd plist 自动安装<br>2. daemon 多 token / 轮换 | — |

---

## 8. 与之前 TODO 的关系

本决策**不替换** `20260607-02-Memex-popup转桌面-TODO.md`，而是补充其架构层。

popup-转桌面 TODO 是 **UI 重构**，daemon 架构决策是 **数据访问层重构**，两者正交。

---

## 9. 需要用户确认的点

1. **D5 解读**：是按 解读 1（本期就一刀切）还是 解读 2（长期方向，本期标记 deprecated）？我推荐解读 2。
2. **Phase 2 优先级**：daemon auth (D3) 是否要在本期做？还是放到下个版本？
3. **launchd 路径完全不要了？** 我推荐保留 `memex daemon install-plist` CLI（高级用户可手动），但 menubar app 不再触发它。

---

## 10. 实施 → 全量回滚（2026-06-07）

本次会话 long_run 模式下完成了 Phase 1-7（daemon auth、19 个新 endpoint、Tauri supervisor、26 个 command 走 HTTP、SystemTab daemon 卡、全局 degraded banner），daemon 测试 30/30、前端测试 76/76 全过。

**但**在打包安装后用户实际验收发现 GUI 没有数据，决定 **daemon 架构改造全量回滚**。

**回滚范围**：
- daemon: 删 auth.rs、Phase 2 新增 19 个 endpoint、POST /shutdown、tests 新增
- daemon Cargo.toml: ureq / toml 依赖回退
- Tauri: 删 daemon_supervisor.rs / daemon_client.rs，26 个 command 全部回滚 git checkout
- memex-core: 多个 struct 的 Deserialize derive 回退
- App.vue: 全局 degraded banner 回退
- SystemTab: 保留（卡片本身只依赖现有 daemon_status/daemon_restart 命令）

**保留**：
- popup `show_main_window(navigate)` 修复（解决"点 dashboard 无反应" bug，跟 daemon 架构无关）
- Settings `?tab=system` query 控制（deep link 友好）
- SystemTab daemon 状态卡 + 重启按钮 + 查看日志（只用现有命令）
- 本文档 + `20260607-03-...测试清单.md` 的 M 节（作为 ADR / 经验沉淀，方便下次再考虑此架构时参考）

**经验**：daemon-as-sole-gateway 这个架构方向是正确的，但**一刀切落地风险太高**。下次重做应该是"渐进式"——先在一个不重要的 view（如 Reports）单独迁移、灰度跑 1-2 周，确认稳定后再扩大。

