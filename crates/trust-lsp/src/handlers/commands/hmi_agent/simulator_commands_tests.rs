//! Tests of the simulator commands (specification 43, AT-08; specification 39 §11 item 6): their
//! envelope, the journey's workspace, steps, session and cookie checks before any request, a
//! journey that starts at once on a thread of its own and reports its answer when it ends, so that
//! journeys waiting on a simulator never keep other commands from answering, and a journey whose
//! draft is refused ending before its first step. That no log line names the cookie or the access
//! token is proven against the language server's binary in
//! `crates/trust-lsp/tests/simulator_journey_transport.rs`.

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::state::ServerState;

use super::super::simulator_fake::{CommandAnswer, FakeSimulator, Frame, Script, STREAM_PERIOD};
use super::super::simulator_journey::Timing;
use super::super::simulator_project::{settings_model, JourneyProject};
use super::super::simulator_sessions::SimulatorSessions;
use super::{
    answer, envelope, start, Clock, ADMIT_LOGIN, CANCEL_JOURNEY, CLOSE_SESSION, LIST_SESSIONS,
    OPEN_SESSION, RUN_JOURNEY,
};

const A: &str = "file:///projects/a";
const COOKIE: &str = "trust_hmi_session=4f1c0ffee";
const PROMPT: Timing = Timing {
    wait: Duration::from_secs(5),
    poll: Duration::from_millis(10),
    request: Duration::from_secs(5),
    stream: Duration::from_secs(5),
    period: Duration::from_millis(100),
    cap: Duration::from_secs(10),
};
/// The journey ID the client chose for the journeys here.
const J: &str = "journey-1";
/// The draft of the journeys that are refused before their thread reads a draft.
const NO_DRAFT: &str = "00000000-0000-4000-8000-000000000000";

/// `seconds` after 2026-11-04T10:00:00Z.
fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_793_786_400 + seconds)
}

fn clock(now: SystemTime) -> Clock {
    Arc::new(move || now)
}

/// Sessions that live as long as the journey threads that use them.
fn sessions() -> &'static SimulatorSessions {
    Box::leak(Box::new(SimulatorSessions::new()))
}

fn call(sessions: &SimulatorSessions, name: &str, argument: Value, now: SystemTime) -> Value {
    answer(sessions, name, vec![argument], &clock(now))
}

/// Starts a journey: the start's envelope, and where its answer arrives when it ends.
fn run(
    sessions: &'static SimulatorSessions,
    state: &Arc<ServerState>,
    argument: Value,
    now: SystemTime,
    timing: Timing,
) -> (Value, Receiver<Value>) {
    let (sender, receiver) = mpsc::channel();
    let report = Box::new(move |answer| {
        let _ = sender.send(answer);
    });
    let started = start(
        sessions,
        Arc::clone(state),
        vec![argument],
        clock(now),
        timing,
        report,
    );
    (envelope(started), receiver)
}

/// The start's envelope of a journey that is refused before it starts.
fn refused(sessions: &'static SimulatorSessions, argument: Value, now: SystemTime) -> Value {
    let state = Arc::new(ServerState::new());
    run(sessions, &state, argument, now, PROMPT).0
}

