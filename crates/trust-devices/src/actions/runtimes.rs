//! The runtime actions that the panel's `act` performs (design §4.3 B7, inventories I9, I11, I73
//! and J36): Connect, the managed Start and Stop, Identify, Add host, Add runtime and Adopt. Each
//! one decodes its payload, applies the rules of specification 25 and answers the notice, the host
//! effects and what to refresh next. The managed lifecycle runs `trust-runtime fleet runtime …`
//! behind the [`Fleet`] seam, so a test replaces it and no process is started.

use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};
use trust_runtime::config::fleet_manifest::{
    read_control_auth_token, runtime_project_path, validate_runtime_name,
};

use super::effects;
use super::write;
use crate::request::{ActRequest, DebugSessionKind, HostFacts};
use crate::session::Session;
use crate::texts::forms as form_texts;
use crate::texts::graph as texts;
use crate::topology;
use crate::view::lifecycle::IDENTIFY_SECONDS;
use crate::view::types::{ActReply, HostEffect, ItemId, Notice, OperationDone, Refresh, Tone};

/// The reached state that makes a managed Start a success (specification 25:292-294).
const RUNNING: &str = "running";
/// The reached state that makes a managed Stop a success (specification 25:292-294).
const STOPPED: &str = "stopped";
/// What a managed Start shows while it runs (design §1.4.1).
const STARTING: &str = "Starting…";
/// What a managed Stop shows while it runs (design §1.4.1).
const STOPPING: &str = "Stopping…";

/// No project folder is chosen, so the fleet root of the managed runtimes is unknown.
const NO_FLEET_ROOT: &str = "The panel has no project folder to manage runtimes in.";
/// The extension sent no path of `trust-runtime` (`host.runtime_executable`).
const NO_TOOLS: &str = "The trust-runtime executable was not found; set trust.runtime.cli.path.";
/// `Add a runtime` without an open workspace folder (`fleetActions.ts:63`).
const ADD_LOCAL_NO_WORKSPACE: &str = "Open a workspace folder to add a runtime.";
/// The message of a `trust-runtime` that does not answer with JSON (`localRuntime.ts:143`).
const NO_RESPONSE: &str = "The runtime tools didn't respond.";

/// Runs a managed runtime's fleet command (design §4.3 B7). The production form spawns
/// `trust-runtime`; a test replaces it, so no process is started in a test.
pub trait Fleet {
    /// `trust-runtime fleet runtime start --fleet-root <root> --name <name> --json`.
    ///
    /// # Errors
    ///
    /// The command cannot be run, or it does not answer with JSON.
    fn start(&self, executable: &Path, fleet_root: &Path, name: &str) -> Result<Value, String>;
    /// `trust-runtime fleet runtime stop --fleet-root <root> --name <name> --json`.
    ///
    /// # Errors
    ///
    /// The command cannot be run, or it does not answer with JSON.
    fn stop(&self, executable: &Path, fleet_root: &Path, name: &str) -> Result<Value, String>;
    /// `trust-runtime fleet runtime add --fleet-root <root> --name <name> --template <t> --json`.
    ///
    /// # Errors
    ///
    /// The command cannot be run, or it does not answer with JSON.
    fn add(
        &self,
        executable: &Path,
        fleet_root: &Path,
        name: &str,
        template: &str,
    ) -> Result<Value, String>;
}

/// The [`Fleet`] that runs the `trust-runtime` executable from the host facts.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemFleet;

impl Fleet for SystemFleet {
    fn start(&self, executable: &Path, fleet_root: &Path, name: &str) -> Result<Value, String> {
        run(executable, fleet_root, "start", name, None)
    }

    fn stop(&self, executable: &Path, fleet_root: &Path, name: &str) -> Result<Value, String> {
        run(executable, fleet_root, "stop", name, None)
    }

    fn add(
        &self,
        executable: &Path,
        fleet_root: &Path,
        name: &str,
        template: &str,
    ) -> Result<Value, String> {
        run(executable, fleet_root, "add", name, Some(template))
    }
}

