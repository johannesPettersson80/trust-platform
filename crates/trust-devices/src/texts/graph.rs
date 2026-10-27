//! The texts of the graph: computers, runtimes and their states and details, the topology's
//! issues, the actions and their reasons, the list and the toolbar (specification 25 §5.1, §5.2,
//! §5.5, §5.6, §5.11 and §5.12, §6.1). Texts ported from the panel's TypeScript keep its words.

/// The breadcrumb of every inspector starts with the panel's title.
pub const PANEL: &str = "Devices & Connections";

// ---- states and health words (specification 25 §5.2 and §5.5) ----

/// The word of a health token of a runtime report; an unknown token reads in title case without
/// underscores, and a blank one as `Unknown`.
#[must_use]
pub fn health_word(token: &str) -> String {
    let token = token.trim().to_ascii_lowercase();
    let word = match token.as_str() {
        "connected" => CONNECTED,
        "stopped" => STOPPED,
        "starting" => STARTING,
        "stopping" => STOPPING,
        "unavailable" => STATUS_UNAVAILABLE,
        "configured_policy" => "Configured",
        "disabled" => "Disabled",
        "not_configured" => "Not configured",
        "runtime_unreachable" => "Runtime unreachable",
        "auth_failed" => AUTHENTICATION_FAILED,
        "degraded" => DEGRADED,
        "error" => ERROR,
        "pending" => PENDING,
        "simulate" => SIMULATOR,
        "unknown" | "" => UNKNOWN,
        _ => return title_case(&token),
    };
    word.to_owned()
}

