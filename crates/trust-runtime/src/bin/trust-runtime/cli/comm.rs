#[derive(Debug, Subcommand)]
pub enum CommAction {
    /// Print the static communication setup schema.
    Schema {
        /// Restrict the schema to one protocol id, for example opcua or modbus-tcp.
        #[arg(long)]
        protocol: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Print project topology from runtime.toml/io.toml without starting the runtime.
    Topology {
        /// Project folder directory.
        #[arg(long = "project", alias = "bundle")]
        project: PathBuf,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Apply a communication setup change to project files without starting the runtime.
    Apply {
        /// Project folder directory.
        #[arg(long = "project", alias = "bundle")]
        project: PathBuf,
        /// Communication protocol id, for example modbus-tcp or mqtt.
        #[arg(long)]
        protocol: String,
        /// JSON object containing protocol parameters.
        #[arg(long)]
        params: String,
        /// Apply action.
        #[arg(long, value_enum, default_value_t = CommApplyCliAction::Upsert)]
        action: CommApplyCliAction,
        /// Existing instance id for edit/remove/disable.
        #[arg(long = "instance-id")]
        instance_id: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Discover communication targets and return setup-form parameters.
    Discover {
        /// Communication protocol id, for example ads, discovery, or modbus-tcp.
        #[arg(long)]
        protocol: String,
        /// Scan origin. `this-host` runs from this CLI process; `runtime` is the same host when the CLI runs on the runtime.
        #[arg(long, value_enum, default_value_t = CommDiscoverOriginArg::ThisHost)]
        origin: CommDiscoverOriginArg,
        /// Optional CIDR for subnet scans, for example 192.168.1.0/24.
        #[arg(long)]
        cidr: Option<String>,
        /// Optional target host for directed discovery/probe.
        #[arg(long)]
        host: Option<String>,
        /// Optional runtime-host hardware adapter for fieldbus discovery, for example eth0.
        #[arg(long)]
        adapter: Option<String>,
        /// Timeout budget in milliseconds.
        #[arg(long = "timeout-ms")]
        timeout_ms: Option<u64>,
        /// Passive/read-only discovery. Active write probes are never used.
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        passive: bool,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Browse symbols/nodes/channels inside a discovered or configured target.
    BrowseSymbols {
        /// Communication protocol id. ADS is supported first.
        #[arg(long)]
        protocol: String,
        /// Project folder directory for offline local symbol/channel browsing.
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
        /// Target JSON object. For ADS: {"host":"192.168.1.10","ams_net_id":"5.23.91.12.1.1","name":"CX"}.
        #[arg(long)]
        target: Option<String>,
        /// Cached ADS symbol snapshot JSON file for offline browsing.
        #[arg(long = "snapshot-file")]
        snapshot_file: Option<PathBuf>,
        /// Existing configured instance id.
        #[arg(long = "instance-id")]
        instance_id: Option<String>,
        /// Browse kind: symbols, nodes, or channels.
        #[arg(long, default_value = "symbols")]
        kind: String,
        /// Connection name for ADS import/generation surfaces.
        #[arg(long = "connection-name")]
        connection_name: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Inspect or clear trusted OPC UA client server certificates.
    #[command(name = "opcua-trust")]
    OpcUaTrust {
        /// Trust-store action.
        #[arg(value_enum)]
        action: CommOpcUaTrustAction,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CommApplyCliAction {
    /// Add another instance of the protocol.
    Add,
    /// Edit the selected instance.
    Edit,
    /// Create or replace the matching instance.
    Upsert,
    /// Remove the matching instance.
    Remove,
    /// Disable the selected protocol or instance while preserving parameters.
    Disable,
    /// Validate only.
    Validate,
}

impl CommApplyCliAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Edit => "edit",
            Self::Upsert => "upsert",
            Self::Remove => "remove",
            Self::Disable => "disable",
            Self::Validate => "validate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CommDiscoverOriginArg {
    /// Scan from the machine running this CLI.
    ThisHost,
    /// Scan from the runtime host. With the CLI, this means this CLI is running on that host.
    Runtime,
}

impl CommDiscoverOriginArg {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ThisHost => "this_host",
            Self::Runtime => "runtime",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CommOpcUaTrustAction {
    /// List trusted OPC UA server certificates.
    List,
    /// Clear trusted OPC UA server certificates.
    Clear,
}
