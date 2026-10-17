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
    Shutdown,
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
