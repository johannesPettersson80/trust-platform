//! Tests of the simulator journey (specification 43, AT-08), each against a fake simulator HMI served
//! on 127.0.0.1 (`simulator_fake`). Ports and corrects the journey assertions of
//! `editors/vscode/src/test/suite/hmi-agent-completion.test.ts`; every expected value comes from the
//! specification's journey paragraph.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::simulator_fake::{CommandAnswer, FakeSimulator, Frame, Script, STREAM_PERIOD};
use super::{following, run, same_model, Follow, Journey, Step, Timing};
use trust_runtime::hmi::page_files::HmiObject;

const COOKIE: &str = "trust_hmi_session=4f1c0ffee";
const TOKEN: &str = "access-token-7";
/// Waits long enough for any answer the fake gives; the stream period of 100 ms counts only until the
/// fake's stream announces its own, 250 ms by default, as the runtime's does.
const PROMPT: Timing = Timing {
    wait: Duration::from_secs(5),
    poll: Duration::from_millis(10),
    request: Duration::from_secs(5),
    stream: Duration::from_secs(5),
    period: Duration::from_millis(100),
    cap: Duration::from_secs(10),
};
/// A short wait for the readback, for the steps that must time out.
const SHORT: Timing = Timing {
    wait: Duration::from_millis(300),
    ..PROMPT
};
/// A readback wait shorter than the setpoint's confirmation time.
const BRIEF: Timing = Timing {
    wait: Duration::from_millis(100),
    ..PROMPT
};
/// The setpoint's and the reset pulse's confirmation time on the test page (`timeout`, 0.3 seconds).
const CONFIRMATION: Duration = Duration::from_millis(300);
/// The reset pulse's length on the test page (`pulse`, 0.4 seconds).
const PULSE: Duration = Duration::from_millis(400);

/// The deployed page as the runtime serves it, its numbers as the runtime writes them (`5.0`): a
/// setpoint, which the runtime follows to its confirmation by the value read back, a button without
/// feedback, whose command is final as applied, and a pulse button with feedback, which the runtime
/// follows to its confirmation after the pulse's end (specification 39 §12.8 item 5).
fn deployed() -> Value {
    json!({
        "title": "Settings",
        "object": [
            {
                "id": "sp", "type": "setpoint", "at": [32, 88], "size": [200, 64],
                "min": 5.0, "max": 95.0, "timeout": 0.3,
                "bind": { "value": { "path": "Main.SP", "direction": "read-write" } }
            },
            {
                "id": "stop", "type": "button", "at": [32, 160], "size": [120, 40],
                "action": "reset",
                "bind": { "target": { "path": "Main.Run", "direction": "write" } }
            },
            {
                "id": "reset", "type": "pulse-button", "at": [32, 220], "size": [120, 40],
                "pulse": 0.4, "timeout": 0.3,
                "bind": {
                    "target": { "path": "Main.Reset", "direction": "write" },
                    "feedback": { "path": "Main.Tripped", "direction": "read" }
                }
            }
        ],
        "pipe": []
    })
}

/// The same page as the extension passes the draft's model: its numbers as JavaScript writes them.
fn draft() -> Value {
    serde_json::from_str(
        r#"{"pipe": [], "object": [{"bind": {"value": {"direction": "read-write", "path": "Main.SP"}},
            "max": 95, "min": 5, "timeout": 0.3, "size": [200, 64], "at": [32, 88], "type": "setpoint",
            "id": "sp"},
            {"bind": {"target": {"direction": "write", "path": "Main.Run"}}, "action": "reset",
            "size": [120, 40], "at": [32, 160], "type": "button", "id": "stop"},
            {"bind": {"feedback": {"direction": "read", "path": "Main.Tripped"},
            "target": {"direction": "write", "path": "Main.Reset"}}, "timeout": 0.3, "pulse": 0.4,
            "size": [120, 40], "at": [32, 220], "type": "pulse-button", "id": "reset"}],
            "title": "Settings"}"#,
    )
    .expect("the draft's model")
}

/// A setpoint command, which the runtime follows, and a stop command, which it does not.
fn set(value: i64) -> Value {
    json!({ "page": "settings", "object": "sp", "action": "write", "value": value })
}

fn stop() -> Value {
    json!({ "page": "settings", "object": "stop", "action": "reset" })
}

/// A command answered `outcome`, then the later outcome `later` reported for it on the stream, with
/// the reason and the value read back the runtime reports with a failure, as late as the runtime
/// reports it: a timed-out outcome at the control's confirmation time, any other a stream period on.
fn reported(
    answered: &'static str,
    later: &'static str,
) -> Box<dyn Fn(&Value) -> CommandAnswer + Send> {
    let delay = if later == "timed-out" {
        CONFIRMATION
    } else {
        STREAM_PERIOD
    };
    reported_after(answered, later, delay)
}

/// As [`reported`], the later outcome `delay` after the answer.
fn reported_after(
    answered: &'static str,
    later: &'static str,
    delay: Duration,
) -> Box<dyn Fn(&Value) -> CommandAnswer + Send> {
    Box::new(move |command| CommandAnswer {
        status: 200,
        body: json!({ "id": command["id"], "outcome": answered }),
        then: vec![Frame::Text(json!({ "command": {
            "id": command["id"], "outcome": later, "reason": "not confirmed in time", "readback": 9.0,
        } }))],
        delay,
    })
}

