#[derive(Debug, Subcommand)]
pub enum AgentAction {
    /// Deprecated alias for `trust-dev agent serve`.
    #[command(after_help = "Canonical:\n  trust-dev agent serve --project ./my-plc\n\nDeprecated compatibility alias, removal no earlier than 2026-10-05:\n  trust-runtime agent serve --project ./my-plc")]
    Serve {
        /// Workspace/project root (defaults to current directory).
        #[arg(long = "project", alias = "bundle")]
        project: Option<PathBuf>,
    },
}