/// Runs `trust-runtime fleet runtime <verb> … --json` and parses its answer. A spawn or parse
/// failure is `NO_RESPONSE` (`localRuntime.ts:63-83`).
fn run(
    executable: &Path,
    fleet_root: &Path,
    verb: &str,
    name: &str,
    template: Option<&str>,
) -> Result<Value, String> {
    let mut command = Command::new(executable);
    command
        .args(["fleet", "runtime", verb, "--fleet-root"])
        .arg(fleet_root)
        .args(["--name", name]);
    if let Some(template) = template {
        command.args(["--template", template]);
    }
    command.arg("--json");
    let output = command.output().map_err(|_| NO_RESPONSE.to_owned())?;
    let stdout = String::from_utf8(output.stdout).map_err(|_| NO_RESPONSE.to_owned())?;
    serde_json::from_str(stdout.trim()).map_err(|_| NO_RESPONSE.to_owned())
}

/// `runtime.connect`: attaches to the runtime that the payload names and selects it as the run
/// target (design §3.3 I9, specification 25:304).
#[must_use]
pub(crate) fn connect(request: &ActRequest) -> ActReply {
    let pane = super::pane(request);
    match super::payload_text(&request.op.payload, "endpoint") {
        Some(endpoint) => {
            let label = request
                .op
                .payload
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default();
            super::reply(
                true,
                None,
                effects::connect(&endpoint, label),
                false,
                Refresh::Now,
            )
        }
        None => super::error(&pane, texts::CONNECT_WITHOUT_ENDPOINT),
    }
}

/// `runtime.managed_start`: starts the managed runtime through the fleet lifecycle, imports its
/// token and attaches (design §4.3 I11, specification 25:308-311 and 858-860).
#[must_use]
pub(crate) fn managed_start(
    session: &mut Session,
    request: &ActRequest,
    fleet: &dyn Fleet,
) -> ActReply {
    let pane = super::pane(request);
    let Some(name) = super::payload_text(&request.op.payload, "name") else {
        return super::error(&pane, "The action did not name the runtime.");
    };
    let Some(fleet_root) = request.project.as_deref() else {
        return super::error(&pane, NO_FLEET_ROOT);
    };
    let Some(executable) = request.host.runtime_executable.as_deref() else {
        return super::error(&pane, NO_TOOLS);
    };
    let operation = session.begin_operation();
    let (notice, effects) = match fleet.start(executable, fleet_root, &name) {
        Ok(answer) => started(&answer, fleet_root, &name, &pane),
        Err(message) => (
            super::notice_line(Tone::Warning, &pane, message, None),
            Vec::new(),
        ),
    };
    session.complete_operation(OperationDone {
        operation: operation.clone(),
        notice: Some(notice),
        effects,
        close_pane: false,
        refresh: Refresh::Now,
        form: None,
    });
    super::pending(operation, pane, STARTING.to_owned())
}

/// `runtime.managed_stop`: stops the managed runtime and ends the session only when it was this
/// runtime's (design §4.3 I11, specification 25:308-311).
#[must_use]
pub(crate) fn managed_stop(
    session: &mut Session,
    request: &ActRequest,
    fleet: &dyn Fleet,
) -> ActReply {
    let pane = super::pane(request);
    let Some(name) = super::payload_text(&request.op.payload, "name") else {
        return super::error(&pane, "The action did not name the runtime.");
    };
    let Some(fleet_root) = request.project.as_deref() else {
        return super::error(&pane, NO_FLEET_ROOT);
    };
    let Some(executable) = request.host.runtime_executable.as_deref() else {
        return super::error(&pane, NO_TOOLS);
    };
    let operation = session.begin_operation();
    let (notice, effects) = match fleet.stop(executable, fleet_root, &name) {
        Ok(answer) => stopped(&answer, request, &name, &pane),
        Err(message) => (
            super::notice_line(Tone::Warning, &pane, message, None),
            Vec::new(),
        ),
    };
    session.complete_operation(OperationDone {
        operation: operation.clone(),
        notice: Some(notice),
        effects,
        close_pane: false,
        refresh: Refresh::Now,
        form: None,
    });
    super::pending(operation, pane, STOPPING.to_owned())
}