/// A pulse answered `applied`, then, as a trust-runtime simulator reports a followed local pulse: the
/// pulse's end (a later `applied` with `ended`) and the outcome `later` of its confirmation, which
/// the runtime decides only after the end, here `delay` after the answer.
fn pulse_reported(
    later: &'static str,
    delay: Duration,
) -> Box<dyn Fn(&Value) -> CommandAnswer + Send> {
    Box::new(move |command| {
        let mut outcome =
            json!({ "id": command["id"], "outcome": later, "readback": later == "confirmed" });
        if later == "timed-out" {
            outcome["reason"] = json!("not confirmed in time");
        }
        CommandAnswer {
            status: 200,
            body: json!({ "id": command["id"], "outcome": "applied", "readback": true }),
            then: vec![
                Frame::Text(
                    json!({ "command": { "ended": true, "id": command["id"], "outcome": "applied" } }),
                ),
                Frame::Text(json!({ "command": outcome })),
            ],
            delay,
        }
    })
}

fn pages() -> BTreeMap<String, Value> {
    BTreeMap::from([("settings".to_owned(), deployed())])
}

/// The journey of `steps` on the fake, with the cookie, the token and the draft's `settings` page.
fn journey(
    fake: &FakeSimulator,
    steps: Value,
    live: &dyn Fn() -> bool,
    cancelled: &Arc<AtomicBool>,
    timing: Timing,
) -> Value {
    let steps: Vec<Step> = serde_json::from_value(steps).expect("steps");
    let target = json!({
        "simulator": true, "url": fake.url, "endpoint": "tcp://127.0.0.1:9100", "token": TOKEN,
    });
    let models = json!({ "settings": draft() });
    let journey = Journey {
        session_id: "session-1",
        name: "Fixture",
        journey_id: "journey-1",
        target: &target,
        cookie: COOKIE,
        steps: &steps,
        models: models.as_object().expect("the models"),
    };
    run(&journey, live, cancelled, timing)
}

/// A journey's cancellation, not set.
fn not_cancelled() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

/// A command answered `outcome`, then the frames `then` builds from the command's ID.
fn answered(
    outcome: &'static str,
    then: fn(&Value) -> Vec<Frame>,
) -> Box<dyn Fn(&Value) -> CommandAnswer + Send> {
    Box::new(move |command| CommandAnswer {
        status: 200,
        body: json!({ "id": command["id"], "outcome": outcome }),
        then: then(&command["id"]),
        delay: STREAM_PERIOD,
    })
}

fn commands(fake: &FakeSimulator) -> Vec<Value> {
    let received = fake.received();
    let posted = received
        .into_iter()
        .filter(|request| request.method == "POST");
    posted.map(|request| request.body).collect()
}

/// Catches: a journey that compares the deployed page with the draft by how its numbers are written
/// (`5.0` against `5`), that reports a step observed before the command's confirmation and the
/// expected readback arrived, or that reaches the simulator without the named session's cookie and
/// the target's access token.
#[test]
fn a_matching_page_with_its_readback_observed_passes() {
    let fake = FakeSimulator::start(Script {
        on_command: answered("applied", |id| {
            vec![
                Frame::Text(json!({ "cycle": 7, "values": { "Main.SP": { "value": 10.0 } } })),
                Frame::Text(json!({ "command": { "id": id, "outcome": "confirmed" } })),
            ]
        }),
        ..Script::serving(pages())
    });
    let steps = json!([{ "page": "settings", "object": "sp", "action": "write", "value": 10,
                         "path": "Main.SP", "expect": 10 }]);
    let answer = journey(&fake, steps, &|| true, &not_cancelled(), PROMPT);
    let received = fake.received();
    let routes: Vec<(&str, &str)> = received
        .iter()
        .map(|request| (request.method.as_str(), request.url.as_str()))
        .collect();
    assert_eq!(
        routes,
        [
            ("GET", "/api/hmi/page?name=settings"),
            ("GET", "/ws/hmi?page=settings"),
            ("POST", "/api/hmi/command"),
        ]
    );
    for request in &received {
        assert_eq!(request.cookie.as_deref(), Some(COOKIE), "{request:?}");
        assert_eq!(request.token.as_deref(), Some(TOKEN), "{request:?}");
    }
    let id = received[2].body["id"]
        .as_str()
        .expect("a command ID")
        .to_owned();
    assert_eq!(id.len(), 36, "a new UUID: {id}");
    assert_eq!(
        received[2].body,
        json!({ "id": id, "bundle": "bundle-1", "page": "settings", "object": "sp",
                "action": "write", "value": 10 })
    );
    assert_eq!(
        answer,
        json!({
            "origin": "simulator", "sessionId": "session-1", "journeyId": "journey-1",
            "name": "Fixture", "passed": true,
            "outcomes": [
                { "page": "settings", "object": "sp", "commandId": id, "status": 200,
                  "outcome": "applied" },
                { "page": "settings", "outcome": "observed", "commandId": id, "path": "Main.SP",
                  "value": 10.0 },
            ],
        })
    );
}

