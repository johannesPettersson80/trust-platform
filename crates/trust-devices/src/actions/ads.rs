//! The ADS route action of the panel's `act` (design §4.3 B7; specification 25 §5.4): the browse
//! pane's `Create route` creates the route on the `TwinCAT` target through the guarded
//! `ads.route_add` operation of the runtime the session resolves.
//!
//! The `TwinCAT` administrator login is held only for the request that carries it: it is read from
//! the form values, put into the one `ads.route_add` request, and never stored, echoed or logged
//! (specification 25:389-394). The runtime's own credential-channel gate decides whether the login
//! may travel over the control connection at all (specification 11 §6.9.7), so the panel classifies
//! the connection it made and claims no more.

use std::net::SocketAddr;

use serde_json::{json, Value};

use super::write::{self, Writes};
use crate::request::ActRequest;
use crate::session::Session;
use crate::texts::plans as texts;
use crate::view::types::{ActReply, Refresh, Tone};

/// `ads.route_add`: creates a route on the `TwinCAT` target named by the payload, through the
/// runtime the session resolves, and asks the browse pane to retry (specification 25 §5.4, its
/// lines 387-394).
#[must_use]
pub(crate) fn route_add(session: &Session, request: &ActRequest) -> ActReply {
    let pane =
        super::payload_text(&request.op.payload, "pane").unwrap_or_else(|| super::pane(request));
    let target = match write::target(session, request) {
        Ok(target) => target,
        Err(message) => return super::error(&pane, &message),
    };
    // A route lives on the TwinCAT target and is added over a running runtime's control connection;
    // the open project's files cannot add one (specification 25 §5.4).
    let Writes::Endpoint(endpoint) = &target.writes else {
        return super::error(&pane, texts::ROUTE_NEEDS_RUNTIME);
    };
    let params = match route_add_params(request, endpoint) {
        Ok(params) => params,
        Err(message) => return super::error(&pane, message),
    };
    let route_name = super::payload_text(&request.op.payload, "route_name").unwrap_or_default();
    match write::send_control(request, endpoint, "ads.route_add", &params) {
        Ok(_answer) => super::reply(
            true,
            Some(super::notice(
                Tone::Ok,
                &pane,
                texts::route_created(&route_name),
            )),
            Vec::new(),
            false,
            Refresh::Now,
        ),
        Err(reason) => super::reply(
            false,
            Some(super::notice(
                Tone::Error,
                &pane,
                texts::route_failed(Some(&reason)),
            )),
            Vec::new(),
            false,
            Refresh::None,
        ),
    }
}

/// The `ads.route_add` parameters that the payload and the form values describe: the route and the
/// `TwinCAT` target and this host's identity from the browse pane's route plan, the login from the
/// form, and the panel's own classification of its control connection (specification 25 §5.4).
///
/// # Errors
///
/// The payload names no route, target or host identity, or the form carries no login.
fn route_add_params(request: &ActRequest, endpoint: &str) -> Result<Value, &'static str> {
    let route_name =
        super::payload_text(&request.op.payload, "route_name").ok_or(texts::ROUTE_NEEDS_TARGET)?;
    let target = request
        .op
        .payload
        .get("target")
        .filter(|value| value.is_object())
        .ok_or(texts::ROUTE_NEEDS_TARGET)?;
    let local = request
        .op
        .payload
        .get("local")
        .filter(|value| value.is_object())
        .ok_or(texts::ROUTE_NEEDS_TARGET)?;
    let username = login(request, "username").ok_or(texts::ROUTE_NEEDS_LOGIN)?;
    let password = login(request, "password").ok_or(texts::ROUTE_NEEDS_LOGIN)?;
    Ok(json!({
        "route_name": route_name,
        "target": target,
        "local": local,
        "channel": credential_channel(endpoint),
        "credentials": { "username": username, "password": password },
    }))
}

/// The non-empty login text at `key` of the form values.
fn login<'a>(request: &'a ActRequest, key: &str) -> Option<&'a str> {
    request
        .values
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

/// The credential channel that the resolved runtime sees the panel's control connection as
/// (specification 11 §6.9.7): a Unix socket or a loopback TCP address is this host, and every other
/// address is a remote plain-TCP channel, over which the runtime refuses to use the login.
fn credential_channel(endpoint: &str) -> &'static str {
    let endpoint = endpoint.trim();
    if endpoint.starts_with("unix://") {
        return "trusted_same_host";
    }
    match endpoint
        .strip_prefix("tcp://")
        .and_then(|address| address.parse::<SocketAddr>().ok())
    {
        Some(address) if address.ip().is_loopback() => "trusted_same_host",
        _ => "untrusted_remote_plain_tcp",
    }
}
