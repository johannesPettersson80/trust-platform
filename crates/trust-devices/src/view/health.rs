//! The health rules of the graph (specification 25 §5.2, §5.3 and §5.5): how a runtime's health
//! rolls up from its own and its connections' health words, which words read as not proven
//! (dashed and never green), which need attention, a computer's reachability, and the product
//! words of every health and connector word. The tone that goes with a word is decided here too.

use trust_runtime::connectors::{ConnectorHealth, ConnectorState, DiscoveryConfidence};

use super::types::{Look, Mark, Pill, Tone};
use crate::texts::graph as texts;
use crate::topology::{Host, HostOrigin};

/// The rolled-up health of a runtime: the worst connection problem, else the runtime's own
/// state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// The runtime or a connection reports an error.
    Error,
    /// The runtime or a connection is degraded or cannot reach its runtime.
    Degraded,
    /// The runtime answers and runs.
    Connected,
    /// The runtime answers and runs a simulation.
    Simulate,
    /// The runtime is stopped, or only its configuration is known.
    Stopped,
    /// Nothing proves its state yet, such as while it starts or stops.
    Pending,
    /// Nothing reports it.
    Unknown,
}

impl Health {
    /// The tone of the health: green only for a running runtime.
    #[must_use]
    pub fn tone(self) -> Tone {
        match self {
            Self::Error => Tone::Error,
            Self::Degraded => Tone::Warning,
            Self::Connected | Self::Simulate => Tone::Ok,
            Self::Stopped | Self::Pending | Self::Unknown => Tone::Neutral,
        }
    }

    /// Whether a card of this health is drawn dashed: its state is not proven.
    #[must_use]
    pub fn look(self) -> Look {
        match self {
            Self::Pending | Self::Unknown => Look::Pending,
            Self::Error | Self::Degraded | Self::Connected | Self::Simulate | Self::Stopped => {
                Look::Normal
            }
        }
    }

    /// Whether the runtime answers and runs.
    #[must_use]
    pub fn running(self) -> bool {
        matches!(self, Self::Connected | Self::Simulate)
    }
}

fn is_error(word: &str) -> bool {
    word == "error"
}

/// An endpoint that cannot reach its runtime degrades the runtime, as the runtime's own report
/// counts it.
fn is_degraded(word: &str) -> bool {
    matches!(word, "degraded" | "runtime_unreachable")
}

/// The lifecycle state that a runtime's own health word names; `None` for a word that names none,
/// such as `configured_policy`, after which the connections decide.
fn own_state(word: &str) -> Option<Health> {
    match word {
        "connected" => Some(Health::Connected),
        "simulate" => Some(Health::Simulate),
        "stopped" => Some(Health::Stopped),
        "pending" | "starting" | "stopping" => Some(Health::Pending),
        "unknown" => Some(Health::Unknown),
        _ => None,
    }
}

/// Rolls a runtime's health up from its own health word and its connections' health words, raw,
/// before any search or filter (specification 25 §5.5). An error anywhere wins, then a degraded
/// word anywhere; otherwise the runtime's own state, and for a runtime whose own word names no
/// state the order `connected`, `simulate`, `configured_policy` (which reads as stopped: the
/// runtime is not running), and pending last.
#[must_use]
pub fn runtime_rollup<'a>(own: &str, endpoints: impl IntoIterator<Item = &'a str>) -> Health {
    let own = normalized(own);
    let endpoints: Vec<String> = endpoints.into_iter().map(normalized).collect();
    let words = || std::iter::once(&own).chain(&endpoints);
    if words().any(|word| is_error(word)) {
        return Health::Error;
    }
    if words().any(|word| is_degraded(word)) {
        return Health::Degraded;
    }
    if let Some(state) = own_state(&own) {
        return state;
    }
    [
        ("connected", Health::Connected),
        ("simulate", Health::Simulate),
        ("configured_policy", Health::Stopped),
    ]
    .into_iter()
    .find(|(word, _)| words().any(|candidate| candidate == word))
    .map_or(Health::Pending, |(_, health)| health)
}

fn normalized(word: &str) -> String {
    word.trim().to_ascii_lowercase()
}

/// The words of a configuration or state that nothing has proven yet: drawn dashed and never
/// green (specification 25 §5.3 and §5.5).
const NOT_PROVEN: [&str; 6] = [
    "pending",
    "not_configured",
    "not_in_build",
    "configured_policy",
    "unknown",
    "disabled",
];

/// How an item with the health word `word` is drawn.
#[must_use]
pub fn look_of(word: &str) -> Look {
    if NOT_PROVEN.contains(&normalized(word).as_str()) {
        Look::Pending
    } else {
        Look::Normal
    }
}

/// The tone of a health word: green only for `connected`.
#[must_use]
pub fn tone_of(word: &str) -> Tone {
    match normalized(word).as_str() {
        "connected" => Tone::Ok,
        "degraded" => Tone::Warning,
        "error" | "runtime_unreachable" => Tone::Error,
        _ => Tone::Neutral,
    }
}