/// Catches: a journey that exercises a deployed page other than the draft's.
#[test]
fn a_page_that_differs_from_the_draft_stops_with_the_conflict() {
    let mut other = deployed();
    other["object"][0]["max"] = json!(90.0);
    let fake = FakeSimulator::start(Script::serving(BTreeMap::from([(
        "settings".into(),
        other,
    )])));
    let steps = json!([{ "page": "settings", "object": "sp", "action": "write", "value": 10 }]);
    let answer = journey(&fake, steps, &|| true, &not_cancelled(), PROMPT);
    assert_eq!(
        answer,
        json!({
            "origin": "simulator", "sessionId": "session-1", "journeyId": "journey-1",
            "name": "Fixture", "passed": false,
            "outcomes": [],
            "error": "CONFLICT: the deployed simulator page does not match this draft; apply and \
                      deploy explicitly before exercising it.",
        })
    );
    assert_eq!(
        fake.received().len(),
        1,
        "only the page was read: no stream, no command"
    );
}

/// Catches: a journey that goes on after a refused command, that sends the next command anyway, or
/// that drops the refusal's status, outcome or reason.
#[test]
fn a_refused_command_stops_the_journey_with_its_outcome() {
    let refusals = [
        (200, json!({ "outcome": "refused", "reason": "interlock" })),
        (
            401,
            json!({ "ok": false, "error": "log in with a named HMI user to operate" }),
        ),
        (
            403,
            json!({ "outcome": "refused", "reason": "named login or policy refused" }),
        ),
    ];
    for (status, body) in refusals {
        let answer_body = body.clone();
        let fake = FakeSimulator::start(Script {
            on_command: Box::new(move |_| CommandAnswer {
                status,
                body: answer_body.clone(),
                then: Vec::new(),
                delay: STREAM_PERIOD,
            }),
            ..Script::serving(pages())
        });
        let steps = json!([
            { "page": "settings", "object": "sp", "action": "write", "value": 10 },
            { "page": "settings", "object": "sp", "action": "write", "value": 20 },
        ]);
        let answer = journey(&fake, steps, &|| true, &not_cancelled(), PROMPT);
        let sent = commands(&fake);
        assert_eq!(sent.len(), 1, "{status}: one command, never the next");
        let mut expected = json!({ "page": "settings", "object": "sp", "commandId": sent[0]["id"],
                                   "status": status });
        for key in ["outcome", "reason"] {
            if let Some(value) = body.get(key) {
                expected[key] = value.clone();
            }
        }
        assert_eq!(answer["outcomes"], json!([expected]), "{status}");
        assert_eq!(answer["passed"], false, "{status}");
        assert!(answer.get("error").is_none(), "{status}: {answer}");
    }
}

/// Catches the defect of the extension's journey: it waited for a confirmation the runtime never
/// sends for a control it does not follow, whose command is final as `applied`, so every such step
/// timed out.
#[test]
fn an_unfollowed_controls_applied_command_is_observed_at_once() {
    let fake = FakeSimulator::start(Script {
        on_command: answered("applied", |_| Vec::new()),
        ..Script::serving(pages())
    });
    let started = Instant::now();
    let answer = journey(&fake, json!([stop()]), &|| true, &not_cancelled(), PROMPT);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "it waited for no confirmation"
    );
    let id = &commands(&fake)[0]["id"];
    assert_eq!(
        answer["outcomes"],
        json!([
            { "page": "settings", "object": "stop", "commandId": id, "status": 200, "outcome": "applied" },
            { "page": "settings", "outcome": "observed", "commandId": id },
        ])
    );
    assert_eq!(answer["passed"], true);
}

/// Catches: a followed control's command taken as complete without its later confirmation, or that
/// confirmation missed.
#[test]
fn a_followed_controls_later_confirmation_is_observed() {
    let fake = FakeSimulator::start(Script {
        on_command: answered("applied", |id| {
            vec![Frame::Text(
                json!({ "command": { "id": id, "outcome": "confirmed" } }),
            )]
        }),
        ..Script::serving(pages())
    });
    let answer = journey(&fake, json!([set(10)]), &|| true, &not_cancelled(), PROMPT);
    let id = &commands(&fake)[0]["id"];
    assert_eq!(
        answer["outcomes"],
        json!([
            { "page": "settings", "object": "sp", "commandId": id, "status": 200, "outcome": "applied" },
            { "page": "settings", "outcome": "observed", "commandId": id },
        ])
    );
    assert_eq!(answer["passed"], true);
}

/// Catches the regression a rule taking every `applied` as final would cause: a followed control's
/// command that the runtime later reports timed out passed, where the extension's journey caught it.
#[test]
fn a_followed_controls_later_timed_out_ends_the_journey() {
    let fake = FakeSimulator::start(Script {
        on_command: reported("applied", "timed-out"),
        ..Script::serving(pages())
    });
    let answer = journey(
        &fake,
        json!([set(10), set(20)]),
        &|| true,
        &not_cancelled(),
        PROMPT,
    );
    let sent = commands(&fake);
    assert_eq!(sent.len(), 1, "the next step never ran");
    assert_eq!(
        answer["outcomes"],
        json!([
            { "page": "settings", "object": "sp", "commandId": sent[0]["id"], "status": 200,
              "outcome": "applied" },
            { "page": "settings", "outcome": "timed-out", "commandId": sent[0]["id"],
              "reason": "not confirmed in time", "readback": 9.0 },
        ])
    );
    assert_eq!(answer["passed"], false);
}

