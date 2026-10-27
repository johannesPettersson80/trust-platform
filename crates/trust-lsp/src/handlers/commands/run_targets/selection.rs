//! The sidebar's run targets and the one action of the selected target (specification 25 §5.1 and
//! §6.1): the Simulator, then the runtimes this computer manages, then the runtimes added in
//! `trust.runtime.fleetEndpoints`, then the runtime the extension is online with when no entry names
//! it. A target is named by its id: `simulator`, a managed runtime's name or a control endpoint.
//! The extension owns the processes of the Simulator and of a managed runtime, so they Start and
//! Stop; it owns only the session of any other runtime, so that one Connects and Disconnects and is
//! never stopped.
//!
//! `selection` `{ snapshot, fleetEndpoints?, managed?, selectedId? }` answers `{ ok: true, options,
//! remotes, selected, selectionValid }`: each option's id, label, kind and the control endpoint that
//! selecting it points the runtime settings at; the configured runtimes; the selected target (the
//! Simulator when `selectedId` names none) with its status, its status label and its primary action;
//! whether `selectedId` names an option; and, while the extension is connected to a runtime (its
//! session is attached, or it is online with a runtime that answers), the name the status bar gives
//! that runtime (`connectedLabel`, `null` when it is connected to none).

use serde::Deserialize;
use serde_json::{json, Value};
use trust_devices::actions::managed_label;
use trust_devices::topology::ManagedState;

use super::{decode, endpoint::remote_label};

/// The id of the Simulator.
pub(super) const SIMULATOR: &str = "simulator";

/// The lifecycle snapshot of the extension: the runtime settings and the debug session.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Snapshot {
    #[serde(default)]
    pub(super) runtime_mode: String,
    #[serde(default)]
    pub(super) runtime_state: String,
    #[serde(default)]
    pub(super) endpoint: String,
    #[serde(default)]
    pub(super) endpoint_configured: bool,
    #[serde(default)]
    pub(super) endpoint_reachable: bool,
    #[serde(default)]
    pub(super) starting: bool,
}

/// A managed runtime as the extension lists it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Managed {
    pub(super) name: String,
    #[serde(default)]
    pub(super) control_endpoint: String,
    #[serde(default)]
    pub(super) state: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Args {
    #[serde(default)]
    snapshot: Snapshot,
    #[serde(default)]
    fleet_endpoints: Vec<Option<String>>,
    #[serde(default)]
    managed: Vec<Managed>,
    #[serde(default)]
    selected_id: Option<String>,
}

/// Who owns a target's process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Simulator,
    Local,
    Remote,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Simulator => "simulator",
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

/// One entry of the target list.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    id: String,
    label: String,
    kind: Kind,
    /// The control endpoint that selecting the target points the runtime settings at: none for the
    /// Simulator and for a managed runtime that does not run (specification 25 §5.1).
    run_endpoint: Option<String>,
}

/// The primary action of the selected target.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Primary {
    action: &'static str,
    label: &'static str,
    enabled: bool,
    hint: Option<&'static str>,
}

/// `{ ok: true, options, remotes, selected, selectionValid }` for `argument`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    let remotes = remotes(&args.fleet_endpoints);
    let options = options(&remotes, &args.managed, &args.snapshot);
    let requested = args.selected_id.as_deref().unwrap_or_default();
    let valid = options.iter().any(|option| option.id == requested);
    let chosen = options
        .iter()
        .find(|option| option.id == requested)
        .unwrap_or(&options[0]);
    let (status, status_label, primary) = match chosen.kind {
        Kind::Remote => remote(chosen, &args.snapshot),
        Kind::Local => managed(chosen, &args.managed),
        Kind::Simulator => simulator(&args.snapshot),
    };
    Ok(json!({
        "ok": true,
        "options": options.iter().map(option_json).collect::<Vec<_>>(),
        "remotes": remotes
            .iter()
            .map(|(id, label)| json!({ "id": id, "label": label }))
            .collect::<Vec<_>>(),
        "selected": {
            "id": chosen.id,
            "label": chosen.label,
            "kind": chosen.kind.name(),
            "runEndpoint": chosen.run_endpoint,
            "status": status,
            "statusLabel": status_label,
            "primary": {
                "action": primary.action,
                "label": primary.label,
                "enabled": primary.enabled,
                "hint": primary.hint,
                // The compile gate blocks only an enabled Start (specification 25 §6.6).
                "gateable": primary.action == "start" && primary.enabled,
            },
        },
        "selectionValid": valid,
        "connectedLabel": connected(&args.snapshot).then(|| connected_label(&args.snapshot.endpoint)),
    }))
}

/// Whether the extension is connected to a runtime: its session is attached, or it is online with a
/// runtime that answers, such as one started outside the extension.
fn connected(snapshot: &Snapshot) -> bool {
    snapshot.runtime_state == "connected"
        || (snapshot.runtime_mode == "online" && snapshot.endpoint_reachable)
}

/// The name the status bar gives the runtime the extension is connected to: `runtime` for a local
/// control socket or no endpoint, else the endpoint without its scheme.
fn connected_label(endpoint: &str) -> String {
    let endpoint = endpoint.trim();
    let socket = endpoint
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("unix://"));
    if endpoint.is_empty() || socket {
        "runtime".to_owned()
    } else {
        remote_label(endpoint)
    }
}

