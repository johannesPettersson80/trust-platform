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
mod replay;
mod run;
mod rust;
mod rust_audit;
mod rust_bench;
mod service;
mod service_manager;
mod style;
mod test;
mod workflow;

#[cfg(test)]
mod docs_tests;
#[cfg(test)]
mod token_tests;

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
    /// Replay a cycle trace in a simulator of the project as it is now: build the project, run the
    /// trace's cycles and compare each cycle's outputs (specification 48 RU-60). Ends with 0 when
    /// every cycle matches, 1 when one differs, 2 when the trace or the build cannot be replayed.
    Replay {
        /// The trace, a `.trusttrace` file that `trust-runtime ctl record` saved.
        trace: PathBuf,
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
    },
    /// The project's Rust POUs (specification 48).
    Rust {
        #[command(subcommand)]
        action: RustAction,
    },
    /// The project's services (specification 48 §13).
    Service {
        #[command(subcommand)]
        action: ServiceAction,
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
        /// The serial port of a device that runs the tests compiled for its `--target`, or
        /// `tcp://<host>:<port>` of a bridge to that port (specification 44 §14.2).
        #[arg(long, requires = "target")]
        device: Option<String>,
        /// The device's target, as `trust-runtime build --target` names it: `esp32c6`.
        #[arg(long, requires = "device")]
        target: Option<String>,
        /// The device's control token, when its settings set one; else `TRUST_CTL_TOKEN`, else
        /// the project's `runtime.control.auth_token` (specification 11 §6.8.1).
        #[arg(long, requires = "device", allow_hyphen_values = true)]
        token: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ServiceAction {
    /// Write the bindings of a service, a Rust module naming each variable of the program it can
    /// read with its Rust type, from the image the project's build wrote (specification 48 RU-62).
    Bindings {
        /// The service's name, as trust.toml declares it in `[services.<name>]`.
        name: String,
        /// The Rust file to write: src/plant.rs, for `plant::P1::level`.
        #[arg(long)]
        out: PathBuf,
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
    },
    /// Put the service `[services.<name>]` under the computer's service manager and start it, so
    /// that it starts at every boot and again 2 s after it ends (specification 48 RU-64). It takes
    /// root or an administrator; without them it changes nothing and names the command to run.
    Install {
        /// The service's name, as trust.toml declares it in `[services.<name>]`.
        name: String,
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// The service's program, as the command without rights names it; by default the release
        /// binary of the service's package, which cargo's metadata names.
        #[arg(long)]
        binary: Option<PathBuf>,
    },
    /// Stop the service `trust-service-<name>` and remove it from the computer's service manager.
    Uninstall {
        /// The service's name.
        name: String,
    },
    /// The program of a Windows service that `install` writes, which Windows' service manager
    /// runs: it answers the service manager and runs the service's binary in the project folder
    /// (specification 48 RU-64).
    Run {
        /// The service's name.
        name: String,
        /// The project folder, the binary's working folder.
        #[arg(long)]
        project: PathBuf,
        /// The service's binary.
        #[arg(long)]
        binary: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum RustAction {
    /// Write the Structured Text declarations of the project's Rust POUs (specification 48 RU-24):
    /// src/generated/rust_pous.st from its crate in rust/, or in a Rust-first project
    /// src/generated/trust/ with the configuration of its trust.toml (RU-67).
    Declarations {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
    },
    /// Check the crates the program's module is built from, the scan crate graph, against the
    /// RustSec advisory database: fails when a vulnerability names one (specification 48 RU-79).
    Audit {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// A copy of the advisory database's repository in a folder, checked without downloading
        /// (for a computer that never reaches GitHub).
        #[arg(long = "db")]
        db: Option<PathBuf>,
    },
    /// Measure the project's Rust functions, function blocks, methods and programs with their
    /// `#[bench_case]` cases, on this computer or a device, and write each one's longest call into
    /// trust.toml [timing.measured] (specification 48 RU-48).
    Bench {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// The scans each case runs, one call of its POU a scan.
        #[arg(long = "scans", default_value_t = rust_bench::DEFAULT_SCANS)]
        scans: u32,
        /// The serial port of a device that runs the bench compiled for its `--target`, or
        /// `tcp://<host>:<port>` of a bridge to that port (specification 11 §6.8.1).
        #[arg(long, requires = "target")]
        device: Option<String>,
        /// The device's target, as `trust-runtime build --target` names it: `esp32c6`.
        #[arg(long, requires = "device")]
        target: Option<String>,
        /// The device's control token, when its settings set one; else `TRUST_CTL_TOKEN`, else
        /// the project's `runtime.control.auth_token` (specification 11 §6.8.1).
        #[arg(long, requires = "device", allow_hyphen_values = true)]
        token: Option<String>,
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
    /// Serve the external agent contract as MCP tools over stdio.
    ServeMcp {
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
            AgentAction::ServeMcp { project } => agent::run_agent_serve_mcp(project),
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
        Command::Replay { trace, project } => replay::run_replay(&trace, project),
        Command::Service { action } => match action {
            ServiceAction::Bindings { name, out, project } => {
                service::run_bindings(project, &name, &out)
            }
            ServiceAction::Install {
                name,
                project,
                binary,
            } => service_manager::run_install(project, &name, binary),
            ServiceAction::Uninstall { name } => service_manager::run_uninstall(&name),
            ServiceAction::Run {
                name,
                project,
                binary,
            } => service_manager::run_host(&name, &project, &binary),
        },
        Command::Rust { action } => match action {
            RustAction::Declarations { project } => rust::run_declarations(project),
            RustAction::Audit { project, db } => rust_audit::run_audit(project, db),
            RustAction::Bench {
                project,
                scans,
                device,
                target,
                token,
            } => rust_bench::run_bench(
                project,
                scans,
                device
                    .zip(target)
                    .map(|(port, target)| rust_bench::OnDevice {
                        port,
                        target,
                        token,
                    }),
            ),
        },
        Command::Test {
            project,
            filter,
            list,
            timeout,
            output,
            ci,
            device,
            target,
            token,
        } => {
            let token = token.or_else(trust_runtime::control::endpoint::control_token_from_env);
            let runner = test::TestRunner::of(device.zip(target), token);
            test::run_test(project, filter, list, timeout, output, ci, runner)
        }
    }
}
