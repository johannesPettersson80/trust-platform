/// What `toolchain` does (specification 44 §4.6).
#[derive(Debug, Subcommand)]
pub enum ToolchainAction {
    /// Name the toolchain the tools use, where it is and its version, or that none is installed.
    Status {
        /// Write one JSON object.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Install this release's toolchain bundle, downloaded from the release or from a bundle file.
    Install {
        /// A bundle file to install instead of the download.
        #[arg(long)]
        from: Option<PathBuf>,
        /// Write the progress as one JSON object per line.
        #[arg(long, action = ArgAction::SetTrue)]
        json: bool,
    },
    /// Write the toolchain bundle of this computer's platform from a development toolchain folder.
    Bundle {
        /// The development toolchain folder (scripts/native/build_toolchain_libraries.sh).
        folder: PathBuf,
        /// The bundle archive to write.
        archive: PathBuf,
    },
}
