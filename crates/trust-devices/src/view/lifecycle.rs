//! The actions of a runtime (specification 25 §5.1, §5.11 and §5.12, decision O4): its lifecycle
//! action by who owns its process, the program's Start and Stop, `Live Values`, `Debug` and
//! `Deploy`, the smaller actions, the context menu and the Edit slot. Every disabled action
//! carries its reason.
//!
//! - The extension owns the simulator's and the managed runtimes' processes: Start and Stop.
//! - It owns only the session of every other runtime: Connect and Disconnect, and the credential
//!   recovery after an authentication failure.
//! - A runtime whose process the extension does not own also offers its program's Start and Stop
//!   when its `status` reports `program_control` `keeps_control`: it still answers after
//!   `shutdown`, so `restart` can start it again, as on a chip. Start, sent as `restart` with the
//!   mode `cold`, while it answers with the state `stopped`; Stop, sent as `shutdown`, while it
//!   answers with another state. A runtime whose `shutdown` ends its process, or that does not
//!   report the field, is never stopped from the panel (decision O4). The roles these requests
//!   need come from the runtime's operation registry.

use serde_json::json;
use trust_runtime::security::required_role_for_control_request;
use trust_runtime_client::AuthFailure;

use super::status::{ProgramControl, ProgramState, RuntimeStatus};
use super::types::{Action, Emphasis, HostEffect, ItemId, Op, Pane, UiOp};
use crate::texts::graph as texts;
use crate::topology::{ManagedRuntime, ManagedState, SimulatorState};

/// The restart mode that `Start` sends: the mode a new trust-runtime process starts with
/// (`trust-runtime run` and `play` start `cold` unless told otherwise).
pub(crate) const START_MODE: &str = "cold";

/// The run target that selects the simulator.
const SIMULATOR_RUN_TARGET: &str = "simulator";

/// Who owns a runtime's process, which decides its lifecycle action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Owner<'a> {
    /// The local simulator.
    Simulator,
    /// A runtime this computer manages.
    Managed(&'a ManagedRuntime),
    /// Any other runtime: the extension owns only its session.
    Session,
}

/// How the panel reached a runtime whose session it owns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Contact<'a> {
    /// It answered `status`.
    Answered(&'a RuntimeStatus),
    /// Nothing asked it: another runtime reported it, or only the configuration describes it.
    NotAsked,
    /// Nothing answered at its endpoint.
    Unreachable,
    /// It refused the token.
    AuthFailed(AuthFailure),
    /// Its endpoint is missing or turned off.
    NoEndpoint,
    /// It did not answer before the gather's deadline.
    Unanswered,
}

/// What the actions of one runtime are decided from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Subject<'a> {
    /// The runtime's identity.
    pub(crate) id: &'a ItemId,
    /// Its name.
    pub(crate) name: &'a str,
    /// Who owns its process.
    pub(crate) owner: Owner<'a>,
    /// How the panel reached it.
    pub(crate) contact: Contact<'a>,
    /// The control endpoint that its session and program actions use.
    pub(crate) endpoint: Option<&'a str>,
    /// The control endpoint that selects it as the run target.
    pub(crate) run_target_endpoint: Option<&'a str>,
    /// Whether the extension holds a session to it.
    pub(crate) attached: bool,
    /// The simulator's lifecycle, for the simulator.
    pub(crate) simulator: SimulatorState,
    /// Whether the simulator's last start failed.
    pub(crate) simulator_failed: bool,
    /// Whether the simulator itself reported the runtime.
    pub(crate) simulator_answered: bool,
}

impl Subject<'_> {
    fn payload(&self) -> serde_json::Value {
        json!({ "runtime": self.id.as_str(), "endpoint": self.endpoint })
    }

    fn status(&self) -> Option<&RuntimeStatus> {
        match self.contact {
            Contact::Answered(status) => Some(status),
            _ => None,
        }
    }

    /// Whether the simulator runs: it reported itself, or its debug session runs.
    fn simulator_running(&self) -> bool {
        self.simulator_answered
            || (self.simulator == SimulatorState::Running && !self.simulator_failed)
    }
}

fn enabled(id: &str, label: &str, emphasis: Emphasis, op: Op) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: true,
        reason: None,
        emphasis,
        confirm: None,
        op,
    }
}

/// A disabled action: always with its reason, and drawn neutral.
fn disabled(id: &str, label: &str, reason: impl Into<String>, op: Op) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: false,
        reason: Some(reason.into()),
        emphasis: Emphasis::Quiet,
        confirm: None,
        op,
    }
}

fn rust(action: &str, payload: serde_json::Value) -> Op {
    Op::Rust {
        action: action.to_owned(),
        payload,
    }
}

fn host(effect: HostEffect) -> Op {
    Op::Host { effect }
}

