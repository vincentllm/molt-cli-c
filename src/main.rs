mod backends;
mod commands;
mod config;
mod pipeline;
mod recording;
mod session;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "molt",
    about = "🦞 Molt — Terminal workflow recorder & ClawBot executor",
    version,
    after_help = "  Examples:\n\
                  \x20   molt record          Start recording\n\
                  \x20   molt mark -l setup   Drop a named anchor\n\
                  \x20   molt stop            Stop + AI-extract pipeline\n\
                  \x20   molt stats           View recording analytics\n\
                  \x20   molt run <name>      Execute a saved pipeline"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start recording your terminal session via asciinema
    Record,

    /// Drop a semantic anchor into the recording
    Mark {
        /// Label for this marker (e.g. "deploy", "setup")
        #[arg(short, long)]
        label: Option<String>,
    },

    /// Stop recording, extract pipeline via AI, save YAML
    Stop,

    /// Show stats for the last (or specified) recording
    Stats {
        /// Path to a .cast file (default: /tmp/molt_session.cast)
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Execute a saved pipeline (local or via ClawBot)
    Run {
        /// Pipeline name (from ~/.molt/pipelines/)
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Record => commands::record::run(),
        Commands::Mark { label } => commands::mark::run(label),
        Commands::Stop => commands::stop::run(),
        Commands::Stats { file } => commands::stats::run(file.as_deref()),
        Commands::Run { name } => commands::run::run(&name),
    }
}
