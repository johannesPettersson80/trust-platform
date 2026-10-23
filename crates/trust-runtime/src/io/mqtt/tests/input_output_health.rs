// Input-path conditions stay off the output path, and the reported health combines both paths
// (specification 32 section 8; specification 11, safe-state handoff confirmation). Included into
// `io::mqtt::tests`, which supplies `packet`, `params`, `MockState`, `MockFactory` and the imports.

/// A driver on `line/in` and `line/out` with a 60 s reconnect window, so a backoff started during
/// a test cannot expire inside it.
fn split_health_driver(state: Arc<Mutex<MockState>>, on_error: &str) -> MqttIoDriver {
    MqttIoDriver::from_params_with_factory(
        &params(&format!(
            r#"
broker = "127.0.0.1:1883"
topic_in = "line/in"
topic_out = "line/out"
on_error = "{on_error}"
reconnect_ms = 60000
"#
        )),
        Arc::new(MockFactory {
            state,
            attempts: Arc::new(AtomicUsize::new(0)),
            fail_first: false,
            always_fail: false,
        }),
    )
    .expect("construct mqtt driver")
}

fn published(state: &Arc<Mutex<MockState>>) -> Vec<(SmolStr, Vec<u8>)> {
    state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .published
        .clone()
}

/// Stale input on a connected link faults the reported health under `fault`, but it starts no
/// backoff and leaves the output path alone.
#[test]
fn fault_policy_stale_input_on_connected_link_leaves_output_path_healthy() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![7])]),
        ..MockState::default()
    }));
    let mut driver = split_health_driver(Arc::clone(&state), "fault");
    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("fresh payload");
    assert_eq!(inputs, [7]);
    driver.write_outputs(&[1]).expect("first publish");
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);

    // No new payload arrives inside the freshness window.
    let error = driver
        .read_inputs(&mut inputs)
        .expect_err("stale input must fault under the fault policy");
    assert!(matches!(error, RuntimeError::IoFreshness(_)), "{error}");
    assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);

    // The stale read holds no worker time, so the next write publishes at once. A backoff would
    // fail it inside the 60 s window.
    driver
        .write_outputs(&[2])
        .expect("write after stale input must publish without backoff");
    assert_eq!(
        published(&state),
        vec![
            (SmolStr::new("line/out"), vec![1]),
            (SmolStr::new("line/out"), vec![2]),
        ]
    );
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// The same stale input under `warn` degrades the reported health only; the output path stays `Ok`.
#[test]
fn warn_policy_stale_input_on_connected_link_degrades_reported_health_only() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![7])]),
        ..MockState::default()
    }));
    let mut driver = split_health_driver(Arc::clone(&state), "warn");
    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("fresh payload");
    driver.write_outputs(&[1]).expect("first publish");

    driver
        .read_inputs(&mut inputs)
        .expect("warn keeps the stale snapshot");
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);

    let health = driver.health();
    assert!(
        matches!(&health, IoDriverHealth::Degraded { error } if error.contains("not fresh")),
        "{health:?}"
    );

    driver
        .write_outputs(&[2])
        .expect("write after stale input must publish");
    assert_eq!(
        published(&state),
        vec![
            (SmolStr::new("line/out"), vec![1]),
            (SmolStr::new("line/out"), vec![2]),
        ]
    );
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// A lost link still starts backoff, and a write inside the backoff window does not connect again.
#[test]
fn lost_link_still_starts_backoff_and_marks_the_output_path() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
topic_out = "line/out"
on_error = "fault"
reconnect_ms = 60000
"#,
        ),
        Arc::new(MockFactory {
            state: Arc::new(Mutex::new(MockState::default())),
            attempts: Arc::clone(&attempts),
            fail_first: false,
            always_fail: true,
        }),
    )
    .expect("construct mqtt driver");

    let first = driver
        .write_outputs(&[1])
        .expect_err("a failed connect must fault the write");
    assert!(first.to_string().contains("connect failed"), "{first}");

    let second = driver
        .write_outputs(&[2])
        .expect_err("a write inside the backoff window must not reconnect");
    let second_detail = second.to_string();
    assert!(
        second_detail.contains("mqtt reconnect backoff active"),
        "{second_detail}"
    );
    assert!(matches!(
        driver.output_health(),
        IoDriverHealth::Faulted { .. }
    ));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

/// A typed-output preflight failure on a connected link sets health per policy but starts no
/// backoff, so the next valid write publishes at once.
#[test]
fn typed_output_preflight_failure_on_connected_link_does_not_start_backoff() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        ..MockState::default()
    }));
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
on_error = "fault"
reconnect_ms = 60000

