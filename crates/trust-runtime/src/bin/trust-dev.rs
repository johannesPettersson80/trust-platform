//! Developer/workbench CLI entrypoint for truST.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[path = "trust-dev/agent.rs"]
mod agent;
#[path = "trust-dev/build.rs"]
mod build;
#[path = "trust-dev/commit.rs"]
mod commit;
#[path = "trust-dev/ctl.rs"]
mod ctl;
#[path = "trust-dev/git.rs"]
mod git;
#[path = "trust-dev/prompt.rs"]
mod prompt;
#[path = "trust-dev/run.rs"]
mod run;
#[path = "trust-dev/style.rs"]
mod style;
#[path = "trust-dev/test.rs"]
mod test;
#[path = "trust-dev/workflow.rs"]
mod workflow;

mod cli {
    use clap::ValueEnum;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
    pub enum TestOutput {
        Human,
        Junit,
        Tap,
        Json,
    }
}

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
    /// Serve the external agent contract over stdio JSON-RPC.
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
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

#[derive(Debug, Subcommand)]
enum AgentAction {
    /// Serve the external agent contract over stdio JSON-RPC.
    Serve {
        /// Workspace/project root (defaults to current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
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
        Command::Agent { action } => match action {
            AgentAction::Serve { project } => agent::run_agent_serve(project),
        },
        Command::Commit {
            project,
            message,
            dry_run,
        } => commit::run_commit(project, message, dry_run),
    }
}
