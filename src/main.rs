mod backends;
mod commands;
mod config;
mod history;
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
                  \x20   molt record                  Start recording\n\
                  \x20   molt mark -l deploy          Drop a named anchor\n\
                  \x20   molt stop                    Stop + AI-extract pipeline\n\
                  \x20   molt list                    Show all saved pipelines\n\
                  \x20   molt run                     Interactive pipeline picker\n\
                  \x20   molt run deploy-app          Run by exact name\n\
                  \x20   molt run -v 'deploy staging' Intent matching\n\
                  \x20   molt stats                   Recording analytics\n\
                  \x20   molt recap                   Usage analytics + OpenClaw lift"
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

    /// List all saved pipelines
    List,

    /// Show stats for the last (or specified) recording
    Stats {
        /// Path to a .cast file (default: /tmp/molt_session.cast)
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Execute a pipeline — exact name, intent match, or interactive picker
    Run {
        /// Pipeline name (exact match). Omit for interactive picker.
        name: Option<String>,

        /// Natural language intent — fuzzy-matches against pipeline names and descriptions
        #[arg(short = 'v', long, value_name = "QUERY")]
        intent: Option<String>,

        /// Auto-run without confirmation when match confidence > 80%
        #[arg(short = 'y', long)]
        yes: bool,

        /// Show steps without executing (preview mode)
        #[arg(long)]
        dry_run: bool,
    },

    /// Usage analytics and OpenClaw capability lift
    Recap {
        /// Look-back window in days
        #[arg(long, default_value = "30")]
        days: u32,

        /// Filter to a specific pipeline
        #[arg(long)]
        pipeline: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Record => commands::record::run(),
        Commands::Mark { label } => commands::mark::run(label),
        Commands::Stop => commands::stop::run(),
        Commands::List => commands::list::run(),
        Commands::Stats { file } => commands::stats::run(file.as_deref()),
        Commands::Run { name, intent, yes, dry_run } => {
            commands::run::run(name.as_deref(), intent.as_deref(), yes, dry_run)
        }
        Commands::Recap { days, pipeline } => {
            commands::recap::run(days, pipeline.as_deref())
        }
    }
}
