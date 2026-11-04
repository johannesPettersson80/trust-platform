//! How long a simulator journey's step waits for its command's later outcome, and how a cancel or
//! a closed session ends a step (specification 43, AT-08), against the fake simulator HMI and with
//! the helpers of the journey tests.

use super::*;

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
