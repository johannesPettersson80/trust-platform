#[test]
fn write_only_tag_mappings_disable_raw_input_subscription_and_freshness_wait() {
    let text = r#"
broker = "127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" }
]
"#;
    let config = MqttIoConfig::from_params(&params(text)).expect("parse write-only mappings");
    assert!(
        config.subscribe_topics().is_empty(),
        "write-only mappings must not subscribe to trust/io/in"
    );

    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state: Arc::new(Mutex::new(MockState::default())),
        attempts: Arc::clone(&attempts),
        fail_first: false,
        always_fail: true,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(&params(text), factory)
        .expect("construct write-only MQTT driver");
    let mut inputs = [0xA5];
    driver
        .read_inputs(&mut inputs)
        .expect("write-only mappings must not require an input snapshot");
    assert_eq!(
        inputs, [0xA5],
        "write-only MQTT must not erase input bytes owned by another driver"
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
}

#[test]
fn read_only_tag_mappings_disable_raw_output_publication() {
    let config = MqttIoConfig::from_params(&params(
        r#"
broker = "127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Command", topic = "traffic/north/command", direction = "read" }
]
"#,
    ))
    .expect("parse read-only mappings");
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state: Arc::clone(&state),
        attempts: Arc::clone(&attempts),
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::new_sync_for_worker(config, factory);

    driver
        .write_outputs(&[0xA5])
        .expect("read-only mappings must not publish raw output");

    let guard = state.lock().unwrap_or_else(|error| error.into_inner());
    assert!(
        guard.published.is_empty(),
        "read-only mappings must not publish the process image to trust/io/out"
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
}

#[test]
fn explicit_empty_tag_mappings_disable_both_raw_directions() {
    let config = MqttIoConfig::from_params(&params(
        r#"
broker = "127.0.0.1:1883"
mappings = []
"#,
    ))
    .expect("parse explicitly empty mappings");

    assert!(
        config.subscribe_topics().is_empty(),
        "present-but-empty mappings must not subscribe to trust/io/in"
    );
    assert!(
        !config.output_enabled,
        "present-but-empty mappings must not publish to trust/io/out"
    );

    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state: Arc::clone(&state),
        attempts: Arc::clone(&attempts),
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::new_sync_for_worker(config, factory);
    let mut inputs = [0xA5];
    driver
        .read_inputs(&mut inputs)
        .expect("empty mappings must not wait for raw input");
    driver
        .write_outputs(&[0x5A])
        .expect("empty mappings must not publish raw output");

    assert_eq!(inputs, [0xA5]);
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
    assert!(
        state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .published
            .is_empty(),
        "empty mappings must not publish any MQTT payload"
    );
}

#[test]
fn explicit_empty_tag_mappings_keep_explicit_points_active_without_raw_fallback() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        ..MockState::default()
    }));
    let factory = Arc::new(MockFactory {
        state: Arc::clone(&state),
        attempts: Arc::new(AtomicUsize::new(0)),
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
mappings = []
output_points = [
  { topic = "traffic/north/green", image_offset = 0, image_bit = 0, data_type = "bool", payload_format = "text" }
]
"#,
        ),
        factory,
    )
    .expect("construct explicit point driver with empty tag mappings");

    driver
        .write_outputs(&[1])
        .expect("publish explicit point without raw fallback");

    assert_eq!(
        state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .published,
        [(SmolStr::new("traffic/north/green"), b"true".to_vec())]
    );
}

#[test]
fn mixed_tag_mappings_use_only_the_lowered_point_topics() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("traffic/north/command", b"true".to_vec())]),
        ..MockState::default()
    }));
    let factory = Arc::new(MockFactory {
        state: Arc::clone(&state),
        attempts: Arc::new(AtomicUsize::new(0)),
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Command", topic = "traffic/north/command", direction = "read" },
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" }
]
input_points = [
  { topic = "traffic/north/command", image_offset = 0, image_bit = 0, data_type = "bool", payload_format = "text" }
]
output_points = [
  { topic = "traffic/north/green", image_offset = 0, image_bit = 0, data_type = "bool", payload_format = "text" }
]
"#,
        ),
        factory,
    )
    .expect("construct mixed mapped MQTT driver");

    let mut inputs = [0u8];
    driver
        .read_inputs(&mut inputs)
        .expect("read mixed mapped input");
    driver
        .write_outputs(&[1])
        .expect("write mixed mapped output");

    assert_eq!(inputs, [1]);
    let published = &state
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .published;
    assert_eq!(
        published,
        &[(SmolStr::new("traffic/north/green"), b"true".to_vec())],
        "mixed mappings must not fall back to trust/io/out"
    );
}

#[test]
fn mqtt_config_rejects_unknown_top_level_parameter() {
    let error = expect_config_error(MqttIoDriver::from_params(&params(
        r#"
broker = "127.0.0.1:1883"
mapppings = []
"#,
    )));
    assert!(error.contains("unknown field `mapppings`"), "{error}");
}
