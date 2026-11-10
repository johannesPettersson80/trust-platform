#[derive(Debug, Subcommand)]
#[command(infer_subcommands = true)]
pub enum ControlAction {
    /// Print the project's control endpoint and the token a client uses with it. No request is
    /// sent, so this works while the runtime is stopped.
    Endpoint {
        /// Print one JSON object: endpoint, auth_token and source.
        #[arg(long)]
        json: bool,
    },
    Status,
    Health,
    Stats,
    Pause,
    Resume,
    StepIn,
    StepOver,
    StepOut,
    /// Set the breakpoints of a source file, in place of its earlier ones. Lines are one-based,
    /// as an editor shows them, and so are the lines and columns the reply prints. The options
    /// apply to each line; a condition and a log message's parts are debug expressions, compiled
    /// with the types of the project (specification 11 §6.13.2).
    BreakpointsSet {
        source: String,
        #[arg(value_parser = clap::value_parser!(u32).range(1..))]
        lines: Vec<u32>,
        /// Stop only where this debug expression is TRUE.
        #[arg(long)]
        condition: Option<String>,
        /// Stop only at the hits this counts: `=N`, `>=N` or `>N`.
        #[arg(long)]
        hit_condition: Option<String>,
        /// Log this message instead of stopping; each `{expression}` in it gives its value.
        #[arg(long)]
        log_message: Option<String>,
    },
    BreakpointsClear {
        source: String,
    },
    /// List the breakpoints, with one-based lines and columns.
    BreakpointsList,
    /// Where the program is held: the reason, and the file, line and column of the stop, the line
    /// and column counted from 1.
    Location,
    /// The call stack of the held program, its frames' lines and columns counted from 1.
    Stack,
    /// The scopes of a frame of the stack, by the frame's id that `stack` prints.
    Scopes { frame_id: u32 },
    /// The variables of a scope or of a structured variable, by the reference that `scopes` or
    /// `variables` prints.
    Variables { reference: u32 },
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
    /// Evaluate a debug expression, compiled with the types of the project: in the held program,
    /// in the frame `--frame` names (an id `stack` prints), or between scans in the last scan
    /// (`debug.evaluate`; specification 11 §6.13.2).
    Evaluate {
        expr: String,
        /// The frame whose names the expression reads.
        #[arg(long)]
        frame: Option<u32>,
    },
    /// Print the compiled form of a debug expression, as `evaluate` and a breakpoint's condition
    /// send it, without sending anything.
    CompileExpression { expr: String },
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
    /// Print what the runtime reports of its device (specification 11, the runtime device
    /// capabilities contract): a chip's network and address, its firmware slots, its memory for
    /// programs and its last reset.
    Capabilities,
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
