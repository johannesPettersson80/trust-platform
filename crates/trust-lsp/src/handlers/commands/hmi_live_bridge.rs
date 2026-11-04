//! What the HMI Builder's extension host bridges to the selected run target's HMI web server, and
//! when it keeps the named session (specification 39, §11 item 6). Live mode's operating view and an
//! HMI agent simulator journey send their requests through one session in the extension host, which
//! asks here first and keeps the session's cookie in its own memory. Only facts come here, never the
//! password or the cookie: a language client's `verbose` trace logs every message.
//!
//! `trust-lsp.hmiLiveBridge` `{ event, … }`:
//! - `request` `{ method, path, session, peer }`, before a request goes out, `session` whether a
//!   named session is held and `peer` whether a login names a peer. It answers `{ ok: true,
//!   bridged: true, reply }`, `reply` whether the answer's status comes back (a login for the
//!   operator, a logout), or `{ ok: true, bridged: false, status, message }`: `403` for an address
//!   Live mode does not bridge, `401` for a command, an alarm action or a settings request without a
//!   named session.
//! - `reply` `{ path, status, peer }`, after a request whose verdict asked for it. It answers
//!   `{ ok: true, session }`, `session` `keep` with `cookie`, the name of the cookie to keep from the
//!   answer's `Set-Cookie` (a login for the operator that the web server accepted), `drop` (a
//!   logout), or `unchanged`.
//! - `stream` `{ path }`, before a value stream opens. It answers `{ ok: true, bridged }`.
//!
//! Arguments it cannot read, an unknown event or a field the event does not take among them, answer
//! `{ ok: false, error }`.

use serde::Deserialize;
use serde_json::{json, Value};

use super::hmi_files::single_argument;
use super::HMI_LIVE_BRIDGE_COMMAND;

#[cfg(test)]
mod tests;

/// What the extension host is about to do, as facts.
#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase", deny_unknown_fields)]
enum Event {
    Request {
        method: String,
        path: String,
        session: bool,
        peer: bool,
    },
    Reply {
        path: String,
        status: u16,
        peer: bool,
    },
    Stream {
        path: String,
    },
}

/// The routes Live mode reads, each with an optional query.
const READS: [&str; 6] = ["page", "popup", "session", "alarms", "history", "remote"];
/// The routes Live mode posts to, without a query.
const WRITES: [&str; 5] = ["login", "logout", "command", "alarm", "settings"];
/// The posts that act as the named operator.
const NEEDS_SESSION: [&str; 3] = ["command", "alarm", "settings"];
/// What a value stream follows.
const STREAMS: [&str; 3] = ["page", "popup", "faceplate"];
/// The named session's cookie, as the web server sets it.
const SESSION_COOKIE: &str = "trust_hmi_session";

const NOT_BRIDGED: &str = "This address is not part of Live operation.";
const NEEDS_LOGIN: &str = "Log in with a named HMI user to operate.";

/// The answer of `trust-lsp.hmiLiveBridge`.
pub(super) fn live_bridge_value(args: Vec<Value>) -> Value {
    match single_argument::<Event>(args, HMI_LIVE_BRIDGE_COMMAND) {
        Ok(Event::Request {
            method,
            path,
            session,
            peer,
        }) => request(&method, &path, session, peer),
        Ok(Event::Reply { path, status, peer }) => reply(&path, status, peer),
        Ok(Event::Stream { path }) => json!({ "ok": true, "bridged": stream(&path) }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

/// The route that a path names under `/api/hmi/`, and its query.
fn route(path: &str) -> Option<(&str, Option<&str>)> {
    let (route, query) = match path.split_once('?') {
        Some((route, query)) => (route, Some(query)),
        None => (path, None),
    };
    route.strip_prefix("/api/hmi/").map(|name| (name, query))
}

/// Plain text to its end: no fragment and no line break.
fn plain(text: &str) -> bool {
    !text.contains(['#', '\r', '\n'])
}

fn request(method: &str, path: &str, session: bool, peer: bool) -> Value {
    let refused = |status: u16, message: &str| json!({ "ok": true, "bridged": false, "status": status, "message": message });
    let Some((name, query)) = route(path) else {
        return refused(403, NOT_BRIDGED);
    };
    let bridged = match method {
        "GET" => READS.contains(&name) && query.is_none_or(plain),
        "POST" => WRITES.contains(&name) && query.is_none(),
        _ => false,
    };
    if !bridged {
        return refused(403, NOT_BRIDGED);
    }
    if method == "POST" && NEEDS_SESSION.contains(&name) && !session {
        return refused(401, NEEDS_LOGIN);
    }
    // A login for the operator may start the named session, and a logout ends it.
    let reply = method == "POST" && ((name == "login" && !peer) || name == "logout");
    json!({ "ok": true, "bridged": true, "reply": reply })
}

fn reply(path: &str, status: u16, peer: bool) -> Value {
    match route(path) {
        Some(("login", None)) if status == 200 && !peer => {
            json!({ "ok": true, "session": "keep", "cookie": SESSION_COOKIE })
        }
        Some(("logout", None)) => json!({ "ok": true, "session": "drop" }),
        _ => json!({ "ok": true, "session": "unchanged" }),
    }
}

fn stream(path: &str) -> bool {
    path.strip_prefix("/ws/hmi?")
        .and_then(|query| query.split_once('='))
        .is_some_and(|(key, value)| STREAMS.contains(&key) && !value.is_empty() && plain(value))
}
