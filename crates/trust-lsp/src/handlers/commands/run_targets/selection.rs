//! The sidebar's run targets and the one action of the selected target (specification 25 §5.1 and
//! §6.1): the Simulator, then the runtimes this computer manages, then the workspace project's own
//! runtime while it runs outside VS Code, then the runtimes added in
//! `trust.runtime.fleetEndpoints`, then the runtime the extension is online with when no entry
//! names it. A target is named by its id: `simulator`, a managed runtime's name, `project`, or a
//! control endpoint. The extension owns the processes of the Simulator and of a managed runtime, so
//! they Start and Stop; it owns only the session of any other runtime, so that one Connects and
//! Disconnects and is never stopped.
//!
//! `selection` `{ snapshot, fleetEndpoints?, managed?, selectedId? }` answers `{ ok: true, options,
//! remotes, selected, selectionValid }`: each option's id, label, kind, the control endpoint that
//! selecting it points the runtime settings at and whether it is the project's own runtime; the
//! configured runtimes; the selected target (the Simulator when `selectedId` names none) with its
//! status, its status label and its primary action; whether `selectedId` names an option; and,
//! while the extension is connected to a runtime (its session is attached, or it is online with a
//! runtime that answers), the name the status bar gives that runtime (`connectedLabel`, `null` when
//! it is connected to none). The project's control endpoint as `selectedId` names the project's
//! entry.
//!
//! `select` `{ snapshot, id, endpoint? }` answers what selecting target `id`, whose control
//! endpoint is `endpoint`, does (§5.1): `{ ok: true, changesTarget, settings, endpoint }`.
//! `changesTarget` says that it is another target than the one the runtime settings are on, so a
//! session ends first and a failure reported for the previous target is forgotten. `settings` is
//! `online` at `endpoint` (empty for the project's own runtime, so that each connection asks for
//! its endpoint again), `simulate`, or `keep`.
//!
//! `stop` `{ snapshot }` answers what Stop or Disconnect does while the extension holds no session
//! (§5.1): `{ ok: true, settings, message }`. A runtime the extension is connected to without a
//! session is left by turning the control endpoint off (`settings` `disableEndpoint`, `Runtime
//! endpoint disabled.`); with no runtime connected nothing changes (`keep`, `Runtime already
//! stopped.`).

use serde::Deserialize;
use serde_json::{json, Value};
use trust_devices::actions::managed_label;
use trust_devices::topology::ManagedState;

use super::{decode, endpoint::remote_label};

/// The id of the Simulator.
pub(super) const SIMULATOR: &str = "simulator";
/// The id of the workspace project's own runtime (specification 25 §5.1).
pub(super) const PROJECT: &str = "project";

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
    /// The workspace project's own control endpoint, as `trust-runtime ctl --project <dir>
    /// endpoint --json` reports it (specification 11 §6.8.0a); empty when it reports none.
    #[serde(default)]
    pub(super) project_endpoint: String,
    /// Whether the project's own runtime answers at that endpoint.
    #[serde(default)]
    pub(super) project_reachable: bool,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SelectArgs {
    #[serde(default)]
    snapshot: Snapshot,
    #[serde(default)]
    id: String,
    #[serde(default)]
    endpoint: Option<String>,
}

#[derive(Deserialize)]
struct StopArgs {
    #[serde(default)]
    snapshot: Snapshot,
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
    /// Whether this is the workspace project's own runtime, which selecting puts the runtime
    /// settings online with no endpoint of their own, so that each connection asks for it again.
    project: bool,
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
    let requested = args.selected_id.as_deref().unwrap_or_default();
    let options = options(&remotes, &args.managed, &args.snapshot, requested);
    // A Connect stores the endpoint it connected to; the project's endpoint is its runtime's entry.
    let chosen = options
        .iter()
        .find(|option| option.id == requested)
        .or_else(|| {
            names_project(&args.snapshot, requested)
                .then(|| options.iter().find(|option| option.project))
                .flatten()
        });
    let valid = chosen.is_some();
    let chosen = chosen.unwrap_or(&options[0]);
    let (status, status_label, primary) = match chosen.kind {
        Kind::Remote if chosen.project => project(&args.snapshot),
        Kind::Remote => remote(&chosen.id, &args.snapshot),
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
        "connectedLabel": connected(&args.snapshot).then(|| connected_label(&args.snapshot, &options)),
    }))
}

/// `{ ok: true, changesTarget, settings, endpoint }` for selecting the target in `argument`.
pub(super) fn select(argument: Value) -> Result<Value, String> {
    let args: SelectArgs = decode(argument)?;
    let id = args.id.trim();
    if id.is_empty() {
        return Err("Selecting a target needs its id.".to_owned());
    }
    let snapshot = &args.snapshot;
    let online = (snapshot.runtime_mode == "online").then(|| snapshot.endpoint.trim());
    let endpoint = args.endpoint.as_deref().map(str::trim).unwrap_or_default();
    // The settings that name the target: the project's runtime is followed with no endpoint of
    // their own, a runtime with an endpoint is named, and a target without one leaves online mode.
    let (same, settings, named) = if id == PROJECT {
        let project = snapshot.project_endpoint.trim();
        (!project.is_empty() && online == Some(project), "online", "")
    } else if !endpoint.is_empty() {
        (online == Some(endpoint), "online", endpoint)
    } else if online.is_some() {
        (false, "simulate", "")
    } else {
        (id == SIMULATOR, "keep", "")
    };
    Ok(json!({
        "ok": true,
        "changesTarget": !same,
        "settings": settings,
        "endpoint": named,
    }))
}

