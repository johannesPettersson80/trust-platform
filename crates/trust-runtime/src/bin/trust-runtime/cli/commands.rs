#[derive(Debug, Parser)]
#[command(
    name = "trust-runtime",
    version,
    about = "Structured Text runtime CLI",
    infer_subcommands = true,
    arg_required_else_help = false,
    after_help = "Examples:\n  trust-runtime                         # start (first run opens setup)\n  trust-runtime --verbose               # show startup details\n  trust-runtime ide serve --project .   # standalone browser IDE\n  trust-runtime ui --project ./my-plc   # terminal UI\n  trust-runtime check --project ./my-plc --json # check sources/config without writing the program image\n  trust-runtime comm topology --project ./my-plc --json # inspect offline comm topology\n  trust-runtime comm discover --protocol ads --json # find communication targets from this host\n  trust-runtime fleet runtime add --fleet-root ./fleet --name cell1 --json # scaffold sibling runtime\n  trust-runtime play --project ./my-plc # compatibility\n\nWorkbench commands:\n  use trust-dev test/docs/commit/agent serve. Deprecated trust-runtime aliases remain only until the removal window, no earlier than 2026-10-05."
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
        #[arg(long = "project", alias = "bundle", global = true)]
        project: Option<PathBuf>,
        /// Control endpoint (tcp://host:port, unix://path or serial://port for a chip on USB).
        #[arg(long, global = true)]
        endpoint: Option<String>,
        /// Control auth token (overrides project value).
        #[arg(long, global = true)]
        token: Option<String>,
        #[command(subcommand)]
        action: ControlAction,
    },
    /// Validate a project folder (config + program image).
    Validate {
        /// Project folder directory.
        #[arg(long = "project", alias = "bundle")]
        project: PathBuf,
        /// Enable CI-friendly behavior and stable exit code mapping.
        #[arg(long, action = ArgAction::SetTrue)]
        ci: bool,
    },
    /// Check project sources and config without writing program.tpi.
    Check {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Sources directory override (defaults to <project>/src).
        #[arg(long)]
        sources: Option<PathBuf>,
        /// Emit machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
        /// Enable CI-friendly behavior, machine-readable output, and stable exit code mapping.
        #[arg(long, action = ArgAction::SetTrue)]
        ci: bool,
    },
    /// Build the project: this computer's program image (program.tpi), or a device's with --target.
    Build {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Sources directory override (defaults to <project>/src).
        #[arg(long)]
        sources: Option<PathBuf>,
        /// Enable CI-friendly behavior and machine-readable output.
        #[arg(long, action = ArgAction::SetTrue)]
        ci: bool,
        /// Compile the project into a program image for this target, in the project's build
        /// folder, instead of this computer's program image (specification 44 §4.1): a device
        /// (esp32c6, stm32u5a9), a CPU (x86_64, aarch64, riscv32imac, thumbv8m), a Rust target
        /// triple, or `host`.
        #[arg(long)]
        target: Option<String>,
        /// With --target: keep the generated Rust crate next to the image.
        #[arg(long, action = ArgAction::SetTrue)]
        emit_source: bool,
    },
    /// Deprecated alias for `trust-dev test`.
    #[command(
        after_help = "Canonical:\n  trust-dev test --project ./my-plc\n\nDeprecated compatibility alias, removal no earlier than 2026-10-05:\n  trust-runtime test --project ./my-plc"
    )]
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
    /// Deprecated alias for `trust-dev docs`.
    #[command(
        after_help = "Canonical:\n  trust-dev docs --project ./my-plc\n\nDeprecated compatibility alias, removal no earlier than 2026-10-05:\n  trust-runtime docs --project ./my-plc"
    )]
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
    /// Human-machine-interface scaffold workflows.
    Hmi {
        /// Project folder directory (defaults to auto-detect or current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        #[command(subcommand)]
        action: HmiAction,
    },
    /// PLCopen XML interchange (ST-complete profile).
    Plcopen {
        #[command(subcommand)]
        action: PlcopenAction,
    },
    /// Beckhoff ADS symbol import and validation workflows.
    Ads {
        #[command(subcommand)]
        action: AdsAction,
    },
    /// Offline communication setup and topology workflows.
    Comm {
        #[command(subcommand)]
        action: CommAction,
    },
    /// Search for devices: truST runtimes, ADS, Modbus TCP, MQTT, OPC UA, EtherCAT and GPIO.
    #[command(
        after_help = "Examples:\n  trust-runtime discover\n  trust-runtime discover --only modbus_tcp,mqtt --host 192.168.1.20 --json\n  trust-runtime discover --cidr 192.168.1.0/24 --deadline-ms 20000\n  trust-runtime discover --endpoint tcp://192.168.1.5:5680 --confirm-fieldbus ethercat:eth1"
    )]
    Discover(DiscoverArgs),
    /// Offline fleet manifest and sibling-runtime workflows.
    Fleet {
        #[command(subcommand)]
        action: FleetAction,
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
        #[arg(long, value_enum)]
        access: Option<SetupAccessArg>,
        /// Project folder for guided browser/CLI setup.
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Browser setup bind address override.
        #[arg(long)]
        bind: Option<String>,
        /// Browser setup HTTP port.
        #[arg(long)]
        port: Option<u16>,
        /// Browser setup token TTL in minutes (`remote` mode only).
        #[arg(long = "token-ttl-minutes")]
        token_ttl_minutes: Option<u64>,
        /// Preview setup plan without applying changes.
        #[arg(long, action = ArgAction::SetTrue)]
        dry_run: bool,
        /// Override driver selection (default is auto-detect).
        #[arg(long)]
        driver: Option<String>,
        /// Override GPIO backend (libgpiod/chardev by default; sysfs for legacy hosts).
        #[arg(long)]
        backend: Option<String>,
        /// Override output path (default: system io.toml).
        #[arg(long)]
        path: Option<PathBuf>,
        /// Overwrite existing system config.
        #[arg(long)]
        force: bool,
    },
    /// Serve the standalone browser IDE without starting PLC execution.
    #[command(
        name = "ide",
        after_help = "Examples:\n  trust-runtime ide serve --project ./my-plc\n  trust-runtime ide serve --project ./workspace --listen 127.0.0.1:18080"
    )]
    Ide {
        #[command(subcommand)]
        action: ConfigUiAction,
    },
    /// Deprecated alias for `ide`.
    #[command(
        name = "config-ui",
        after_help = "Examples:\n  trust-runtime config-ui serve --project ./my-plc\n  trust-runtime config-ui serve --project ./workspace --listen 127.0.0.1:18080"
    )]
    ConfigUi {
        #[command(subcommand)]
        action: ConfigUiAction,
    },
    /// Deprecated alias for `trust-dev agent serve`.
    #[command(
        after_help = "Canonical:\n  trust-dev agent serve --project ./my-plc\n\nDeprecated compatibility alias, removal no earlier than 2026-10-05:\n  trust-runtime agent serve --project ./my-plc"
    )]
    Agent {
        #[command(subcommand)]
        action: AgentAction,
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
    /// Deprecated alias for `trust-dev commit`.
    #[command(
        after_help = "Canonical:\n  trust-dev commit --project .\n\nDeprecated compatibility alias, removal no earlier than 2026-10-05:\n  trust-runtime commit --project ."
    )]
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
    /// Run runtime communication benchmark scenarios.
    #[command(after_help = "Examples:
  trust-runtime bench project --project examples/plcopen_motion_single_axis_demo --watch g_motion_demo_completed_sequences --output json
  trust-runtime bench t0-shm --samples 2000 --output json
  trust-runtime bench mesh-zenoh --samples 1000 --loss-rate 0.01 --reorder-rate 0.02
  trust-runtime bench dispatch --fanout 4 --output table")]
    Bench {
        #[command(subcommand)]
        action: BenchAction,
    },
    /// Run deterministic conformance suite cases.
    #[command(
        after_help = "Examples:\n  trust-runtime conformance\n  trust-runtime conformance --suite-root ./conformance --output ./conformance/reports/local.json\n  trust-runtime conformance --update-expected"
    )]
    Conformance(ConformanceArgs),
    /// The compiler toolchain that builds compiled programs (specification 44 §4.6).
    #[command(
        after_help = "Examples:\n  trust-runtime toolchain status\n  trust-runtime toolchain install\n  trust-runtime toolchain install --from ./trust-toolchain-linux-x64.tar.gz"
    )]
    Toolchain {
        #[command(subcommand)]
        action: ToolchainAction,
    },
}