/// Whether a connection with the health word `word` needs attention: every word but the
/// healthy, configured, disabled, stopped and simulated ones, and never a blank one.
#[must_use]
pub fn needs_attention(word: &str) -> bool {
    let word = normalized(word);
    !matches!(
        word.as_str(),
        "connected" | "configured" | "configured_policy" | "disabled" | "stopped" | "simulate"
    ) && !word.is_empty()
}

/// Whether a computer is reachable. A computer's state is the machine's reachability, never a
/// rollup of its runtimes or connections (specification 25 §5.5): this computer is always
/// reachable, a computer drawn for a target that did not answer is not, and a reported one is
/// when it reports its architecture or addresses or one of its runtimes runs.
#[must_use]
pub fn host_reachable(host: &Host, runtimes_running: bool) -> bool {
    match host.origin {
        HostOrigin::ThisComputer => true,
        HostOrigin::Unanswered => false,
        HostOrigin::Reported => {
            !host.arch.trim().is_empty() || !host.ips.is_empty() || runtimes_running
        }
    }
}

/// The `Reachable` or `Unreachable` pill of a computer. Not reaching a machine is a warning, not
/// an error.
#[must_use]
pub fn reachability_pill(reachable: bool) -> Pill {
    if reachable {
        Pill {
            word: texts::REACHABLE.to_owned(),
            tone: Tone::Ok,
            live: true,
        }
    } else {
        Pill {
            word: texts::UNREACHABLE.to_owned(),
            tone: Tone::Warning,
            live: false,
        }
    }
}

/// The product word of a connector's lifecycle state (specification 25 §5.5).
#[must_use]
pub fn connector_state_word(state: ConnectorState) -> &'static str {
    match state {
        ConnectorState::Ready => texts::CONNECTOR_READY,
        ConnectorState::Configured => texts::CONNECTOR_CONFIGURED,
        ConnectorState::Starting => texts::CONNECTOR_STARTING,
        ConnectorState::Reconnecting => texts::CONNECTOR_RECONNECTING,
        ConnectorState::Degraded | ConnectorState::Stale | ConnectorState::NotReady => {
            texts::NEEDS_ATTENTION
        }
        ConnectorState::Faulted => texts::FAULT,
        ConnectorState::Disabled => texts::CONNECTOR_DISABLED,
    }
}

/// The product word of a connector's health (specification 25 §5.5).
#[must_use]
pub fn connector_health_word(health: ConnectorHealth) -> &'static str {
    match health {
        ConnectorHealth::Ok => texts::HEALTH_OK,
        ConnectorHealth::Degraded => texts::HEALTH_DEGRADED,
        ConnectorHealth::Faulted => texts::FAULT,
        ConnectorHealth::Unknown => texts::HEALTH_UNKNOWN,
    }
}

/// The product word of a discovery confidence (specification 25 §5.5).
#[must_use]
pub fn confidence_word(confidence: DiscoveryConfidence) -> &'static str {
    match confidence {
        DiscoveryConfidence::Confirmed => texts::CONFIDENCE_CONFIRMED,
        DiscoveryConfidence::Likely => texts::CONFIDENCE_LIKELY,
        DiscoveryConfidence::PortReachable => texts::CONFIDENCE_PORT_REACHABLE,
        DiscoveryConfidence::Unavailable => texts::CONFIDENCE_UNAVAILABLE,
    }
}

/// The protocols of the advanced integrations: a connection of one is a draft until evidence
/// arrives.
const ADVANCED_PROTOCOLS: [&str; 6] = [
    "mesh",
    "openot",
    "realtime",
    "realtime_t0",
    "runtime_cloud",
    "federation",
];

/// The health words of an advanced integration without evidence.
const NO_EVIDENCE: [&str; 4] = ["pending", "configured_policy", "not_configured", "unknown"];

/// Whether a connection is drawn as a draft (specification 25 §5.3 and §5.13): one of the add
/// flow, whose role is `draft`, or an advanced integration (`mesh`, `openot`, `realtime`,
/// `runtime_cloud`) whose health word shows no evidence yet.
#[must_use]
pub fn draft_like(protocol: &str, role: &str, health: &str) -> bool {
    normalized(role) == "draft"
        || (ADVANCED_PROTOCOLS.contains(&normalized(protocol).as_str())
            && NO_EVIDENCE.contains(&normalized(health).as_str()))
}

/// The marks of a connection pill (specification 25 §5.13): `DRAFT` while it is a draft, and
/// `Disabled` while its health word is `disabled`.
#[must_use]
pub fn connection_marks(protocol: &str, role: &str, health: &str) -> Vec<Mark> {
    let mut marks = Vec::new();
    if draft_like(protocol, role, health) {
        marks.push(Mark::Draft);
    }
    if normalized(health) == "disabled" {
        marks.push(Mark::Disabled);
    }
    marks
}

/// The statuses that prove a link or a mesh peer.
const PROVEN: [&str; 3] = ["connected", "degraded", "error"];

/// Whether a link or a mesh peer is proven and drawn solid (specification 25 §5.7): only an
/// established `connected`, `degraded` or `error` status proves it; every other status, an
/// unknown future one included, fails closed and is drawn dashed.
#[must_use]
pub fn link_proven(status: &str) -> bool {
    PROVEN.contains(&normalized(status).as_str())
}
