//! The texts of the issues list and its button, the banner, the summary strip and the protocol
//! filter (specification 25 §5.5, §5.10 and §5.11). Texts ported from the panel's TypeScript
//! keep its words.

/// A count with its noun, such as `1 runtime` or `2 runtimes`.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

// ---- the issues list (specification 25 §5.11) ----

/// The issues button without an issue.
pub const NO_ISSUES: &str = "No issues";
/// The group of the issues that belong to no item of the canvas.
pub const GENERAL: &str = "General";
/// Zooms to the item of an issue.
pub const SHOW: &str = "Show";
/// Starts the simulator again after a failed start.
pub const RETRY: &str = "Retry";
/// Opens the simulator's debug log.
pub const OPEN_LOGS: &str = "Open logs";
/// The simulator, in the text of its start failure.
pub const SIMULATOR: &str = "Simulator";

/// The issues button: the number of faults and warnings.
#[must_use]
pub fn issue_count(count: usize) -> String {
    counted(count, "issue")
}

/// An issue of an item: its name and what is wrong, such as `MQTT broker: Broker connection
/// refused.`.
#[must_use]
pub fn item_issue(name: &str, text: &str) -> String {
    format!("{name}: {text}")
}

// ---- the banner (specification 25 §5.10 and §5.16) ----

/// The banner while the drawn simulator is stopped.
pub const SIMULATOR_IS_STOPPED: &str = "Simulator is stopped.";
/// The banner while the drawn simulator runs.
pub const SIMULATOR_RUNNING: &str = "Simulator running.";
/// Starts the simulator from the banner.
pub const START_SIMULATOR: &str = "Start simulator";
/// Stops the simulator from the banner.
pub const STOP_SIMULATOR: &str = "Stop simulator";

/// The banner's part about the other runtimes' reports that could not be read.
#[must_use]
pub fn peer_topology_degraded(problems: &str) -> String {
    format!("Peer topology degraded: {problems}")
}

// ---- the summary strip (specification 25 §5.11) ----

/// The summary strip, such as `1 computer · 2 runtimes · 3 connections · 2 running · 0 faults ·
/// 1 warning`.
#[must_use]
pub fn summary(
    computers: usize,
    runtimes: usize,
    connections: usize,
    running: usize,
    faults: usize,
    warnings: usize,
) -> String {
    [
        counted(computers, "computer"),
        counted(runtimes, "runtime"),
        counted(connections, "connection"),
        format!("{running} running"),
        counted(faults, "fault"),
        counted(warnings, "warning"),
    ]
    .join(" · ")
}

// ---- the protocol filter (specification 25 §5.5) ----

/// The filter hides no protocol.
pub const ALL_PROTOCOLS_VISIBLE: &str = "All protocols are visible.";
/// The filter hides nothing that needs attention.
pub const NO_HIDDEN_PROBLEMS: &str = "No hidden warnings or faults.";
/// The graph has no connection to filter.
pub const NO_CONNECTIONS: &str = "No connections.";

/// The connections that the filter hides.
#[must_use]
pub fn connections_hidden(count: usize) -> String {
    format!("{} hidden.", counted(count, "connection"))
}

/// The hidden items that need attention.
#[must_use]
pub fn hidden_need_attention(count: usize) -> String {
    if count == 1 {
        "1 hidden item needs attention.".to_owned()
    } else {
        format!("{count} hidden items need attention.")
    }
}

/// The hidden errors and warnings.
#[must_use]
pub fn hidden_errors_and_warnings(errors: usize, warnings: usize) -> String {
    format!(
        "{} · {}",
        counted(errors, "error"),
        counted(warnings, "warning")
    )
}
