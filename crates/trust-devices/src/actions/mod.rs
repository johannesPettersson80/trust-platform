//! The panel's action path (design §4.3 B7): [`act`](crate::actions::act) decodes the action name
//! and payload of a view's `Op::Rust` and performs it, answering the notice, the host effects and
//! what to refresh next. A slow action answers `pending`; its result arrives exactly once, in
//! `operations_done`.
//!
//! - `effects`: the host-effect lists of the runtime actions.
//! - `runtimes`: Connect, the managed lifecycle, Add host, Add runtime and Adopt.
//! - `config`: the forms and the connection actions, `form.open`, `form.validate`, save, test,
//!   apply to running runtime, diagnose, remove, disable and enable. Each resolves the runtime it
//!   writes to from what the session last showed (design §4.3 B7).
//! - `plans`: the connection plans, expose, the `EtherCAT` channel selection, an OPC UA client add
//!   and an ADS device add, each written through `comm.apply` to the runtime the session resolves.
//! - `ads`: the browse pane's `Create route`, which sends the `TwinCAT` administrator login once to
//!   the runtime's guarded `ads.route_add`.
//! - `write`: the one write path of `config` and `plans`: the runtime that a payload names, resolved
//!   from the session, written over its control endpoint or in process into the open project's
//!   files, and refused when it is unknown or silent (design §4.3 B7).
//!
//! Every notice names the pane that started the action (specification 25 §5.13). An unknown action
//! name, or one whose payload does not decode, is an error reply naming it, never a silent no-op;
//! a name of the closed set that this task does not perform yet answers that it is not available.

mod ads;
mod config;
mod effects;
pub(crate) mod plans;
mod runtimes;
pub(crate) mod write;

use serde_json::Value;

use crate::request::ActRequest;
use crate::session::Session;
use crate::view::types::{ActReply, FieldError, HostEffect, Notice, PendingAction, Refresh, Tone};

pub use runtimes::{Fleet, SystemFleet};

/// The closed set of actions a view may offer (design §1.4.1).
const CLOSED_ACTIONS: [&str; 33] = [
    "runtime.connect",
    "runtime.disconnect",
    "runtime.program_start",
    "runtime.program_stop",
    "runtime.managed_start",
    "runtime.managed_stop",
    "runtime.managed_logs",
    "runtime.set_run_target",
    "runtime.identify",
    "runtime.add_host",
    "runtime.add_local",
    "runtime.adopt",
    "runtime.install_chip",
    "runtime.install_status",
    "runtime.install_cancel",
    "connection.save",
    "connection.remove",
    "connection.disable",
    "connection.enable",
    "connection.apply_live",
    "connection.test",
    "connection.diagnose",
    "connection.expose",
    "connection.ethercat_channels",
    "connection.opcua_add",
    "connection.ads_add",
    "form.open",
    "form.validate",
    "browse.add",
    "ads.tags_set",
    "ads.route_add",
    "search.ignore",
    "search.compare",
];

/// Performs one action that a view offered (design §1.4.1), through the production
/// [`SystemFleet`].
#[must_use]
#[allow(
    clippy::needless_pass_by_value,
    reason = "the panel's entry point names the request by value (design §1.4.1); act_with borrows it"
)]
pub fn act(session: &mut Session, request: ActRequest) -> ActReply {
    act_with(session, &request, &SystemFleet)
}

