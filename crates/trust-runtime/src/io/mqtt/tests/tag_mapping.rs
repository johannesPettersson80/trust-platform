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
    assert_eq!(inputs, [0]);
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
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
