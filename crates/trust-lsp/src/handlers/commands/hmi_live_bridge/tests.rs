//! Specification 39, §11 item 6: what Live mode bridges to the run target's HMI web server, when it
//! keeps the named session, and that it decides from facts only.

use serde_json::{json, Value};

use super::live_bridge_value;

fn answer(event: Value) -> Value {
    live_bridge_value(vec![event])
}

fn request(method: &str, path: &str, session: bool) -> Value {
    answer(
        json!({ "event": "request", "method": method, "path": path, "session": session, "peer": false }),
    )
}

const NOT_BRIDGED: &str = "This address is not part of Live operation.";

/// Catches: a read Live mode needs refused, or one with a query (a page by its name, an alarm
/// list's filter) refused.
#[test]
fn the_named_reads_are_bridged_with_or_without_a_query() {
    for route in ["page", "popup", "session", "alarms", "history", "remote"] {
        for path in [
            format!("/api/hmi/{route}"),
            format!("/api/hmi/{route}?name=overview&x=1"),
        ] {
            assert_eq!(
                request("GET", &path, false),
                json!({ "ok": true, "bridged": true, "reply": false }),
                "{path}"
            );
        }
    }
}

/// Catches: a command, an alarm action or a settings request sent without a named session, so that
/// it would act as nobody; and a login or logout refused for want of one.
#[test]
fn the_named_posts_are_bridged_and_the_acting_ones_need_a_session() {
    for route in ["login", "logout"] {
        assert_eq!(
            request("POST", &format!("/api/hmi/{route}"), false)["bridged"],
            true,
            "{route}"
        );
    }
    for route in ["command", "alarm", "settings"] {
        let path = format!("/api/hmi/{route}");
        assert_eq!(
            request("POST", &path, false),
            json!({ "ok": true, "bridged": false, "status": 401, "message": "Log in with a named HMI user to operate." }),
            "{route} without a session"
        );
        assert_eq!(
            request("POST", &path, true),
            json!({ "ok": true, "bridged": true, "reply": false }),
            "{route} within a session"
        );
    }
}

/// Catches: an address outside Live operation forwarded, another host, a path that climbs out of
/// `/api/hmi/`, deployment, a fragment or line break, a read posted or a post read, a query on a
/// post, another method.
#[test]
fn any_other_address_is_refused() {
    let refused = json!({ "ok": true, "bridged": false, "status": 403, "message": NOT_BRIDGED });
    for (method, path) in [
        ("GET", "https://elsewhere/api/hmi/page"),
        ("GET", "//elsewhere/api/hmi/page"),
        ("GET", "/api/hmi/../control"),
        ("GET", "/api/hmi/deploy"),
        ("GET", "/api/hmi/page#fragment"),
        ("GET", "/api/hmi/page?name=a#b"),
        ("GET", "/api/hmi/page?name=a\nb"),
        ("GET", "/api/hmi/page\n"),
        ("GET", "/api/hmi/"),
        ("GET", "/api/hmi/settings"),
        ("GET", "/api/hmi/login"),
        ("POST", "/api/hmi/page"),
        ("POST", "/api/hmi/deploy"),
        ("POST", "/api/hmi/rollback"),
        ("POST", "/api/hmi/settings/export"),
        ("POST", "/api/hmi/settings?action=read"),
        ("POST", "/api/control"),
        ("PUT", "/api/hmi/command"),
        ("DELETE", "/api/hmi/session"),
    ] {
        assert_eq!(request(method, path, true), refused, "{method} {path}");
    }
}

/// Catches: a login's answer that never comes back, so the session is never kept; a peer's login
/// (one an operator makes on another runtime's behalf) replacing the operator's own session; and a
/// logout whose session lives on.
#[test]
fn a_login_for_the_operator_and_a_logout_send_their_answer_back() {
    let reply = |path: &str, peer: bool| {
        answer(json!({ "event": "request", "method": "POST", "path": path, "session": true, "peer": peer }))["reply"].clone()
    };
    assert_eq!(reply("/api/hmi/login", false), true);
    assert_eq!(reply("/api/hmi/login", true), false);
    assert_eq!(reply("/api/hmi/logout", false), true);
    assert_eq!(reply("/api/hmi/command", false), false);
}

/// Catches: a refused login starting a session, a peer's login replacing the operator's, another
/// cookie than the named session's kept, and a logout that leaves the session, also when the web
/// server refuses the logout.
#[test]
fn an_accepted_operator_login_keeps_the_named_session_and_a_logout_drops_it() {
    let session = |path: &str, status: u16, peer: bool| {
        answer(json!({ "event": "reply", "path": path, "status": status, "peer": peer }))
    };
    assert_eq!(
        session("/api/hmi/login", 200, false),
        json!({ "ok": true, "session": "keep", "cookie": "trust_hmi_session" })
    );
    let unchanged = json!({ "ok": true, "session": "unchanged" });
    assert_eq!(session("/api/hmi/login", 401, false), unchanged);
    assert_eq!(session("/api/hmi/login", 200, true), unchanged);
    assert_eq!(session("/api/hmi/command", 200, false), unchanged);
    for status in [200, 500] {
        assert_eq!(
            session("/api/hmi/logout", status, false),
            json!({ "ok": true, "session": "drop" }),
            "logout {status}"
        );
    }
}

/// Catches: a value stream of something Live mode does not show opened, one without what it
/// follows, and one on another address.
#[test]
fn only_page_popup_and_faceplate_streams_are_bridged() {
    let bridged =
        |path: &str| answer(json!({ "event": "stream", "path": path }))["bridged"].clone();
    for path in [
        "/ws/hmi?page=overview",
        "/ws/hmi?popup=valve%20details",
        "/ws/hmi?faceplate=overview/p101&page=overview",
    ] {
        assert_eq!(bridged(path), true, "{path}");
    }
    for path in [
        "/ws/hmi?page=",
        "/ws/hmi?alarms=1",
        "/ws/hmi?page=a#b",
        "/ws/hmi?pagex=1",
        "/ws/hmi",
        "/ws/other?page=a",
        "wss://elsewhere/ws/hmi?page=a",
    ] {
        assert_eq!(bridged(path), false, "{path}");
    }
}

/// Catches: a fact it cannot read answered as if bridged, and a secret taken in: an event carrying a
/// `cookie` or a `password` is refused, so that no caller can make one part of the verdict.
#[test]
fn facts_it_cannot_read_and_secrets_are_refused() {
    for event in [
        json!({ "event": "connect", "path": "/api/hmi/page" }),
        json!({ "event": "request", "method": "GET", "path": "/api/hmi/page" }),
        json!({ "event": "request", "method": "POST", "path": "/api/hmi/login", "session": false, "peer": false, "password": "secret" }),
        json!({ "event": "reply", "path": "/api/hmi/login", "status": 200, "peer": false, "cookie": "trust_hmi_session=abc" }),
        json!({ "event": "stream", "path": "/ws/hmi?page=a", "session": true }),
    ] {
        let refused = answer(event.clone());
        assert_eq!(refused["ok"], false, "{event}");
        let error = refused["error"].as_str().expect("an error");
        assert!(!error.is_empty(), "{event}");
        assert!(
            !error.contains("secret") && !error.contains("trust_hmi_session=abc"),
            "the error never repeats a secret: {error}"
        );
    }
}
