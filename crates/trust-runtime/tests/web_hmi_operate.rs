//! Specification 39, §12.4 and §12.8: logging in to the runtime HMI, the session and its end.

use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::hmi::access::config::{HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::security::AccessRole;
use tungstenite::Message;

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

fn access(idle: Duration) -> HmiAccessConfig {
    let user = |name: &str, role| HmiUser {
        name: name.into(),
        role,
        password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
            .expect("hash"),
    };
    HmiAccessConfig {
        idle,
        session: Duration::from_secs(3600),
        users: vec![
            user("anna", AccessRole::Operator),
            user("erik", AccessRole::Engineer),
        ],
        write: Vec::new(),
    }
}

/// A request's status, its `Set-Cookie` header and its JSON body.
fn call(
    method: &str,
    base: &str,
    path: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (u16, String, Value) {
    let url = format!("{base}{path}");
    let mut response = match (method, body) {
        ("POST", body) => {
            // The HMI write routes take JSON only (specification 39, §21.5 item 3).
            let mut request = ureq::post(&url)
                .config()
                .http_status_as_error(false)
                .build()
                .header("Content-Type", "application/json");
            if let Some(cookie) = cookie {
                request = request.header("Cookie", cookie);
            }
            request.send(body.unwrap_or(json!({})).to_string())
        }
        _ => {
            let mut request = ureq::get(&url).config().http_status_as_error(false).build();
            if let Some(cookie) = cookie {
                request = request.header("Cookie", cookie);
            }
            request.call()
        }
    }
    .unwrap_or_else(|error| panic!("{method} {path}: {error}"));
    let status = response.status().as_u16();
    let set_cookie = response
        .headers()
        .get("Set-Cookie")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    let body = serde_json::from_str(&text).unwrap_or(Value::Null);
    (status, set_cookie, body)
}

fn login(base: &str, user: &str, password: &str) -> (u16, String, Value) {
    call(
        "POST",
        base,
        "/api/hmi/login",
        None,
        Some(json!({ "user": user, "password": password })),
    )
}

/// The `name=value` part of a `Set-Cookie` header.
fn cookie_of(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[test]
fn logging_in_starts_a_session_named_in_a_safe_cookie() {
    let root = hmi_project("login");
    let running = start_with_access(
        &root,
        WebAuthMode::Local,
        None,
        access(Duration::from_secs(900)),
    );
    let (status, set_cookie, body) = login(&running.base, "anna", "secret");
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        (&body["user"], &body["role"]),
        (&json!("anna"), &json!("operator")),
        "{body}"
    );
    assert_eq!(body["idle_seconds"], json!(900), "{body}");
    for part in [
        "trust_hmi_session=",
        "HttpOnly",
        "SameSite=Strict",
        "Path=/",
    ] {
        assert!(set_cookie.contains(part), "{part} in {set_cookie}");
    }
    let cookie = cookie_of(&set_cookie);
    let (status, _, session) = call(
        "GET",
        &running.base,
        "/api/hmi/session",
        Some(&cookie),
        None,
    );
    assert_eq!(
        (status, &session["user"], &session["role"]),
        (200, &json!("anna"), &json!("operator")),
        "{session}"
    );
    // No session is an answer, not an error, so a logged-out browser logs nothing.
    let (status, _, none) = call("GET", &running.base, "/api/hmi/session", None, None);
    assert_eq!((status, &none["session"]), (200, &Value::Null), "{none}");
}

#[test]
fn a_wrong_password_is_refused_and_five_refuse_the_user_for_a_minute() {
    let root = hmi_project("login-refused");
    let running = start_with_access(
        &root,
        WebAuthMode::Local,
        None,
        access(Duration::from_secs(900)),
    );
    let (status, set_cookie, body) = login(&running.base, "erik", "wrong");
    assert_eq!(status, 401, "{body}");
    assert!(set_cookie.is_empty(), "no session: {set_cookie}");
    assert_eq!(body["error"], json!("unknown user or wrong password"));
    assert_eq!(
        login(&running.base, "nobody", "secret").2["error"],
        json!("unknown user or wrong password")
    );
    for _ in 0..4 {
        login(&running.base, "erik", "wrong");
    }
    let (status, _, locked) = login(&running.base, "erik", "secret");
    assert_eq!(status, 429, "{locked}");
    assert!(
        locked["error"]
            .as_str()
            .is_some_and(|error| error.contains("try again")),
        "{locked}"
    );
}

#[test]
fn logging_out_ends_the_session() {
    let root = hmi_project("logout");
    let running = start_with_access(
        &root,
        WebAuthMode::Local,
        None,
        access(Duration::from_secs(900)),
    );
    let cookie = cookie_of(&login(&running.base, "anna", "secret").1);
    let (status, set_cookie, _) = call(
        "POST",
        &running.base,
        "/api/hmi/logout",
        Some(&cookie),
        None,
    );
    assert_eq!(status, 200);
    assert!(set_cookie.contains("Max-Age=0"), "{set_cookie}");
    let (status, _, after) = call(
        "GET",
        &running.base,
        "/api/hmi/session",
        Some(&cookie),
        None,
    );
    assert_eq!((status, &after["session"]), (200, &Value::Null), "{after}");
}

#[test]
fn logging_in_needs_the_viewer_role_like_the_page_data() {
    let root = hmi_project("login-token");
    let running = start_with_access(
        &root,
        WebAuthMode::Token,
        Some("secret-token"),
        access(Duration::from_secs(900)),
    );
    assert_eq!(login(&running.base, "anna", "secret").0, 401);
}

#[test]
fn the_value_stream_reports_the_session_and_its_end() {
    let root = hmi_project("stream-session");
    let running = start_with_access(
        &root,
        WebAuthMode::Local,
        None,
        access(Duration::from_secs(1)),
    );
    let cookie = cookie_of(&login(&running.base, "anna", "secret").1);
    let address = running.base.trim_start_matches("http://").to_string();
    let stream = TcpStream::connect(&address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("timeout");
    let request = tungstenite::http::Request::builder()
        .uri(format!("ws://{address}/ws/hmi?page=overview"))
        .header("Host", &address)
        .header("Cookie", &cookie)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tungstenite::handshake::client::generate_key(),
        )
        .body(())
        .expect("a request");
    let (mut socket, _) = tungstenite::client(request, stream).expect("the stream opens");
    let mut sessions = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    while std::time::Instant::now() < deadline && sessions.len() < 2 {
        if let Ok(Message::Text(text)) = socket.read() {
            let message: Value = serde_json::from_str(text.as_str()).expect("JSON");
            if let Some(session) = message.get("session") {
                sessions.push(session.clone());
            }
        }
    }
    assert_eq!(
        sessions.first(),
        Some(&json!({ "user": "anna", "role": "operator" })),
        "{sessions:?}"
    );
    assert_eq!(
        sessions.get(1),
        Some(&Value::Null),
        "the end, after 1 s without activity: {sessions:?}"
    );
}
