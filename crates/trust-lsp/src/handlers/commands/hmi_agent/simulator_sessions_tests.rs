//! Tests of the named simulator sessions (specification 43, AT-08: at most five unexpired per
//! workspace, ten minutes each, opaque IDs, one journey at a time; specification 39 §11 item 6: a
//! target change invalidates the session). The clock is passed in, so each time is exact.

use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::super::error::{CANCELLED, CONFLICT, EXPIRED_DRAFT, LIMIT_EXCEEDED, NOT_CONNECTED};
use super::{iso_time, SimulatorSessions, MAX_SESSIONS};

const A: &str = "file:///projects/a";
/// The journey ID the client chose for the journeys here.
const J: &str = "journey-1";
const B: &str = "file:///projects/b";

/// `seconds` after 2026-11-04T10:00:00Z, which is 1 793 786 400 seconds after the epoch.
fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_793_786_400 + seconds)
}

/// A local simulator with its access token.
fn local() -> Value {
    json!({
        "simulator": true, "url": "http://127.0.0.1:8080", "endpoint": "tcp://127.0.0.1:9100",
        "token": "access-token-7", "label": "Fixture",
    })
}

fn open(sessions: &SimulatorSessions, workspace: &str, now: SystemTime) -> String {
    let view = sessions
        .open(workspace, "Fixture", &local(), now)
        .expect("a session");
    view["sessionId"].as_str().expect("an ID").to_owned()
}

/// Catches: a session that outlives its ten minutes, or ends before them.
#[test]
fn a_session_lasts_ten_minutes() {
    let sessions = SimulatorSessions::new();
    let view = sessions
        .open(A, "Fixture", &local(), at(0))
        .expect("a session");
    let id = view["sessionId"].as_str().expect("an ID");
    assert_eq!(
        view,
        json!({ "sessionId": id, "name": "Fixture", "expiresAt": "2026-11-04T10:10:00.000Z" })
    );
    let listed = sessions.list(A, at(600));
    assert_eq!(listed, std::slice::from_ref(&view), "its last moment");
    let after = at(600) + Duration::from_millis(1);
    assert!(sessions.list(A, after).is_empty());
    assert!(!sessions.is_live(id, after));
    let refused = sessions
        .begin(A, id, J, &local(), after)
        .err()
        .expect("expired");
    assert_eq!(refused.code, EXPIRED_DRAFT);
}

/// Catches: a limit that counts other workspaces' or expired sessions, or that is not enforced.
#[test]
fn a_workspace_holds_at_most_five_unexpired_sessions() {
    let sessions = SimulatorSessions::new();
    for minute in 0..5 {
        open(&sessions, A, at(minute * 60));
    }
    let refused = sessions
        .open(A, "Sixth", &local(), at(250))
        .expect_err("the limit");
    assert_eq!(refused.code, LIMIT_EXCEEDED);
    assert_eq!(sessions.list(A, at(250)).len(), MAX_SESSIONS);
    open(&sessions, B, at(250));
    // At 10:10:01 the first of A's sessions has expired: A holds four.
    open(&sessions, A, at(601));
    let refused = sessions
        .open(A, "Seventh", &local(), at(601))
        .expect_err("the limit");
    assert_eq!(refused.code, LIMIT_EXCEEDED);
}

/// Catches: a session opened for a physical or remote runtime, which a journey would then command.
#[test]
fn a_session_opens_only_for_a_local_simulator() {
    let sessions = SimulatorSessions::new();
    let mut physical = local();
    physical["simulator"] = json!(false);
    let mut remote = local();
    remote["url"] = json!("http://192.168.1.2:8080");
    for target in [physical, remote, Value::Null] {
        let refused = sessions
            .open(A, "Fixture", &target, at(0))
            .expect_err("refused");
        assert_eq!(refused.code, NOT_CONNECTED, "{target}");
    }
    assert!(sessions.list(A, at(0)).is_empty());
}

/// Catches: a session that runs two journeys at once, or stays busy after its journey ended.
#[test]
fn a_session_runs_one_journey_at_a_time() {
    let sessions = SimulatorSessions::new();
    let id = open(&sessions, A, at(0));
    let running = sessions
        .begin(A, &id, J, &local(), at(1))
        .expect("a journey");
    assert_eq!(running.name(), "Fixture");
    let refused = sessions
        .begin(A, &id, J, &local(), at(2))
        .err()
        .expect("busy");
    assert_eq!(refused.code, CONFLICT);
    assert_eq!(
        refused.message,
        "this simulator journey is already running."
    );
    drop(running);
    assert!(sessions.begin(A, &id, J, &local(), at(3)).is_ok());
}

