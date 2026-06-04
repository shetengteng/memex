use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;

#[derive(Parser)]
#[command(
    name = "memex",
    version,
    about = "本地优先的跨 LLM 会话记忆中枢"
)]
struct Cli {
    #[arg(long, global = true, help = "以 JSON 格式输出结果")]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
        #[arg(
            long,
            help = "按适配器过滤（claude_code、cursor、codex、opencode）"
        )]
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
enum ReflectAction {
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
enum ConfigAction {
    /// 显示当前配置
    Show,
    /// 设置某个配置项
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// 在后台启动 daemon
    Start,
    /// 停止运行中的 daemon
    Stop,
    /// 显示 daemon 状态
    Status,
}

#[derive(Subcommand)]
enum HooksAction {
    /// 安装 SessionStart hook 到指定 IDE（cursor / claude-code / codex）
    Install {
        /// IDE 名称
        target: String,
    },
    /// 移除 SessionStart hook 但保留 wrapper 脚本
    Uninstall {
        target: String,
    },
    /// 显示某个 IDE 的 hook 状态
    Status {
        target: String,
    },
    /// 显示所有 IDE 的 hook 状态
    All,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Ingest { adapter } => commands::ingest::run(adapter.as_deref(), cli.json),
        Commands::Search {
            query,
            limit,
            adapter,
            project,
            chunk_type,
            after,
            before,
        } => commands::search::run(
            &query, limit, cli.json, adapter, project, chunk_type, after, before,
        ),
        Commands::Sessions { recent, days } => commands::sessions::run(recent, days, cli.json),
        Commands::Session { id } => commands::session::run(&id, cli.json),
        Commands::Stats => commands::stats::run(cli.json),
        Commands::Config { action } => match action {
            ConfigAction::Show => commands::config::show(cli.json),
            ConfigAction::Set { key, value } => commands::config::set(&key, &value, cli.json),
        },
        Commands::Doctor => commands::doctor::run(cli.json),
        Commands::Reflect { action, period } => match action {
            None => commands::reflect::run(&period, cli.json),
            Some(ReflectAction::Run { period }) => commands::reflect::run(&period, cli.json),
            Some(ReflectAction::List { limit }) => commands::reflect::list(limit, cli.json),
            Some(ReflectAction::Show { key }) => commands::reflect::show(&key, cli.json),
        },
        Commands::Backup { path } => commands::backup::run(&path, cli.json),
        Commands::RebuildIndex => commands::rebuild::run(cli.json),
        Commands::Mcp => commands::mcp::run(),
        Commands::Context {
            project,
            top,
            redact,
        } => commands::context::run(commands::context::ContextArgs {
            project,
            top,
            redact,
            json: cli.json,
        }),
        Commands::Setup {
            target,
            uninstall,
            status,
        } => {
            let ide = commands::setup::Ide::parse(&target).ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown IDE: {}. Supported: cursor, claude-code, codex, opencode",
                    target
                )
            })?;
            if status {
                let s = commands::setup::status(ide)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&s)?);
                } else {
                    println!(
                        "{}: installed={}, exists={}, path={}",
                        s.ide, s.installed, s.config_exists, s.config_path
                    );
                }
                Ok(())
            } else if uninstall {
                commands::setup::uninstall(ide).map(|_| ())
            } else {
                commands::setup::run(&target)
            }
        }
        Commands::SetupStatus => {
            let all = commands::setup::list_status();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&all)?);
            } else {
                for s in &all {
                    let mark = if s.installed { "[✓]" } else { "[ ]" };
                    println!(
                        "{} {:<14} {} (config: {})",
                        mark,
                        s.ide,
                        if s.config_exists {
                            "config present"
                        } else {
                            "no config"
                        },
                        s.config_path
                    );
                }
            }
            Ok(())
        }
        Commands::Skill {
            target,
            uninstall,
            status,
        } => {
            let ide = commands::setup::Ide::parse(&target).ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown IDE: {}. Supported: cursor, claude-code, codex, opencode",
                    target
                )
            })?;
            if status {
                let s = commands::skill::status(ide)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&s)?);
                } else {
                    println!(
                        "{}: installed={}, path={}, size={:?}",
                        s.ide, s.installed, s.dest_path, s.size
                    );
                }
                Ok(())
            } else if uninstall {
                commands::skill::uninstall(ide).map(|_| ())
            } else {
                commands::skill::install(ide).map(|_| ())
            }
        }
        Commands::SkillStatus => {
            let all = commands::skill::list_status();
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&all)?);
            } else {
                for s in &all {
                    let mark = if s.installed { "[✓]" } else { "[ ]" };
                    println!(
                        "{} {:<14} {} ({} bytes)",
                        mark,
                        s.ide,
                        s.dest_path,
                        s.size.unwrap_or(0)
                    );
                }
            }
            Ok(())
        }
        Commands::Daemon { action } => match action {
            DaemonAction::Start => commands::daemon::start(cli.json),
            DaemonAction::Stop => commands::daemon::stop(cli.json),
            DaemonAction::Status => commands::daemon::status(cli.json),
        },
        Commands::Hooks { action } => run_hooks(action, cli.json),
    }
}

fn run_hooks(action: HooksAction, json: bool) -> Result<()> {
    use commands::hooks;
    let memex_home = memex_core::memex_dir();
    // 用「current_exe」让 wrapper 指向正在跑的这份 memex —— 自定义安装
    // 路径、CLI 用户 PATH 不一致时也不会拼错。
    let memex_bin = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("memex"));

    let parse_ide = |t: &str| -> Result<commands::setup::Ide> {
        commands::setup::Ide::parse(t).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown IDE: {}. Supported: cursor, claude-code, codex (opencode 暂不支持自动 hook)",
                t
            )
        })
    };

    let report = |st: &hooks::HookStatus| {
        if json {
            if let Ok(s) = serde_json::to_string_pretty(st) {
                println!("{}", s);
            }
        } else {
            let mark = if !st.supported {
                "[—]"
            } else if st.installed {
                "[✓]"
            } else {
                "[ ]"
            };
            println!(
                "{} {:<14} {}{}",
                mark,
                st.ide,
                if st.supported { "" } else { "(unsupported) " },
                st.config_path,
            );
            if let Some(w) = &st.wrapper_path {
                println!("    wrapper: {}", w);
            }
        }
    };

    match action {
        HooksAction::Install { target } => {
            let ide = parse_ide(&target)?;
            let st = hooks::install(ide, &memex_bin, &memex_home)?;
            report(&st);
        }
        HooksAction::Uninstall { target } => {
            let ide = parse_ide(&target)?;
            let st = hooks::uninstall(ide)?;
            report(&st);
        }
        HooksAction::Status { target } => {
            let ide = parse_ide(&target)?;
            let st = hooks::status(ide)?;
            report(&st);
        }
        HooksAction::All => {
            let all = hooks::list_status();
            if json {
                println!("{}", serde_json::to_string_pretty(&all)?);
            } else {
                for st in &all {
                    report(st);
                }
            }
        }
    }
    Ok(())
}
