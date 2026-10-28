/// The arguments of `trust-runtime identify`, which makes a runtime show where it is by blinking
/// its identify output (specification 11, "Identify").
#[derive(Debug, clap::Args)]
pub struct IdentifyArgs {
    /// Control endpoint of the runtime (tcp://host:port or unix:///path).
    #[arg(long)]
    pub endpoint: Option<String>,
    /// Project folder: its runtime.toml gives the control endpoint and token that --endpoint and
    /// --token do not.
    #[arg(long = "project", alias = "bundle")]
    pub project: Option<std::path::PathBuf>,
    /// Control auth token (defaults to TRUST_CTL_TOKEN, then the project's token).
    #[arg(long)]
    pub token: Option<String>,
    /// How long the runtime blinks, in seconds: 1 to 60.
    #[arg(long = "duration-s")]
    pub duration_s: u64,
}