/// Catches: a followed control's command reported observed although no later outcome came within the
/// wait.
#[test]
fn a_followed_control_without_a_later_outcome_times_out() {
    let fake = FakeSimulator::start(Script {
        on_command: answered("applied", |_| Vec::new()),
        ..Script::serving(pages())
    });
    let started = Instant::now();
    let answer = journey(&fake, json!([set(10)]), &|| true, &not_cancelled(), SHORT);
    assert!(
        started.elapsed() >= SHORT.wait,
        "it waited for the later outcome"
    );
    let id = &commands(&fake)[0]["id"];
    assert_eq!(
        answer["outcomes"],
        json!([
            { "page": "settings", "object": "sp", "commandId": id, "status": 200, "outcome": "applied" },
            { "page": "settings", "outcome": "timed-out", "commandId": id },
        ])
    );
    assert_eq!(answer["passed"], false);
}

/// Catches: a `pending` answer (a repeated command ID) taken for a completed command, or its later
/// `applied` or `confirmed` missed.
#[test]
fn a_pending_command_completes_with_its_later_outcome() {
    for (step, later) in [(stop(), "applied"), (set(10), "confirmed")] {
        let fake = FakeSimulator::start(Script {
            on_command: Box::new(move |command| CommandAnswer {
                status: 200,
                body: json!({ "id": command["id"], "outcome": "pending" }),
                then: vec![Frame::Text(
                    json!({ "command": { "id": command["id"], "outcome": later } }),
                )],
                delay: STREAM_PERIOD,
            }),
            ..Script::serving(pages())
        });
        let answer = journey(&fake, json!([step]), &|| true, &not_cancelled(), PROMPT);
        let id = &commands(&fake)[0]["id"];
        assert_eq!(
            answer["outcomes"][1],
            json!({ "page": "settings", "outcome": "observed", "commandId": id }),
            "{later}"
        );
        assert_eq!(answer["passed"], true, "{later}");
    }
}

/// Catches: a journey that reports a `pending` command as observed although the simulator never
/// decided it within the wait.
#[test]
fn a_pending_command_never_decided_times_out() {
    let fake = FakeSimulator::start(Script {
        on_command: answered("pending", |_| Vec::new()),
        ..Script::serving(pages())
    });
    let answer = journey(&fake, json!([set(10)]), &|| true, &not_cancelled(), SHORT);
    let id = &commands(&fake)[0]["id"];
    assert_eq!(
        answer["outcomes"][1],
        json!({ "page": "settings", "outcome": "timed-out", "commandId": id })
    );
    assert_eq!(answer["passed"], false);
}

/// Catches: a journey that goes on after the stream reported its command refused, timed out or of
/// unknown outcome (a remote pulse whose end cannot be observed), while the step waited for its later
/// outcome or for its readback, or that drops the report's reason and the value it read back.
#[test]
fn a_command_the_stream_reports_failed_ends_the_journey_with_that_outcome() {
    let mut stop_with_readback = stop();
    stop_with_readback["path"] = json!("Main.Run");
    let cases = [
        (set(10), "pending", "refused"),
        (stop(), "pending", "timed-out"),
        (stop(), "pending", "unknown"),
        (stop_with_readback, "applied", "timed-out"),
    ];
    for (step, answered, later) in cases {
        let fake = FakeSimulator::start(Script {
            on_command: reported(answered, later),
            ..Script::serving(pages())
        });
        let object = step["object"].clone();
        let answer = journey(
            &fake,
            json!([step, stop()]),
            &|| true,
            &not_cancelled(),
            PROMPT,
        );
        let sent = commands(&fake);
        assert_eq!(
            sent.len(),
            1,
            "{answered} then {later}: the next step never ran"
        );
        assert_eq!(
            answer["outcomes"],
            json!([
                { "page": "settings", "object": object, "commandId": sent[0]["id"], "status": 200,
                  "outcome": answered },
                { "page": "settings", "outcome": later, "commandId": sent[0]["id"],
                  "reason": "not confirmed in time", "readback": 9.0 },
            ]),
            "{answered} then {later}"
        );
        assert_eq!(answer["passed"], false);
        assert!(answer.get("error").is_none(), "{answer}");
    }
}

/// Catches: a journey that decides itself which commands the runtime follows, instead of by the
/// runtime's own plan of each (`Plan::is_followed`), or that takes a control whose plan it cannot read
/// as unfollowed, which could pass a command the runtime later reports failed.
#[test]
fn the_runtime_plans_which_commands_are_followed() {
    let page = deployed();
    let step = |value: Value| serde_json::from_value::<Step>(value).expect("a step");
    let setpoint = &page["object"][0];
    let button = &page["object"][1];
    serde_json::from_value::<HmiObject>(setpoint.clone())
        .expect("the setpoint as the runtime reads it");
    serde_json::from_value::<HmiObject>(button.clone())
        .expect("the button as the runtime reads it");
    let setpoint_follows = following(setpoint, &step(set(10)));
    assert_eq!(setpoint_follows.follow, Follow::Confirmation);
    assert_eq!(
        setpoint_follows.decided, CONFIRMATION,
        "the control's own timeout"
    );
    assert_eq!(following(button, &step(stop())).follow, Follow::None);
    let reset = following(
        &page["object"][2],
        &step(json!({ "page": "settings", "object": "reset", "action": "pulse" })),
    );
    assert_eq!(reset.follow, Follow::Confirmation);
    assert_eq!(
        reset.decided,
        PULSE + Duration::from_secs(2),
        "the longer of the pulse and the timeout, and the runtime's 2 s to write the pulse's end"
    );
    let remote = json!({
        "id": "start", "type": "pulse-button", "at": [0, 0], "size": [120, 40], "pulse": 0.5,
        "bind": { "target": { "path": "Owner:Main.Start", "direction": "write" } }
    });
    let pulse = step(json!({ "page": "settings", "object": "start", "action": "pulse" }));
    let remote_follows = following(&remote, &pulse);
    assert_eq!(remote_follows.follow, Follow::PulseEnd);
    assert_eq!(
        remote_follows.decided,
        Duration::from_secs(7),
        "the default timeout of 5 s, longer than the pulse, and 2 s to write"
    );
    let unread = following(&json!({ "id": "odd" }), &step(stop()));
    assert_eq!(unread.follow, Follow::Confirmation);
    assert_eq!(
        unread.decided,
        Duration::from_secs(5),
        "the runtime's default"
    );
}