/// What Stop or Disconnect does while the extension holds no session (§5.1): a runtime it is
/// connected to is left by turning the control endpoint off; with none connected, nothing changes.
pub(super) fn stop(argument: Value) -> Result<Value, String> {
    let args: StopArgs = decode(argument)?;
    let (settings, message) = if args.snapshot.runtime_state == "connected" {
        ("disableEndpoint", "Runtime endpoint disabled.")
    } else {
        ("keep", "Runtime already stopped.")
    };
    Ok(json!({ "ok": true, "settings": settings, "message": message }))
}

/// Whether the extension is connected to a runtime: its session is attached, or it is online with a
/// runtime that answers, such as one started outside the extension.
fn connected(snapshot: &Snapshot) -> bool {
    snapshot.runtime_state == "connected"
        || (snapshot.runtime_mode == "online" && snapshot.endpoint_reachable)
}

/// The name the status bar gives the runtime the extension is connected to: the project's own
/// runtime by that name when the list holds it and the endpoint is its, `runtime` for another local
/// control socket or no endpoint, else the endpoint without its scheme.
fn connected_label(snapshot: &Snapshot, options: &[Target]) -> String {
    let endpoint = snapshot.endpoint.trim();
    if let Some(project) = options.iter().find(|option| option.project)
        && !endpoint.is_empty()
        && endpoint == snapshot.project_endpoint.trim()
    {
        return project.label.clone();
    }
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
        "project": option.project,
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

/// The target list: the Simulator, the managed runtimes, the project's own runtime while it
/// answers or while `selected` names it, the configured runtimes, and the runtime the extension is
/// online with when no entry names it, no managed runtime has its endpoint and it is not the
/// project's runtime.
fn options(
    remotes: &[(String, String)],
    managed: &[Managed],
    snapshot: &Snapshot,
    selected: &str,
) -> Vec<Target> {
    let mut options = vec![Target {
        id: SIMULATOR.to_owned(),
        label: "Simulator".to_owned(),
        kind: Kind::Simulator,
        run_endpoint: None,
        project: false,
    }];
    for local in managed {
        let running = state_of(local) == ManagedState::Running;
        options.push(Target {
            id: local.name.clone(),
            label: managed_label(&local.name),
            kind: Kind::Local,
            run_endpoint: (running && !local.control_endpoint.trim().is_empty())
                .then(|| local.control_endpoint.trim().to_owned()),
            project: false,
        });
    }
    let project_endpoint = snapshot.project_endpoint.trim();
    let project_listed = (snapshot.project_reachable && !project_endpoint.is_empty())
        || selected == PROJECT
        || names_project(snapshot, selected);
    if project_listed {
        options.push(Target {
            id: PROJECT.to_owned(),
            label: "This project's runtime".to_owned(),
            kind: Kind::Remote,
            run_endpoint: None,
            project: true,
        });
    }
    for (id, label) in remotes {
        options.push(Target {
            id: id.clone(),
            label: label.clone(),
            kind: Kind::Remote,
            run_endpoint: Some(id.clone()),
            project: false,
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
        && !(project_listed && online == project_endpoint)
    {
        options.push(Target {
            id: online.to_owned(),
            label: remote_label(online),
            kind: Kind::Remote,
            run_endpoint: Some(online.to_owned()),
            project: false,
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

/// Whether `id` is the control endpoint of the project's own runtime.
fn names_project(snapshot: &Snapshot, id: &str) -> bool {
    let endpoint = snapshot.project_endpoint.trim();
    !endpoint.is_empty() && id.trim() == endpoint
}

/// The status and the action of the project's own runtime: not running while it does not answer,
/// else those of a runtime at its endpoint that answers.
fn project(snapshot: &Snapshot) -> (&'static str, &'static str, Primary) {
    let endpoint = snapshot.project_endpoint.trim();
    let connected = snapshot.runtime_state == "connected" && snapshot.endpoint.trim() == endpoint;
    if endpoint.is_empty() || !(snapshot.project_reachable || connected) {
        return (
            "unreachable",
            "Not running",
            Primary {
                action: "connect",
                label: "Connect",
                enabled: false,
                hint: Some("This project's runtime is not running."),
            },
        );
    }
    // The project's own probe tells whether it answers: the configured endpoint is not probed while
    // `runtime.controlEndpointEnabled` is off, even when it is the project's.
    remote(
        endpoint,
        &Snapshot {
            endpoint_reachable: true,
            ..snapshot.clone()
        },
    )
}

/// Any other runtime at `endpoint`: only its session is the extension's, so Connect and Disconnect,
/// and Connect disabled with its reason while the runtime is known not to answer (specification 25
/// §5.1).
fn remote(endpoint: &str, snapshot: &Snapshot) -> (&'static str, &'static str, Primary) {
    let active = snapshot.endpoint.trim() == endpoint;
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
