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
        /// Project folder directory (defaults to standard bundle detection).
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