/// Catches: a command sent to a target that no longer runs in simulation mode, which the runtime's
/// stream says as it opens (specification 39 §11 item 6: a target change invalidates the session
/// before another command can go out).
#[test]
fn a_target_that_no_longer_simulates_stops_before_the_command() {
    let fake = FakeSimulator::start(Script {
        simulation: Some(false),
        on_command: answered("applied", |_| Vec::new()),
        ..Script::serving(pages())
    });
    let answer = journey(&fake, json!([set(10)]), &|| true, &not_cancelled(), PROMPT);
    assert_eq!(
        answer["error"],
        "CONFLICT: the simulator target no longer runs in simulation mode; connect the simulator \
         journey again."
    );
    assert!(commands(&fake).is_empty(), "no command went out");
}

/// Catches: a command sent after the journey was cancelled while its step read the page and opened
/// the stream: the journey checks the cancel again right before the command.
#[test]
fn a_cancel_before_the_command_sends_none() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel = Arc::clone(&cancelled);
    let fake = FakeSimulator::start(Script {
        on_stream: Box::new(move || cancel.store(true, Ordering::SeqCst)),
        on_command: answered("applied", |_| Vec::new()),
        ..Script::serving(pages())
    });
    let answer = journey(&fake, json!([set(10)]), &|| true, &cancelled, PROMPT);
    assert_eq!(
        answer["error"],
        "CANCELLED: simulator journey cancelled; completed commands are retained in outcomes and \
         are not undone."
    );
    assert!(commands(&fake).is_empty(), "no command went out");
}

/// Catches: a followed command's wait cut at the readback wait instead of its control's confirmation
/// time and the stream's delay, so that the runtime's own outcome, with its reason and value read
/// back, arrives too late and a confirmation is reported as a time-out.
#[test]
fn a_followed_command_waits_for_its_controls_confirmation_time() {
    let timed_out = FakeSimulator::start(Script {
        on_command: reported("applied", "timed-out"),
        ..Script::serving(pages())
    });
    let answer = journey(
        &timed_out,
        json!([set(10)]),
        &|| true,
        &not_cancelled(),
        BRIEF,
    );
    assert_eq!(
        answer["outcomes"][1],
        json!({ "page": "settings", "outcome": "timed-out", "commandId": commands(&timed_out)[0]["id"],
                "reason": "not confirmed in time", "readback": 9.0 }),
        "the runtime's own outcome, at the control's confirmation time"
    );
    let confirmed = FakeSimulator::start(Script {
        on_command: answered("applied", |id| {
            vec![Frame::Text(
                json!({ "command": { "id": id, "outcome": "confirmed" } }),
            )]
        }),
        ..Script::serving(pages())
    });
    let answer = journey(
        &confirmed,
        json!([set(10)]),
        &|| true,
        &not_cancelled(),
        BRIEF,
    );
    assert_eq!(
        answer["passed"], true,
        "confirmed a stream period on: {answer}"
    );
}

/// Catches: a followed pulse's wait cut at its control's confirmation time, although the runtime
/// decides the confirmation only after the pulse's end (`Plan::decided_within`), so that the journey
/// reports a time-out of its own without the runtime's reason; and the report of the pulse's end, a
/// later `applied` with `ended`, taken for the pulse's completion. Seen against a trust-runtime
/// simulator: a 1 s pulse with a 1 s confirmation time reported its end at 1.2 s and its time-out at
/// 1.5 s. Here the outcome comes 1 s after the answer, after the confirmation time and two stream
/// periods (0.8 s), and before the pulse, the runtime's 2 s to write its end and two periods.
#[test]
fn a_followed_pulse_ends_with_the_runtimes_outcome_after_its_end() {
    let step = json!([{ "page": "settings", "object": "reset", "action": "pulse" }]);
    let timed_out = FakeSimulator::start(Script {
        on_command: pulse_reported("timed-out", Duration::from_secs(1)),
        ..Script::serving(pages())
    });
    let answer = journey(&timed_out, step.clone(), &|| true, &not_cancelled(), BRIEF);
    let id = &commands(&timed_out)[0]["id"];
    assert_eq!(
        answer["outcomes"],
        json!([
            { "page": "settings", "object": "reset", "commandId": id, "status": 200, "outcome": "applied",
              "readback": true },
            { "page": "settings", "outcome": "timed-out", "commandId": id,
              "reason": "not confirmed in time", "readback": false },
        ])
    );
    assert_eq!(answer["passed"], false);
    let confirmed = FakeSimulator::start(Script {
        on_command: pulse_reported("confirmed", Duration::from_secs(1)),
        ..Script::serving(pages())
    });
    let answer = journey(&confirmed, step, &|| true, &not_cancelled(), BRIEF);
    let id = &commands(&confirmed)[0]["id"];
    assert_eq!(
        answer["outcomes"][1],
        json!({ "page": "settings", "outcome": "observed", "commandId": id }),
        "observed at its confirmation, not at the report of its end: {answer}"
    );
    assert_eq!(answer["passed"], true);
}

