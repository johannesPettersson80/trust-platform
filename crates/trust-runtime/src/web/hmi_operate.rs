//! Operating the runtime HMI (specification 39, §12.4, §12.8): logging in and out and the
//! session. Every request first passes the web authentication with the Viewer role, like the
//! page data; commanding then needs the session of a named user.

use std::io::Read;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{json, Value};
use smol_str::SmolStr;
use tiny_http::{Header, Method, Request, Response, StatusCode};

use super::auth_helpers::{auth_error_response, check_auth};
use super::http_post_policy::hmi_write_route;
use super::pairing::PairingStore;
use crate::config::WebAuthMode;
use crate::hmi::access::policy::{acts_as, may_change_settings};
use crate::hmi::access::sessions::{LoginRefusal, SessionInfo, SessionStore};
use crate::security::AccessRole;

/// The session cookie's name.
pub(super) const SESSION_COOKIE: &str = "trust_hmi_session";
/// The largest request body read.
const MAX_BODY: u64 = 16 * 1024;

pub(super) struct HmiOperateContext<'a> {
    pub(super) auth_mode: WebAuthMode,
    pub(super) auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub(super) pairing: Option<&'a PairingStore>,
    pub(super) sessions: &'a Arc<SessionStore>,
    pub(super) commands: &'a Arc<super::hmi_command::Commands>,
    /// The page is served over TLS, so the cookie is marked `Secure`.
    pub(super) secure: bool,
}

pub(super) enum HmiOperateOutcome {
    Handled,
    NotHandled(Request),
}

pub(super) fn handle_hmi_operate_route(
    request: Request,
    method: &Method,
    url: &str,
    ctx: HmiOperateContext<'_>,
) -> HmiOperateOutcome {
    let path = url.split('?').next().unwrap_or_default();
    // Every write route answers a cross-site request 403 before it has any effect (§21.5 item 3).
    match (method, path) {
        (Method::Post, "/api/hmi/login") => {
            hmi_write_route(request, |request| login(request, &ctx))
        }
        (Method::Post, "/api/hmi/logout") => {
            hmi_write_route(request, |request| logout(request, &ctx))
        }
        (Method::Get, "/api/hmi/session") => session(request, &ctx),
        (Method::Get, "/api/hmi/remote") => {
            if let Err(error) = viewer(&request, &ctx) {
                let _ = request.respond(auth_error_response(error));
            } else {
                let state = &ctx.commands.control_state;
                let exchange = state.resource.hmi_exchange();
                let peers = state
                    .mesh_topology
                    .lock()
                    .ok()
                    .and_then(|mesh| mesh.as_ref().map(|mesh| mesh.liveliness_snapshot().peers))
                    .unwrap_or_default();
                let pending = peers
                    .iter()
                    .any(|peer| peer != state.resource_name.as_str())
                    && !exchange.remote_values.catalogue_ready();
                respond(
                    request,
                    200,
                    &json!({"ok":true,"catalogue":exchange.remote_values.catalogue(),"pending":pending}),
                    None,
                );
            }
        }
        (Method::Post, "/api/hmi/command") => {
            hmi_write_route(request, |request| command(request, &ctx))
        }
        (Method::Post, "/api/hmi/settings") => {
            hmi_write_route(request, |request| settings(request, &ctx))
        }
        _ => return HmiOperateOutcome::NotHandled(request),
    }
    HmiOperateOutcome::Handled
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    user: String,
    password: String,
    #[serde(default)]
    peer: Option<String>,
}

fn login(mut request: Request, ctx: &HmiOperateContext<'_>) {
    if let Err(error) = viewer(&request, ctx) {
        let _ = request.respond(auth_error_response(error));
        return;
    }
    let Some(body) = read_json::<LoginRequest>(&mut request) else {
        return respond(
            request,
            400,
            &json!({ "ok": false, "error": "send { \"user\", \"password\" }" }),
            None,
        );
    };
    if let Some(peer) = &body.peer {
        let Some(local) = session_token(&request).and_then(|token| ctx.sessions.touch(&token))
        else {
            return respond(
                request,
                401,
                &json!({"ok":false,"error":"log in locally before selecting an owning runtime"}),
                None,
            );
        };
        let answer = ctx
            .commands
            .control_state
            .resource
            .hmi_exchange()
            .remote_operations
            .login(&local, peer, &body.user, &body.password);
        return match answer {
            Ok(answer) => respond(request, 200, &answer, None),
            Err(reason) => respond(request, 403, &json!({"ok":false,"error":reason}), None),
        };
    }
    match ctx.sessions.login(&body.user, &body.password) {
        Ok((token, info)) => {
            let cookie = format!(
                "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict{}",
                if ctx.secure { "; Secure" } else { "" }
            );
            respond(request, 200, &info_json(&info), Some(cookie));
        }
        Err(LoginRefusal::WrongUserOrPassword) => {
            respond(
                request,
                401,
                &json!({ "ok": false, "error": "unknown user or wrong password" }),
                None,
            );
        }
        Err(LoginRefusal::Locked { retry_in }) => respond(
            request,
            429,
            &json!({
                "ok": false,
                "error": format!("too many failed logins; try again in {} s", retry_in.as_secs().max(1)),
            }),
            None,
        ),
    }
}