fn target(url: &str, token: &str) -> Value {
    json!({ "simulator": true, "url": url, "endpoint": "tcp://127.0.0.1:9100", "authToken": token })
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

/// The argument of the journey `J` of `steps` on the session `id` of `project`, with the cookie
/// `cookie`, on revision 1 of `draft`.
fn journey(
    project: &JourneyProject,
    draft: &str,
    id: &str,
    cookie: Value,
    target: &Value,
    steps: Value,
) -> Value {
    let mut argument = project.fields();
    for (field, value) in [
        ("sessionId", json!(id)),
        ("journeyId", json!(J)),
        ("cookie", cookie),
        ("target", target.clone()),
        ("steps", steps),
        ("draftId", json!(draft)),
        ("revision", json!("1")),
    ] {
        argument[field] = value;
    }
    argument
}

/// The deployed pages of a simulator: the settings page.
fn deployed(state: &ServerState) -> BTreeMap<String, Value> {
    BTreeMap::from([("settings".to_owned(), settings_model(state))])
}

fn refusal(code: &str, message: &str) -> Value {
    json!({ "ok": false, "code": code, "message": message, "retryable": false })
}

fn success(data: Value) -> Value {
    json!({ "ok": true, "data": data, "revisions": {}, "warnings": [] })
}

/// Catches: an answer form the extension cannot unwrap, or a refusal that is not the shared envelope;
/// and a journey run on the path without the language client, which could not report its end.
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
    assert_eq!(opened, success(view.clone()));
    let listed = call(&sessions, LIST_SESSIONS, json!({ "workspace": A }), at(1));
    assert_eq!(listed, success(json!({ "sessions": [view] })));
    let admit = json!({ "sessionId": id, "target": local });
    let admitted = call(&sessions, ADMIT_LOGIN, admit, at(1));
    assert_eq!(admitted, success(json!({ "admitted": true })));
    let cancel = json!({ "sessionId": id, "journeyId": J });
    let cancelled = call(&sessions, CANCEL_JOURNEY, cancel, at(1));
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
        &clock(at(2)),
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
    let without_client = call(&sessions, RUN_JOURNEY, json!({}), at(2));
    assert_eq!(without_client["code"], "INVALID_ARGUMENT");
}

/// Catches: a journey that does not name its project folder, the workspace of its session, or that
/// runs on another workspace's session.
#[test]
fn a_journey_names_its_workspace_and_refuses_another_workspaces_session() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = sessions();
    let (a, b) = (JourneyProject::new("a"), JourneyProject::new("b"));
    let local = target(&fake.url, "access-token-7");
    let id = opened(sessions, &a.root_uri(), &local, at(0));
    let steps = json!([{ "page": "settings" }]);
    let other = journey(&b, NO_DRAFT, &id, json!(COOKIE), &local, steps.clone());
    assert_eq!(
        refused(sessions, other, at(1)),
        refusal(
            "EXPIRED_DRAFT",
            "select an unexpired named simulator session in this workspace."
        )
    );
    let mut nameless = journey(&a, NO_DRAFT, &id, json!(COOKIE), &local, steps);
    nameless
        .as_object_mut()
        .expect("an object")
        .remove("root_uri");
    assert_eq!(
        refused(sessions, nameless, at(1)),
        refusal("INVALID_ARGUMENT", "root_uri is required.")
    );
    assert!(fake.received().is_empty());
}

/// Catches: a journey of no steps or of more than twelve, which the specification bounds, reaching
/// the simulator.
#[test]
fn more_than_twelve_steps_are_refused_before_any_request() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = sessions();
    let project = JourneyProject::new("twelve");
    let local = target(&fake.url, "access-token-7");
    let id = opened(sessions, &project.root_uri(), &local, at(0));
    for count in [0, 13] {
        let steps = Value::Array(vec![json!({ "page": "settings" }); count]);
        let argument = journey(&project, NO_DRAFT, &id, json!(COOKIE), &local, steps);
        assert_eq!(
            refused(sessions, argument, at(1)),
            refusal("LIMIT_EXCEEDED", "use 1 to 12 bounded simulator steps."),
            "{count} steps"
        );
    }
    assert!(fake.received().is_empty());
}

/// Catches: a method command (`invoke`) taken for a journey step, which the runtime runs as a
/// capability request rather than a control's command.
#[test]
fn a_method_command_is_not_a_journey_step() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = sessions();
    let project = JourneyProject::new("invoke");
    let local = target(&fake.url, "access-token-7");
    let id = opened(sessions, &project.root_uri(), &local, at(0));
    let steps = json!([{ "page": "settings", "object": "sp", "action": "invoke" }]);
    let argument = journey(&project, NO_DRAFT, &id, json!(COOKIE), &local, steps);
    assert_eq!(
        refused(sessions, argument, at(1)),
        refusal(
            "INVALID_ARGUMENT",
            "a method command (`invoke`) is not a simulator journey step; a step's command writes, \
             sets, resets, toggles or pulses."
        )
    );
    assert!(fake.received().is_empty());
}