fn open_add(runtime: Option<&ItemId>, host: Option<&ItemId>) -> Op {
    Op::Ui {
        ui: UiOp::Open {
            pane: Pane::Add {
                runtime: runtime.cloned(),
                host: host.cloned(),
            },
        },
    }
}

/// The inspector's larger actions: the lifecycle action, the program's Start or Stop, then
/// `Live Values`, `Debug` and `Deploy` (specification 25 §5.12).
pub(crate) fn primary_actions(subject: &Subject<'_>) -> Vec<Action> {
    let mut actions = match subject.owner {
        Owner::Simulator => vec![simulator_action(subject)],
        Owner::Managed(managed) => vec![managed_action(subject, managed)],
        Owner::Session => {
            let mut actions = session_actions(subject);
            actions.extend(program_action(subject));
            actions
        }
    };
    actions.push(live_values(subject));
    actions.push(debug(subject));
    actions.push(disabled(
        "deploy",
        texts::DEPLOY,
        texts::DEPLOY_UNAVAILABLE,
        rust("runtime.deploy", json!({ "runtime": subject.id.as_str() })),
    ));
    actions
}

/// Start and Stop of the simulator: Start again after a failed start, `Starting…` while it
/// starts.
fn simulator_action(subject: &Subject<'_>) -> Action {
    if subject.simulator_running() {
        return enabled(
            "stop",
            texts::STOP,
            Emphasis::Secondary,
            host(HostEffect::StopSession),
        );
    }
    if subject.simulator == SimulatorState::Starting && !subject.simulator_failed {
        return disabled(
            "start",
            texts::STARTING_ELLIPSIS,
            texts::SIMULATOR_STARTING,
            host(HostEffect::StartSimulator),
        );
    }
    enabled(
        "start",
        texts::START,
        Emphasis::Primary,
        host(HostEffect::StartSimulator),
    )
}

/// Start and Stop of a managed runtime through the fleet lifecycle: `Updating…` while it starts
/// or stops, and Start disabled while its status is unavailable, which is never shown as stopped
/// (specification 25 §5.1 and §6.5).
fn managed_action(subject: &Subject<'_>, managed: &ManagedRuntime) -> Action {
    let payload = json!({ "runtime": subject.id.as_str(), "name": managed.name });
    let start = rust("runtime.managed_start", payload.clone());
    let stop = rust("runtime.managed_stop", payload);
    match managed.state {
        ManagedState::Running => enabled("stop", texts::STOP, Emphasis::Secondary, stop),
        ManagedState::Stopped => enabled("start", texts::START, Emphasis::Primary, start),
        ManagedState::Starting => disabled(
            "start",
            texts::UPDATING_ELLIPSIS,
            texts::MANAGED_UPDATING,
            start,
        ),
        ManagedState::Stopping => disabled(
            "stop",
            texts::UPDATING_ELLIPSIS,
            texts::MANAGED_UPDATING,
            stop,
        ),
        ManagedState::Unavailable => {
            disabled("start", texts::START, texts::MANAGED_UNAVAILABLE, start)
        }
    }
}

/// Connect and Disconnect. After an authentication failure the credential recovery comes first
/// and Connect stays as the retry; a runtime without an endpoint, or one that does not answer,
/// keeps Connect visible but disabled with its reason (specification 25 §5.1).
fn session_actions(subject: &Subject<'_>) -> Vec<Action> {
    let connect = |emphasis| {
        enabled(
            "connect",
            texts::CONNECT,
            emphasis,
            rust(
                "runtime.connect",
                json!({
                    "runtime": subject.id.as_str(),
                    "endpoint": subject.endpoint,
                    "label": subject.name,
                }),
            ),
        )
    };
    let blocked = |reason: &str| {
        disabled(
            "connect",
            texts::CONNECT,
            reason,
            rust("runtime.connect", subject.payload()),
        )
    };
    let Some(endpoint) = subject.endpoint else {
        return vec![blocked(texts::CONNECT_WITHOUT_ENDPOINT)];
    };
    if matches!(subject.contact, Contact::AuthFailed(_)) {
        return vec![
            enabled(
                "set_auth_token",
                texts::SET_AUTH_TOKEN,
                Emphasis::Primary,
                host(HostEffect::PromptToken {
                    endpoint: endpoint.to_owned(),
                }),
            ),
            connect(Emphasis::Secondary),
        ];
    }
    if subject.attached {
        return vec![enabled(
            "disconnect",
            texts::DISCONNECT,
            Emphasis::Secondary,
            rust("runtime.disconnect", subject.payload()),
        )];
    }
    vec![match subject.contact {
        Contact::NoEndpoint => blocked(texts::CONNECT_WITHOUT_ENDPOINT),
        Contact::Unreachable => blocked(texts::CONNECT_UNREACHABLE),
        Contact::Unanswered => blocked(texts::CONNECT_UNANSWERED),
        Contact::Answered(_) | Contact::NotAsked | Contact::AuthFailed(_) => {
            connect(Emphasis::Primary)
        }
    }]
}