/// Catches: a journey that keeps its own stream period instead of the one the runtime's stream
/// announces (`period_ms`), so that a slow stream's later outcome arrives after the wait and the
/// journey reports a time-out of its own instead of the runtime's. The stream here announces 600 ms
/// and reports the time-out 1.2 s after the answer: inside the confirmation time and two announced
/// periods (1.5 s), after the confirmation time and two of the journey's own periods (0.5 s).
#[test]
fn the_streams_announced_period_lengthens_the_wait() {
    let fake = FakeSimulator::start(Script {
        period: Duration::from_millis(600),
        on_command: reported_after("applied", "timed-out", Duration::from_millis(1200)),
        ..Script::serving(pages())
    });
    let answer = journey(&fake, json!([set(10)]), &|| true, &not_cancelled(), BRIEF);
    assert_eq!(
        answer["outcomes"][1],
        json!({ "page": "settings", "outcome": "timed-out", "commandId": commands(&fake)[0]["id"],
                "reason": "not confirmed in time", "readback": 9.0 }),
        "the runtime's own outcome"
    );
}

/// Catches: a control's confirmation time taken as it is, which can be any positive number of
/// seconds: `timeout = 1e19` overflowed the wait's deadline and the journey's thread panicked (the
/// reviewer's probe). The wait is at most the cap, here 1 s.
#[test]
fn a_huge_confirmation_time_waits_at_most_the_cap() {
    let mut page = deployed();
    page["object"][0]["timeout"] = json!(1e19);
    let fake = FakeSimulator::start(Script {
        on_command: answered("applied", |_| Vec::new()),
        ..Script::serving(BTreeMap::from([("settings".to_owned(), page.clone())]))
    });
    let steps: Vec<Step> = serde_json::from_value(json!([set(10)])).expect("steps");
    let target = json!({ "simulator": true, "url": fake.url, "endpoint": "tcp://127.0.0.1:9100" });
    let models = json!({ "settings": page });
    let journey = Journey {
        session_id: "session-1",
        name: "Fixture",
        journey_id: "journey-1",
        target: &target,
        cookie: COOKIE,
        steps: &steps,
        models: models.as_object().expect("the models"),
    };
    let capped = Timing {
        cap: Duration::from_secs(1),
        ..BRIEF
    };
    let started = Instant::now();
    let answer = run(&journey, &|| true, &not_cancelled(), capped);
    let took = started.elapsed();
    assert_eq!(answer["outcomes"][1]["outcome"], "timed-out", "{answer}");
    assert!(
        took >= Duration::from_secs(1) && took < Duration::from_secs(3),
        "{took:?}"
    );
}

/// Catches: a journey that keeps waiting for a later outcome after its named session expired or
/// closed, up to the control's confirmation time; the wait ends with the session.
#[test]
fn a_session_that_ends_during_the_wait_ends_the_step() {
    let fake = FakeSimulator::start(Script {
        on_command: answered("applied", |_| Vec::new()),
        ..Script::serving(pages())
    });
    let started = Instant::now();
    let live = || started.elapsed() < Duration::from_millis(400);
    let answer = journey(&fake, json!([set(10)]), &live, &not_cancelled(), PROMPT);
    assert_eq!(
        answer["error"], "CONFLICT: the simulator target or named session changed.",
        "{answer}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "it waited no longer"
    );
}

/// Catches: a command sent although the stream never said that the target runs in simulation
/// mode; the runtime says so as the stream opens, so silence fails closed.
#[test]
fn a_stream_that_never_says_simulation_stops_before_the_command() {
    let fake = FakeSimulator::start(Script {
        simulation: None,
        on_command: answered("applied", |_| Vec::new()),
        ..Script::serving(pages())
    });
    let answer = journey(&fake, json!([set(10)]), &|| true, &not_cancelled(), PROMPT);
    assert_eq!(
        answer["error"],
        "CONFLICT: the simulator's readback stream did not say that the target runs in simulation \
         mode; connect the simulator journey again."
    );
    assert!(commands(&fake).is_empty(), "no command went out");
}

/// Catches: a later `unknown` taken as final for a control with a confirmation to come: the runtime
/// reports the unobserved end of a remote pulse as `unknown` and then still decides its
/// confirmation, so the step waits for it.
#[test]
fn an_unknown_end_waits_for_a_confirmation_to_come() {
    let mut page = deployed();
    page["object"][2]["bind"]["target"]["path"] = json!("Owner:Main.Reset");
    let fake = FakeSimulator::start(Script {
        on_command: answered("applied", |id| {
            vec![
                Frame::Text(json!({ "command": { "id": id, "outcome": "unknown",
                    "reason": "the owner's pulse end could not be observed" } })),
                Frame::Text(json!({ "command": { "id": id, "outcome": "confirmed" } })),
            ]
        }),
        ..Script::serving(BTreeMap::from([("settings".to_owned(), page.clone())]))
    });
    let steps: Vec<Step> = serde_json::from_value(json!([
        { "page": "settings", "object": "reset", "action": "pulse" }
    ]))
    .expect("steps");
    let target = json!({ "simulator": true, "url": fake.url, "endpoint": "tcp://127.0.0.1:9100" });
    let models = json!({ "settings": page });
    let journey = Journey {
        session_id: "session-1",
        name: "Fixture",
        journey_id: "journey-1",
        target: &target,
        cookie: COOKIE,
        steps: &steps,
        models: models.as_object().expect("the models"),
    };
    let answer = run(&journey, &|| true, &not_cancelled(), PROMPT);
    assert_eq!(answer["outcomes"][1]["outcome"], "observed", "{answer}");
    assert_eq!(answer["passed"], true);
}

