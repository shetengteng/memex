# 维护与排障

按问题症状索引。每条都给原因 + 修复步骤 + 怎么验证修好。

## 1. "读取失败：unable to open database file (Error code 14)"

**症状**：Reset Index 或 Rebuild Index 后，前端页面短暂显示 SQLite 错误，几秒后恢复。

**原因**：reset 流程中 daemon 重启的 ~300ms 窗口里，前端 polling 跟 daemon 都尝试 `Db::open` 跑 PRAGMA WAL，互相抢锁。

**修复**：**v1.0.4+ 已修复**——reset 加了 300ms grace + 预 open 验证；前端 polling 失败时静默返回空（看到的不是红色 error 而是"暂无调用"）。

**手动恢复**（旧版本）：重启 Memex.app（菜单栏 (M) 图标 → 退出 → 重新打开），或确认 `~/.memex/memex.db` 健康后直接重开。

## 2. "Bad file descriptor (os error 9)"

**症状**：IDE 集成卡片刷新时弹 `IDE: Bad file descriptor (os error 9)` toast，重启应用就好。

**原因**：daemon 的 watcher / scheduler 用 fire-and-forget tokio task 启动，shutdown 时没 await abort，macOS fsevent FD 累积泄漏；下次 `Command::output()` 拿到失效 FD 就 EBADF。

**修复**：**v1.0.4+ 已修复**——watcher / scheduler 返回 JoinHandle，shutdown 时 abort + await；`std::process::Command` 也换成 `tokio::process::Command` 配 `kill_on_drop`。

## 3. "0 / 0 已接入" 但 IDE 明明在跑

**症状**：Connect 页显示已接入 0 个 IDE，但本机确实在用 Cursor / Claude Code。

**可能原因 + 修复**：

- **IDE 的 sqlite db 路径变了**（Cursor 升大版本时偶发）：删 `~/.memex/memex.db`，主窗口右上角"重建索引"，触发 collector 重新探测路径。
- **Memex 没有 Full Disk Access**（macOS 系统沙箱拦截）：系统设置 → 隐私与安全性 → 完全磁盘访问 → 加 `/Applications/Memex.app`，重启 Memex。
- **IDE 自己还没产生数据**（刚装 Cursor / Claude Code，还没开过会话）：先开几个会话再回来。

**怎么验证**：终端跑 `ls ~/Library/Application\ Support/Cursor/User/workspaceStorage/`——目录里有内容 + Memex 仍 0 接入 = 是 Memex 端没读到，多半是 Full Disk Access。

## 4. daemon 起不来 / 主窗口空白

**看日志**：Settings → 维护 → 查看日志，最近 100 行 tracing 输出。

**最常见原因**：

- **端口冲突**（默认 9999）：被占时自动 fallback 到 10000-10010，日志会有 `daemon listening on http://127.0.0.1:1xxxx`；如果一路 fallback 失败就要手动 `lsof -i :9999` 看谁占着。
- **db 损坏**：日志有 `database disk image is malformed`。Settings → System → "重建索引"（保留原始 md，纯 SQL 重建）。
- **daemon.lock 残留**：异常崩溃后 `~/.memex/daemon.lock` 没删，`rm ~/.memex/daemon.lock` 后重启。

## 5. 重建索引 / 彻底重置在哪

Settings → System 页：

| 操作 | 删什么 | 保留什么 | 需要时间 |
|---|---|---|---|
| **重建索引** | `memex.db*` | `sessions/*.md` + 配置 | ~30-60s / 1000 条 |
| **彻底重置** | 全部 `~/.memex/` 内容 | 无 | 几秒 |

> 重建索引是日常调试首选——能修绝大部分"搜不到 / 摘要乱"的问题；彻底重置是"我要重新开始"。

## 6. MCP 调用没反应

按这个顺序排查：

1. **看 Connect 页 → MCP 活动卡片** 有没有调用记录
   - **有记录但失败**：右栏会显示错误原因，最常见 "Project not found: /old/path"（IDE 工作目录变了），或 "session not found"（私有过滤）
   - **完全没记录**：IDE 端 MCP server 没连上 Memex，进 IDE 的 MCP 设置看 memex 是不是绿灯
2. **重新装一次 MCP**：Connect 页 → IDE 集成 → 该 IDE → 卸载 → 重装（覆盖 ~/.cursor/mcp.json 等配置）
3. **重启 IDE**：MCP 配置变更需要重启 IDE 进程
4. **如果 daemon 端口变了**：IDE 端 mcp.json 里的 endpoint 是写死的，端口 fallback 后就连不上了。重新装一次会写入新 port。

## 7. 搜索结果不准 / 漏命中

**常见原因**：

- **会话还没 ingest**：Library 页的会话计数 = 已索引；首次扫描完成前搜索会漏。等扫描进度跑完。
- **关键词包含 SQL 特殊字符**（如 `_` `%`）：FTS5 会把它当通配符；先试简单词试试。
- **L2 摘要还在跑**：摘要里的关键词搜得到，原文里不一定。如果你搜的词在 AI 回复里，等摘要跑完命中率会上去。

**强制重建 FTS 索引**：Settings → 维护 → 重建 FTS 索引（不删 sessions / messages，只重建 chunks 表 + FTS 虚表）。

## 8. 看 daemon 实际日志

**Memex 主窗口**：Settings → 维护 → 查看日志。

**终端**：

```bash
# 最近 100 行
tail -n 100 ~/.memex/logs/memex.log

# 实时跟随
tail -f ~/.memex/logs/memex.log

# 找错误
grep -E "ERROR|WARN" ~/.memex/logs/memex.log
```

## 9. redactions.yaml 改完不生效

Memex 是**实时读** redactions.yaml 的（不缓存到 SQL）。MCP 下一次调用就按新规则过滤——不需要重启 daemon。

如果不生效：

1. **YAML 格式错**：`yamllint ~/.memex/redactions.yaml` 验证
2. **路径规则用了 `~`**：会自动展开成 `$HOME`；用绝对路径 `/Users/me/...` 更稳
3. **glob 通配 `**`** vs `*`：`**` 跨多层，`*` 只一层
4. **关键词大小写**：默认大小写敏感；想忽略的话写两份（`"API_KEY"` + `"api_key"`）

**怎么验证**：把一个项目路径加进 `paths`，然后在 IDE 里问 AI "搜一下 那个项目的会话"——看 MCP 活动卡是不是返回空 / 是否过滤掉了。

## 10. 升级 / 卸载

**升级**（保留所有数据）：

```bash
# 1. 下载新版 DMG（按当前架构）
#    https://github.com/shetengteng/memex/releases

# 2. 拖 Memex.app 到 /Applications 覆盖

# 3. 重跑 install 脚本（清新版本的 quarantine + 重启）
curl -fsSL https://raw.githubusercontent.com/shetengteng/memex/main/scripts/install-macos.sh | bash
```

新版会自动跑 SQLite migration（v5 之类），数据无损升级。

**卸载**：

```bash
# 1. 退出 Memex（菜单栏 (M) → Quit）

# 2. 删应用
sudo rm -rf /Applications/Memex.app

# 3. 删数据（如果想保留索引以便下次装回来，跳过这步）
rm -rf ~/.memex
```

> 注意：删了 `~/.memex/` 之后，重新装 Memex 会触发**完整重新扫描所有 IDE**，1000 条会话约需 30-60 秒。