/// The program's Start while the runtime answers `stopped`, and its Stop while it answers another
/// state, for a runtime that keeps control after `shutdown` (decision O4). A runtime whose
/// `shutdown` ends its process would leave nothing to start again, so it gets neither. Each
/// needs the role that the operation registry names for its request.
fn program_action(subject: &Subject<'_>) -> Option<Action> {
    let status = subject
        .status()
        .filter(|status| status.program_control == ProgramControl::KeepsControl)?;
    let state = status.state?;
    let (id, label, request, params, what, emphasis) = if state == ProgramState::Stopped {
        (
            "program_start",
            texts::START,
            "restart",
            Some(json!({ "mode": START_MODE })),
            texts::STARTING_THE_PROGRAM,
            Emphasis::Primary,
        )
    } else {
        (
            "program_stop",
            texts::STOP,
            "shutdown",
            None,
            texts::STOPPING_THE_PROGRAM,
            Emphasis::Secondary,
        )
    };
    let op = rust(
        &format!("runtime.{id}"),
        json!({
            "runtime": subject.id.as_str(),
            "endpoint": subject.endpoint,
            "params": params,
        }),
    );
    let required = required_role_for_control_request(request, params.as_ref());
    if let Some(role) = status.role
        && !role.allows(required)
    {
        let reason = texts::role_needed(what, required.as_str(), role.as_str());
        return Some(disabled(id, label, reason, op));
    }
    Some(enabled(id, label, emphasis, op))
}

/// `Live Values` shows the runtime the extension is connected to; another runtime says how to
/// connect (specification 25 §4.4).
fn live_values(subject: &Subject<'_>) -> Action {
    let op = host(HostEffect::OpenLiveValues);
    if subject.attached {
        return enabled("live_values", texts::LIVE_VALUES, Emphasis::Secondary, op);
    }
    let reason = match subject.owner {
        Owner::Simulator => texts::LIVE_VALUES_START_SIMULATOR,
        Owner::Managed(_) => texts::LIVE_VALUES_START_RUNTIME,
        Owner::Session => texts::LIVE_VALUES_CONNECT,
    };
    disabled("live_values", texts::LIVE_VALUES, reason, op)
}

/// `Debug`: the simulator is launched under the debugger, another runtime is attached at its
/// endpoint while it answers and its debugging is on.
fn debug(subject: &Subject<'_>) -> Action {
    if let Owner::Simulator = subject.owner {
        let op = host(HostEffect::Debug { endpoint: None });
        return if subject.simulator == SimulatorState::Starting && !subject.simulator_answered {
            disabled("debug", texts::DEBUG, texts::DEBUG_UNREACHABLE, op)
        } else {
            enabled("debug", texts::DEBUG, Emphasis::Secondary, op)
        };
    }
    let op = host(HostEffect::Debug {
        endpoint: subject.endpoint.map(str::to_owned),
    });
    match subject.status() {
        Some(status) if status.debug_enabled == Some(false) => {
            disabled("debug", texts::DEBUG, texts::DEBUG_DISABLED, op)
        }
        Some(_) if subject.endpoint.is_some() => {
            enabled("debug", texts::DEBUG, Emphasis::Secondary, op)
        }
        _ => disabled("debug", texts::DEBUG, texts::DEBUG_UNREACHABLE, op),
    }
}

/// The identity that selects the runtime as the run target: `simulator`, a managed runtime's
/// name, or the runtime's control endpoint.
pub(crate) fn run_target_id(subject: &Subject<'_>) -> Option<String> {
    match subject.owner {
        Owner::Simulator => Some(SIMULATOR_RUN_TARGET.to_owned()),
        Owner::Managed(managed) => Some(managed.name.clone()),
        Owner::Session => subject.run_target_endpoint.map(str::to_owned),
    }
}

fn set_as_run_target(subject: &Subject<'_>) -> Action {
    match run_target_id(subject) {
        Some(id) => enabled(
            "set_run_target",
            texts::SET_AS_RUN_TARGET,
            Emphasis::Secondary,
            host(HostEffect::SetRunTarget { id }),
        ),
        None => disabled(
            "set_run_target",
            texts::SET_AS_RUN_TARGET,
            texts::RUN_TARGET_WITHOUT_ENDPOINT,
            host(HostEffect::SetRunTarget { id: String::new() }),
        ),
    }
}

fn settings() -> Action {
    enabled(
        "settings",
        texts::SETTINGS,
        Emphasis::Secondary,
        host(HostEffect::OpenRuntimeSettings),
    )
}

