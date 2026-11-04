//! Tests of the simulator commands (specification 43, AT-08; specification 39 §11 item 6): their
//! envelope, the journey's workspace, steps, session and cookie checks before any request, and a
//! whole journey whose cookie and access token appear in no answer and no log line.

use std::collections::BTreeMap;
use std::io;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use super::super::simulator_fake::{CommandAnswer, FakeSimulator, Frame, Script};
use super::super::simulator_journey::Timing;
use super::super::simulator_sessions::SimulatorSessions;
use super::{answer, CANCEL_JOURNEY, CLOSE_SESSION, LIST_SESSIONS, OPEN_SESSION, RUN_JOURNEY};

const A: &str = "file:///projects/a";
const B: &str = "file:///projects/b";
const COOKIE: &str = "trust_hmi_session=4f1c0ffee";
const PROMPT: Timing = Timing {
    wait: Duration::from_secs(5),
    poll: Duration::from_millis(10),
    request: Duration::from_secs(5),
    stream: Duration::from_secs(5),
};

/// `seconds` after 2026-11-04T10:00:00Z.
fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_793_786_400 + seconds)
}

fn call(sessions: &SimulatorSessions, name: &str, argument: Value, now: SystemTime) -> Value {
    answer(sessions, name, vec![argument], &move || now, PROMPT)
}

fn target(url: &str, token: &str) -> Value {
    json!({ "simulator": true, "url": url, "endpoint": "tcp://127.0.0.1:9100", "token": token })
}

/// The page every journey here exercises.
fn page() -> Value {
    json!({ "title": "Settings", "object": [{ "id": "sp", "type": "setpoint", "at": [32, 88],
            "size": [200, 64] }], "pipe": [] })
}

/// Opens a session of `workspace` for `target`: its ID.
fn opened(
    sessions: &SimulatorSessions,
    workspace: &str,
    target: &Value,
    now: SystemTime,
) -> String {
    let open = json!({ "workspace": workspace, "name": "Fixture", "target": target });
    let answer = call(sessions, OPEN_SESSION, open, now);
    answer["data"]["sessionId"]
        .as_str()
        .expect("a session")
        .to_owned()
}

/// The argument of a journey of `steps` on the session `id` with the cookie `cookie`.
fn journey(workspace: &str, id: &str, cookie: Value, target: &Value, steps: Value) -> Value {
    json!({ "workspace": workspace, "sessionId": id, "cookie": cookie, "target": target,
            "steps": steps, "models": { "settings": page() } })
}

fn refusal(code: &str, message: &str) -> Value {
    json!({ "ok": false, "code": code, "message": message, "retryable": false })
}

/// Catches: an answer form the extension cannot unwrap, or a refusal that is not the shared envelope.
#[test]
fn the_commands_answer_the_authoring_envelope() {
    let sessions = SimulatorSessions::new();
    let local = target("http://127.0.0.1:8080", "access-token-7");
    let open = json!({ "workspace": A, "name": "Fixture", "target": local });
    let opened = call(&sessions, OPEN_SESSION, open, at(0));
    let id = opened["data"]["sessionId"]
        .as_str()
        .expect("a session")
        .to_owned();
    let view =
        json!({ "sessionId": id, "name": "Fixture", "expiresAt": "2026-11-04T10:10:00.000Z" });
    let success =
        |data: Value| json!({ "ok": true, "data": data, "revisions": {}, "warnings": [] });
    assert_eq!(opened, success(view.clone()));
    let listed = call(&sessions, LIST_SESSIONS, json!({ "workspace": A }), at(1));
    assert_eq!(listed, success(json!({ "sessions": [view] })));
    let cancelled = call(&sessions, CANCEL_JOURNEY, json!({ "sessionId": id }), at(1));
    assert_eq!(cancelled, success(json!({ "cancelled": false })));
    let closed = call(&sessions, CLOSE_SESSION, json!({ "sessionId": id }), at(1));
    assert_eq!(closed, success(json!({ "closed": true })));

    let blank = json!({ "workspace": A, "name": " ", "target": local });
    assert_eq!(
        call(&sessions, OPEN_SESSION, blank, at(2)),
        refusal("INVALID_ARGUMENT", "name is required.")
    );
    let two = answer(
        &sessions,
        LIST_SESSIONS,
        vec![json!({}), json!({})],
        &|| at(2),
        PROMPT,
    );
    assert_eq!(
        two,
        refusal(
            "INVALID_ARGUMENT",
            "the simulator command expects one argument object."
        )
    );
    let unknown = call(
        &sessions,
        LIST_SESSIONS,
        json!({ "workspace": A, "extra": 1 }),
        at(2),
    );
    assert_eq!(unknown["code"], "INVALID_ARGUMENT");
}

