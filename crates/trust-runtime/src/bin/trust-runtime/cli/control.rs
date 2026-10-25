#[derive(Debug, Subcommand)]
#[command(infer_subcommands = true)]
pub enum ControlAction {
    Status,
    Health,
    Stats,
    Pause,
    Resume,
    StepIn,
    StepOver,
    StepOut,
    BreakpointsSet {
        source: String,
        lines: Vec<u32>,
    },
    BreakpointsClear {
        source: String,
    },
    BreakpointsList,
    IoRead,
    IoWrite {
        address: String,
        value: String,
    },
    IoForce {
        address: String,
        value: String,
    },
    IoUnforce {
        address: String,
    },
    Eval {
        expr: String,
    },
    Set {
        target: String,
        value: String,
    },
    Restart {
        mode: String,
    },
    /// Upload a program container and install it on a stopped runtime (specification 34,
    /// section 11.2), on a chip and on a computer alike.
    Install {
        /// The file of the `TRSTA002` program container.
        container: PathBuf,
        /// Start the installed program afterwards, cold or warm, and wait until it runs.
        #[arg(long, value_parser = ["cold", "warm"])]
        restart: Option<String>,
    },
    /// Read a chip's device settings, `device.toml`, without its Wi-Fi password (specification 34
    /// TARGET-02).
    DeviceGet,
    /// Store a chip's device settings from a `device.toml` file while its program is stopped; they
    /// apply at the chip's next boot (`device-reset`).
    DeviceSet {
        /// The `device.toml` file.
        file: PathBuf,
    },
    /// Reset a chip while its program is stopped; it boots with its stored device settings.
    DeviceReset,
    /// Update a chip's truST firmware over the control connection (specification 34 TARGET-04): the
    /// runtime must be stopped; the image is written into the firmware slot that does not run, the
    /// chip restarts into it, and the command waits until it runs the new firmware.
    Firmware {
        /// The firmware image: the chip's application image, as the release carries it.
        image: PathBuf,
        /// How long to wait for the chip to run the new firmware, in seconds.
        #[arg(long, default_value_t = 60)]
        wait: u64,
    },
    /// Stop the program. The runtime keeps running in the Stopped state and `restart` starts the
    /// program again.
    Shutdown {
        /// Also end the runtime's process (the explicit exit).
        #[arg(long)]
        exit: bool,
    },
    ConfigGet,
    ConfigSet {
        key: String,
        value: String,
    },
    /// Print the newest runtime events, as the raw events.tail response.
    Events {
        /// Entries to return, from 1 to 1000. Without it the runtime default of 50 applies.
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Print the newest runtime faults, as the raw faults response.
    Faults {
        /// Entries to return, from 1 to 1000. Without it the runtime default of 50 applies.
        #[arg(long)]
        limit: Option<u64>,
    },
}
