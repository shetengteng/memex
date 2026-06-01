use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;

#[derive(Parser)]
#[command(
    name = "memex",
    version,
    about = "Local-first cross-LLM session memory hub"
)]
struct Cli {
    #[arg(long, global = true, help = "Output in JSON format")]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest sessions from AI tool history
    Ingest {
        #[arg(long)]
        adapter: Option<String>,
    },
    /// Search across all sessions
    Search {
        query: String,
        #[arg(short, long, default_value = "10")]
        limit: usize,
        #[arg(
            long,
            help = "Filter by adapter (claude_code, cursor, codex, opencode)"
        )]
        adapter: Option<String>,
        #[arg(long, help = "Filter by project name")]
        project: Option<String>,
        #[arg(long, help = "Filter by chunk type")]
        chunk_type: Option<String>,
        #[arg(long, help = "Only results after this date (RFC3339)")]
        after: Option<String>,
        #[arg(long, help = "Only results before this date (RFC3339)")]
        before: Option<String>,
    },
    /// List sessions
    Sessions {
        #[arg(long, default_value = "20")]
        recent: usize,
        #[arg(long, help = "Only show sessions updated within N days")]
        days: Option<u32>,
    },
    /// Show a specific session with its messages
    Session {
        /// Session ID (full or prefix)
        id: String,
    },
    /// Show statistics
    Stats,
    /// Show or modify configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Run system diagnostics
    Doctor,
    /// Export data to a tar.gz archive
    Backup {
        /// Output file path
        path: String,
    },
    /// Rebuild SQLite index from Markdown session files
    RebuildIndex,
    /// Start MCP server (stdio JSON-RPC)
    Mcp,
    /// Configure MCP for a specific AI tool
    Setup {
        /// Target tool (cursor, claude-code)
        target: String,
    },
    /// Manage the background daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Manage cloud LLM credentials (`~/.memex/credentials.toml`, chmod 0600)
    Credentials {
        #[command(subcommand)]
        action: CredentialsAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon in background
    Start,
    /// Stop a running daemon
    Stop,
    /// Show daemon status
    Status,
}

#[derive(Subcommand)]
enum CredentialsAction {
    /// Set the API key (and optional model) for a cloud provider
    Set {
        /// Provider name. Currently supported: anthropic
        provider: String,
        /// API key value
        api_key: String,
        /// Override the default model
        #[arg(long)]
        model: Option<String>,
    },
    /// Show which providers are currently configured
    Show,
    /// Clear stored credentials for a provider
    Clear {
        /// Provider name. Currently supported: anthropic
        provider: String,
    },
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
        Commands::Setup { target } => commands::setup::run(&target),
        Commands::Daemon { action } => match action {
            DaemonAction::Start => commands::daemon::start(cli.json),
            DaemonAction::Stop => commands::daemon::stop(cli.json),
            DaemonAction::Status => commands::daemon::status(cli.json),
        },
        Commands::Credentials { action } => match action {
            CredentialsAction::Set { provider, api_key, model } => match provider.as_str() {
                "anthropic" => commands::credentials::set_anthropic(&api_key, model, cli.json),
                other => {
                    eprintln!("unsupported credentials provider: {} (supported: anthropic)", other);
                    Ok(())
                }
            },
            CredentialsAction::Show => commands::credentials::show(cli.json),
            CredentialsAction::Clear { provider } => match provider.as_str() {
                "anthropic" => commands::credentials::clear_anthropic(cli.json),
                other => {
                    eprintln!("unsupported credentials provider: {} (supported: anthropic)", other);
                    Ok(())
                }
            },
        },
    }
}
