//! `browse.port` (design §4.3 B8 and §3.3 J59; specification 25 §5.4 and its lines 880-883): the
//! typed ADS port of a browse pane. The typed text is read here with the rule of the panel it
//! replaces (`webview/adsTargetPort.ts:9-22`): an empty text is the default port, and any text that
//! is not whole digits, or names a port outside 1 through 65,535, is refused with its message in the
//! pane. A valid port starts its browse in the background; its card joins the pane on the next poll.

use serde_json::Value;

use super::run::{self, DEFAULT_ADS_PORT};
use crate::actions;
use crate::request::{ActRequest, BrowseRequest};
use crate::session::Session;
use crate::texts::browse as texts;
use crate::view::types::{ActReply, Refresh};

/// The port that a typed text names, or the message the pane shows for a text that names none.
///
/// # Errors
///
/// The text is not whole digits, or names a port outside 1 through 65,535.
pub(crate) fn parse_port(typed: &str) -> Result<u16, &'static str> {
    let text = typed.trim();
    if text.is_empty() {
        return Ok(DEFAULT_ADS_PORT);
    }
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(texts::ENTER_WHOLE_PORT);
    }
    match text.parse::<u16>() {
        Ok(port) if port >= 1 => Ok(port),
        _ => Err(texts::ADS_PORT_RANGE),
    }
}

/// `browse.port`: browses the typed port of the pane's own request. The pane posts the typed text as
/// the `port` value and the request as the payload's `request`, which the view built (specification
/// 25 §5.4).
#[must_use]
pub(crate) fn typed_port(session: &Session, request: &ActRequest) -> ActReply {
    let pane = actions::payload_text(&request.op.payload, "pane")
        .unwrap_or_else(|| actions::pane(request));
    let typed = request
        .values
        .get("port")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let port = match parse_port(typed) {
        Ok(port) => port,
        Err(message) => return actions::error(&pane, message),
    };
    let Some(body) = request.op.payload.get("request") else {
        return actions::error(&pane, texts::ADD_DID_NOT_DECODE);
    };
    let mut pane_request: BrowseRequest = match serde_json::from_value(body.clone()) {
        Ok(pane_request) => pane_request,
        Err(error) => {
            return actions::error(&pane, &texts::add_did_not_decode(&error.to_string()));
        }
    };
    pane_request.port = Some(port);
    // The view's request carries no host facts: the action's own facts (the stored token) reach the run.
    pane_request.host = request.host.clone();
    let (resolved, work) = match run::resolve(session, &pane_request) {
        Ok(resolved) => resolved,
        Err(unresolved) => {
            return actions::error(
                &pane,
                &format!("{}: {}", unresolved.error.title, unresolved.error.detail),
            );
        }
    };
    session
        .browse()
        .open(&resolved.key, pane_request.seq, move || run::execute(&work));
    actions::reply(true, None, Vec::new(), false, Refresh::Now)
}
