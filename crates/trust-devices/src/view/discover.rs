//! The Discover pane (specification 25 §2): the status of one search, turned into what the pane
//! draws. One card per device with every protocol found on it, the source rows, the scan choices
//! with the reason of each scan that cannot run, the empty-result checks, and the card actions that
//! fit the device's identity. The engine adds this view to the answer of a search status (design
//! §1.4.1); the search itself runs in the engine's search module, and its sources in the runtime.
//!
//! - `runtime_origins`: the runtimes the panel offers as origins of a search, and whether each is
//!   ready: attached, or with `connected`, `running` or `online` health (specification 25 §2).
//! - A runtime card (a device that a truST runtime answered for) gets `Adopt`; any other card gets
//!   `+ Add` with the connection it would create on the owning runtime.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::{json, Map, Value};
use trust_runtime::connectors::DiscoveryConfidence;
use trust_runtime::search::SourceId;

use super::health;
use crate::gather::Gathered;
use crate::inputs;
use crate::panes::forms::FormRequest;
use crate::request::HostFacts;
use crate::session::KnownRuntime;
use crate::texts::discover as texts;
use crate::texts::graph;
use crate::topology;
use crate::view::types::{
    Action, CardProtocol, DeviceCard, DiscoverView, Emphasis, OnlyChoice, Op, OriginChoice, Row,
    SourceRow, TargetsInput, Tone,
};

/// The origin of a search that runs on this computer (`origin` in `comm.search.start`).
const THIS_HOST: &str = "this_host";

/// One runtime that the panel offers as the origin of a search (specification 25 §2, `Scan from`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeOrigin {
    /// Its item ID, which a search names as its origin.
    pub(crate) id: String,
    /// Its name, which its origin choice shows.
    pub(crate) name: String,
    /// Whether it is attached, or has `connected`, `running` or `online` health: only then do its
    /// hardware scans run.
    pub(crate) ready: bool,
}

/// Where the search that the pane shows runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scan {
    /// On this computer.
    ThisHost,
    /// Inside the runtime with this item ID.
    Runtime(String),
}

/// The runtimes that the panel offers as origins, in the order its last view showed them, each with
/// whether it is ready. Readiness is read from the panel's last gather (specification 25 §2).
#[must_use]
pub(crate) fn runtime_origins(
    known: &[KnownRuntime],
    gathered: Option<&Gathered>,
    host: &HostFacts,
) -> Vec<RuntimeOrigin> {
    let ready = gathered.map_or_else(BTreeSet::new, |gathered| ready_runtimes(gathered, host));
    known
        .iter()
        .map(|runtime| {
            let id = runtime.id.as_str().to_owned();
            RuntimeOrigin {
                ready: ready.contains(&id),
                name: runtime.name.clone(),
                id,
            }
        })
        .collect()
}

/// The item IDs of the runtimes that are attached, or have `connected`, `running` or `online`
/// health, as the gather's topology reports them.
fn ready_runtimes(gathered: &Gathered, host: &HostFacts) -> BTreeSet<String> {
    let inputs = inputs::read(gathered, host, None);
    let topology = topology::merge(&inputs.topology);
    topology
        .runtimes()
        .filter(|runtime| {
            runtime.attached
                || matches!(runtime.health.as_str(), "connected" | "running" | "online")
        })
        .map(|runtime| runtime.id.clone())
        .collect()
}

/// The Discover view of one search status (specification 25 §2). `status` is the body that
/// `comm.search.status` answers, or the body the engine builds for a runtime that cannot search;
/// `scan` is the origin the search ran at, `runtimes` the origins the panel offers, and `now_ms`
/// the time of the view in milliseconds since the Unix epoch.
///
/// # Errors
/// The body is not a search status: a source or a device lacks a member that the answer requires.
pub(crate) fn project(
    status: &Map<String, Value>,
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    now_ms: u64,
) -> Result<DiscoverView, serde_json::Error> {
    let answer: Answer = serde_json::from_value(Value::Object(status.clone()))?;
    let sources = answer
        .sources
        .iter()
        .map(source_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DiscoverView {
        search_id: answer.search_id,
        running: answer.state == Run::Running,
        progress_text: None,
        origins: origin_choices(scan, runtimes),
        only: only_choices(&answer.sources, scan, runtimes),
        targets: targets(),
        sources,
        fieldbus: Vec::new(),
        cards: answer
            .devices
            .iter()
            .map(|device| card(device, scan, runtimes, now_ms))
            .collect(),
        compare: None,
        empty_text: (answer.state != Run::Running && answer.devices.is_empty())
            .then(|| empty_text(scan, runtimes)),
        actions: Vec::new(),
    })
}