/// Catches: a cancel that waits for the command's answer, which a hung runtime never gives, and a
/// step that forgets a command it sent when a cancel interrupts the answer: the command may have been
/// applied, so the step ends `cancelled` with its ID.
#[test]
fn a_cancel_during_the_commands_answer_keeps_its_id() {
    let cancelled = not_cancelled();
    let cancel = Arc::clone(&cancelled);
    let fake = FakeSimulator::start(Script {
        on_command: Box::new(move |command| {
            cancel.store(true, Ordering::SeqCst);
            thread::sleep(Duration::from_secs(2));
            CommandAnswer {
                status: 200,
                body: json!({ "id": command["id"], "outcome": "applied" }),
                then: Vec::new(),
                delay: Duration::ZERO,
            }
        }),
        ..Script::serving(pages())
    });
    let started = Instant::now();
    let answer = journey(&fake, json!([set(10)]), &|| true, &cancelled, PROMPT);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "it ended at the cancel"
    );
    assert_eq!(
        answer["outcomes"],
        json!([{ "page": "settings", "object": "sp", "commandId": commands(&fake)[0]["id"],
                 "outcome": "cancelled" }])
    );
    assert_eq!(answer["passed"], false);
}

/// Catches: a journey that reports success, or keeps waiting, after the readback stream closed while
/// the command was still pending.
#[test]
fn a_closed_stream_is_disconnected() {
    let fake = FakeSimulator::start(Script {
        on_command: answered("pending", |_| vec![Frame::Close]),
        ..Script::serving(pages())
    });
    let steps = json!([{ "page": "settings", "object": "sp", "action": "write", "value": 10 }]);
    let answer = journey(&fake, steps, &|| true, &not_cancelled(), PROMPT);
    let id = &commands(&fake)[0]["id"];
    assert_eq!(
        answer["outcomes"],
        json!([
            { "page": "settings", "object": "sp", "commandId": id, "status": 200, "outcome": "pending" },
            { "page": "settings", "outcome": "disconnected", "commandId": id },
        ])
    );
    assert_eq!(answer["passed"], false);
}

/// Catches: a cancelled journey that loses the outcomes its steps completed, that is not cancelled
/// while it waits, or that starts a step after it was cancelled.
#[test]
fn cancel_keeps_the_completed_outcomes() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel = Arc::clone(&cancelled);
    let fake = FakeSimulator::start(Script {
        on_open: vec![Frame::Text(
            json!({ "values": { "Main.SP": { "value": 50.0 } } }),
        )],
        on_command: Box::new(move |command| {
            // Cancelled while the step waits for the command's later outcome, after its answer.
            let cancel = Arc::clone(&cancel);
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(100));
                cancel.store(true, Ordering::SeqCst);
            });
            CommandAnswer {
                status: 200,
                body: json!({ "id": command["id"], "outcome": "applied" }),
                then: Vec::new(),
                delay: STREAM_PERIOD,
            }
        }),
        ..Script::serving(pages())
    });
    let steps = json!([
        { "page": "settings", "path": "Main.SP" },
        { "page": "settings", "object": "sp", "action": "write", "value": 10 },
        { "page": "settings", "path": "Main.SP" },
    ]);
    let answer = journey(&fake, steps, &|| true, &cancelled, PROMPT);
    let id = &commands(&fake)[0]["id"];
    assert_eq!(
        answer["outcomes"],
        json!([
            { "page": "settings", "outcome": "observed", "path": "Main.SP", "value": 50.0 },
            { "page": "settings", "object": "sp", "commandId": id, "status": 200, "outcome": "applied" },
            { "page": "settings", "outcome": "cancelled", "commandId": id },
        ])
    );
    assert_eq!(answer["passed"], false);
    assert_eq!(fake.received().len(), 5, "the third step never started");

    let before = FakeSimulator::start(Script::serving(pages()));
    let answer = journey(
        &before,
        json!([{ "page": "settings" }]),
        &|| true,
        &Arc::new(AtomicBool::new(true)),
        PROMPT,
    );
    assert_eq!(
        answer["error"],
        "CANCELLED: simulator journey cancelled; completed commands are retained in outcomes and \
         are not undone."
    );
    assert_eq!(answer["outcomes"], json!([]));
    assert!(before.received().is_empty());
}

/// Catches: a journey whose last outcome is `observed` reported as passed although an error ended it
/// before its remaining steps ran: it has passed exactly when no error ended it and its last outcome
/// is `observed`.
#[test]
fn an_error_after_an_observed_step_does_not_pass() {
    let fake = FakeSimulator::start(Script {
        on_open: vec![Frame::Text(
            json!({ "values": { "Main.SP": { "value": 50.0 } } }),
        )],
        ..Script::serving(pages())
    });
    let steps = json!([{ "page": "settings", "path": "Main.SP" }, { "page": "alarms" }]);
    let answer = journey(&fake, steps, &|| true, &not_cancelled(), PROMPT);
    assert_eq!(
        answer["outcomes"],
        json!([{ "page": "settings", "outcome": "observed", "path": "Main.SP", "value": 50.0 }])
    );
    assert_eq!(
        answer["error"],
        "CONFLICT: the deployed simulator page does not match this draft; apply and deploy \
         explicitly before exercising it."
    );
    assert_eq!(answer["passed"], false);
}