/// Catches: a journey that sends the named session's cookie to a target other than the one it was
/// connected to (another web address, control endpoint or access token, or no target), and a session
/// that survives such a change.
#[test]
fn a_journey_with_another_target_is_refused_and_closes_the_session() {
    let changes = [
        ("url", json!("http://localhost:8080")),
        ("endpoint", json!("tcp://127.0.0.1:9101")),
        ("token", json!("other-token")),
    ];
    let targets = changes
        .into_iter()
        .map(|(key, value)| {
            let mut target = local();
            target[key] = value;
            target
        })
        .chain([Value::Null]);
    for target in targets {
        let sessions = SimulatorSessions::new();
        let id = open(&sessions, A, at(0));
        let refused = sessions
            .begin(A, &id, J, &target, at(1))
            .err()
            .expect("refused");
        assert_eq!(refused.code, CONFLICT, "{target}");
        assert!(
            sessions.list(A, at(1)).is_empty(),
            "{target}: the session is closed"
        );
        let again = sessions
            .begin(A, &id, J, &local(), at(2))
            .err()
            .expect("closed");
        assert_eq!(again.code, EXPIRED_DRAFT, "{target}");
    }
}

/// Catches: a workspace running a journey on another workspace's session.
#[test]
fn another_workspaces_session_is_refused() {
    let sessions = SimulatorSessions::new();
    let id = open(&sessions, A, at(0));
    let refused = sessions
        .begin(B, &id, J, &local(), at(1))
        .err()
        .expect("refused");
    assert_eq!(refused.code, EXPIRED_DRAFT);
    assert_eq!(sessions.list(A, at(1)).len(), 1, "the session stays A's");
    assert!(sessions.list(B, at(1)).is_empty());
    assert!(sessions.begin(A, &id, J, &local(), at(2)).is_ok());
}

/// Catches: a cancellation that does not reach the running journey, or that claims to cancel when
/// none runs.
#[test]
fn cancel_reaches_only_a_running_journey() {
    let sessions = SimulatorSessions::new();
    let id = open(&sessions, A, at(0));
    assert!(!sessions.cancel(&id, J, at(0)));
    let running = sessions
        .begin(A, &id, J, &local(), at(3))
        .expect("a journey");
    assert!(!running.cancellation().load(Ordering::SeqCst));
    assert!(sessions.cancel(&id, J, at(4)));
    assert!(running.cancellation().load(Ordering::SeqCst));
    drop(running);
    assert!(!sessions.cancel(&id, J, at(5)));
    assert!(!sessions.cancel("no-such-session", J, at(5)));
}

/// Catches the race of a cancel that reaches the language server just before the journey it was sent
/// for: the journey would run although the agent cancelled it. Such a cancel refuses that journey's
/// start for two seconds, and only for two seconds.
#[test]
fn a_cancel_just_before_the_start_refuses_it() {
    let sessions = SimulatorSessions::new();
    let id = open(&sessions, A, at(0));
    assert!(!sessions.cancel(&id, J, at(10)), "no journey ran");
    let refused = sessions
        .begin(A, &id, J, &local(), at(12))
        .err()
        .expect("cancelled");
    assert_eq!(refused.code, CANCELLED);
    assert!(
        sessions.begin(A, &id, J, &local(), at(12)).is_ok(),
        "the cancel counted once"
    );
    let late = open(&sessions, A, at(20));
    assert!(!sessions.cancel(&late, J, at(20)));
    let later = UNIX_EPOCH + Duration::from_millis(1_793_786_422_001);
    assert!(
        sessions.begin(A, &late, J, &local(), later).is_ok(),
        "2.001 s on, it is forgotten"
    );
}

/// Catches: a cancel that stops whatever journey the session runs, so that a cancel the client sent
/// for an earlier journey stops the next one; a cancel names the journey it cancels.
#[test]
fn a_cancel_names_its_journey() {
    let sessions = SimulatorSessions::new();
    let id = open(&sessions, A, at(0));
    let running = sessions
        .begin(A, &id, J, &local(), at(1))
        .expect("a journey");
    assert!(!sessions.cancel(&id, "journey-0", at(2)), "another journey");
    assert!(!running.cancellation().load(Ordering::SeqCst));
    assert!(sessions.cancel(&id, J, at(2)));
    assert!(running.cancellation().load(Ordering::SeqCst));
}

