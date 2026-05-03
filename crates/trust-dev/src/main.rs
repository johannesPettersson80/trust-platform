//! Developer/workbench CLI entrypoint for truST.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod agent;
mod build;
mod ci;
mod commit;
mod ctl;
mod docs;
mod git;
mod prompt;
mod run;
mod style;
mod test;
mod workflow;

mod cli {
    use clap::ValueEnum;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
    pub enum DocsFormat {
        Markdown,
        Html,
        Both,
    }

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
    /// Generate API documentation from tagged ST comments.
    Docs {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Output directory for generated documentation files.
        #[arg(long = "out-dir")]
        out_dir: Option<PathBuf>,
        /// Output format (`markdown`, `html`, `both`).
        #[arg(long, value_enum, default_value_t = cli::DocsFormat::Both)]
        format: cli::DocsFormat,
    },
    /// Discover and execute ST tests in a project.
    Test {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Optional case-insensitive substring filter for test names.
        #[arg(long)]
        filter: Option<String>,
        /// List discovered tests without executing them.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        list: bool,
        /// Per-test timeout in seconds.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
        /// Output format (`human`, `junit`, `tap`, `json`).
        #[arg(long, value_enum, default_value_t = cli::TestOutput::Human)]
        output: cli::TestOutput,
        /// Enable CI-friendly behavior (`human` output defaults to `junit`).
        #[arg(long, action = clap::ArgAction::SetTrue)]
        ci: bool,
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
    let raw_args: Vec<String> = std::env::args().collect();
    let ci_mode = raw_args.iter().any(|arg| arg == "--ci");
    let ci_command = raw_args
        .iter()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .map(|arg| arg.as_str());
    if let Err(err) = run() {
        let message = format!("{err:#}");
        eprintln!("{}", style::error(format!("Error: {message}")));
        let exit_code = if ci_mode {
            ci::classify_error_with_command(&message, ci_command)
        } else {
            1
        };
        std::process::exit(exit_code);
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
        Command::Docs {
            project,
            out_dir,
            format,
        } => docs::run_docs(project, out_dir, format),
        Command::Test {
            project,
            filter,
            list,
            timeout,
            output,
            ci,
        } => test::run_test(project, filter, list, timeout, output, ci),
    }
}
