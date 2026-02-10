//! CLI definitions for trust-runtime.

use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "trust-runtime",
    version,
    about = "Structured Text runtime CLI",
    infer_subcommands = true,
    arg_required_else_help = false,
    after_help = "Examples:\n  trust-runtime                       # start (first run opens setup)\n  trust-runtime --verbose             # show startup details\n  trust-runtime ui --project ./my-plc # terminal UI\n  trust-runtime play --project ./my-plc # compatibility"
)]
pub struct Cli {
    /// Show verbose startup details.
    #[arg(long, short, global = true)]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run a runtime instance (PLC mode).
    Run {
        /// Project folder directory.
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Configuration entry file (dev mode).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Root directory for ST sources (dev mode).
        #[arg(long)]
        runtime_root: Option<PathBuf>,
        /// Restart mode on startup.
        #[arg(long, default_value = "cold")]
        restart: String,
        /// Run in explicit simulation mode.
        #[arg(long, action = ArgAction::SetTrue)]
        simulation: bool,
        /// Simulation time acceleration factor (>= 1).
        #[arg(long, default_value_t = 1)]
        time_scale: u32,
    },
    /// Start the runtime with project auto-detection (production UX).
    #[command(
        after_help = "Examples:\n  trust-runtime play\n  trust-runtime play --project ./my-plc\n  trust-runtime play --restart warm\n  trust-runtime play --project ./my-plc --simulation --time-scale 8"
    )]
    Play {
        /// Project folder directory (auto-creates a default project if missing).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Restart mode on startup.
        #[arg(long, default_value = "cold")]
        restart: String,
        /// Force-enable the interactive console (TTY only).
        #[arg(long, action = ArgAction::SetTrue, conflicts_with = "no_console")]
        console: bool,
        /// Disable the interactive console.
        #[arg(long, action = ArgAction::SetTrue, conflicts_with = "console")]
        no_console: bool,
        /// Use beginner mode (limited controls).
        #[arg(long, action = ArgAction::SetTrue)]
        beginner: bool,
        /// Run in explicit simulation mode.
        #[arg(long, action = ArgAction::SetTrue)]
        simulation: bool,
        /// Simulation time acceleration factor (>= 1).
        #[arg(long, default_value_t = 1)]
        time_scale: u32,
    },
    /// Interactive TUI for monitoring and control.
    Ui {
        /// Project folder directory (auto-detect if omitted).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Control endpoint override (tcp://host:port or unix://path).
        #[arg(long)]
        endpoint: Option<String>,
        /// Control auth token (overrides project value).
        #[arg(long)]
        token: Option<String>,
        /// UI refresh interval in milliseconds.
        #[arg(long, default_value = "250")]
        refresh: u64,
        /// Read-only mode (monitor only).
        #[arg(long)]
        no_input: bool,
        /// Beginner mode (Play/Stop/Download/Debug only).
        #[arg(long)]
        beginner: bool,
    },
    /// Send control commands to a running runtime.
    Ctl {
        /// Project folder directory (to read control endpoint).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Control endpoint (tcp://host:port or unix://path).
        #[arg(long)]
        endpoint: Option<String>,
        /// Control auth token (overrides project value).
        #[arg(long)]
        token: Option<String>,
        #[command(subcommand)]
        action: ControlAction,
    },
    /// Validate a project folder (config + bytecode).
    Validate {
        /// Project folder directory.
        #[arg(long = "project", alias = "bundle")]
        project: PathBuf,
        /// Enable CI-friendly behavior and stable exit code mapping.
        #[arg(long, action = ArgAction::SetTrue)]
        ci: bool,
    },
    /// Build program.stbc from project sources.
    Build {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Sources directory override (defaults to <project>/sources).
        #[arg(long)]
        sources: Option<PathBuf>,
        /// Enable CI-friendly behavior and machine-readable output.
        #[arg(long, action = ArgAction::SetTrue)]
        ci: bool,
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
        #[arg(long, action = ArgAction::SetTrue)]
        list: bool,
        /// Per-test timeout in seconds.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
        /// Output format (`human`, `junit`, `tap`, `json`).
        #[arg(long, value_enum, default_value_t = TestOutput::Human)]
        output: TestOutput,
        /// Enable CI-friendly behavior (`human` output defaults to `junit`).
        #[arg(long, action = ArgAction::SetTrue)]
        ci: bool,
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
        #[arg(long, value_enum, default_value_t = DocsFormat::Both)]
        format: DocsFormat,
    },
    /// PLCopen XML interchange (strict subset profile).
    Plcopen {
        #[command(subcommand)]
        action: PlcopenAction,
    },
    /// Package registry workflows.
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },
    /// Initialize system IO configuration (writes /etc/trust/io.toml).
    #[command(
        after_help = "Examples:\n  trust-runtime setup\n  trust-runtime setup --mode cancel\n  trust-runtime setup --mode browser --access remote --project ./my-plc\n  trust-runtime setup --mode cli --project ./my-plc\n  trust-runtime setup --driver gpio --force\n  trust-runtime setup --path ./io.toml"
    )]
    Setup {
        /// Setup mode (`browser`, `cli`, `cancel`).
        #[arg(long, value_enum)]
        mode: Option<SetupModeArg>,
        /// Browser setup access profile (`local` uses loopback, `remote` requires token).
        #[arg(long, value_enum, default_value_t = SetupAccessArg::Local)]
        access: SetupAccessArg,
        /// Project folder for guided browser/CLI setup.
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Browser setup bind address override.
        #[arg(long)]
        bind: Option<String>,
        /// Browser setup HTTP port.
        #[arg(long, default_value_t = 8080)]
        port: u16,
        /// Browser setup token TTL in minutes (`remote` mode only).
        #[arg(long = "token-ttl-minutes")]
        token_ttl_minutes: Option<u64>,
        /// Preview setup plan without applying changes.
        #[arg(long, action = ArgAction::SetTrue)]
        dry_run: bool,
        /// Override driver selection (default is auto-detect).
        #[arg(long)]
        driver: Option<String>,
        /// Override GPIO backend (e.g., sysfs).
        #[arg(long)]
        backend: Option<String>,
        /// Override output path (default: system io.toml).
        #[arg(long)]
        path: Option<PathBuf>,
        /// Overwrite existing system config.
        #[arg(long)]
        force: bool,
    },
    /// Guided wizard to create a new project folder.
    #[command(alias = "init")]
    Wizard {
        /// Target directory (defaults to current directory).
        #[arg(long)]
        path: Option<PathBuf>,
        /// Start the runtime after creating the project folder.
        #[arg(long)]
        start: bool,
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
    /// Deploy a project folder into a versioned store with rollback support.
    Deploy {
        /// Source project folder directory.
        #[arg(long = "project", alias = "bundle")]
        project: PathBuf,
        /// Deployment root (defaults to current directory).
        #[arg(long)]
        root: Option<PathBuf>,
        /// Custom deployment label (defaults to project-<timestamp>).
        #[arg(long)]
        label: Option<String>,
        /// Restart mode after deployment (optional).
        #[arg(long)]
        restart: Option<String>,
    },
    /// Roll back to the previous project version in a deployment root.
    Rollback {
        /// Deployment root (defaults to current directory).
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Generate shell completions.
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TestOutput {
    Human,
    Junit,
    Tap,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DocsFormat {
    Markdown,
    Html,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SetupModeArg {
    Browser,
    Cli,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SetupAccessArg {
    Local,
    Remote,
}

#[derive(Debug, Subcommand)]
pub enum PlcopenAction {
    /// Print supported PLCopen profile and strict subset contract.
    Profile {
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Export project sources to PLCopen XML.
    Export {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Output XML file path (defaults to <project>/interop/plcopen.xml).
        #[arg(long = "output")]
        output: Option<PathBuf>,
    },
    /// Import PLCopen XML into project sources.
    Import {
        /// Input PLCopen XML file.
        #[arg(long = "input")]
        input: PathBuf,
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RegistryVisibilityArg {
    Public,
    Private,
}

#[derive(Debug, Subcommand)]
pub enum RegistryAction {
    /// Print package registry API contract and metadata model.
    Profile {
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Initialize a local registry root directory.
    Init {
        /// Registry root directory.
        #[arg(long = "root")]
        root: PathBuf,
        /// Registry visibility mode.
        #[arg(long, value_enum, default_value_t = RegistryVisibilityArg::Public)]
        visibility: RegistryVisibilityArg,
        /// Shared access token for private registries.
        #[arg(long)]
        token: Option<String>,
    },
    /// Publish a bundle into the registry.
    Publish {
        /// Registry root directory.
        #[arg(long = "registry")]
        registry: PathBuf,
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Override package name (defaults to runtime resource name).
        #[arg(long = "name")]
        name: Option<String>,
        /// Package version identifier.
        #[arg(long = "version")]
        version: String,
        /// Access token for private registries.
        #[arg(long)]
        token: Option<String>,
    },
    /// Download a bundle from the registry.
    Download {
        /// Registry root directory.
        #[arg(long = "registry")]
        registry: PathBuf,
        /// Package name.
        #[arg(long = "name")]
        name: String,
        /// Package version identifier.
        #[arg(long = "version")]
        version: String,
        /// Output directory for the downloaded bundle payload.
        #[arg(long = "output")]
        output: PathBuf,
        /// Access token for private registries.
        #[arg(long)]
        token: Option<String>,
        /// Verify digest metadata before and after install copy.
        #[arg(long, action = ArgAction::SetTrue)]
        verify: bool,
    },
    /// Verify registry payload digests against package metadata.
    Verify {
        /// Registry root directory.
        #[arg(long = "registry")]
        registry: PathBuf,
        /// Package name.
        #[arg(long = "name")]
        name: String,
        /// Package version identifier.
        #[arg(long = "version")]
        version: String,
        /// Access token for private registries.
        #[arg(long)]
        token: Option<String>,
    },
    /// List published packages.
    List {
        /// Registry root directory.
        #[arg(long = "registry")]
        registry: PathBuf,
        /// Access token for private registries.
        #[arg(long)]
        token: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
#[command(infer_subcommands = true)]
pub enum ControlAction {
    Status,
    Health,
    Stats,
    Pause,
    Resume,
    StepIn,
    StepOver,
    StepOut,
    BreakpointsSet { source: String, lines: Vec<u32> },
    BreakpointsClear { source: String },
    BreakpointsList,
    IoRead,
    IoWrite { address: String, value: String },
    IoForce { address: String, value: String },
    IoUnforce { address: String },
    Eval { expr: String },
    Set { target: String, value: String },
    Restart { mode: String },
    Shutdown,
    ConfigGet,
    ConfigSet { key: String, value: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_build_ci_flag() {
        let cli = Cli::parse_from(["trust-runtime", "build", "--ci"]);
        match cli.command.expect("command") {
            Command::Build { ci, .. } => assert!(ci),
            other => panic!("expected build command, got {other:?}"),
        }
    }

    #[test]
    fn parse_validate_ci_flag() {
        let cli = Cli::parse_from(["trust-runtime", "validate", "--project", "project", "--ci"]);
        match cli.command.expect("command") {
            Command::Validate { ci, .. } => assert!(ci),
            other => panic!("expected validate command, got {other:?}"),
        }
    }

    #[test]
    fn parse_test_ci_flag() {
        let cli = Cli::parse_from(["trust-runtime", "test", "--project", "project", "--ci"]);
        match cli.command.expect("command") {
            Command::Test { ci, .. } => assert!(ci),
            other => panic!("expected test command, got {other:?}"),
        }
    }

    #[test]
    fn parse_docs_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "docs",
            "--project",
            "project",
            "--out-dir",
            "out",
            "--format",
            "markdown",
        ]);
        match cli.command.expect("command") {
            Command::Docs {
                project,
                out_dir,
                format,
            } => {
                assert_eq!(project, Some(PathBuf::from("project")));
                assert_eq!(out_dir, Some(PathBuf::from("out")));
                assert_eq!(format, DocsFormat::Markdown);
            }
            other => panic!("expected docs command, got {other:?}"),
        }
    }

    #[test]
    fn parse_plcopen_export_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "plcopen",
            "export",
            "--project",
            "project",
            "--output",
            "out.xml",
        ]);
        match cli.command.expect("command") {
            Command::Plcopen { action } => match action {
                PlcopenAction::Export { project, output } => {
                    assert_eq!(project, Some(PathBuf::from("project")));
                    assert_eq!(output, Some(PathBuf::from("out.xml")));
                }
                other => panic!("expected plcopen export action, got {other:?}"),
            },
            other => panic!("expected plcopen command, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_simulation_flags() {
        let cli = Cli::parse_from(["trust-runtime", "play", "--simulation", "--time-scale", "8"]);
        match cli.command.expect("command") {
            Command::Play {
                simulation,
                time_scale,
                ..
            } => {
                assert!(simulation);
                assert_eq!(time_scale, 8);
            }
            other => panic!("expected play command, got {other:?}"),
        }
    }

    #[test]
    fn parse_setup_cancel_mode() {
        let cli = Cli::parse_from(["trust-runtime", "setup", "--mode", "cancel"]);
        match cli.command.expect("command") {
            Command::Setup { mode, .. } => assert_eq!(mode, Some(SetupModeArg::Cancel)),
            other => panic!("expected setup command, got {other:?}"),
        }
    }

    #[test]
    fn parse_registry_private_init_command() {
        let cli = Cli::parse_from([
            "trust-runtime",
            "registry",
            "init",
            "--root",
            "registry",
            "--visibility",
            "private",
            "--token",
            "secret",
        ]);
        match cli.command.expect("command") {
            Command::Registry { action } => match action {
                RegistryAction::Init {
                    root,
                    visibility,
                    token,
                } => {
                    assert_eq!(root, PathBuf::from("registry"));
                    assert_eq!(visibility, RegistryVisibilityArg::Private);
                    assert_eq!(token, Some("secret".to_string()));
                }
                other => panic!("expected registry init action, got {other:?}"),
            },
            other => panic!("expected registry command, got {other:?}"),
        }
    }
}
