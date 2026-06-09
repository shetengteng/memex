//! Memex CLI 的 clap 类型定义。
//!
//! 这里只声明命令面（接口），实际执行流程见 [`crate::dispatch`]。
//! main.rs 仅负责 tracing 初始化 + `Cli::parse()` + 转交给 dispatch。

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "memex", version, about = "本地优先的跨 LLM 会话记忆中枢")]
pub struct Cli {
    #[arg(long, global = true, help = "以 JSON 格式输出结果")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 从 AI 工具的历史会话中拉取数据
    Ingest {
        #[arg(long)]
        adapter: Option<String>,
    },
    /// 跨所有会话搜索
    Search {
        query: String,
        #[arg(short, long, default_value = "10")]
        limit: usize,
        #[arg(long, help = "按适配器过滤（claude_code、cursor、codex、opencode）")]
        adapter: Option<String>,
        #[arg(long, help = "按项目名称过滤")]
        project: Option<String>,
        #[arg(long, help = "按 chunk 类型过滤")]
        chunk_type: Option<String>,
        #[arg(long, help = "只返回此时间之后的结果（RFC3339）")]
        after: Option<String>,
        #[arg(long, help = "只返回此时间之前的结果（RFC3339）")]
        before: Option<String>,
    },
    /// 列出会话
    Sessions {
        #[arg(long, default_value = "20")]
        recent: usize,
        #[arg(long, help = "只显示最近 N 天内更新过的会话")]
        days: Option<u32>,
    },
    /// 显示某个会话及其消息
    Session {
        /// 会话 ID（完整或前缀均可）
        id: String,
    },
    /// 显示统计信息
    Stats,
    /// 显示或修改配置
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// 运行系统诊断
    Doctor,
    /// 将数据导出为 tar.gz 归档
    Backup {
        /// 输出文件路径
        path: String,
    },
    /// 从 Markdown 会话文件重建 SQLite 索引
    RebuildIndex,
    /// 启动 MCP server（stdio JSON-RPC）
    Mcp,
    /// 输出当前项目的「工作记忆」上下文，供 IDE hook 注入到 AI 会话
    Context {
        /// 显式指定项目目录；默认用当前工作目录
        #[arg(long)]
        project: Option<String>,
        /// 最多列多少个最近会话
        #[arg(long, default_value = "3")]
        top: usize,
        /// 是否脱敏；不传则按 config.privacy.redaction_enabled
        #[arg(long)]
        redact: Option<bool>,
    },
    /// 为指定 AI 工具配置 MCP
    Setup {
        /// 目标工具（cursor、claude-code、codex、opencode）
        target: String,
        /// 卸载（移除条目）而非安装
        #[arg(long)]
        uninstall: bool,
        /// 只输出状态，不修改任何文件
        #[arg(long)]
        status: bool,
    },
    /// 显示所有 IDE 当前 MCP 集成状态
    SetupStatus,
    /// 安装 / 卸载 / 查询 SKILL.md 到 4 个 IDE 的 skills 目录
    Skill {
        /// 目标工具（cursor、claude-code、codex、opencode）
        target: String,
        /// 卸载（删除文件）而非安装
        #[arg(long)]
        uninstall: bool,
        /// 只输出状态，不动文件
        #[arg(long)]
        status: bool,
    },
    /// 显示所有 IDE 当前 SKILL.md 安装状态
    SkillStatus,
    /// 管理后台 daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// 管理 IDE SessionStart hook（自动在 AI 会话启动时注入项目工作记忆）
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },
    /// 基于 daily 摘要做反思级别回顾（shipped / patterns / open loops）
    Reflect {
        #[command(subcommand)]
        action: Option<ReflectAction>,
        /// 直接调用 run 的快捷写法：`memex reflect --period week`
        /// 与 `Run` 子命令互斥；如果同时给了 action，以 action 为准。
        #[arg(long, default_value = "week")]
        period: String,
    },
}

#[derive(Subcommand)]
pub enum ReflectAction {
    /// 生成一份新的 reflect（如果同期已存在则覆盖）
    Run {
        /// 回顾周期：week / month / Nd（如 7d、14d、30d）
        #[arg(long, default_value = "week")]
        period: String,
    },
    /// 列出已经生成过的 reflect 历史，按 scope_key 倒序
    List {
        /// 最多列多少条，默认 20
        #[arg(long, default_value = "20")]
        limit: u32,
    },
    /// 显示某条 reflect 的完整 markdown
    Show {
        /// scope_key（如 `week:2026-W23` / `days7:2026-06-04`）
        key: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// 显示当前配置
    Show,
    /// 设置某个配置项
    Set { key: String, value: String },
}

#[derive(Subcommand)]
pub enum DaemonAction {
    /// 在后台启动 daemon
    Start,
    /// 停止运行中的 daemon
    Stop,
    /// 显示 daemon 状态
    Status,
}

#[derive(Subcommand)]
pub enum HooksAction {
    /// 安装 SessionStart hook 到指定 IDE（cursor / claude-code / codex）
    Install {
        /// IDE 名称
        target: String,
    },
    /// 移除 SessionStart hook 但保留 wrapper 脚本
    Uninstall { target: String },
    /// 显示某个 IDE 的 hook 状态
    Status { target: String },
    /// 显示所有 IDE 的 hook 状态
    All,
}
