#[derive(Debug, Subcommand)]
pub enum AgentAction {
    /// Deprecated alias for `trust-dev agent serve`.
    Serve {
        /// Workspace/project root (defaults to current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
    },
}
