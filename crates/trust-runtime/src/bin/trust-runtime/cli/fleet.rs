#[derive(Debug, Subcommand)]
pub enum FleetAction {
    /// Manage offline fleet runtime projects.
    Runtime {
        #[command(subcommand)]
        action: FleetRuntimeAction,
    },
    /// List runtimes registered in an offline fleet manifest.
    List {
        /// Fleet root directory containing fleet.toml.
        #[arg(long = "fleet-root")]
        fleet_root: PathBuf,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum FleetRuntimeAction {
    /// Scaffold and register a sibling runtime project under a fleet root.
    Add {
        /// Fleet root directory containing or receiving fleet.toml.
        #[arg(long = "fleet-root")]
        fleet_root: PathBuf,
        /// Runtime project name. Used as the folder name.
        #[arg(long)]
        name: String,
        /// Runtime project template.
        #[arg(long, value_enum, default_value_t = FleetRuntimeTemplateArg::Simulate)]
        template: FleetRuntimeTemplateArg,
        /// Control TCP port. If omitted, a free loopback port is selected.
        #[arg(long = "control-port")]
        control_port: Option<u16>,
        /// Web UI TCP port. If omitted, a free loopback port is selected.
        #[arg(long = "web-port")]
        web_port: Option<u16>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Start a registered local runtime project as a managed background process.
    Start {
        /// Fleet root directory containing fleet.toml.
        #[arg(long = "fleet-root")]
        fleet_root: PathBuf,
        /// Runtime project name from fleet.toml.
        #[arg(long)]
        name: String,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Stop a registered local runtime through its control endpoint.
    Stop {
        /// Fleet root directory containing fleet.toml.
        #[arg(long = "fleet-root")]
        fleet_root: PathBuf,
        /// Runtime project name from fleet.toml.
        #[arg(long)]
        name: String,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Report managed local runtime status.
    Status {
        /// Fleet root directory containing fleet.toml.
        #[arg(long = "fleet-root")]
        fleet_root: PathBuf,
        /// Runtime project name from fleet.toml.
        #[arg(long)]
        name: String,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Print recent managed local runtime logs.
    Logs {
        /// Fleet root directory containing fleet.toml.
        #[arg(long = "fleet-root")]
        fleet_root: PathBuf,
        /// Runtime project name from fleet.toml.
        #[arg(long)]
        name: String,
        /// Number of recent lines to return.
        #[arg(long, default_value_t = 200)]
        lines: usize,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FleetRuntimeTemplateArg {
    /// Simulated I/O project for first-run/zero-hardware setup.
    Simulate,
    /// Minimal source project with loopback I/O wiring.
    Empty,
}
