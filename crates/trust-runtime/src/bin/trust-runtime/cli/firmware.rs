/// What `firmware` does (specification 34 TARGET-04).
#[derive(Debug, Subcommand)]
pub enum FirmwareAction {
    /// Install truST's firmware on a chip on USB through the chip's own boot loader: the first
    /// install, which needs no truST on the chip. Later updates go over the control connection
    /// (`trust-runtime ctl firmware`).
    Install {
        /// The chip: `esp32c6`.
        #[arg(long, value_parser = ["esp32c6"])]
        chip: String,
        /// The chip's USB serial port; found by the chip's USB identity when it is the only one.
        #[arg(long)]
        port: Option<String>,
        /// The firmware folder of the release for this chip: `bootloader.bin`, `partitions.bin` and
        /// `firmware.bin`.
        folder: PathBuf,
        /// Also erase the chip's device settings, `device.toml`, and its pairings, so that it boots
        /// without a control token, without pairing tokens and without a network: the way back to a
        /// chip whose control token is lost. It needs the chip on USB, as every first install does.
        #[arg(long)]
        reset_settings: bool,
    },
}