/// The result of a managed Start: the effects of a reached `running`, else a failure notice naming
/// the reached state.
fn started(answer: &Value, fleet_root: &Path, name: &str, pane: &str) -> (Notice, Vec<HostEffect>) {
    let state = state_of(answer);
    if !lifecycle_reached(true, state) {
        return (failure(answer, "start", name, state, pane), Vec::new());
    }
    match running_effects(answer, fleet_root, name) {
        Ok(effects) => (
            super::notice_line(Tone::Ok, pane, success_message(answer, name, state), None),
            effects,
        ),
        Err(problem) => (
            super::notice_line(Tone::Warning, pane, problem, None),
            Vec::new(),
        ),
    }
}

/// The result of a managed Stop: `StopSession` only when the runtime that stopped is the one the
/// session is attached to, else a failure notice naming the reached state.
fn stopped(
    answer: &Value,
    request: &ActRequest,
    name: &str,
    pane: &str,
) -> (Notice, Vec<HostEffect>) {
    let state = state_of(answer);
    if !lifecycle_reached(false, state) {
        return (failure(answer, "stop", name, state, pane), Vec::new());
    }
    let endpoint = super::payload_text(answer, "control_endpoint");
    let attached = attached_endpoint(&request.host);
    let effects = if endpoint.is_some() && endpoint.as_deref() == attached {
        vec![HostEffect::StopSession]
    } else {
        Vec::new()
    };
    (
        super::notice_line(Tone::Ok, pane, success_message(answer, name, state), None),
        effects,
    )
}

/// A managed Start's effects: the token of the runtime's project, the attach with the reached
/// endpoint and the friendly label, then the run target (design §4.3 I11).
fn running_effects(
    answer: &Value,
    fleet_root: &Path,
    name: &str,
) -> Result<Vec<HostEffect>, String> {
    let Some(endpoint) = super::payload_text(answer, "control_endpoint") else {
        return Err(format!(
            "Runtime {name} started, but it did not report a control endpoint. Live Values could not connect."
        ));
    };
    let token = match answer.get("path").and_then(Value::as_str) {
        Some(path) => {
            read_control_auth_token(&runtime_project_path(fleet_root, path)).map_err(|error| {
                format!("Runtime {name} started, but its auth token could not be read: {error}")
            })?
        }
        None => None,
    };
    Ok(effects::managed_started(name, &endpoint, token.as_deref()))
}

/// The reached state of a lifecycle answer; a missing state is not success either
/// (specification 25:292-294).
/// Whether a managed Start (`start`) or Stop reached its state: `running` after a Start, `stopped`
/// after a Stop. `starting`, `stopping` and every other state are no success (specification 25
/// §5.1). The panel, the canvas and the sidebar decide with it.
#[must_use]
pub fn lifecycle_reached(start: bool, status: &str) -> bool {
    status == if start { RUNNING } else { STOPPED }
}