// ---- the answer that `comm.search.status` gives (specification 11, "One search") ----

/// The body of a search status, with the members that the Discover view reads.
#[derive(Debug, Deserialize)]
struct Answer {
    search_id: Option<String>,
    state: Run,
    sources: Vec<SourceAnswer>,
    devices: Vec<DeviceAnswer>,
}

/// The state of a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Run {
    Running,
    Done,
    Cancelled,
}

/// One source of a search.
#[derive(Debug, Deserialize)]
struct SourceAnswer {
    id: SourceId,
    state: Progress,
    count: Option<u32>,
    reason: Option<String>,
}

/// The progress of one source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Progress {
    Searching,
    Found,
    TimedOut,
    Cancelled,
    Skipped,
}

/// One device: every result that shares an address, an AMS Net ID, a USB serial number or a
/// runtime ID.
#[derive(Debug, Deserialize)]
struct DeviceAnswer {
    id: String,
    #[serde(default)]
    addresses: Vec<String>,
    #[serde(default)]
    ams_net_ids: Vec<String>,
    #[serde(default)]
    usb_serials: Vec<String>,
    #[serde(default)]
    runtime_ids: Vec<String>,
    last_seen_ms: u64,
    finds: Vec<FindAnswer>,
}

/// One protocol result on a device.
#[derive(Debug, Deserialize)]
struct FindAnswer {
    protocol: SourceId,
    source: String,
    confidence: DiscoveryConfidence,
    #[serde(default)]
    simulated: bool,
    #[serde(default)]
    identity: Value,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    warnings: Vec<String>,
}

// ---- the scan choices and the source rows ----

/// The origin choices: this computer, then each runtime the panel offers (specification 25 §2).
fn origin_choices(scan: &Scan, runtimes: &[RuntimeOrigin]) -> Vec<OriginChoice> {
    let this_host = OriginChoice {
        id: THIS_HOST.to_owned(),
        label: graph::THIS_COMPUTER.to_owned(),
        enabled: true,
        reason: None,
        selected: *scan == Scan::ThisHost,
    };
    std::iter::once(this_host)
        .chain(runtimes.iter().map(|runtime| OriginChoice {
            id: runtime.id.clone(),
            label: runtime.name.clone(),
            enabled: true,
            reason: None,
            selected: *scan == Scan::Runtime(runtime.id.clone()),
        }))
        .collect()
}

/// The `Only search` choices, one per source in the order of the source table. A source is ticked
/// when this search asked for it; a scan that cannot run from `scan` is disabled with its reason.
fn only_choices(
    answered: &[SourceAnswer],
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
) -> Vec<OnlyChoice> {
    SourceId::ALL
        .into_iter()
        .map(|source| {
            let reason = scan_reason(source, scan, runtimes);
            OnlyChoice {
                source: source.as_str().to_owned(),
                label: texts::source_name(source.as_str()),
                checked: answered.iter().any(|answer| answer.id == source),
                enabled: reason.is_none(),
                reason,
            }
        })
        .collect()
}