/// Catches: a journey that does not name its workspace, or that runs on another workspace's session.
#[test]
fn a_journey_names_its_workspace_and_refuses_another_workspaces_session() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = SimulatorSessions::new();
    let local = target(&fake.url, "access-token-7");
    let id = opened(&sessions, A, &local, at(0));
    let steps = json!([{ "page": "settings" }]);
    let other = journey(B, &id, json!(COOKIE), &local, steps.clone());
    assert_eq!(
        call(&sessions, RUN_JOURNEY, other, at(1)),
        refusal(
            "EXPIRED_DRAFT",
            "select an unexpired named simulator session in this workspace."
        )
    );
    let mut nameless = journey(A, &id, json!(COOKIE), &local, steps);
    nameless
        .as_object_mut()
        .expect("an object")
        .remove("workspace");
    assert_eq!(
        call(&sessions, RUN_JOURNEY, nameless, at(1))["code"],
        "INVALID_ARGUMENT"
    );
    assert!(fake.received().is_empty());
}

/// Catches: a journey of no steps or of more than twelve, which the specification bounds, reaching
/// the simulator.
#[test]
fn more_than_twelve_steps_are_refused_before_any_request() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = SimulatorSessions::new();
    let local = target(&fake.url, "access-token-7");
    let id = opened(&sessions, A, &local, at(0));
    for count in [0, 13] {
        let steps = Value::Array(vec![json!({ "page": "settings" }); count]);
        let argument = journey(A, &id, json!(COOKIE), &local, steps);
        assert_eq!(
            call(&sessions, RUN_JOURNEY, argument, at(1)),
            refusal("LIMIT_EXCEEDED", "use 1 to 12 bounded simulator steps."),
            "{count} steps"
        );
    }
    assert!(fake.received().is_empty());
}

/// Catches: a journey on an expired session, or a second journey on a session that runs one,
/// reaching the simulator.
#[test]
fn an_expired_or_busy_session_is_refused_before_any_request() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = SimulatorSessions::new();
    let local = target(&fake.url, "access-token-7");
    let id = opened(&sessions, A, &local, at(0));
    let steps = json!([{ "page": "settings" }]);
    let late = journey(A, &id, json!(COOKIE), &local, steps.clone());
    let expired = call(&sessions, RUN_JOURNEY, late, at(601));
    assert_eq!(
        expired,
        refusal(
            "EXPIRED_DRAFT",
            "select an unexpired named simulator session in this workspace."
        )
    );
    let busy_id = opened(&sessions, A, &local, at(700));
    let running = sessions
        .begin(A, &busy_id, &local, at(701))
        .expect("a journey");
    let second = journey(A, &busy_id, json!(COOKIE), &local, steps);
    assert_eq!(
        call(&sessions, RUN_JOURNEY, second, at(702)),
        refusal("CONFLICT", "this simulator journey is already running.")
    );
    drop(running);
    assert!(fake.received().is_empty());
}

/// Catches: a journey sent to a target other than its session's, and a session that survives the
/// change (specification 39 §11 item 6: a target change invalidates the session).
#[test]
fn a_journey_with_another_target_closes_its_session() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = SimulatorSessions::new();
    let local = target(&fake.url, "access-token-7");
    let id = opened(&sessions, A, &local, at(0));
    let repaired = target(&fake.url, "access-token-8");
    let argument = journey(
        A,
        &id,
        json!(COOKIE),
        &repaired,
        json!([{ "page": "settings" }]),
    );
    assert_eq!(
        call(&sessions, RUN_JOURNEY, argument, at(1))["code"],
        "CONFLICT"
    );
    let listed = call(&sessions, LIST_SESSIONS, json!({ "workspace": A }), at(2));
    assert_eq!(listed["data"]["sessions"], json!([]));
    assert!(fake.received().is_empty());
}

