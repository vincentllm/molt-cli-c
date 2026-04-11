mod ai;
mod cast_parser;
mod commands;
mod config;
mod pipeline;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "molt",
    about = "🦞 Molt — Terminal workflow evolution tool",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start recording your terminal session via asciinema
    Record,
    /// Drop a semantic marker into the recording
    Mark {
        /// Optional label for this marker
        #[arg(short, long)]
        label: Option<String>,
    },
    /// Stop recording and show what was captured
    Stop,
    /// Run a saved pipeline
    Run {
        /// Pipeline name or ID
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Record => commands::record::run(),
        Commands::Mark { label } => commands::mark::run(label),
        Commands::Stop => commands::stop::run(),
        Commands::Run { name } => commands::run_pipeline::run(&name),
    }
}