/// The seconds that `Identify` asks the runtime to blink its identify output for (specification
/// 25 §5.12). The runtime accepts 1 to 60 (specification 11, "Identify"); the panel always asks
/// for ten.
pub(crate) const IDENTIFY_SECONDS: u64 = 10;

/// `Identify`: the runtime blinks its identify output for [`IDENTIFY_SECONDS`] seconds, so that
/// someone can find the box (specification 25 §5.12). It needs a runtime that answered `status`,
/// over the control endpoint, and the Operator role that the operation registry names for
/// `identify` (specification 11). Whether the runtime has an identify output is the runtime's to
/// answer: a runtime without one refuses the request, and the refusal is the notice.
fn identify(subject: &Subject<'_>) -> Action {
    // The action finds the control endpoint from what the session showed, so the payload names
    // only the runtime and shows no address.
    let op = rust(
        "runtime.identify",
        json!({ "runtime": subject.id.as_str() }),
    );
    let Some(status) = subject.status().filter(|_| subject.endpoint.is_some()) else {
        return disabled("identify", texts::IDENTIFY, texts::IDENTIFY_UNREACHABLE, op);
    };
    let required = required_role_for_control_request(
        "identify",
        Some(&json!({ "duration_s": IDENTIFY_SECONDS })),
    );
    if let Some(role) = status.role
        && !role.allows(required)
    {
        let reason = texts::role_needed(texts::IDENTIFY, required.as_str(), role.as_str());
        return disabled("identify", texts::IDENTIFY, reason, op);
    }
    enabled("identify", texts::IDENTIFY, Emphasis::Secondary, op)
}

fn focus(subject: &Subject<'_>) -> Action {
    enabled(
        "focus",
        texts::FOCUS,
        Emphasis::Secondary,
        Op::Ui {
            ui: UiOp::Focus {
                item: subject.id.clone(),
            },
        },
    )
}

/// `Logs`, only where a log backend exists: the simulator's debug log and a managed runtime's
/// log file (specification 25 §5.1).
fn logs(subject: &Subject<'_>) -> Option<Action> {
    let op = match subject.owner {
        Owner::Simulator => host(HostEffect::OpenDebugLog),
        Owner::Managed(managed) => rust(
            "runtime.managed_logs",
            json!({ "runtime": subject.id.as_str(), "name": managed.name }),
        ),
        Owner::Session => return None,
    };
    Some(enabled("logs", texts::LOGS, Emphasis::Secondary, op))
}

/// The inspector's smaller actions: `Set as run target`, `Settings`, `Identify`, `Logs` and
/// `Focus`.
pub(crate) fn secondary_actions(subject: &Subject<'_>) -> Vec<Action> {
    let mut actions = vec![set_as_run_target(subject), settings(), identify(subject)];
    actions.extend(logs(subject));
    actions.push(focus(subject));
    actions
}

/// The right-click menu of a runtime: `Add device or connection…`, which opens `+ Add` for the
/// runtime without choosing a protocol, `Set as run target`, `Settings` and `Focus`
/// (specification 25 §5.11).
pub(crate) fn context_menu(subject: &Subject<'_>) -> Vec<Action> {
    vec![
        enabled(
            "add",
            texts::ADD_DEVICE_OR_CONNECTION_MENU,
            Emphasis::Secondary,
            open_add(Some(subject.id), None),
        ),
        set_as_run_target(subject),
        settings(),
        focus(subject),
    ]
}

/// The Edit slot of a runtime: `Add connection`.
pub(crate) fn runtime_slots(id: &ItemId) -> Vec<Action> {
    vec![enabled(
        "add_connection",
        texts::ADD_CONNECTION,
        Emphasis::Secondary,
        open_add(Some(id), None),
    )]
}

/// The Edit slot of a computer: `Set up runtime`, which opens `+ Add` for another runtime on it
/// (specification 34 ARCH-15).
pub(crate) fn computer_slots(id: &ItemId) -> Vec<Action> {
    vec![enabled(
        "set_up_runtime",
        texts::SET_UP_RUNTIME,
        Emphasis::Secondary,
        open_add(None, Some(id)),
    )]
}

/// The toolbar's actions: `+ Add`, `Discover`, and the Edit slot `Add host`.
pub(crate) fn toolbar_actions() -> [Action; 3] {
    [
        enabled("add", texts::ADD, Emphasis::Primary, open_add(None, None)),
        enabled(
            "discover",
            texts::DISCOVER,
            Emphasis::Secondary,
            Op::Ui {
                ui: UiOp::Open {
                    pane: Pane::Discover,
                },
            },
        ),
        enabled(
            "add_host",
            texts::ADD_HOST,
            Emphasis::Secondary,
            open_add(None, None),
        ),
    ]
}