/// Catches: a journey run without its named session's cookie or with one that would add headers of
/// its own, a refusal that quotes the cookie, and a session left busy by the refusal.
#[test]
fn a_journey_needs_its_sessions_cookie_and_never_quotes_it() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = SimulatorSessions::new();
    let local = target(&fake.url, "access-token-7");
    let id = opened(&sessions, A, &local, at(0));
    let steps = json!([{ "page": "settings" }]);
    let needs = refusal(
        "INVALID_ARGUMENT",
        "the journey needs the cookie of its named session; connect the simulator journey again.",
    );
    let cookies = [
        Value::Null,
        json!(""),
        json!("trust_hmi_session="),
        json!("other_session=5ecret-a"),
        json!("trust_hmi_session=5ecret;b"),
        json!("trust_hmi_session=5ecret-c\r\nX-Injected: 1"),
        json!(5),
    ];
    for cookie in cookies {
        let argument = journey(A, &id, cookie.clone(), &local, steps.clone());
        assert_eq!(
            call(&sessions, RUN_JOURNEY, argument, at(1)),
            needs,
            "{cookie}"
        );
    }
    let mut missing = journey(A, &id, Value::Null, &local, steps.clone());
    missing.as_object_mut().expect("an object").remove("cookie");
    assert_eq!(call(&sessions, RUN_JOURNEY, missing, at(1)), needs);
    let mut malformed = journey(A, &id, json!("trust_hmi_session=5ecret-d"), &local, steps);
    malformed["steps"] = json!("not steps");
    let refused = call(&sessions, RUN_JOURNEY, malformed, at(1));
    assert_eq!(refused["code"], "INVALID_ARGUMENT");
    assert!(!refused.to_string().contains("5ecret"), "{refused}");
    assert!(fake.received().is_empty());
    assert!(
        sessions.begin(A, &id, &local, at(2)).is_ok(),
        "no refusal left the session busy"
    );
}

/// A writer that keeps what the log subscriber writes.
#[derive(Clone)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut kept = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        kept.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Catches: the named session's cookie or the target's access token written into the journey's
/// answer or into any log line of the journey, the language server's own or a library's (the
/// language server forwards the `log` records of tungstenite to its subscriber, as here).
#[test]
fn the_cookie_and_token_never_appear_in_the_answer_or_the_log() {
    const SECRET_COOKIE: &str = "c00kie-5ecret-91";
    const SECRET_TOKEN: &str = "t0ken-5ecret-37";
    let kept = Arc::new(Mutex::new(Vec::new()));
    let journey_thread = thread::current().id();
    let writer = Captured(Arc::clone(&kept));
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish()
        .with(filter_fn(move |_| thread::current().id() == journey_thread))
        .try_init()
        .expect("this test binary's only global log subscriber");

    let fake = FakeSimulator::start(Script {
        on_open: vec![Frame::Text(
            json!({ "values": { "Main.SP": { "value": 10.0 } } }),
        )],
        on_command: Box::new(|command| CommandAnswer {
            status: 200,
            body: json!({ "id": command["id"], "outcome": "applied" }),
            then: vec![Frame::Text(
                json!({ "command": { "id": command["id"], "outcome": "confirmed" } }),
            )],
        }),
        ..Script::serving(BTreeMap::from([("settings".to_owned(), page())]))
    });
    let sessions = SimulatorSessions::new();
    let local = target(&fake.url, SECRET_TOKEN);
    let id = opened(&sessions, A, &local, at(0));
    let cookie = json!(format!("trust_hmi_session={SECRET_COOKIE}"));
    let steps = json!([{ "page": "settings", "object": "sp", "action": "set", "value": 10,
                         "path": "Main.SP", "expect": 10 }]);
    let answer = call(
        &sessions,
        RUN_JOURNEY,
        journey(A, &id, cookie, &local, steps),
        at(1),
    );

    assert_eq!(answer["data"]["passed"], true, "the journey ran: {answer}");
    for request in fake.received() {
        let sent = request
            .cookie
            .as_deref()
            .is_some_and(|cookie| cookie.contains(SECRET_COOKIE));
        assert!(sent, "{request:?} carried the cookie");
        assert_eq!(request.token.as_deref(), Some(SECRET_TOKEN), "{request:?}");
    }
    let log =
        String::from_utf8_lossy(&kept.lock().unwrap_or_else(PoisonError::into_inner)).into_owned();
    assert!(
        log.contains("HMI agent simulator journey ended"),
        "our events reached the capture"
    );
    assert!(
        log.contains("Received message"),
        "tungstenite's trace reached the capture: {log}"
    );
    for secret in [SECRET_COOKIE, SECRET_TOKEN] {
        assert!(!log.contains(secret), "the log names {secret}: {log}");
        assert!(
            !answer.to_string().contains(secret),
            "the answer names {secret}: {answer}"
        );
    }
}