fn logout(request: Request, ctx: &HmiOperateContext<'_>) {
    if let Err(error) = viewer(&request, ctx) {
        let _ = request.respond(auth_error_response(error));
        return;
    }
    if let Some(token) = session_token(&request) {
        if let Some(info) = ctx.sessions.session(&token) {
            ctx.commands
                .control_state
                .resource
                .hmi_exchange()
                .remote_operations
                .logout(&info);
        }
        ctx.sessions.logout(&token);
    }
    let cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0");
    respond(request, 200, &json!({ "ok": true }), Some(cookie));
}

fn session(request: Request, ctx: &HmiOperateContext<'_>) {
    if let Err(error) = viewer(&request, ctx) {
        let _ = request.respond(auth_error_response(error));
        return;
    }
    // No session is an answer, not an error: a logged-out page asks without a failed request.
    match session_token(&request).and_then(|token| ctx.sessions.session(&token)) {
        Some(info) => respond(request, 200, &info_json(&info), None),
        None => respond(request, 200, &json!({ "ok": true, "session": null }), None),
    }
}

/// A command (§12.5): the session's activity, then the answer of the command service.
fn command(mut request: Request, ctx: &HmiOperateContext<'_>) {
    if let Err(error) = viewer(&request, ctx) {
        let _ = request.respond(auth_error_response(error));
        return;
    }
    let session = session_token(&request).and_then(|token| ctx.sessions.touch(&token));
    let Some(body) = read_json::<super::hmi_command::CommandRequest>(&mut request) else {
        return respond(
            request,
            400,
            &json!({ "ok": false, "error": "send { \"id\", \"bundle\", \"page\", \"object\", \"action\", \"value\"?, \"shown\"? }" }),
            None,
        );
    };
    let answer = ctx.commands.execute(body, session);
    respond(
        request,
        200,
        &serde_json::to_value(answer).unwrap_or(Value::Null),
        None,
    );
}

/// The Settings dialog (§21.4): the session's activity, then the answer of the settings route.
fn settings(mut request: Request, ctx: &HmiOperateContext<'_>) {
    if let Err(error) = viewer(&request, ctx) {
        let _ = request.respond(auth_error_response(error));
        return;
    }
    let session = session_token(&request).and_then(|token| ctx.sessions.touch(&token));
    let Some(body) = read_json::<super::hmi_command::settings::SettingsRequest>(&mut request)
    else {
        return respond(
            request,
            400,
            &json!({ "ok": false, "error": "send { \"faceplate\", \"action\": \"read\" or \"apply\", \"changes\"?, \"expected\"? }" }),
            None,
        );
    };
    let answer = ctx.commands.settings(body, session);
    respond(request, 200, &answer, None);
}

fn viewer(request: &Request, ctx: &HmiOperateContext<'_>) -> Result<(), &'static str> {
    check_auth(
        request,
        ctx.auth_mode,
        ctx.auth_token,
        ctx.pairing,
        AccessRole::Viewer,
    )
    .map(|_| ())
}

/// What a session may do, for the page (§12.4 item 4): the user, the role, the roles it may act as
/// (`acts_as`, where the page looks up the role a control needs) and whether it may change
/// settings (`may_change_settings`, §21.4 item 1). The login and session answers and the value
/// stream carry the same facts, so the page ranks no roles itself.
pub(super) fn session_facts(info: &SessionInfo) -> Value {
    let roles: Vec<&str> = acts_as(info.role)
        .into_iter()
        .map(AccessRole::as_str)
        .collect();
    json!({
        "user": info.user.as_str(),
        "role": info.role.as_str(),
        "acts_as": roles,
        "may_change_settings": may_change_settings(info.role),
    })
}

/// What a session is, for the page: its facts ([`session_facts`]), its id and the time left.
pub(super) fn info_json(info: &SessionInfo) -> Value {
    let mut answer = session_facts(info);
    answer["ok"] = json!(true);
    answer["session"] = json!(info.id.as_str());
    answer["idle_seconds"] = json!(info.idle_expires_in.as_secs());
    answer["expires_seconds"] = json!(info.expires_in.as_secs());
    answer
}

/// The session token from the request's cookies.
pub(super) fn session_token(request: &Request) -> Option<String> {
    request
        .headers()
        .iter()
        .filter(|header| header.field.equiv("Cookie"))
        .flat_map(|header| header.value.as_str().split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, value)| *name == SESSION_COOKIE && !value.is_empty())
        .map(|(_, value)| value.to_string())
}

fn read_json<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Option<T> {
    let mut body = String::new();
    request
        .as_reader()
        .take(MAX_BODY)
        .read_to_string(&mut body)
        .ok()?;
    serde_json::from_str(&body).ok()
}

fn respond(request: Request, status: u16, body: &Value, cookie: Option<String>) {
    let mut response = Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(
            Header::from_bytes("Content-Type", "application/json").expect("a valid header"),
        )
        .with_header(Header::from_bytes("Cache-Control", "no-store").expect("a valid header"));
    if let Some(cookie) = cookie
        && let Ok(header) = Header::from_bytes("Set-Cookie", cookie)
    {
        response = response.with_header(header);
    }
    let _ = request.respond(response);
}