/// Catches: a journey on an expired session, or a second journey on a session that runs one,
/// reaching the simulator.
#[test]
fn an_expired_or_busy_session_is_refused_before_any_request() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = sessions();
    let project = JourneyProject::new("busy");
    let root = project.root_uri();
    let local = target(&fake.url, "access-token-7");
    let id = opened(sessions, &root, &local, at(0));
    let steps = json!([{ "page": "settings" }]);
    let late = journey(
        &project,
        NO_DRAFT,
        &id,
        json!(COOKIE),
        &local,
        steps.clone(),
    );
    assert_eq!(
        refused(sessions, late, at(601)),
        refusal(
            "EXPIRED_DRAFT",
            "select an unexpired named simulator session in this workspace."
        )
    );
    let busy_id = opened(sessions, &root, &local, at(700));
    let running = sessions
        .begin(&root, &busy_id, J, &local, at(701))
        .expect("a journey");
    let second = journey(&project, NO_DRAFT, &busy_id, json!(COOKIE), &local, steps);
    assert_eq!(
        refused(sessions, second, at(702)),
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
    let sessions = sessions();
    let project = JourneyProject::new("target");
    let root = project.root_uri();
    let local = target(&fake.url, "access-token-7");
    let id = opened(sessions, &root, &local, at(0));
    let repaired = target(&fake.url, "access-token-8");
    let argument = journey(
        &project,
        NO_DRAFT,
        &id,
        json!(COOKIE),
        &repaired,
        json!([{ "page": "settings" }]),
    );
    assert_eq!(refused(sessions, argument, at(1))["code"], "CONFLICT");
    let listed = call(sessions, LIST_SESSIONS, json!({ "workspace": root }), at(2));
    assert_eq!(listed["data"]["sessions"], json!([]));
    assert!(fake.received().is_empty());
}

/// Catches: a journey run without its named session's cookie or with one that would add headers of
/// its own, a refusal that quotes the cookie, and a session left busy by the refusal.
#[test]
fn a_journey_needs_its_sessions_cookie_and_never_quotes_it() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = sessions();
    let project = JourneyProject::new("cookie");
    let root = project.root_uri();
    let local = target(&fake.url, "access-token-7");
    let id = opened(sessions, &root, &local, at(0));
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
        let argument = journey(
            &project,
            NO_DRAFT,
            &id,
            cookie.clone(),
            &local,
            steps.clone(),
        );
        assert_eq!(refused(sessions, argument, at(1)), needs, "{cookie}");
    }
    let mut missing = journey(&project, NO_DRAFT, &id, Value::Null, &local, steps.clone());
    missing.as_object_mut().expect("an object").remove("cookie");
    assert_eq!(refused(sessions, missing, at(1)), needs);
    let secret = json!("trust_hmi_session=5ecret-d");
    let mut malformed = journey(&project, NO_DRAFT, &id, secret, &local, steps);
    malformed["steps"] = json!("not steps");
    let refusal = refused(sessions, malformed, at(1));
    assert_eq!(refusal["code"], "INVALID_ARGUMENT");
    assert!(!refusal.to_string().contains("5ecret"), "{refusal}");
    assert!(fake.received().is_empty());
    assert!(
        sessions.begin(&root, &id, J, &local, at(2)).is_ok(),
        "no refusal left the session busy"
    );
}