/// The options of `conformance`.
#[derive(Debug, clap::Args)]
pub struct ConformanceArgs {
    /// Conformance suite root directory.
    #[arg(long = "suite-root")]
    pub suite_root: Option<PathBuf>,
    /// Summary output file path (JSON).
    #[arg(long = "output")]
    pub output: Option<PathBuf>,
    /// Refresh expected artifacts from current runtime behavior.
    #[arg(long, action = ArgAction::SetTrue)]
    pub update_expected: bool,
    /// Optional case-insensitive substring filter for case IDs.
    #[arg(long)]
    pub filter: Option<String>,
    /// The serial port of a device that runs the runtime cases, or
    /// `tcp://<host>:<port>` of a bridge to that port: each case is compiled for the device's
    /// `--target`, installed, and stepped cycle by cycle with `cycle.step` (specification 44 §14.2).
    #[arg(long, requires = "target")]
    pub device: Option<String>,
    /// The device's target, as `trust-runtime build --target` names it: `esp32c6`, `stm32u5a9`.
    #[arg(long, requires = "device")]
    pub target: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigUiAction {
    /// Serve config/project UI against TOML/ST files only.
    Serve {
        /// Project root (single runtime project or workspace containing runtime-* folders).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// HTTP listen address.
        #[arg(long, default_value = "127.0.0.1:18080")]
        listen: String,
    },
}
