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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FleetRuntimeTemplateArg {
    /// Simulated I/O project for first-run/zero-hardware setup.
    Simulate,
    /// Minimal source project with loopback I/O wiring.
    Empty,
}
