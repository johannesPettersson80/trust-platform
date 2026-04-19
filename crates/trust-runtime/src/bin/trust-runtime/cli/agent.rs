#[derive(Debug, Subcommand)]
pub enum AgentAction {
    /// Serve the external agent contract over stdio JSON-RPC.
    Serve {
        /// Workspace/project root (defaults to current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
    },
}