/// Why a scan of `source` from `scan` cannot run, or `None` when it can (specification 25 §2,
/// lines 82-88). `EtherCAT` and GPIO run only from a runtime; every scan from a runtime that is not
/// ready is disabled.
fn scan_reason(source: SourceId, scan: &Scan, runtimes: &[RuntimeOrigin]) -> Option<String> {
    if source == SourceId::Bluetooth {
        return Some(texts::BLUETOOTH_UNAVAILABLE.to_owned());
    }
    let needs_runtime = matches!(source, SourceId::Ethercat | SourceId::Gpio);
    match scan {
        Scan::ThisHost => needs_runtime.then(|| texts::CHOOSE_RUNTIME_FOR_HARDWARE.to_owned()),
        Scan::Runtime(id) => match runtimes.iter().find(|runtime| runtime.id == *id) {
            Some(runtime) if runtime.ready => None,
            _ if needs_runtime => Some(texts::START_RUNTIME_FOR_HARDWARE.to_owned()),
            Some(runtime) => Some(texts::start_or_connect(&runtime.name)),
            None => Some(texts::START_A_RUNTIME.to_owned()),
        },
    }
}

/// The row of one source: its word, its count and, when it did not run, its reason (specification
/// 25 §2, lines 55-58). A source that ran must report its count.
fn source_row(source: &SourceAnswer) -> Result<SourceRow, serde_json::Error> {
    let id = source.id.as_str();
    let (word, tone, count, reason) = match (source.state, source.count) {
        (Progress::Skipped, _) => (
            texts::SKIPPED.to_owned(),
            Tone::Muted,
            None,
            Some(
                source
                    .reason
                    .clone()
                    .unwrap_or_else(|| texts::SKIPPED_WITHOUT_REASON.to_owned()),
            ),
        ),
        (_, None) => {
            return Err(serde::de::Error::custom(format!(
                "the source {id} ran but reports no count"
            )));
        }
        (Progress::Searching, Some(count)) => (
            texts::SEARCHING.to_owned(),
            Tone::Neutral,
            Some(count),
            None,
        ),
        (Progress::Found, Some(count)) => {
            let tone = if count > 0 { Tone::Ok } else { Tone::Neutral };
            (texts::found(count), tone, Some(count), None)
        }
        (Progress::TimedOut, Some(count)) => (
            texts::TIMED_OUT.to_owned(),
            Tone::Warning,
            Some(count),
            None,
        ),
        (Progress::Cancelled, Some(count)) => (
            texts::CANCELLED.to_owned(),
            Tone::Neutral,
            Some(count),
            None,
        ),
    };
    Ok(SourceRow {
        id: id.to_owned(),
        label: texts::source_name(id),
        word,
        count,
        reason,
        tone,
    })
}

/// The inputs of `Known address or subnet`.
fn targets() -> TargetsInput {
    TargetsInput {
        hosts_label: texts::TARGET_HOSTS_LABEL.to_owned(),
        cidr_label: texts::TARGET_CIDR_LABEL.to_owned(),
        hosts_placeholder: texts::TARGET_HOSTS_PLACEHOLDER.to_owned(),
        cidr_placeholder: texts::TARGET_CIDR_PLACEHOLDER.to_owned(),
    }
}

/// The empty-result text (specification 25 §2, lines 75-77): the checks, and what the hardware
/// scans need from a runtime.
fn empty_text(scan: &Scan, runtimes: &[RuntimeOrigin]) -> String {
    let selected_ready = match scan {
        Scan::Runtime(id) => runtimes
            .iter()
            .any(|runtime| runtime.id == *id && runtime.ready),
        Scan::ThisHost => false,
    };
    let hardware = if selected_ready {
        texts::HARDWARE_FROM_SELECTED
    } else if runtimes.iter().any(|runtime| runtime.ready) {
        texts::CHOOSE_RUNNING_RUNTIME
    } else {
        texts::START_RUNTIME_FIRST
    };
    format!("{} {hardware}", texts::EMPTY_CHECKS)
}

// ---- the cards ----

