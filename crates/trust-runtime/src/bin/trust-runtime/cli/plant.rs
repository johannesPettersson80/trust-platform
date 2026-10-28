#[derive(Debug, Subcommand)]
pub enum PlantAction {
    /// Serve stopped-project plant requests as JSON lines on stdin/stdout.
    Serve {
        /// Project folder containing simulation.toml.
        #[arg(long = "project", alias = "bundle")]
        project: PathBuf,
    },
}
