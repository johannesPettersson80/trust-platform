//! The runtime's alarm service on the web (specification 40, §5.1): the runtime keeps the alarm
//! list and the records on its own thread; `GET /api/hmi/alarms` gives them, `POST /api/hmi/alarm`
//! applies an operator action, and the value stream sends both as they change.

mod actions;

use std::sync::{Arc, Mutex};

use serde_json::json;
use smol_str::SmolStr;
use tiny_http::{Header, Method, Request, Response, StatusCode};

use super::auth_helpers::{auth_error_response, check_auth};
use super::pairing::PairingStore;
use crate::config::WebAuthMode;
use crate::control::ControlState;
use crate::hmi::access::sessions::SessionStore;
use crate::hmi::alarms::{AlarmSnapshot, AlarmStatus, EventKind, OperatorAction};
use crate::hmi::commands::registry::{CommandRegistry, Outcome};
use crate::scheduler::{ResourceControl, StdClock};
use crate::security::AccessRole;

/// The web server's way to the runtime's alarm list and records.
pub(super) struct AlarmService {
    resource: ResourceControl<StdClock>,
}

impl AlarmService {
    pub(super) fn start(resource: ResourceControl<StdClock>) -> Arc<Self> {
        Arc::new(Self { resource })
    }

    /// The list when it changed since `generation`, and the records after `after`, as the
    /// program's last `Hmi.Display` call read them (specification 39, §18 Phase 5).
    pub(super) fn snapshot(&self, generation: Option<u64>, after: u64) -> Option<AlarmSnapshot> {
        Some(
            self.resource
                .hmi_exchange()
                .alarm_snapshot(generation, after),
        )
    }

    /// The alarm's path as the program declares it, matched without regard to case.
    pub(super) fn alarm_path(&self, path: &str) -> Option<String> {
        self.resource.hmi_exchange().alarm_path(path)
    }

    /// The alarm's state, for the check of the actions it allows (specification 40, §4).
    pub(super) fn alarm_status(&self, path: &str) -> Option<AlarmStatus> {
        self.resource.hmi_exchange().alarm_status(path)
    }

    /// Records a command's final outcome among the records (§5.1).
    pub(super) fn command(
        &self,
        path: &str,
        message: &str,
        user: Option<&str>,
        outcome: Outcome,
        reason: Option<String>,
    ) {
        let outcome = serde_json::to_value(outcome)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        self.resource
            .hmi_exchange()
            .alarm_command(path, message, user, &outcome, reason);
    }

    /// Records an operator's applied action for the transition it causes.
    pub(super) fn operator_action(
        &self,
        path: &str,
        kind: EventKind,
        user: &str,
        detail: Option<String>,
        seconds: Option<u32>,
    ) {
        self.resource.hmi_exchange().alarm_operator(OperatorAction {
            path: path.to_string(),
            kind,
            user: user.to_string(),
            detail,
            seconds,
        });
    }
}

pub(super) struct HmiAlarmContext<'a> {
    pub(super) auth_mode: WebAuthMode,
    pub(super) auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub(super) pairing: Option<&'a PairingStore>,
    pub(super) sessions: &'a Arc<SessionStore>,
    pub(super) registry: &'a Arc<CommandRegistry>,
    pub(super) control_state: &'a Arc<ControlState>,
    pub(super) alarms: &'a Arc<AlarmService>,
}

pub(super) enum HmiAlarmOutcome {
    Handled,
    NotHandled(Request),
}

pub(super) fn handle_hmi_alarm_route(
    request: Request,
    method: &Method,
    url: &str,
    ctx: HmiAlarmContext<'_>,
) -> HmiAlarmOutcome {
    let path = url.split('?').next().unwrap_or_default();
    match (method, path) {
        (Method::Get, "/api/hmi/alarms") => list(request, &ctx),
        // A cross-site request is answered 403 before it has any effect (spec. 39, §21.5 item 3).
        (Method::Post, "/api/hmi/alarm") => {
            super::http_post_policy::hmi_write_route(request, |request| actions::act(request, &ctx))
        }
        _ => return HmiAlarmOutcome::NotHandled(request),
    }
    HmiAlarmOutcome::Handled
}

/// The list and the records kept in memory (§5.1, item 1).
fn list(request: Request, ctx: &HmiAlarmContext<'_>) {
    if let Err(error) = viewer(&request, ctx) {
        let _ = request.respond(auth_error_response(error));
        return;
    }
    // Each alarm carries the actions its state allows this request's session (§5.7 item 4).
    let role = super::hmi_operate::session_token(&request)
        .and_then(|token| ctx.sessions.session(&token))
        .map(|session| session.role);
    match ctx.alarms.snapshot(None, 0) {
        Some(snapshot) => respond(
            request,
            200,
            &json!({
                "alarms": snapshot.summary.unwrap_or_default().for_role(role),
                "events": snapshot.events,
            }),
        ),
        None => respond(
            request,
            503,
            &json!({ "error": "the runtime did not answer in time" }),
        ),
    }
}

fn viewer(request: &Request, ctx: &HmiAlarmContext<'_>) -> Result<(), &'static str> {
    check_auth(
        request,
        ctx.auth_mode,
        ctx.auth_token,
        ctx.pairing,
        AccessRole::Viewer,
    )
    .map(|_| ())
}

fn respond(request: Request, status: u16, body: &serde_json::Value) {
    let response = Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(
            Header::from_bytes("Content-Type", "application/json").expect("a valid header"),
        )
        .with_header(Header::from_bytes("Cache-Control", "no-store").expect("a valid header"));
    let _ = request.respond(response);
}