fn state_of(answer: &Value) -> &str {
    answer
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// A failure notice naming the reached state in `detail`.
fn failure(answer: &Value, verb: &str, name: &str, state: &str, pane: &str) -> Notice {
    let text = super::payload_text(answer, "message")
        .unwrap_or_else(|| format!("Could not {verb} {name}."));
    super::notice_line(Tone::Warning, pane, text, Some(state.to_owned()))
}

/// What a successful lifecycle shows: the command's message, else what it reached.
fn success_message(answer: &Value, name: &str, state: &str) -> String {
    super::payload_text(answer, "message").unwrap_or_else(|| format!("Runtime {name} is {state}."))
}

/// The control endpoint of the runtime that the session is attached to, if any.
fn attached_endpoint(host: &HostFacts) -> Option<&str> {
    host.debug_session
        .as_ref()
        .filter(|session| session.kind == DebugSessionKind::Attach && !session.starting)
        .and_then(|session| session.endpoint.as_deref())
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
}

/// `runtime.identify`: asks the runtime that the payload names to show where it is, with
/// `identify {"duration_s": 10}` over its control endpoint (specification 25 §5.12; specification
/// 11, "Identify"), and shows the runtime's own answer as the notice of the pane that started it:
/// `Identifying <name>` with the seconds the runtime reports, or the runtime's refusal text as it
/// is, such as `identify is not configured: set [runtime.identify] output in runtime.toml`. A
/// runtime that did not answer the last round is refused by name, and nothing is sent.
#[must_use]
pub(crate) fn identify(session: &Session, request: &ActRequest) -> ActReply {
    let pane = super::pane(request);
    let Some(id) = super::payload_text(&request.op.payload, "runtime") else {
        return super::error(&pane, form_texts::NO_RUNTIME);
    };
    let Some(runtime) = session.runtime(&id) else {
        return super::error(&pane, &form_texts::runtime_not_shown(&id));
    };
    let endpoint = runtime
        .control_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| runtime.answers && !endpoint.is_empty());
    let Some(endpoint) = endpoint else {
        return super::error(&pane, &texts::identify_unanswered(&runtime.name));
    };
    let params = json!({ "duration_s": IDENTIFY_SECONDS });
    match write::send_control(request, endpoint, "identify", &params) {
        Ok(answer) => {
            let seconds = answer
                .get("duration_s")
                .and_then(Value::as_u64)
                .unwrap_or(IDENTIFY_SECONDS);
            super::reply(
                true,
                Some(super::notice(
                    Tone::Ok,
                    &pane,
                    texts::identifying(&runtime.name, seconds),
                )),
                Vec::new(),
                false,
                Refresh::None,
            )
        }
        Err(message) => super::error(&pane, &message),
    }
}

/// `runtime.add_host`: checks and normalises the typed address, keeps the label and adds the
/// endpoint to the fleet, then focuses the new runtime (`fleetActions.ts:17-52`; design §3.3 I73
/// and I75, J21; specification 25:330-334).
#[must_use]
pub(crate) fn add_host(request: &ActRequest) -> ActReply {
    let pane = super::pane(request);
    let typed = request
        .values
        .get("endpoint")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Some(endpoint) = normalize_endpoint(typed) else {
        return super::field_error(&pane, "endpoint", form_texts::RUNTIME_ADDRESS_HELP);
    };
    let token = value_text(request, "token");
    let label = super::payload_text(&request.op.payload, "label");
    let mut effects = Vec::new();
    if let Some(token) = &token {
        effects.push(effects::store_secret(&endpoint, token));
    }
    if let Some(label) = &label {
        effects.push(effects::store_label(&endpoint, label));
    }
    if token.is_none() {
        effects.push(effects::select(ItemId(topology::configured_runtime_id(
            &endpoint,
        ))));
    }
    let already = request
        .host
        .settings
        .additional_endpoints
        .iter()
        .any(|known| known == &endpoint);
    let notice = if already {
        Some(super::notice(
            Tone::Neutral,
            &pane,
            format!("{endpoint} is already in the fleet."),
        ))
    } else {
        let mut endpoints = request.host.settings.additional_endpoints.clone();
        endpoints.push(endpoint);
        effects.push(effects::write_fleet_endpoints(&endpoints));
        None
    };
    super::reply(true, notice, effects, true, Refresh::Now)
}