/// The card of one device (specification 25 §2, lines 64-72): every protocol found on it, one
/// address for a runtime, and the actions that fit its identity.
fn card(device: &DeviceAnswer, scan: &Scan, runtimes: &[RuntimeOrigin], now_ms: u64) -> DeviceCard {
    let runtime = device
        .finds
        .iter()
        .find(|find| find.protocol == SourceId::TrustRuntime);
    let endpoint = runtime.and_then(control_endpoint);
    let title = device_title(device);
    let stale_reason = stale_reason(scan, runtimes);
    let mut actions = match runtime {
        Some(_) => runtime_actions(device, endpoint.as_deref(), &title),
        None => device_actions(device, owner(scan, runtimes)),
    };
    if let Some(reason) = &stale_reason {
        disable(&mut actions, reason);
    }
    DeviceCard {
        id: device.id.clone(),
        title,
        addresses: match &endpoint {
            Some(endpoint) => vec![host_and_port(endpoint)],
            None => device.addresses.clone(),
        },
        protocols: device.finds.iter().map(card_protocol).collect(),
        identity: identity_rows(device),
        last_seen: graph::ago(seconds_since(now_ms, device.last_seen_ms)),
        project: None,
        differs: Vec::new(),
        warnings: device
            .finds
            .iter()
            .flat_map(|find| find.warnings.iter().cloned())
            .collect(),
        actions,
        stale_reason,
    }
}

/// A protocol line of a card: its name, how it was found and its confidence.
fn card_protocol(find: &FindAnswer) -> CardProtocol {
    CardProtocol {
        name: texts::source_name(find.protocol.as_str()),
        source: texts::discovery_source(&find.source),
        confidence: health::confidence_word(find.confidence).to_owned(),
        simulated: find.simulated,
    }
}

/// The control endpoint that a runtime's result names, as `tcp://host:port`.
fn control_endpoint(find: &FindAnswer) -> Option<String> {
    text(&find.identity, "control").or_else(|| text(&find.params, "control_endpoint"))
}

/// The endpoint as a card shows it: its `tcp://` prefix is left out (specification 25 §2, lines
/// 69-70).
fn host_and_port(endpoint: &str) -> String {
    endpoint
        .strip_prefix("tcp://")
        .unwrap_or(endpoint)
        .to_owned()
}

/// The name that the device's protocols read about it, else its AMS Net ID, its address or its ID.
fn device_title(device: &DeviceAnswer) -> String {
    const NAMES: [&str; 8] = [
        "name",
        "computer_name",
        "application_name",
        "product",
        "model",
        "port",
        "chip_label",
        "chip",
    ];
    device
        .finds
        .iter()
        .find_map(|find| NAMES.iter().find_map(|key| text(&find.identity, key)))
        .or_else(|| device.ams_net_ids.first().cloned())
        .or_else(|| device.addresses.first().cloned())
        .unwrap_or_else(|| device.id.clone())
}

/// The identity rows of a device: its AMS Net ID, USB serial number and runtime ID, and a runtime's
/// truST version and platform. A fact that the device does not have has no row.
fn identity_rows(device: &DeviceAnswer) -> Vec<Row> {
    let mut facts = vec![
        (texts::ROW_AMS_NET_ID, device.ams_net_ids.join(" · ")),
        (texts::ROW_USB_SERIAL, device.usb_serials.join(" · ")),
        (texts::ROW_RUNTIME_ID, device.runtime_ids.join(" · ")),
    ];
    if let Some(runtime) = device
        .finds
        .iter()
        .find(|find| find.protocol == SourceId::TrustRuntime)
    {
        facts.push((
            graph::ROW_VERSION,
            text(&runtime.identity, "version").unwrap_or_default(),
        ));
        facts.push((
            texts::ROW_PLATFORM,
            text(&runtime.identity, "platform").unwrap_or_default(),
        ));
    }
    facts
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(label, value)| Row {
            label: label.to_owned(),
            value,
            tone: None,
            wide: false,
            copy: None,
        })
        .collect()
}

/// The actions of a runtime card: `Adopt`, which keeps the discovered label and needs the endpoint
/// to reach the runtime (specification 25 §2, lines 89-90).
fn runtime_actions(device: &DeviceAnswer, endpoint: Option<&str>, title: &str) -> Vec<Action> {
    let Some(endpoint) = endpoint else {
        return vec![disabled(
            "adopt",
            texts::ADOPT,
            graph::CONNECT_WITHOUT_ENDPOINT,
            "runtime.adopt",
        )];
    };
    vec![Action {
        id: "adopt".to_owned(),
        label: texts::ADOPT.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Primary,
        confirm: None,
        op: Op::Rust {
            action: "runtime.adopt".to_owned(),
            payload: json!({ "runtime": device.id, "endpoint": endpoint, "label": title }),
        },
    }]
}

