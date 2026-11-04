//! Tests of the simulator journey (specification 43, AT-08), each against a fake simulator HMI served
//! on 127.0.0.1 (`simulator_fake`). Ports and corrects the journey assertions of
//! `editors/vscode/src/test/suite/hmi-agent-completion.test.ts`; every expected value comes from the
//! specification's journey paragraph.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::simulator_fake::{CommandAnswer, FakeSimulator, Frame, Script};
use super::{follow_of, run, same_model, Follow, Journey, Step, Timing};
use trust_runtime::hmi::page_files::HmiObject;

const COOKIE: &str = "trust_hmi_session=4f1c0ffee";
const TOKEN: &str = "access-token-7";
/// Waits long enough for any answer the fake gives at once.
const PROMPT: Timing = Timing {
    wait: Duration::from_secs(5),
    poll: Duration::from_millis(10),
    request: Duration::from_secs(5),
    stream: Duration::from_secs(5),
};
/// A short wait for confirmation and readback, for the steps that must time out.
const SHORT: Timing = Timing {
    wait: Duration::from_millis(300),
    ..PROMPT
};

/// The deployed page as the runtime serves it, its numbers as the runtime writes them (`5.0`): a
/// setpoint, which the runtime follows to its confirmation by the value read back, and a button
/// without feedback, whose command is final as applied (specification 39 §12.8 item 5).
fn deployed() -> Value {
    json!({
        "title": "Settings",
        "object": [
            {
                "id": "sp", "type": "setpoint", "at": [32, 88], "size": [200, 64],
                "min": 5.0, "max": 95.0,
                "bind": { "value": { "path": "Main.SP", "direction": "read-write" } }
            },
            {
                "id": "stop", "type": "button", "at": [32, 160], "size": [120, 40],
                "action": "reset",
                "bind": { "target": { "path": "Main.Run", "direction": "write" } }
            }
        ],
        "pipe": []
    })
}

/// The same page as the extension passes the draft's model: its numbers as JavaScript writes them.
fn draft() -> Value {
    serde_json::from_str(
        r#"{"pipe": [], "object": [{"bind": {"value": {"direction": "read-write", "path": "Main.SP"}},
            "max": 95, "min": 5, "size": [200, 64], "at": [32, 88], "type": "setpoint", "id": "sp"},
            {"bind": {"target": {"direction": "write", "path": "Main.Run"}}, "action": "reset",
            "size": [120, 40], "at": [32, 160], "type": "button", "id": "stop"}],
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
/// the reason and the value read back the runtime reports with a failure.
fn reported(
    answered: &'static str,
    later: &'static str,
) -> Box<dyn Fn(&Value) -> CommandAnswer + Send> {
    Box::new(move |command| CommandAnswer {
        status: 200,
        body: json!({ "id": command["id"], "outcome": answered }),
        then: vec![Frame::Text(json!({ "command": {
            "id": command["id"], "outcome": later, "reason": "not confirmed in time", "readback": 9.0,
        } }))],
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
    cancelled: &AtomicBool,
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
        target: &target,
        cookie: COOKIE,
        steps: &steps,
        models: models.as_object().expect("the models"),
    };
    run(&journey, live, cancelled, timing)
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
    let answer = journey(&fake, steps, &|| true, &AtomicBool::new(false), PROMPT);
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
            "origin": "simulator", "sessionId": "session-1", "name": "Fixture", "passed": true,
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
    let answer = journey(&fake, steps, &|| true, &AtomicBool::new(false), PROMPT);
    assert_eq!(
        answer,
        json!({
            "origin": "simulator", "sessionId": "session-1", "name": "Fixture", "passed": false,
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
            }),
            ..Script::serving(pages())
        });
        let steps = json!([
            { "page": "settings", "object": "sp", "action": "write", "value": 10 },
            { "page": "settings", "object": "sp", "action": "write", "value": 20 },
        ]);
        let answer = journey(&fake, steps, &|| true, &AtomicBool::new(false), PROMPT);
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
    let answer = journey(
        &fake,
        json!([stop()]),
        &|| true,
        &AtomicBool::new(false),
        PROMPT,
    );
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
    let answer = journey(
        &fake,
        json!([set(10)]),
        &|| true,
        &AtomicBool::new(false),
        PROMPT,
    );
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
        &AtomicBool::new(false),
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
    let answer = journey(
        &fake,
        json!([set(10)]),
        &|| true,
        &AtomicBool::new(false),
        SHORT,
    );
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
            }),
            ..Script::serving(pages())
        });
        let answer = journey(
            &fake,
            json!([step]),
            &|| true,
            &AtomicBool::new(false),
            PROMPT,
        );
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
    let answer = journey(
        &fake,
        json!([stop()]),
        &|| true,
        &AtomicBool::new(false),
        SHORT,
    );
    let id = &commands(&fake)[0]["id"];
    assert_eq!(
        answer["outcomes"][1],
        json!({ "page": "settings", "outcome": "timed-out", "commandId": id })
    );
    assert_eq!(answer["passed"], false);
}

/// Catches: a journey that goes on after the stream reported its command refused or timed out, while
/// the step waited for its later outcome or for its readback, or that drops the report's reason and
/// the value it read back.
#[test]
fn a_command_the_stream_reports_failed_ends_the_journey_with_that_outcome() {
    let mut stop_with_readback = stop();
    stop_with_readback["path"] = json!("Main.Run");
    let cases = [
        (set(10), "pending", "refused"),
        (stop(), "pending", "timed-out"),
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
            &AtomicBool::new(false),
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
    assert_eq!(follow_of(setpoint, &step(set(10))), Follow::Confirmation);
    assert_eq!(follow_of(button, &step(stop())), Follow::None);
    let remote = json!({
        "id": "start", "type": "pulse-button", "at": [0, 0], "size": [120, 40], "pulse": 0.5,
        "bind": { "target": { "path": "Owner:Main.Start", "direction": "write" } }
    });
    let pulse = step(json!({ "page": "settings", "object": "start", "action": "pulse" }));
    assert_eq!(follow_of(&remote, &pulse), Follow::PulseEnd);
    assert_eq!(
        follow_of(&json!({ "id": "odd" }), &step(stop())),
        Follow::Confirmation
    );
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
    let answer = journey(&fake, steps, &|| true, &AtomicBool::new(false), PROMPT);
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
            cancel.store(true, Ordering::SeqCst);
            CommandAnswer {
                status: 200,
                body: json!({ "id": command["id"], "outcome": "applied" }),
                then: Vec::new(),
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
        &AtomicBool::new(true),
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
    let answer = journey(&fake, steps, &|| true, &AtomicBool::new(false), PROMPT);
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

/// Catches: a journey that goes on after its named session expired or closed, or after its target
/// stopped being a local simulator, between two steps.
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
    let steps = json!([{ "page": "settings", "path": "Main.SP" }, { "page": "settings" }]);
    let answer = journey(&fake, steps, &live, &AtomicBool::new(false), PROMPT);
    assert_eq!(
        answer["outcomes"],
        json!([{ "page": "settings", "outcome": "observed", "path": "Main.SP", "value": 50.0 }])
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
    let answer = journey(&fake, steps, &|| true, &AtomicBool::new(false), SHORT);
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
    let answer = journey(&fake, steps, &|| true, &AtomicBool::new(false), SHORT);
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
    let answer = journey(&fake, steps, &|| true, &AtomicBool::new(false), PROMPT);
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
    let answer = journey(&fake, steps, &|| true, &AtomicBool::new(false), PROMPT);
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