/// Catches: a journey that goes on after its named session expired or was closed between two steps,
/// as closing it does when the selected run target changes. (A target that stops simulating is
/// caught by `a_target_that_no_longer_simulates_stops_before_the_command`.)
#[test]
fn a_session_that_ends_between_steps_stops_the_journey() {
    let fake = FakeSimulator::start(Script {
        on_open: vec![Frame::Text(
            json!({ "values": { "Main.SP": { "value": 50.0 } } }),
        )],
        ..Script::serving(pages())
    });
    let checks = Cell::new(0);
    let live = || {
        checks.set(checks.get() + 1);
        checks.get() == 1
    };
    // The first step neither commands nor reads back, so that only the second step's start asks
    // whether the session is still open.
    let steps = json!([{ "page": "settings" }, { "page": "settings", "path": "Main.SP" }]);
    let answer = journey(&fake, steps, &live, &not_cancelled(), PROMPT);
    assert_eq!(
        answer["outcomes"],
        json!([{ "page": "settings", "outcome": "observed" }])
    );
    assert_eq!(
        answer["error"],
        "CONFLICT: the simulator target or named session changed."
    );
    assert_eq!(answer["passed"], false);
    assert_eq!(fake.received().len(), 2, "the second step read nothing");
}

/// Catches the defect of the extension's journey: a path the running program does not have, which
/// the runtime answers `null` and lists in `missing`, counted as observed, so that a misspelled
/// readback path passed at once.
#[test]
fn a_path_answered_null_has_no_value_and_times_out() {
    let fake = FakeSimulator::start(Script {
        on_open: vec![Frame::Text(
            json!({ "cycle": 1, "values": { "Main.Typo": null }, "missing": ["Main.Typo"] }),
        )],
        ..Script::serving(pages())
    });
    let steps = json!([{ "page": "settings", "path": "Main.Typo" }]);
    let answer = journey(&fake, steps, &|| true, &not_cancelled(), SHORT);
    assert_eq!(
        answer["outcomes"],
        json!([{ "page": "settings", "outcome": "timed-out" }])
    );
    assert_eq!(answer["passed"], false);
}

/// Catches: a journey that observes whatever value a readback path has instead of the expected one.
#[test]
fn a_readback_other_than_the_expected_value_times_out() {
    let fake = FakeSimulator::start(Script {
        on_open: vec![Frame::Text(
            json!({ "values": { "Main.SP": { "value": 50.0 } } }),
        )],
        ..Script::serving(pages())
    });
    let steps = json!([{ "page": "settings", "path": "Main.SP", "expect": 10 }]);
    let answer = journey(&fake, steps, &|| true, &not_cancelled(), SHORT);
    assert_eq!(
        answer["outcomes"],
        json!([{ "page": "settings", "outcome": "timed-out" }])
    );
}

/// Catches: a journey that sends a command for a control the deployed page does not have.
#[test]
fn an_unknown_control_is_refused_before_any_command() {
    let fake = FakeSimulator::start(Script::serving(pages()));
    let steps = json!([{ "page": "settings", "object": "nope", "action": "set", "value": 1 }]);
    let answer = journey(&fake, steps, &|| true, &not_cancelled(), PROMPT);
    assert_eq!(
        answer["error"],
        "INVALID_ARGUMENT: simulator step names an unknown control."
    );
    assert!(commands(&fake).is_empty());
}

/// Catches: a journey that sends a command although it cannot follow the page's readback stream, so
/// that the command's outcome could never be observed.
#[test]
fn a_readback_stream_that_does_not_open_stops_before_the_command() {
    let fake = FakeSimulator::start(Script {
        streams: false,
        ..Script::serving(pages())
    });
    let steps = json!([{ "page": "settings", "object": "sp", "action": "write", "value": 10 }]);
    let answer = journey(&fake, steps, &|| true, &not_cancelled(), PROMPT);
    let error = answer["error"].as_str().expect("an error");
    assert!(
        error.starts_with("NOT_CONNECTED: simulator readback stream unavailable"),
        "{error}"
    );
    assert!(commands(&fake).is_empty());
}

/// Catches: a comparison that tells models apart by the order of their keys or by how a number is
/// written, or that takes a model with a missing, extra, reordered or retyped item for the same.
#[test]
fn models_are_the_same_whatever_their_key_order_and_number_form() {
    let parse = |text: &str| serde_json::from_str::<Value>(text).expect("JSON");
    assert!(same_model(
        &parse(r#"{"a": 1, "b": [1, 2.5, {"c": true}]}"#),
        &parse(r#"{"b": [1.0, 2.50, {"c": true}], "a": 1.0}"#)
    ));
    assert!(!same_model(
        &json!({ "a": 1 }),
        &json!({ "a": 1, "b": null })
    ));
    assert!(!same_model(&json!([1, 2]), &json!([2, 1])));
    assert!(!same_model(&json!({ "a": 1 }), &json!({ "a": 2 })));
    assert!(!same_model(&json!({ "a": "1" }), &json!({ "a": 1 })));
    assert!(!same_model(&json!([1]), &json!([1, 1])));
}
