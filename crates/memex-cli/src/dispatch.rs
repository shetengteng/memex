//! 把 [`crate::cli::Cli`] 解析后的命令路由到对应的 `commands::*` 模块。
//!
//! 大 `match` 集中在这里，避免 main.rs 同时承担 clap 定义、tracing 初始化
//! 和 dispatch 三个职责（规约 §7.2 单文件 ≤ 300 行）。

use anyhow::Result;

use crate::cli::{Cli, Commands, ConfigAction, DaemonAction, HooksAction, ReflectAction};
use crate::commands;

pub fn run(cli: Cli) -> Result<()> {
    // 把 clap 顶层 --json flag 一次性注入 io 子系统，让 `out!` / `err!` /
    // `crate::io::json()` 都能在任何位置感知到，而不必把 `json: bool`
    // 沿着每个 commands::*::run 调用链手动传递。
    crate::io::init(cli.json);

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
        } => run_setup(&target, uninstall, status, cli.json),
        Commands::SetupStatus => run_setup_status(cli.json),
        Commands::Skill {
            target,
            uninstall,
            status,
        } => run_skill(&target, uninstall, status, cli.json),
        Commands::SkillStatus => run_skill_status(cli.json),
        Commands::Rules {
            target,
            uninstall,
            status,
        } => run_rules(&target, uninstall, status, cli.json),
        Commands::RulesStatus => run_rules_status(cli.json),
        Commands::Daemon { action } => match action {
            DaemonAction::Start => commands::daemon::start(cli.json),
            DaemonAction::Stop => commands::daemon::stop(cli.json),
            DaemonAction::Status => commands::daemon::status(cli.json),
        },
        Commands::Hooks { action } => run_hooks(action, cli.json),
    }
}

fn parse_ide(target: &str) -> Result<commands::setup::Ide> {
    commands::setup::Ide::parse(target).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown IDE: {}. Supported: cursor, claude-code, codex, opencode",
            target
        )
    })
}

fn run_setup(target: &str, uninstall: bool, status: bool, json: bool) -> Result<()> {
    let ide = parse_ide(target)?;
    if status {
        let s = commands::setup::status(ide)?;
        if json {
            crate::io::json(&s)?;
        } else {
            crate::out!(
                "{}: installed={}, exists={}, path={}",
                s.ide,
                s.installed,
                s.config_exists,
                s.config_path
            );
        }
        return Ok(());
    }
    if uninstall {
        return commands::setup::uninstall(ide).map(|_| ());
    }
    commands::setup::run(target)
}

fn run_setup_status(json: bool) -> Result<()> {
    let all = commands::setup::list_status();
    if json {
        crate::io::json(&all)?;
        return Ok(());
    }
    for s in &all {
        let mark = if s.installed { "[✓]" } else { "[ ]" };
        crate::out!(
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
    Ok(())
}

fn run_skill(target: &str, uninstall: bool, status: bool, json: bool) -> Result<()> {
    let ide = parse_ide(target)?;
    if status {
        let s = commands::skill::status(ide)?;
        if json {
            crate::io::json(&s)?;
        } else {
            crate::out!(
                "{}: installed={}, path={}, size={:?}",
                s.ide,
                s.installed,
                s.dest_path,
                s.size
            );
        }
        return Ok(());
    }
    if uninstall {
        return commands::skill::uninstall(ide).map(|_| ());
    }
    commands::skill::install(ide).map(|_| ())
}

fn run_rules(target: &str, uninstall: bool, status: bool, json: bool) -> Result<()> {
    let ide = parse_ide(target)?;
    if status {
        let s = commands::rules::status(ide)?;
        if json {
            crate::io::json(&s)?;
        } else {
            crate::out!(
                "{}: installed={}, supported={}, path={}, size={:?}",
                s.ide,
                s.installed,
                s.supported,
                s.dest_path,
                s.size
            );
        }
        return Ok(());
    }
    if uninstall {
        return commands::rules::uninstall(ide).map(|_| ());
    }
    commands::rules::install(ide).map(|_| ())
}

fn run_rules_status(json: bool) -> Result<()> {
    let all = commands::rules::list_status();
    if json {
        crate::io::json(&all)?;
        return Ok(());
    }
    for s in &all {
        let mark = if s.installed {
            "[✓]"
        } else if s.supported {
            "[ ]"
        } else {
            "[—]"
        };
        crate::out!(
            "{} {:<14} {} (path: {})",
            mark,
            s.ide,
            if s.supported {
                if s.installed {
                    "installed"
                } else {
                    "not installed"
                }
            } else {
                "unsupported"
            },
            s.dest_path
        );
    }
    Ok(())
}

fn run_skill_status(json: bool) -> Result<()> {
    let all = commands::skill::list_status();
    if json {
        crate::io::json(&all)?;
        return Ok(());
    }
    for s in &all {
        let mark = if s.installed { "[✓]" } else { "[ ]" };
        crate::out!(
            "{} {:<14} {} ({} bytes)",
            mark,
            s.ide,
            s.dest_path,
            s.size.unwrap_or(0)
        );
    }
    Ok(())
}

fn run_hooks(action: HooksAction, json: bool) -> Result<()> {
    use commands::hooks;
    let memex_home = memex_core::memex_dir();
    // 用「current_exe」让 wrapper 指向正在跑的这份 memex —— 自定义安装
    // 路径、CLI 用户 PATH 不一致时也不会拼错。
    let memex_bin = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("memex"));

    let report = |st: &hooks::HookStatus| {
        if json {
            // `crate::out!` 在 --json 模式下会被压制（io::write_out 的契约），
            // 因此必须走 `crate::io::json` 这条专用的 JSON stdout 通道，否则
            // Tauri / `jq` 之类的下游会收到空 stdout 并 fail to parse。
            let _ = crate::io::json(st);
            return;
        }
        let mark = if !st.supported {
            "[—]"
        } else if st.installed {
            "[✓]"
        } else {
            "[ ]"
        };
        crate::out!(
            "{} {:<14} {}{}",
            mark,
            st.ide,
            if st.supported { "" } else { "(unsupported) " },
            st.config_path,
        );
        if let Some(w) = &st.wrapper_path {
            crate::out!("    wrapper: {}", w);
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
                crate::io::json(&all)?;
            } else {
                for st in &all {
                    report(st);
                }
            }
        }
    }
    Ok(())
}