[[output_points]]
topic = "line/out"
image_offset = 4
data_type = "u16"
payload_format = "binary_le"
"#,
        ),
        Arc::new(MockFactory {
            state: Arc::clone(&state),
            attempts: Arc::new(AtomicUsize::new(0)),
            fail_first: false,
            always_fail: false,
        }),
    )
    .expect("construct mqtt driver");

    let error = driver
        .write_outputs(&[1])
        .expect_err("an image too short for the typed point must fail the write");
    let detail = error.to_string();
    assert!(
        detail.contains("image range 4..6 exceeds process image size 1"),
        "{detail}"
    );
    assert!(matches!(
        driver.output_health(),
        IoDriverHealth::Faulted { .. }
    ));

    driver
        .write_outputs(&[1, 2, 3, 4, 5, 6])
        .expect("a valid write after a preflight failure must publish at once");
    assert_eq!(
        published(&state),
        vec![(SmolStr::new("line/out"), vec![5, 6])]
    );
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// An input-only driver publishes nothing, so its output path is always healthy: a lost link is an
/// input condition only, and a safe-state handoff through it is confirmed.
#[test]
fn input_only_driver_output_path_stays_healthy_while_its_link_is_down() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", b"7".to_vec())]),
        ..MockState::default()
    }));
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
on_error = "fault"
reconnect_ms = 60000
mappings = [
  { tag = "MainInstance.Level", topic = "line/in", direction = "read" }
]

[[input_points]]
topic = "line/in"
image_offset = 0
data_type = "u16"
"#,
        ),
        Arc::new(MockFactory {
            state: Arc::clone(&state),
            attempts: Arc::new(AtomicUsize::new(0)),
            fail_first: false,
            always_fail: false,
        }),
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 2];
    driver.read_inputs(&mut inputs).expect("fresh payload");
    assert_eq!(inputs, [7, 0]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);

    // The link drops. The input path faults; the output path stays Ok because this driver has
    // no outputs to deliver.
    state.lock().unwrap_or_else(|e| e.into_inner()).connected = false;
    let error = driver
        .read_inputs(&mut inputs)
        .expect_err("a lost link must fault the read under the fault policy");
    assert!(matches!(error, RuntimeError::IoFreshness(_)), "{error}");
    assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);

    driver
        .write_outputs(&[0])
        .expect("an input-only driver writes nothing");
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// After a stale read, the safe-state handoff is confirmed when the safe output publishes, because
/// the stale input never reached the output path.
#[test]
fn safe_state_after_stale_input_is_confirmed_when_the_safe_output_publishes() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![7])]),
        ..MockState::default()
    }));
    let mut driver = split_health_driver(Arc::clone(&state), "fault");
    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("fresh payload");
    assert!(
        driver.read_inputs(&mut inputs).is_err(),
        "stale input must fault under the fault policy"
    );

    let mut builder = crate::RuntimeBuilder::new();
    builder.io_mut().resize(0, 1, 0);
    let mut runtime = builder.build().expect("install MQTT output image");
    let mut safe_state = crate::io::IoSafeState::default();
    safe_state.outputs.push((
        crate::io::IoAddress::parse("%QB0").expect("safe-state output byte"),
        crate::value::Value::Byte(0x5a),
    ));
    runtime.set_io_safe_state(safe_state);
    runtime.add_io_driver("mqtt-stale-input", Box::new(driver));

    runtime
        .apply_io_safe_state()
        .expect("stale input must not unconfirm the safe state");
    assert_eq!(
        published(&state),
        vec![(SmolStr::new("line/out"), vec![0x5a])]
    );
}

/// Without a worker, output health is `Ok` while the link is connected, whatever the reported
/// health says, and it becomes the driver's health once the link is lost.
#[test]
fn sync_driver_output_health_follows_the_link_not_the_reported_health() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![7])]),
        ..MockState::default()
    }));
    let config = MqttIoConfig::from_params(&params(
        r#"
broker = "127.0.0.1:1883"
topic_in = "line/in"
on_error = "warn"
"#,
    ))
    .expect("parse MQTT config");
    let mut driver = MqttIoDriver::new_sync_for_worker(
        config,
        Arc::new(MockFactory {
            state: Arc::clone(&state),
            attempts: Arc::new(AtomicUsize::new(0)),
            fail_first: false,
            always_fail: false,
        }),
    );

    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("fresh payload");
    driver
        .read_inputs(&mut inputs)
        .expect("warn keeps the stale snapshot");
    assert!(matches!(driver.health(), IoDriverHealth::Degraded { .. }));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);

    state.lock().unwrap_or_else(|e| e.into_inner()).connected = false;
    driver
        .read_inputs(&mut inputs)
        .expect("warn degrades a lost link");
    assert!(matches!(
        driver.output_health(),
        IoDriverHealth::Degraded { .. }
    ));
}

/// A stale read returns at once: the freshness check takes what has arrived and does not wait for
/// more (specification 32 section 8), so the fault is reported well inside the scan bound.
#[test]
fn stale_read_under_fault_returns_at_once_with_not_fresh() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![7])]),
        ..MockState::default()
    }));
    let mut driver = split_health_driver(Arc::clone(&state), "fault");
    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("fresh payload");

    let started = Instant::now();
    let error = driver
        .read_inputs(&mut inputs)
        .expect_err("stale input must fault under the fault policy");
    let elapsed = started.elapsed();

    assert!(
        elapsed < MQTT_WORKER_SCAN_WAIT_BOUND / 2,
        "a stale read must not wait for a payload, elapsed={elapsed:?}"
    );
    assert!(matches!(error, RuntimeError::IoFreshness(_)), "{error}");
    assert!(
        error.to_string().contains("mqtt input not fresh"),
        "{error}"
    );
}

