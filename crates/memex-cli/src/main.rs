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
        Commands::Backup { path } => commands::backup::run(&path, cli.json),
        Commands::RebuildIndex => commands::rebuild::run(cli.json),
        Commands::Mcp => commands::mcp::run(),
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
    }
}