/// Catches: an early cancel that refuses the session's next journey whatever it is, and a stale
/// cancel of a journey that already ended refusing the one after it; an early cancel refuses only
/// the journey it names.
#[test]
fn an_early_or_stale_cancel_refuses_only_the_journey_it_names() {
    let sessions = SimulatorSessions::new();
    let id = open(&sessions, A, at(0));
    assert!(
        !sessions.cancel(&id, "journey-2", at(10)),
        "not yet running"
    );
    let first = sessions
        .begin(A, &id, J, &local(), at(11))
        .expect("another journey runs");
    drop(first);
    let refused = sessions
        .begin(A, &id, "journey-2", &local(), at(11))
        .err()
        .expect("the named journey");
    assert_eq!(refused.code, CANCELLED);
    let ended = sessions
        .begin(A, &id, "journey-3", &local(), at(12))
        .expect("a journey");
    drop(ended);
    assert!(!sessions.cancel(&id, "journey-3", at(12)), "it ended");
    assert!(
        sessions
            .begin(A, &id, "journey-4", &local(), at(12))
            .is_ok(),
        "the stale cancel affects no other journey"
    );
}

/// Catches: a closed session that is still listed or still runs journeys, or whose running journey
/// goes on: closing it cancels that journey.
#[test]
fn close_forgets_the_session_and_cancels_its_journey() {
    let sessions = SimulatorSessions::new();
    let id = open(&sessions, A, at(0));
    let running = sessions
        .begin(A, &id, J, &local(), at(1))
        .expect("a journey");
    assert!(sessions.close(&id));
    assert!(
        running.cancellation().load(Ordering::SeqCst),
        "its journey is cancelled"
    );
    assert!(sessions.list(A, at(1)).is_empty());
    assert!(!sessions.is_live(&id, at(1)));
    assert!(!sessions.close(&id));
}

/// Catches: a running journey that goes on after another journey request named another target,
/// which closes the session (specification 39 §11 item 6).
#[test]
fn a_target_change_cancels_the_running_journey() {
    let sessions = SimulatorSessions::new();
    let id = open(&sessions, A, at(0));
    let running = sessions
        .begin(A, &id, J, &local(), at(1))
        .expect("a journey");
    let mut other = local();
    other["token"] = json!("other-token");
    let refused = sessions
        .begin(A, &id, J, &other, at(2))
        .err()
        .expect("refused");
    assert_eq!(refused.code, CONFLICT);
    assert!(running.cancellation().load(Ordering::SeqCst));
    assert!(!sessions.is_live(&id, at(2)));
}

/// Catches: a simulator login sent to a target other than the session's own (another web address,
/// control endpoint or access token), to a target that is not a local simulator, or for a session that
/// is closed or expired.
#[test]
fn a_login_is_admitted_only_to_the_sessions_own_local_target() {
    let sessions = SimulatorSessions::new();
    let id = open(&sessions, A, at(0));
    assert!(sessions.admits(&id, &local(), at(1)));
    for (key, value) in [
        ("url", json!("http://localhost:8080")),
        ("endpoint", json!("tcp://127.0.0.1:9101")),
        ("token", json!("other-token")),
        ("simulator", json!(false)),
    ] {
        let mut target = local();
        target[key] = value;
        assert!(!sessions.admits(&id, &target, at(1)), "{key}");
    }
    assert!(!sessions.admits(&id, &local(), at(601)), "expired");
    assert!(!sessions.admits("no-such-session", &local(), at(1)));
}

/// Catches: session IDs that repeat or carry meaning instead of being opaque random identifiers.
#[test]
fn session_ids_are_opaque_random_uuids() {
    let sessions = SimulatorSessions::new();
    let first = open(&sessions, A, at(0));
    let second = open(&sessions, A, at(0));
    assert_ne!(first, second);
    for id in [&first, &second] {
        assert_eq!(id.len(), 36, "{id}");
        assert_eq!(id.as_bytes()[14], b'4', "{id}: version 4");
    }
}

/// Catches: an expiry written in another form than JavaScript's `toISOString`, which the context
/// listed before (hand-computed: 2026-11-04 is day 20 761 after the epoch, 2024-02-29 day 19 782).
#[test]
fn an_expiry_is_written_as_javascripts_iso_time() {
    assert_eq!(iso_time(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
    assert_eq!(
        iso_time(UNIX_EPOCH + Duration::from_secs(1_793_786_400)),
        "2026-11-04T10:00:00.000Z"
    );
    assert_eq!(
        iso_time(UNIX_EPOCH + Duration::from_millis(1_709_251_199_999)),
        "2024-02-29T23:59:59.999Z"
    );
}