/// A publish after a stale read clears only the output path. The reported health stays faulted with
/// the freshness detail until a fresh read (specification 32 section 8).
#[test]
fn reported_health_keeps_stale_input_after_a_publish_until_a_fresh_read() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![7])]),
        ..MockState::default()
    }));
    let mut driver = split_health_driver(Arc::clone(&state), "fault");
    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("fresh payload");
    assert!(
        driver.read_inputs(&mut inputs).is_err(),
        "stale input must fault under the fault policy"
    );
    driver
        .write_outputs(&[1])
        .expect("publish after stale input");

    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    let health = driver.health();
    assert!(
        matches!(&health, IoDriverHealth::Faulted { error } if error.contains("not fresh")),
        "{health:?}"
    );

    state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .payloads
        .push_back(packet("line/in", vec![8]));
    driver.read_inputs(&mut inputs).expect("fresh payload");
    assert_eq!(inputs, [8]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

/// An output handoff issued right after a stale read completes at once: the stale read holds no
/// worker time, so the handoff is never reported as pending.
#[test]
fn handoff_right_after_a_stale_read_is_not_held_as_pending() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![7])]),
        ..MockState::default()
    }));
    let mut driver = split_health_driver(Arc::clone(&state), "fault");
    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("fresh payload");
    assert!(
        driver.read_inputs(&mut inputs).is_err(),
        "stale input must fault under the fault policy"
    );

    let started = Instant::now();
    driver
        .write_outputs(&[2])
        .expect("a handoff after a stale read must not report pending");
    let elapsed = started.elapsed();

    assert!(
        elapsed < MQTT_WORKER_SCAN_WAIT_BOUND,
        "a handoff after a stale read must not wait out the bound, elapsed={elapsed:?}"
    );
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    assert_eq!(published(&state), vec![(SmolStr::new("line/out"), vec![2])]);
}

/// A link loss seen by a read is cleared by one fresh read once the link is back, with no publish.
/// A typed-output preflight failure is a publish condition: reads leave it reported until a
/// successful publish clears it (specification 32 section 8).
#[test]
fn fresh_read_clears_a_recovered_link_but_a_preflight_failure_waits_for_a_publish() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![1])]),
        ..MockState::default()
    }));
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
topic_in = "line/in"
on_error = "fault"
reconnect_ms = 1

[[output_points]]
topic = "line/out"
image_offset = 4
data_type = "u16"
payload_format = "binary_le"
"#,
        ),
        Arc::new(MockFactory {
            state: Arc::clone(&state),
            attempts: Arc::new(AtomicUsize::new(0)),
            fail_first: false,
            always_fail: false,
        }),
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 1];
    driver.read_inputs(&mut inputs).expect("fresh payload");
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);

    // The link drops, and a read sees it.
    state.lock().unwrap_or_else(|e| e.into_inner()).connected = false;
    let error = driver
        .read_inputs(&mut inputs)
        .expect_err("a lost link must fault the read under the fault policy");
    assert!(matches!(error, RuntimeError::IoFreshness(_)), "{error}");
    assert!(matches!(
        driver.output_health(),
        IoDriverHealth::Faulted { .. }
    ));

    // The link recovers. The 1 ms reconnect window has passed, so one fresh read observes the link
    // up and clears the link condition, with no publish.
    thread::sleep(StdDuration::from_millis(5));
    {
        let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
        guard.connected = true;
        guard.payloads.push_back(packet("line/in", vec![2]));
    }
    driver
        .read_inputs(&mut inputs)
        .expect("fresh payload after recovery");
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);

    // A typed-output preflight failure is a publish condition, so the reads that follow leave it
    // reported.
    let error = driver
        .write_outputs(&[1])
        .expect_err("an image too short for the typed point must fail the write");
    let detail = error.to_string();
    assert!(
        detail.contains("image range 4..6 exceeds process image size 1"),
        "{detail}"
    );
    assert!(matches!(
        driver.output_health(),
        IoDriverHealth::Faulted { .. }
    ));

    state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .payloads
        .push_back(packet("line/in", vec![3]));
    driver
        .read_inputs(&mut inputs)
        .expect("fresh payload with no publish");
    let reported = match driver.output_health() {
        IoDriverHealth::Faulted { error } => error.to_string(),
        other => panic!("a read must not clear the preflight failure: {other:?}"),
    };
    assert!(
        reported.contains("image range 4..6 exceeds process image size 1"),
        "{reported}"
    );

    // Only a successful publish clears it.
    driver
        .write_outputs(&[1, 2, 3, 4, 5, 6])
        .expect("a valid write must publish");
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    assert_eq!(
        published(&state),
        vec![(SmolStr::new("line/out"), vec![5, 6])]
    );
}
