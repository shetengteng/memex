use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;

#[derive(Parser)]
#[command(name = "memex", version, about = "Local-first cross-LLM session memory hub")]
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
        /// Only ingest from a specific adapter
        #[arg(long)]
        adapter: Option<String>,
    },
    /// Search across all sessions
    Search {
        /// Search query
        query: String,
        /// Max results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// List sessions
    Sessions {
        /// Show N most recent sessions
        #[arg(long, default_value = "20")]
        recent: usize,
    },
    /// Show statistics
    Stats,
    /// Show or modify configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Ingest { adapter } => commands::ingest::run(adapter.as_deref(), cli.json),
        Commands::Search { query, limit } => commands::search::run(&query, limit, cli.json),
        Commands::Sessions { recent } => commands::sessions::run(recent, cli.json),
        Commands::Stats => commands::stats::run(cli.json),
        Commands::Config { action } => match action {
            ConfigAction::Show => commands::config::show(cli.json),
        },
    }
}