/// [`act`] with the managed-runtime seam `fleet`: a test passes a fake, so no process is started.
#[must_use]
pub fn act_with(session: &mut Session, request: &ActRequest, fleet: &dyn Fleet) -> ActReply {
    match request.op.action.as_str() {
        "runtime.connect" => runtimes::connect(request),
        "runtime.managed_start" => runtimes::managed_start(session, request, fleet),
        "runtime.managed_stop" => runtimes::managed_stop(session, request, fleet),
        "runtime.add_host" => runtimes::add_host(request),
        "runtime.add_local" => runtimes::add_local(request, fleet),
        "runtime.adopt" => runtimes::adopt(request),
        "form.open" => config::open(session, request),
        "form.validate" => config::validate(session, request),
        "connection.save" => config::save(session, request),
        "connection.test" => config::test(session, request),
        "connection.apply_live" => config::apply_live(session, request),
        "connection.diagnose" => config::diagnose(request),
        "connection.remove" => config::remove(session, request),
        "connection.disable" => config::disable(session, request),
        "connection.enable" => config::enable(session, request),
        "connection.expose" => plans::expose(session, request),
        "connection.ethercat_channels" => plans::ethercat_channels(session, request),
        "connection.opcua_add" => plans::opcua_add(session, request),
        "connection.ads_add" => plans::ads_add(session, request),
        "browse.add" => crate::browse::add(session, request),
        "ads.route_add" => ads::route_add(session, request),
        action if CLOSED_ACTIONS.contains(&action) => error(
            &pane(request),
            &format!("The action '{action}' is not available yet."),
        ),
        action => error(&pane(request), &format!("Unknown action '{action}'.")),
    }
}

/// The pane that started the action (specification 25 §5.13): the form it opened, the runtime's
/// inspector, or a pane of the action itself. A payload that names a form starts in that form, as
/// a connection's actions do; else the runtime's inspector started it.
pub(crate) fn pane(request: &ActRequest) -> String {
    if matches!(
        request.op.action.as_str(),
        "runtime.adopt" | "search.ignore" | "search.compare"
    ) {
        return "discover".to_owned();
    }
    if let Some(form) = payload_text(&request.op.payload, "form") {
        return format!("form:{form}");
    }
    if let Some(runtime) = payload_text(&request.op.payload, "runtime") {
        return format!("inspector:{runtime}");
    }
    String::new()
}

/// The trimmed, non-empty string at `key` of `value`.
pub(crate) fn payload_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// A notice without a detail.
pub(crate) fn notice(tone: Tone, pane: &str, text: impl Into<String>) -> Notice {
    notice_line(tone, pane, text, None)
}

/// A notice with an optional second line.
pub(crate) fn notice_line(
    tone: Tone,
    pane: &str,
    text: impl Into<String>,
    detail: Option<String>,
) -> Notice {
    Notice {
        pane: pane.to_owned(),
        tone,
        text: text.into(),
        detail,
        field_errors: Vec::new(),
        actions: Vec::new(),
    }
}

/// A refused field, which the pane shows beside that field, and nothing is applied.
pub(crate) fn field_error(pane: &str, field: &str, message: &str) -> ActReply {
    let mut notice = notice(Tone::Error, pane, message);
    notice.field_errors = vec![FieldError {
        field: field.to_owned(),
        message: message.to_owned(),
    }];
    reply(false, Some(notice), Vec::new(), false, Refresh::None)
}

/// A failure that is not a field error.
pub(crate) fn error(pane: &str, text: &str) -> ActReply {
    reply(
        false,
        Some(notice(Tone::Error, pane, text)),
        Vec::new(),
        false,
        Refresh::None,
    )
}

/// A direct reply of an action that finished now.
pub(crate) fn reply(
    ok: bool,
    notice: Option<Notice>,
    effects: Vec<HostEffect>,
    close_pane: bool,
    refresh: Refresh,
) -> ActReply {
    ActReply {
        ok,
        notice,
        effects,
        close_pane,
        refresh,
        pending: None,
        form: None,
    }
}

/// The reply of a slow action: what it is doing now, and nothing else; its result arrives later in
/// `operations_done` (design §1.5).
pub(crate) fn pending(operation: String, pane: String, text: String) -> ActReply {
    ActReply {
        ok: true,
        notice: None,
        effects: Vec::new(),
        close_pane: false,
        refresh: Refresh::Soon,
        pending: Some(PendingAction {
            operation,
            pane,
            text,
        }),
        form: None,
    }
}

#[cfg(test)]
mod tests;