/// The actions of a device card: `+ Add` with the connection it would create on the owning runtime,
/// from the first of its protocols that a connection of truST uses.
fn device_actions(device: &DeviceAnswer, owner: Option<&RuntimeOrigin>) -> Vec<Action> {
    let connection = device
        .finds
        .iter()
        .find_map(|find| connection_protocol(find.protocol).map(|protocol| (protocol, find)));
    let action = match (connection, owner) {
        (Some((protocol, find)), Some(owner)) => {
            let request = FormRequest {
                protocol: Some(protocol.to_owned()),
                runtime: Some(owner.id.clone()),
                prefill: find
                    .params
                    .as_object()
                    .cloned()
                    .filter(|map| !map.is_empty()),
                ..FormRequest::default()
            };
            Action {
                id: "add".to_owned(),
                label: graph::ADD.to_owned(),
                enabled: true,
                reason: None,
                emphasis: Emphasis::Secondary,
                confirm: None,
                op: Op::Rust {
                    action: "form.open".to_owned(),
                    payload: request.payload(),
                },
            }
        }
        (Some(_), None) => disabled("add", graph::ADD, texts::ADD_NEEDS_RUNTIME, "form.open"),
        (None, _) => disabled("add", graph::ADD, texts::ADD_NOT_OFFERED, "form.open"),
    };
    vec![action]
}

/// The connection of truST that a device of `protocol` would be added as; `None` when no
/// connection uses it.
fn connection_protocol(protocol: SourceId) -> Option<&'static str> {
    match protocol {
        SourceId::Ads => Some("ads"),
        SourceId::ModbusTcp => Some("modbus_tcp"),
        SourceId::Mqtt => Some("mqtt"),
        SourceId::Opcua => Some("opcua_client"),
        SourceId::Ethercat => Some("ethercat"),
        SourceId::Gpio => Some("gpio"),
        SourceId::TrustRuntime | SourceId::Serial | SourceId::Usb | SourceId::Bluetooth => None,
    }
}

/// The runtime that a device's connection is added to: the origin of the search, else the first
/// runtime the panel offers.
fn owner<'a>(scan: &Scan, runtimes: &'a [RuntimeOrigin]) -> Option<&'a RuntimeOrigin> {
    match scan {
        Scan::ThisHost => runtimes.first(),
        Scan::Runtime(id) => runtimes.iter().find(|runtime| runtime.id == *id),
    }
}

/// Why the results of a runtime that the panel no longer shows cannot be added (specification 25
/// §2): the runtime that found them is gone.
fn stale_reason(scan: &Scan, runtimes: &[RuntimeOrigin]) -> Option<String> {
    match scan {
        Scan::ThisHost => None,
        Scan::Runtime(id) => (!runtimes.iter().any(|runtime| runtime.id == *id))
            .then(|| texts::STALE_RUNTIME.to_owned()),
    }
}

/// Disables every action with `reason`; a disabled action is neutral, not a primary one.
fn disable(actions: &mut [Action], reason: &str) {
    for action in actions {
        action.enabled = false;
        action.reason = Some(reason.to_owned());
        action.emphasis = Emphasis::Quiet;
    }
}

/// A disabled action with its reason. It is never sent, so its operation names the action only.
fn disabled(id: &str, label: &str, reason: &str, action: &str) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: false,
        reason: Some(reason.to_owned()),
        emphasis: Emphasis::Quiet,
        confirm: None,
        op: Op::Rust {
            action: action.to_owned(),
            payload: json!({}),
        },
    }
}

/// The whole seconds from `last_seen_ms` to `now_ms`, never negative.
fn seconds_since(now_ms: u64, last_seen_ms: u64) -> u64 {
    now_ms.saturating_sub(last_seen_ms) / 1000
}

/// The trimmed, non-empty text at `key` of `value`.
fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
