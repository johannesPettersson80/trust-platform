//! Developer/workbench CLI entrypoint for truST.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[path = "trust-dev/commit.rs"]
mod commit;
#[path = "trust-dev/git.rs"]
mod git;
#[path = "trust-dev/prompt.rs"]
mod prompt;
#[path = "trust-dev/style.rs"]
mod style;

#[derive(Debug, Parser)]
#[command(
    name = "trust-dev",
    version,
    about = "Developer and workbench tools for truST"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Commit project changes with a human-friendly summary.
    Commit {
        /// Project folder directory (defaults to current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Commit message (skip prompt).
        #[arg(long)]
        message: Option<String>,
        /// Print summary without committing.
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> anyhow::Result<()> {
    if let Err(err) = run() {
        eprintln!("{}", style::error(format!("Error: {err:#}")));
        std::process::exit(1);
    }
    Ok(())
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Commit {
            project,
            message,
            dry_run,
        } => commit::run_commit(project, message, dry_run),
    }
}