fn option_json(option: &Target) -> Value {
    json!({
        "id": option.id,
        "label": option.label,
        "kind": option.kind.name(),
        "runEndpoint": option.run_endpoint,
    })
}

/// The runtimes of `trust.runtime.fleetEndpoints`: each endpoint trimmed, once, blank ones left out,
/// with the name its endpoint gives it.
fn remotes(endpoints: &[Option<String>]) -> Vec<(String, String)> {
    let mut remotes: Vec<(String, String)> = Vec::new();
    for raw in endpoints {
        let endpoint = raw.as_deref().unwrap_or_default().trim();
        if endpoint.is_empty() || remotes.iter().any(|(id, _)| id == endpoint) {
            continue;
        }
        remotes.push((endpoint.to_owned(), remote_label(endpoint)));
    }
    remotes
}

/// The target list: the Simulator, the managed runtimes, the configured runtimes, and the runtime
/// the extension is online with when no entry names it and no managed runtime has its endpoint.
fn options(remotes: &[(String, String)], managed: &[Managed], snapshot: &Snapshot) -> Vec<Target> {
    let mut options = vec![Target {
        id: SIMULATOR.to_owned(),
        label: "Simulator".to_owned(),
        kind: Kind::Simulator,
        run_endpoint: None,
    }];
    for local in managed {
        let running = state_of(local) == ManagedState::Running;
        options.push(Target {
            id: local.name.clone(),
            label: managed_label(&local.name),
            kind: Kind::Local,
            run_endpoint: (running && !local.control_endpoint.trim().is_empty())
                .then(|| local.control_endpoint.trim().to_owned()),
        });
    }
    for (id, label) in remotes {
        options.push(Target {
            id: id.clone(),
            label: label.clone(),
            kind: Kind::Remote,
            run_endpoint: Some(id.clone()),
        });
    }
    let online = if snapshot.runtime_mode == "online" {
        snapshot.endpoint.trim()
    } else {
        ""
    };
    if !online.is_empty()
        && !options.iter().any(|option| option.id == online)
        && !managed.iter().any(|local| local.control_endpoint == online)
    {
        options.push(Target {
            id: online.to_owned(),
            label: remote_label(online),
            kind: Kind::Remote,
            run_endpoint: Some(online.to_owned()),
        });
    }
    options
}

/// The state of a managed runtime as its fleet status names it; an unknown or missing one is
/// `Unavailable`, never stopped (specification 25 §6.5).
fn state_of(local: &Managed) -> ManagedState {
    ManagedState::from_status(local.state.as_deref())
}

/// A managed runtime: its process is the extension's, so Start and Stop by its reported state.
fn managed(option: &Target, managed: &[Managed]) -> (&'static str, &'static str, Primary) {
    let state = managed
        .iter()
        .find(|local| local.name == option.id)
        .map_or(ManagedState::Unavailable, state_of);
    match state {
        ManagedState::Running => ("running", "Running", enabled("stop", "Stop")),
        ManagedState::Stopped => ("stopped", "Stopped", enabled("start", "Start")),
        ManagedState::Starting => ("starting", "Starting…", transition("Starting…")),
        ManagedState::Stopping => ("stopping", "Stopping…", transition("Stopping…")),
        ManagedState::Unavailable => (
            "unavailable",
            "Status unavailable",
            Primary {
                action: "start",
                label: "Start",
                enabled: false,
                hint: Some("Status unavailable — refresh managed runtimes before starting."),
            },
        ),
    }
}

/// The Simulator: its debug process is the extension's, so Start and Stop by the session.
fn simulator(snapshot: &Snapshot) -> (&'static str, &'static str, Primary) {
    if snapshot.starting {
        return ("starting", "Starting…", transition("Starting…"));
    }
    if snapshot.runtime_mode == "simulate" && snapshot.runtime_state == "running" {
        return ("running", "Running", enabled("stop", "Stop"));
    }
    ("stopped", "Stopped", enabled("start", "Start"))
}

/// Any other runtime: only its session is the extension's, so Connect and Disconnect, and Connect
/// disabled with its reason while the runtime is known not to answer (specification 25 §5.1).
fn remote(option: &Target, snapshot: &Snapshot) -> (&'static str, &'static str, Primary) {
    let active = snapshot.endpoint == option.id;
    if active && snapshot.runtime_mode == "online" && snapshot.runtime_state == "connected" {
        return (
            "connected",
            "Connected",
            enabled("disconnect", "Disconnect"),
        );
    }
    if active && snapshot.starting {
        return ("starting", "Connecting…", transition("Connecting…"));
    }
    if active && snapshot.endpoint_configured && !snapshot.endpoint_reachable {
        return (
            "unreachable",
            "Not reachable",
            Primary {
                action: "connect",
                label: "Connect",
                enabled: false,
                hint: Some(
                    "Not reachable — open Devices & Connections to start or diagnose this runtime.",
                ),
            },
        );
    }
    (
        "disconnected",
        "Not connected",
        enabled("connect", "Connect"),
    )
}

fn enabled(action: &'static str, label: &'static str) -> Primary {
    Primary {
        action,
        label,
        enabled: true,
        hint: None,
    }
}

/// A target in transition: no action, its progress as the label.
fn transition(label: &'static str) -> Primary {
    Primary {
        action: "none",
        label,
        enabled: false,
        hint: None,
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
