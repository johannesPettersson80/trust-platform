/// The arguments of `trust-runtime discover`, the one search (specification 11, "One search,
/// device identity, and presence").
#[derive(Debug, clap::Args)]
pub struct DiscoverArgs {
    /// Run the search inside the runtime at this control endpoint (tcp://host:port,
    /// unix:///path or serial:///dev/ttyACM0 for a chip on USB) instead of on this computer.
    #[arg(long)]
    pub endpoint: Option<String>,
    /// Control auth token for --endpoint (defaults to TRUST_CTL_TOKEN).
    #[arg(long)]
    pub token: Option<String>,
    /// Search only these sources: trust_runtime, ads, modbus_tcp, mqtt, opcua, ethercat, gpio,
    /// serial, usb, bluetooth.
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,
    /// A known address: a host name or IP address with an optional port. Repeat for more.
    #[arg(long = "host")]
    pub hosts: Vec<String>,
    /// An IPv4 subnet of at most /24 to search, for example 192.168.1.0/24.
    #[arg(long)]
    pub cidr: Option<String>,
    /// How long the search may take, in milliseconds: 1 to 60000 (default 10000).
    #[arg(long = "deadline-ms")]
    pub deadline_ms: Option<u64>,
    /// Confirm a fieldbus scan: `ethercat`, or `ethercat:<adapter>` to name the adapter. Repeat
    /// for more adapters.
    #[arg(long = "confirm-fieldbus")]
    pub confirm_fieldbus: Vec<String>,
    /// Print one JSON object per line.
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
    /// A project folder: read the choices that its Discover pane remembers
    /// (`<project>/.trust-runtime/devices.toml`). Without --only, the remembered sources run, and
    /// the devices that the project ignores are left out.
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Show the devices that the project ignores; they are left out without this option.
    #[arg(long = "show-ignored", action = ArgAction::SetTrue)]
    pub show_ignored: bool,
}