/// Catches: a journey that runs inside the request that starts it, and so holds one of the language
/// server's request slots until it ends; and a journey whose answer never reaches the client, or
/// reaches it after the session took another journey.
#[test]
fn a_journey_starts_at_once_and_reports_its_answer_when_it_ends() {
    let project = JourneyProject::new("at-once");
    let state = Arc::new(ServerState::new());
    let draft = project.settings_draft(&state, "at-once");
    let fake = FakeSimulator::start(Script {
        on_command: Box::new(|command| CommandAnswer {
            status: 200,
            body: json!({ "id": command["id"], "outcome": "applied" }),
            then: vec![Frame::Text(
                json!({ "command": { "id": command["id"], "outcome": "confirmed" } }),
            )],
            delay: STREAM_PERIOD,
        }),
        ..Script::serving(deployed(&state))
    });
    let sessions = sessions();
    let root = project.root_uri();
    let local = target(&fake.url, "access-token-7");
    let id = opened(sessions, &root, &local, at(0));
    let steps = json!([{ "page": "settings", "object": "sp", "action": "write", "value": 10 }]);
    let started_at = Instant::now();
    let (started, ended) = run(
        sessions,
        &state,
        journey(&project, &draft, &id, json!(COOKIE), &local, steps.clone()),
        at(1),
        PROMPT,
    );
    let start_took = started_at.elapsed();
    assert_eq!(
        started,
        success(json!({ "started": true, "sessionId": id, "journeyId": J }))
    );
    let again = run(
        sessions,
        &state,
        journey(&project, &draft, &id, json!(COOKIE), &local, steps),
        at(1),
        PROMPT,
    );
    assert_eq!(again.0["code"], "CONFLICT", "the session runs its journey");
    let answer = ended
        .recv_timeout(Duration::from_secs(10))
        .expect("the journey's answer");
    let journey_took = started_at.elapsed();
    assert!(
        start_took < journey_took / 2 && journey_took >= STREAM_PERIOD,
        "the start answered in {start_took:?}, before the journey ended at {journey_took:?}"
    );
    assert_eq!(answer["sessionId"], id);
    assert_eq!(answer["journeyId"], J);
    assert_eq!(answer["origin"], "simulator");
    assert_eq!(answer["passed"], true, "{answer}");
    let free = (0..100).any(|_| {
        let begun = sessions.begin(&root, &id, J, &local, at(2)).is_ok();
        if !begun {
            thread::sleep(Duration::from_millis(10));
        }
        begun
    });
    assert!(free, "the session is free once its answer was reported");
}

/// Catches: journeys waiting on a simulator that never answers keeping other commands, a cancel
/// among them, from answering (the language server serves four requests at once); and a journey
/// that never ends when its simulator hangs.
#[test]
fn journeys_waiting_on_a_hung_simulator_leave_other_commands_answering() {
    let hung = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let url = format!("http://{}", hung.local_addr().expect("an address"));
    let project = JourneyProject::new("hung");
    let state = Arc::new(ServerState::new());
    let draft = project.settings_draft(&state, "hung");
    let root = project.root_uri();
    let sessions = sessions();
    let local = target(&url, "access-token-7");
    let hurried = Timing {
        request: Duration::from_secs(3),
        ..PROMPT
    };
    let mut ends = Vec::new();
    for _ in 0..5 {
        let id = opened(sessions, &root, &local, at(0));
        let argument = journey(
            &project,
            &draft,
            &id,
            json!(COOKIE),
            &local,
            json!([{ "page": "settings" }]),
        );
        let (started, ended) = run(sessions, &state, argument, at(1), hurried);
        assert_eq!(started["data"]["started"], true, "{started}");
        ends.push((id, ended));
    }
    let asked = Instant::now();
    let listed = call(sessions, LIST_SESSIONS, json!({ "workspace": root }), at(1));
    let cancelled = call(
        sessions,
        CANCEL_JOURNEY,
        json!({ "sessionId": ends[0].0, "journeyId": J }),
        at(1),
    );
    // A command held behind a journey would wait for its 3 s request; one second leaves a loaded
    // runner a margin.
    assert!(
        asked.elapsed() < Duration::from_secs(1),
        "answered in {:?}",
        asked.elapsed()
    );
    assert_eq!(listed["data"]["sessions"].as_array().map(Vec::len), Some(5));
    assert_eq!(cancelled["data"]["cancelled"], true);
    for (index, (id, ended)) in ends.into_iter().enumerate() {
        let answer = ended
            .recv_timeout(Duration::from_secs(30))
            .expect("the hung journey's answer");
        assert_eq!(answer["sessionId"], id);
        // The cancelled journey ends at its cancel, within one read slice of its request in flight;
        // the others when their request times out.
        let expected = if index == 0 {
            "CANCELLED:"
        } else {
            "NOT_CONNECTED:"
        };
        let error = answer["error"].as_str().unwrap_or_default();
        assert!(error.starts_with(expected), "{answer}");
    }
    drop(hung);
}