/// `not_in_build` reads `Not In Build`, as the panel showed an unknown token before.
fn title_case(token: &str) -> String {
    token
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect::<String>()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The state of a runtime whose program runs.
pub const RUNNING: &str = "Running";
/// The state of a runtime whose program is paused.
pub const PAUSED: &str = "Paused";
/// The state of a runtime whose program stopped on a fault.
pub const FAULTED: &str = "Faulted";
/// The state of a runtime whose program is stopped.
pub const STOPPED: &str = "Stopped";
/// The state of a runtime whose program waits for its start.
pub const READY: &str = "Ready";
/// The state of a runtime that is starting.
pub const STARTING: &str = "Starting";
/// The state of a runtime that is stopping.
pub const STOPPING: &str = "Stopping";
/// A status that could not be read (specification 25 §5.10 and §6.5).
pub const STATUS_UNAVAILABLE: &str = "Status unavailable";
/// A runtime that refused the token.
pub const AUTHENTICATION_FAILED: &str = "Authentication failed";
/// A runtime with a degraded connection.
pub const DEGRADED: &str = "Degraded";
/// A runtime with an error.
pub const ERROR: &str = "Error";
/// A state that has no proof yet.
pub const PENDING: &str = "Pending";
/// A state that nothing reports.
pub const UNKNOWN: &str = "Unknown";
/// The simulator, as a mode and as a runtime's name.
pub const SIMULATOR: &str = "Simulator";

/// The word of a connector's lifecycle state (specification 25 §5.5).
pub const CONNECTOR_READY: &str = "Ready";
/// A configured connector that has not started.
pub const CONNECTOR_CONFIGURED: &str = "Configured";
/// A connector that is starting.
pub const CONNECTOR_STARTING: &str = "Starting";
/// A connector that reconnects.
pub const CONNECTOR_RECONNECTING: &str = "Reconnecting";
/// A degraded, stale or not ready connector.
pub const NEEDS_ATTENTION: &str = "Needs attention";
/// A faulted connector, or a faulted connector health, and the word of an issue that is an error
/// (specification 25 §5.5 and §5.11).
pub const FAULT: &str = "Fault";
/// A disabled connector.
pub const CONNECTOR_DISABLED: &str = "Disabled";
/// A healthy connector.
pub const HEALTH_OK: &str = "OK";
/// A degraded connector health.
pub const HEALTH_DEGRADED: &str = "Degraded";
/// A connector health without evidence.
pub const HEALTH_UNKNOWN: &str = "Unknown";
/// Proof at the protocol level.
pub const CONFIDENCE_CONFIRMED: &str = "Confirmed";
/// Strong evidence without the protocol's proof.
pub const CONFIDENCE_LIKELY: &str = "Likely";
/// Only the port answered.
pub const CONFIDENCE_PORT_REACHABLE: &str = "Port reachable only";
/// No evidence.
pub const CONFIDENCE_UNAVAILABLE: &str = "Not verified";

/// A computer that the panel reached.
pub const REACHABLE: &str = "Reachable";
/// A computer that the panel could not reach.
pub const UNREACHABLE: &str = "Unreachable";

// ---- the details and issues of the topology (specification 25 §5.6) ----

/// The detail of a configured runtime where nothing answers.
pub const UNREACHABLE_PEER_DETAIL: &str =
    "Configured endpoint not reachable — open it in Devices & Connections to connect or diagnose.";
/// The detail of a runtime that requires a token the request did not carry.
pub const AUTH_MISSING_DETAIL: &str = "No auth token provided — this runtime requires one.";
/// The detail of a runtime that rejected the token.
pub const AUTH_REJECTED_DETAIL: &str = "Auth token rejected — check it and try again.";
/// The name of a configured runtime without a label that did not answer.
pub const UNNAMED_PEER: &str = "runtime";
/// The detail of the simulator while it runs.
pub const SIMULATOR_RUNNING_DETAIL: &str = "● Running";
/// The detail of the simulator while it starts.
pub const SIMULATOR_STARTING_DETAIL: &str = "Starting Simulator…";
/// The detail of the simulator before the user starts it.
pub const SIMULATOR_STOPPED_DETAIL: &str = "Stopped — start the simulator to run it.";
/// The detail of a local connection with live evidence.
pub const CONNECTED: &str = "Connected";
/// The detail of a local connection without live evidence yet.
pub const PENDING_PROOF: &str = "Pending proof";
/// The detail of the canvas's draft of the connection that its add form sets up.
pub const DRAFT_SETUP: &str = "Draft setup";
/// The name of the first device that the canvas draws on the simulator when the user asks for one.
pub const FIRST_RUN_DEVICE: &str = "Drive A";
/// The detail of that device until the simulator reports I/O values.
pub const WAITING_FOR_RUNTIME_IO: &str = "Waiting for runtime I/O";
/// The canvas's summary word for the drawn simulator while it does not run.
pub const SIMULATOR_STOPPED_SUMMARY: &str = "Simulator stopped";
/// The detail of a managed runtime that runs.
pub const MANAGED_RUNNING_DETAIL: &str = "Running (managed local runtime).";
/// The detail of a managed runtime that is stopped.
pub const MANAGED_STOPPED_DETAIL: &str = "Stopped — Start it from this node.";
/// The detail of a managed runtime that starts.
pub const MANAGED_STARTING_DETAIL: &str = "Starting managed local runtime…";
/// The detail of a managed runtime that stops.
pub const MANAGED_STOPPING_DETAIL: &str = "Stopping managed local runtime…";
/// The detail of a configured connection that a running runtime does not run yet.
pub const RESTART_TO_APPLY: &str =
    "Configured in project files; restart the runtime to apply this change.";

/// The issue of a project topology that cannot be read.
#[must_use]
pub fn topology_unavailable(reason: &str) -> String {
    format!("Fleet topology unavailable: {reason}")
}

/// The issue of another runtime's topology that cannot be read.
#[must_use]
pub fn peer_topology_failed(source: &str, reason: &str) -> String {
    format!("{source} topology: {reason}")
}

/// The issue of a connector status that cannot be read or is invalid.
#[must_use]
pub fn connector_status_failed(source: &str, reason: &str) -> String {
    format!("{source} connector status: {reason}")
}

/// Why a connector report is invalid: a value is missing.
#[must_use]
pub fn connector_value_missing(field: &str) -> String {
    format!("missing connector {field}")
}

/// Why a connector report is invalid: a value is outside its closed vocabulary.
#[must_use]
pub fn connector_value_unknown(field: &str, value: &str) -> String {
    format!("unknown connector {field}: {value}")
}

/// The issue of the managed runtimes of a `fleet.toml` that cannot be read.
#[must_use]
pub fn managed_runtimes_unavailable(reason: &str) -> String {
    format!("Managed runtimes unavailable: {reason}")
}

// ---- computers (specification 25 §5.5 and §5.12) ----

/// The heading of the computer the panel runs on.
pub const THIS_COMPUTER: &str = "This computer";

/// The heading of another computer, by its address or host name.
#[must_use]
pub fn computer_at(address: &str) -> String {
    format!("Computer {address}")
}

/// The heading of a chip: its name and, when known, its address.
#[must_use]
pub fn chip_heading(model: &str, address: Option<&str>) -> String {
    match address {
        Some(address) => format!("{model} {address}"),
        None => model.to_owned(),
    }
}

/// The platform line of a chip.
#[must_use]
pub fn chip_platform(model: &str) -> String {
    format!("{model} chip")
}

/// The name of an operating system as a host reports it, such as `linux`.
#[must_use]
pub fn operating_system(os: &str) -> String {
    match os.trim().to_ascii_lowercase().as_str() {
        "linux" => "Linux".to_owned(),
        "windows" => "Windows".to_owned(),
        "macos" => "macOS".to_owned(),
        "freebsd" => "FreeBSD".to_owned(),
        _ => os.trim().to_owned(),
    }
}

/// The band of the computers of one network (decision O29).
#[must_use]
pub fn network(subnet: &str) -> String {
    format!("Network · {subnet}")
}

/// Joins the parts of a line, such as `Raspberry Pi 5 · Linux aarch64`.
#[must_use]
pub fn joined(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The breadcrumb of an inspector of `kind`.
#[must_use]
pub fn breadcrumb(kind: &str) -> String {
    format!("{PANEL} / {kind}")
}

/// An inspector's kind: a computer.
pub const KIND_COMPUTER: &str = "Computer";
/// An inspector's kind: a chip.
pub const KIND_CHIP: &str = "Chip";
/// An inspector's kind: a container.
pub const KIND_CONTAINER: &str = "Container";
/// An inspector's kind: a runtime.
pub const KIND_RUNTIME: &str = "Runtime";
/// A list row's kind: a connection.
pub const KIND_CONNECTION: &str = "Connection";
/// A list row's kind: a device.
pub const KIND_DEVICE: &str = "Device";

// ---- rows ----

/// The row of a state.
pub const ROW_STATE: &str = "State";
/// The row of the run target.
pub const ROW_RUN_TARGET: &str = "Run target";
/// The row of the mode.
pub const ROW_MODE: &str = "Mode";
/// The row of the cycle time.
pub const ROW_CYCLE: &str = "Cycle";
/// The row of an address.
pub const ROW_ADDRESS: &str = "Address";
/// The row of the truST version.
pub const ROW_VERSION: &str = "truST version";
/// The row of the update a runtime and this tool need when their versions differ.
pub const ROW_UPDATE: &str = "Update";
/// The row of the up time.
pub const ROW_UP_FOR: &str = "Up for";
/// The row of the fault count.
pub const ROW_FAULTS: &str = "Faults";
/// The row of the access role and rights.
pub const ROW_ACCESS: &str = "Access";
/// The row of the connection count.
pub const ROW_CONNECTIONS: &str = "Connections";
/// The row of the runtime count.
pub const ROW_RUNTIMES: &str = "Runtimes";
/// The row of the host name.
pub const ROW_HOST_NAME: &str = "Host name";
/// The row of the board.
pub const ROW_BOARD: &str = "Board";
/// The row of the operating system and architecture.
pub const ROW_SYSTEM: &str = "System";
/// The row of the processor temperature.
pub const ROW_TEMPERATURE: &str = "Temperature";
/// The row of the load.
pub const ROW_LOAD: &str = "Load";
/// The row of a detail.
pub const ROW_DETAIL: &str = "Detail";
/// The row of a container's image.
pub const ROW_IMAGE: &str = "Image";
/// The row of a container's restart policy.
pub const ROW_RESTART_POLICY: &str = "Restart policy";
/// The row of a container's processor limit.
pub const ROW_CPU_LIMIT: &str = "CPU limit";
/// The row of a container's memory limit.
pub const ROW_MEMORY_LIMIT: &str = "Memory limit";
/// The row of a container's mounted paths.
pub const ROW_MOUNTS: &str = "Mounts";
/// The row of the container a runtime runs in.
pub const ROW_CONTAINER: &str = "Container";

/// The run target row of the selected runtime.
pub const SELECTED: &str = "Selected";
/// The run target row of another runtime.
pub const NOT_SELECTED: &str = "Not selected";
/// The address of a runtime reached through a local control socket, whose path is never shown.
pub const LOCAL_CONTROL_SOCKET: &str = "Local control socket";

/// The cycle time of a card, such as `4.2 ms`.
#[must_use]
pub fn milliseconds(text: &str) -> String {
    format!("{text} ms")
}

/// The cycle row: the last cycle time and the longest.
#[must_use]
pub fn cycle_with_maximum(last: &str, max: &str) -> String {
    format!("{last} ms · max {max} ms")
}

/// The fault count.
#[must_use]
pub fn faults(count: u64) -> String {
    if count == 1 {
        "1 fault".to_owned()
    } else {
        format!("{count} faults")
    }
}

/// A duration in whole units: `6 h 14 min`, `3 d 2 h`, `5 min` or `42 s`.
#[must_use]
pub fn duration(seconds: u64) -> String {
    let (days, hours, minutes) = (
        seconds / 86_400,
        seconds % 86_400 / 3_600,
        seconds % 3_600 / 60,
    );
    if days > 0 {
        format!("{days} d {hours} h")
    } else if hours > 0 {
        format!("{hours} h {minutes} min")
    } else if minutes > 0 {
        format!("{minutes} min")
    } else {
        format!("{seconds} s")
    }
}

/// How long ago an item was seen.
#[must_use]
pub fn ago(seconds: u64) -> String {
    format!("{} ago", duration(seconds))
}

/// A processor temperature.
#[must_use]
pub fn temperature(celsius: f64) -> String {
    format!("{celsius:.1} °C")
}

/// A load average.
#[must_use]
pub fn load(load: f64) -> String {
    format!("{load:.2}")
}

/// The access row: the role and whether it may write and force values (specification 11).
#[must_use]
pub fn access(role: &str, may_write: bool, may_force: bool) -> String {
    let role = capitalized(role);
    match (may_write, may_force) {
        (true, true) => format!("{role} · may write and force"),
        (true, false) => format!("{role} · may write"),
        (false, true) => format!("{role} · may force"),
        (false, false) => format!("{role} · may not write or force"),
    }
}

/// `admin` reads `Admin`.
#[must_use]
pub fn capitalized(word: &str) -> String {
    let mut characters = word.trim().chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

/// The mode word of a runtime report's mode token; empty for a token that names no mode but a
/// state (specification 25 §5.2).
#[must_use]
pub fn mode_word(mode: &str) -> String {
    let mode = mode.trim().to_ascii_lowercase();
    match mode.as_str() {
        "simulate" | "simulator" => SIMULATOR.to_owned(),
        "managed" => "Managed".to_owned(),
        "remote" => "Remote".to_owned(),
        "attached" => "Attached".to_owned(),
        "local" => "Local".to_owned(),
        ""
        | "stopped"
        | "running"
        | "connected"
        | "online"
        | "pending"
        | "degraded"
        | "error"
        | "auth_failed"
        | "runtime_unreachable"
        | "unknown" => String::new(),
        _ => title_case(&mode),
    }
}

// ---- actions and their reasons (specification 25 §5.1, §5.11, §5.12 and §6.1) ----

/// Starts a process or a program.
pub const START: &str = "Start";
/// Stops a process or a program.
pub const STOP: &str = "Stop";
/// Opens a session.
pub const CONNECT: &str = "Connect";
/// Closes the session.
pub const DISCONNECT: &str = "Disconnect";
/// Asks for a runtime's token.
pub const SET_AUTH_TOKEN: &str = "Set auth token";
/// The simulator's start in progress.
pub const STARTING_ELLIPSIS: &str = "Starting…";
/// A managed runtime's start or stop in progress.
pub const UPDATING_ELLIPSIS: &str = "Updating…";
/// Opens Live Values.
pub const LIVE_VALUES: &str = "Live Values";
/// Starts debugging.
pub const DEBUG: &str = "Debug";
/// Deploys the project.
pub const DEPLOY: &str = "Deploy";
/// Selects the runtime without connecting.
pub const SET_AS_RUN_TARGET: &str = "Set as run target";
/// Opens the runtime settings.
pub const SETTINGS: &str = "Settings";
/// Opens the logs.
pub const LOGS: &str = "Logs";
/// Zooms to the item.
pub const FOCUS: &str = "Focus";
/// Asks the runtime to show where it is by blinking its identify output (specification 11,
/// "Identify").
pub const IDENTIFY: &str = "Identify";
/// Identify of a runtime that does not answer.
pub const IDENTIFY_UNREACHABLE: &str = "Identify is unavailable until the runtime answers.";
/// The context menu's entry that opens `+ Add` for the runtime.
pub const ADD_DEVICE_OR_CONNECTION_MENU: &str = "Add device or connection…";
/// The Edit slot of a runtime.
pub const ADD_CONNECTION: &str = "Add connection";
/// The Edit slot of a computer.
pub const SET_UP_RUNTIME: &str = "Set up runtime";
/// The Edit slot after the computers.
pub const ADD_HOST: &str = "Add host";
/// The toolbar's add action.
pub const ADD: &str = "+ Add";
/// The toolbar's search action.
pub const DISCOVER: &str = "Discover";
/// Opens the issues list from the summary strip.
pub const OPEN_ISSUES: &str = "Issues";
/// The word of the canvas view in the toolbar's switch (specification 25 §5.11).
pub const VIEW_TOPOLOGY: &str = "Topology";
/// The word of the table view in the toolbar's switch (specification 25 §5.14).
pub const VIEW_LIST: &str = "List";

/// Deploy stays disabled until a real backend exists (specification 25 §6.1).
pub const DEPLOY_UNAVAILABLE: &str = "Deploy is not available for this target yet.";
/// Debug of a runtime that does not answer (the sidebar's text).
pub const DEBUG_UNREACHABLE: &str = "Debug is unavailable until the target is reachable.";
/// Debug of a runtime whose debugging is turned off.
pub const DEBUG_DISABLED: &str = "Debugging is turned off on this runtime.";
/// Connect without a control endpoint (specification 25 §5.1).
pub const CONNECT_WITHOUT_ENDPOINT: &str =
    "This runtime has no control endpoint. Add its address with Connect existing runtime.";
/// Connect to a runtime that does not answer (specification 25 §5.1).
pub const CONNECT_UNREACHABLE: &str =
    "Not reachable — check that the runtime is running and that its address is right.";
/// Connect to a runtime that has not answered in time.
pub const CONNECT_UNANSWERED: &str =
    "Status unavailable — the runtime did not answer in time; it is asked again.";
/// Start of a managed runtime whose status is unavailable (specification 25 §5.1 and §6.5).
pub const MANAGED_UNAVAILABLE: &str = "Status unavailable — refresh before starting.";
/// The lifecycle action of a managed runtime that is starting or stopping.
pub const MANAGED_UPDATING: &str = "Wait until the runtime has started or stopped.";
/// The lifecycle action of the simulator while it starts.
pub const SIMULATOR_STARTING: &str = "Wait until the simulator has started.";
/// Live Values of the simulator before it runs.
pub const LIVE_VALUES_START_SIMULATOR: &str = "Start the simulator to see its live values.";
/// Live Values of a managed runtime that is not connected.
pub const LIVE_VALUES_START_RUNTIME: &str = "Start this runtime to see its live values.";
/// Live Values of another runtime that is not connected.
pub const LIVE_VALUES_CONNECT: &str = "Connect to this runtime to see its live values.";
/// Set as run target of a runtime without a control endpoint.
pub const RUN_TARGET_WITHOUT_ENDPOINT: &str =
    "This runtime has no control endpoint to select as the run target.";

/// The reason of a program action that the connection's role may not run (specification 11).
#[must_use]
pub fn role_needed(action: &str, required: &str, role: &str) -> String {
    format!(
        "{action} needs the {} role; this connection has the {} role.",
        capitalized(required),
        capitalized(role)
    )
}

/// The refusal of an Identify for a runtime that did not answer the last round: nothing is sent.
#[must_use]
pub fn identify_unanswered(name: &str) -> String {
    format!("{name} did not answer, so nothing was sent.")
}

/// The notice of an accepted Identify: the runtime blinks its identify output for `seconds`
/// seconds (specification 25 §5.12).
#[must_use]
pub fn identifying(name: &str, seconds: u64) -> String {
    format!("Identifying {name}: its identify output blinks for {seconds} seconds.")
}

/// What the program actions are called in their reasons.
pub const STARTING_THE_PROGRAM: &str = "Starting the program";
/// What the program actions are called in their reasons.
pub const STOPPING_THE_PROGRAM: &str = "Stopping the program";

/// The hint of a runtime without connections (specification 25 §5.12).
pub const NO_DEVICES_YET: &str = "No devices yet";
/// What the hint suggests.
pub const NO_DEVICES_HINT: &str = "Use + Add to add one, or Discover to scan the network.";

// ---- the toolbar and the parts the later views fill ----

/// The placeholder of the search (specification 25 §5.11).
pub const SEARCH_PLACEHOLDER: &str = "Search devices, connections, signals, issues";
/// The title of the add dialog (specification 25 §5.4).
pub const ADD_DIALOG_TITLE: &str = "Add device or connection";
/// The heading of the finds of the last search in the add dialog (`AddDialog.tsx:124`).
pub const FOUND_ON_THE_NETWORK: &str = "Found on the network";
/// The accessible name of the add dialog's close button (`AddDialog.tsx`).
pub const ADD_DIALOG_CLOSE: &str = "Close";
/// The search entry of the add dialog (specification 25 §5.14).
pub const DISCOVER_DEVICES_AND_RUNTIMES: &str = "Discover devices and runtimes";
/// Its help (specification 25 §5.14).
pub const DISCOVER_HELP: &str = "Scan first when hardware is already on the network.";
/// The project bar without a truST project in the workspace.
pub const NO_PROJECT: &str = "No truST project";
/// The first group of the add dialog (specification 25 §5.9 and §5.14).
pub const RUNTIMES_GROUP: &str = "Runtimes";

// ---- the list (specification 25 §5.14) ----

/// The list's column of the item kind.
pub const COLUMN_KIND: &str = "Kind";
/// The list's column of the name.
pub const COLUMN_NAME: &str = "Name";
/// The list's column of the state.
pub const COLUMN_STATE: &str = "State";
/// The list's column of the address.
pub const COLUMN_ADDRESS: &str = "Address";
/// The list's column of the platform, runtime kind, protocol or role.
pub const COLUMN_TYPE: &str = "Type";
/// The list's column of the item that holds the row's item.
pub const COLUMN_BELONGS_TO: &str = "Belongs to";
/// The list's column of the detail.
pub const COLUMN_DETAIL: &str = "Detail";
/// The list's column of the live values.
pub const COLUMN_VALUES: &str = "Live values";
/// The list's column of the last time an item was seen.
pub const COLUMN_LAST_SEEN: &str = "Last seen";
/// The list's column of the response time.
pub const COLUMN_RESPONSE_TIME: &str = "Response time";
/// The list's column of the identity.
pub const COLUMN_IDENTITY: &str = "Identity";

/// A runtime's kind word in the list: the simulator.
pub const RUNTIME_KIND_SIMULATOR: &str = "Simulator";
/// A runtime's kind word in the list: a runtime on a computer.
pub const RUNTIME_KIND_COMPUTER: &str = "Runtime on a computer";
/// A runtime's kind word in the list: the truST firmware of a chip.
pub const RUNTIME_KIND_CHIP: &str = "Runtime on a chip";

/// The update a runtime and this tool need (specification 34 TARGET-05): the older side is updated
/// to the newer, and a version that cannot be compared brings the runtime to this tool's release.
#[must_use]
pub fn update_advice(advice: &trust_runtime_client::VersionAdvice) -> String {
    use trust_runtime_client::VersionAdvice;
    match advice {
        VersionAdvice::UpdateRuntime { tools, .. } | VersionAdvice::Unreadable { tools, .. } => {
            format!("Update this runtime to truST {tools}")
        }
        VersionAdvice::UpdateTools { runtime, .. } => {
            format!("Update truST on this computer to {runtime}")
        }
    }
}