/// `runtime.add_local`: checks the name with the fleet's own rule and scaffolds the runtime
/// (`fleetActions.ts:54-93`, J22).
#[must_use]
pub(crate) fn add_local(request: &ActRequest, fleet: &dyn Fleet) -> ActReply {
    let pane = super::pane(request);
    let name = request
        .values
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if let Err(error) = validate_runtime_name(name) {
        return super::field_error(&pane, "name", &error.to_string());
    }
    let template = if value_text(request, "template").as_deref() == Some("empty") {
        "empty"
    } else {
        "simulate"
    };
    let Some(fleet_root) = request.project.as_deref() else {
        return super::error(&pane, ADD_LOCAL_NO_WORKSPACE);
    };
    let Some(executable) = request.host.runtime_executable.as_deref() else {
        return super::error(&pane, NO_TOOLS);
    };
    match fleet.add(executable, fleet_root, name, template) {
        Ok(answer) => {
            let endpoint = super::payload_text(&answer, "control_endpoint");
            let path = super::payload_text(&answer, "path").unwrap_or_default();
            let created = super::payload_text(&answer, "name").unwrap_or_else(|| name.to_owned());
            let mut effects = Vec::new();
            if let Some(endpoint) = &endpoint
                && !request
                    .host
                    .settings
                    .additional_endpoints
                    .iter()
                    .any(|known| known == endpoint)
            {
                let mut endpoints = request.host.settings.additional_endpoints.clone();
                endpoints.push(endpoint.clone());
                effects.push(effects::write_fleet_endpoints(&endpoints));
            }
            let text = format!(
                "Created runtime \"{created}\" at {path} ({}). Start it to see it on the canvas.",
                endpoint.as_deref().unwrap_or_default()
            );
            super::reply(
                true,
                Some(super::notice(Tone::Ok, &pane, text)),
                effects,
                true,
                Refresh::Now,
            )
        }
        Err(message) => super::reply(
            false,
            Some(super::notice_line(
                Tone::Warning,
                &pane,
                format!(
                    "Could not create runtime \"{name}\" (it may already exist, or needs a newer trust-runtime)."
                ),
                Some(message),
            )),
            Vec::new(),
            false,
            Refresh::None,
        ),
    }
}

/// `runtime.adopt`: keeps the found label, adds the runtime to the fleet and selects it (design
/// §3.2 J36, specification 25:89-90).
#[must_use]
pub(crate) fn adopt(request: &ActRequest) -> ActReply {
    let pane = super::pane(request);
    let item = super::payload_text(&request.op.payload, "runtime");
    let endpoint = super::payload_text(&request.op.payload, "endpoint");
    let label = super::payload_text(&request.op.payload, "label");
    let (Some(item), Some(endpoint)) = (item, endpoint) else {
        return super::error(&pane, "The action did not name the runtime to adopt.");
    };
    let mut effects = Vec::new();
    if let Some(label) = &label {
        effects.push(effects::store_label(&endpoint, label));
    }
    effects.push(effects::select(ItemId(item)));
    if !request
        .host
        .settings
        .additional_endpoints
        .iter()
        .any(|known| known == &endpoint)
    {
        let mut endpoints = request.host.settings.additional_endpoints.clone();
        endpoints.push(endpoint);
        effects.push(effects::write_fleet_endpoints(&endpoints));
    }
    super::reply(true, None, effects, false, Refresh::Now)
}

/// Normalises a typed runtime address (design §3.3 I75, J21; specification 25:332): a bare
/// `host:port` becomes `tcp://host:port`, a `tcp://` or `unix://` endpoint is kept as typed, and
/// anything else is `None`.
#[must_use]
pub fn normalize_endpoint(typed: &str) -> Option<String> {
    let trimmed = typed.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("tcp://") || trimmed.starts_with("unix://") {
        return Some(trimmed.to_owned());
    }
    if is_host_port(trimmed) {
        return Some(format!("tcp://{trimmed}"));
    }
    None
}

/// Whether `value` is `host:port` (`/^[^/\s:]+:\d+$/`) or `[host]:port` (`/^\[[^\]]+\]:\d+$/`).
fn is_host_port(value: &str) -> bool {
    let Some((host, port)) = value.rsplit_once(':') else {
        return false;
    };
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    match host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
    {
        Some(inner) => {
            !inner.is_empty() && !inner.chars().any(|ch| ch.is_whitespace() || ch == ']')
        }
        None => {
            !host.is_empty()
                && !host
                    .chars()
                    .any(|ch| ch == '/' || ch == ':' || ch.is_whitespace())
        }
    }
}

/// The trimmed, non-empty value of the form field `key`.
fn value_text(request: &ActRequest, key: &str) -> Option<String> {
    request
        .values
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