/// Catches: a journey whose thread panics never reporting its end, so that the client's wait never
/// ends and the next answer of the session is taken for this journey's; the end is sent from a guard
/// the panic drops, with the journey's ID.
#[test]
fn a_journey_thread_that_panics_still_reports_its_end() {
    let (sender, ended) = mpsc::channel();
    let report: super::Report = Box::new(move |answer| {
        let _ = sender.send(answer);
    });
    let thread = thread::spawn(move || {
        let _ending = super::Ending::new(report, "session-1", J, "Fixture");
        panic!("a journey that fails");
    });
    assert!(thread.join().is_err(), "the thread panicked");
    let answer = ended
        .recv_timeout(Duration::from_secs(5))
        .expect("an answer");
    assert_eq!(answer["journeyId"], J);
    assert_eq!(answer["sessionId"], "session-1");
    assert_eq!(answer["passed"], false);
    assert_eq!(answer["outcomes"], json!([]));
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.starts_with("PROJECT_UNAVAILABLE:")),
        "{answer}"
    );
}

/// Catches: a journey without the ID its client waits by, or with an ID the client cannot have
/// meant, started all the same.
#[test]
fn a_journey_needs_an_id_of_at_most_128_characters() {
    let fake = FakeSimulator::start(Script::serving(BTreeMap::new()));
    let sessions = sessions();
    let project = JourneyProject::new("journey-id");
    let local = target(&fake.url, "access-token-7");
    let id = opened(sessions, &project.root_uri(), &local, at(0));
    for journey_id in [json!(" "), json!("j".repeat(129))] {
        let mut argument = journey(
            &project,
            NO_DRAFT,
            &id,
            json!(COOKIE),
            &local,
            json!([{ "page": "settings" }]),
        );
        argument["journeyId"] = journey_id.clone();
        assert_eq!(
            refused(sessions, argument, at(1))["code"],
            "INVALID_ARGUMENT",
            "{journey_id}"
        );
    }
    let mut nameless = journey(
        &project,
        NO_DRAFT,
        &id,
        json!(COOKIE),
        &local,
        json!([{ "page": "settings" }]),
    );
    nameless
        .as_object_mut()
        .expect("an object")
        .remove("journeyId");
    assert_eq!(
        refused(sessions, nameless, at(1))["code"],
        "INVALID_ARGUMENT"
    );
    assert!(fake.received().is_empty());
}

/// Catches: a journey that exercises a draft composed against an older program, sending its
/// commands to pages whose bindings may no longer hold, and a refused draft whose journey never
/// ends, so that the client's wait never ends; the refusal ends the journey before its first step,
/// with no request to the simulator, and frees its session.
#[test]
fn a_journey_on_a_draft_of_an_older_program_ends_with_conflict_before_its_first_step() {
    let project = JourneyProject::new("older");
    let state = Arc::new(ServerState::new());
    let draft = project.settings_draft(&state, "older");
    let fake = FakeSimulator::start(Script::serving(deployed(&state)));
    project.write(
        "src/main.st",
        "PROGRAM Main\nVAR Level : REAL := 2.0; END_VAR\nEND_PROGRAM\n",
    );
    let sessions = sessions();
    let root = project.root_uri();
    let local = target(&fake.url, "access-token-7");
    let id = opened(sessions, &root, &local, at(0));
    let steps = json!([{ "page": "settings", "object": "sp", "action": "write", "value": 10 }]);
    let argument = journey(&project, &draft, &id, json!(COOKIE), &local, steps);
    let (started, ended) = run(sessions, &state, argument, at(1), PROMPT);
    assert_eq!(started["data"]["started"], true, "{started}");
    let answer = ended
        .recv_timeout(Duration::from_secs(30))
        .expect("the journey's answer");
    assert_eq!(
        answer,
        json!({
            "origin": "simulator", "sessionId": id, "journeyId": J, "name": "Fixture",
            "outcomes": [], "passed": false,
            "error": "CONFLICT: the program changed since this simulation proposal was created.",
        })
    );
    assert!(
        fake.received().is_empty(),
        "no request reached the simulator"
    );
    let free = (0..100).any(|_| {
        let begun = sessions.begin(&root, &id, J, &local, at(2)).is_ok();
        if !begun {
            thread::sleep(Duration::from_millis(10));
        }
        begun
    });
    assert!(free, "the session is free once its answer was reported");
}
