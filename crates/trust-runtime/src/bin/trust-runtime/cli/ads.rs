#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AdsRouteArtifactFormat {
    /// Generate a PowerShell script that adds or replaces the route.
    Powershell,
    /// Generate a StaticRoutes.xml snippet.
    Staticroutes,
    /// Generate manual TwinCAT route instructions.
    Gui,
    /// Generate a PowerShell script that removes the named route.
    RemovalPowershell,
}

#[derive(Debug, Subcommand)]
pub enum AdsAction {
    /// Discover ADS routes/devices on the local network.
    Discover {
        /// Optional target PLC IP/hostname for directed identify.
        #[arg(long = "target")]
        target: Option<String>,
        /// Optional target AMS Net ID for manual routed-network entry.
        #[arg(long = "target-net-id")]
        target_net_id: Option<String>,
        /// Target PLC AMS port.
        #[arg(long = "ams-port", default_value_t = 851)]
        ams_port: u16,
        /// Skip directed broadcast discovery.
        #[arg(long = "no-broadcast", action = ArgAction::SetTrue)]
        no_broadcast: bool,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Run the ADS doctor from this host.
    Doctor {
        /// Target PLC IP address.
        #[arg(long = "target")]
        target: String,
        /// Target PLC AMS Net ID.
        #[arg(long = "target-net-id")]
        target_net_id: Option<String>,
        /// Target PLC AMS port.
        #[arg(long = "ams-port", default_value_t = 851)]
        ams_port: u16,
        /// Writable TwinCAT symbol to probe. Requires --write-type and --write-value.
        #[arg(long = "write-symbol")]
        write_symbol: Option<String>,
        /// IEC scalar type for --write-symbol, for example REAL.
        #[arg(long = "write-type")]
        write_type: Option<String>,
        /// Probe value to write, read back, and restore.
        #[arg(long = "write-value")]
        write_value: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Browse symbols from a configured ADS connection.
    Browse {
        /// ADS config file.
        #[arg(long = "config")]
        config: PathBuf,
        /// Connection name to browse.
        #[arg(long = "connection")]
        connection: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Generate ADS route setup artifacts.
    RouteScript {
        /// Route name to create on the TwinCAT target.
        #[arg(long = "route-name")]
        route_name: String,
        /// Target PLC IP address.
        #[arg(long = "target")]
        target: String,
        /// Target PLC AMS Net ID.
        #[arg(long = "target-net-id")]
        target_net_id: String,
        /// Target PLC AMS port.
        #[arg(long = "ams-port", default_value_t = 851)]
        ams_port: u16,
        /// Runtime host IP address that the PLC should route back to.
        #[arg(long = "local-ip")]
        local_ip: String,
        /// Runtime host AMS Net ID that the PLC should route back to.
        #[arg(long = "local-net-id")]
        local_net_id: String,
        /// Artifact format.
        #[arg(long = "format", value_enum, default_value_t = AdsRouteArtifactFormat::Powershell)]
        format: AdsRouteArtifactFormat,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Add an ADS route by sending UDP AddRoute directly to the PLC.
    AddRoute {
        /// Route name to create on the TwinCAT target.
        #[arg(long = "route-name")]
        route_name: String,
        /// Target PLC IP address.
        #[arg(long = "target")]
        target: String,
        /// Target PLC AMS Net ID.
        #[arg(long = "target-net-id")]
        target_net_id: String,
        /// Target PLC AMS port.
        #[arg(long = "ams-port", default_value_t = 851)]
        ams_port: u16,
        /// Runtime host IP address that the PLC should route back to.
        #[arg(long = "local-ip")]
        local_ip: String,
        /// Runtime host AMS Net ID that the PLC should route back to.
        #[arg(long = "local-net-id")]
        local_net_id: String,
        /// TwinCAT user name.
        #[arg(long = "username", default_value = "Administrator")]
        username: String,
        /// Read the TwinCAT password from standard input. Password argv is intentionally unsupported.
        #[arg(long = "password-stdin", action = ArgAction::SetTrue)]
        password_stdin: bool,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Generate a route-removal artifact.
    RouteRemove {
        /// Route name to remove from the TwinCAT target.
        #[arg(long = "route-name")]
        route_name: String,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Import TwinCAT symbols in local authoring-only mode.
    ImportSymbols {
        /// Target PLC IP address.
        #[arg(long = "target")]
        target: String,
        /// Target PLC AMS Net ID.
        #[arg(long = "target-net-id")]
        target_net_id: Option<String>,
        /// Target PLC AMS port.
        #[arg(long = "ams-port", default_value_t = 851)]
        ams_port: u16,
        /// Connection name to write into ads.toml.
        #[arg(long = "connection", default_value = "line1")]
        connection: String,
        /// Optional generated variable name prefix.
        #[arg(long = "name-prefix")]
        name_prefix: Option<String>,
        /// Optional symbol include pattern. Repeat to select a subset.
        #[arg(long = "include")]
        include_patterns: Vec<String>,
        /// Output ads.toml path.
        #[arg(long = "out")]
        out: PathBuf,
        /// Output cached symbol snapshot path. Defaults to ads/snapshots/<connection>.symbols.json beside ads.toml.
        #[arg(long = "snapshot-out")]
        snapshot_out: Option<PathBuf>,
        /// Existing cached ADS symbol snapshot for other connections. Repeat for multiple connections.
        #[arg(long = "existing-snapshot")]
        existing_snapshots: Vec<PathBuf>,
        /// Output generated ST path.
        #[arg(long = "gen")]
        generated: PathBuf,
        /// Overwrite a changed generated ST file.
        #[arg(long, action = ArgAction::SetTrue)]
        force: bool,
        /// Preview generated files without writing them. JSON output includes file contents for UI diff.
        #[arg(long = "dry-run", action = ArgAction::SetTrue)]
        dry_run: bool,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Generate deterministic ST globals from ads.toml and cached symbol snapshots.
    Import {
        /// ADS config file.
        #[arg(long = "config")]
        config: PathBuf,
        /// Cached ADS symbol snapshot JSON. Repeat for multiple connections.
        #[arg(long = "snapshot", required = true)]
        snapshots: Vec<PathBuf>,
        /// Output ST file.
        #[arg(long = "output")]
        output: PathBuf,
        /// Overwrite a changed output file.
        #[arg(long, action = ArgAction::SetTrue)]
        force: bool,
        /// Print machine-readable JSON report.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Validate generated ADS ST against ads.toml and cached/live ADS metadata.
    Validate {
        /// Validate against cached snapshots only; no PLC connection is opened.
        #[arg(long, action = ArgAction::SetTrue, conflicts_with = "live")]
        offline: bool,
        /// Validate against the live ADS endpoint.
        #[arg(long, action = ArgAction::SetTrue, conflicts_with = "offline")]
        live: bool,
        /// ADS config file.
        #[arg(long = "config")]
        config: PathBuf,
        /// Cached ADS symbol snapshot JSON. Required for --offline; repeat for multiple connections.
        #[arg(long = "snapshot")]
        snapshots: Vec<PathBuf>,
        /// Generated ADS ST file to validate.
        #[arg(long = "generated")]
        generated: PathBuf,
        /// Print machine-readable JSON report.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// ADS server operations for exposing truST variables to external ADS clients.
    Server {
        #[command(subcommand)]
        action: AdsServerAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum AdsServerAction {
    /// Query ADS server status from a running runtime control endpoint.
    Status {
        /// Project folder directory (to read control endpoint).
        #[arg(long = "project")]
        project: Option<PathBuf>,
        /// Runtime control endpoint, for example tcp://127.0.0.1:9000.
        #[arg(long = "endpoint")]
        endpoint: Option<String>,
        /// Runtime control auth token. Defaults to TRUST_CTL_TOKEN or project config.
        #[arg(long = "token")]
        token: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Browse symbols exposed by a running truST ADS server.
    Symbols {
        /// Project folder directory (to read control endpoint).
        #[arg(long = "project")]
        project: Option<PathBuf>,
        /// Runtime control endpoint, for example tcp://127.0.0.1:9000.
        #[arg(long = "endpoint")]
        endpoint: Option<String>,
        /// Runtime control auth token. Defaults to TRUST_CTL_TOKEN or project config.
        #[arg(long = "token")]
        token: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Run the ADS server doctor against a running runtime control endpoint.
    Doctor {
        /// Project folder directory (to read control endpoint).
        #[arg(long = "project")]
        project: Option<PathBuf>,
        /// Runtime control endpoint, for example tcp://127.0.0.1:9000.
        #[arg(long = "endpoint")]
        endpoint: Option<String>,
        /// Runtime control auth token. Defaults to TRUST_CTL_TOKEN or project config.
        #[arg(long = "token")]
        token: Option<String>,
        /// External client kind that has independently verified the server, for example pyads.
        #[arg(long = "external-kind")]
        external_kind: Option<String>,
        /// External client name.
        #[arg(long = "external-name")]
        external_name: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Generate ADS-server route setup artifacts for external clients.
    RouteScript {
        /// Route name to create on the ADS client / TwinCAT engineering station.
        #[arg(long = "route-name")]
        route_name: String,
        /// truST runtime host IP address.
        #[arg(long = "server-ip")]
        server_ip: String,
        /// truST ADS server AMS Net ID.
        #[arg(long = "server-net-id")]
        server_net_id: String,
        /// truST ADS logical AMS port.
        #[arg(long = "ads-port", default_value_t = 851)]
        ads_port: u16,
        /// Artifact format.
        #[arg(long = "format", value_enum, default_value_t = AdsRouteArtifactFormat::Powershell)]
        format: AdsRouteArtifactFormat,
        /// Print machine-readable JSON.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
}
