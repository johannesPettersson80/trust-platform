use super::*;
use std::collections::VecDeque;
use std::sync::atomic::AtomicUsize;

const MQTT_CI_TIMING_SLACK: StdDuration = StdDuration::from_millis(100);

#[derive(Default)]
struct MockState {
    connected: bool,
    last_error: Option<SmolStr>,
    payloads: VecDeque<MqttInboundPayload>,
    published: Vec<(SmolStr, Vec<u8>)>,
    fail_publish_once: bool,
}

struct MockSession {
    state: Arc<Mutex<MockState>>,
}

impl MqttSession for MockSession {
    fn is_connected(&self) -> bool {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.connected
    }

    fn take_payloads(&mut self) -> Vec<MqttInboundPayload> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.payloads.drain(..).collect()
    }

    fn publish(&mut self, topic: &str, payload: &[u8]) -> Result<(), RuntimeError> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if guard.fail_publish_once {
            guard.fail_publish_once = false;
            guard.last_error = Some(SmolStr::new("publish failed"));
            return Err(RuntimeError::IoDriver("publish failed".into()));
        }
        guard.published.push((SmolStr::new(topic), payload.to_vec()));
        Ok(())
    }

    fn last_error(&self) -> Option<SmolStr> {
        let guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.last_error.clone()
    }
}

struct MockFactory {
    state: Arc<Mutex<MockState>>,
    attempts: Arc<AtomicUsize>,
    fail_first: bool,
    always_fail: bool,
}

impl MqttSessionFactory for MockFactory {
    fn connect(&self, _config: &MqttIoConfig) -> Result<Box<dyn MqttSession>, RuntimeError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if self.always_fail || (self.fail_first && attempt == 0) {
            return Err(RuntimeError::IoDriver("connect failed".into()));
        }
        Ok(Box::new(MockSession {
            state: Arc::clone(&self.state),
        }))
    }
}

fn params(text: &str) -> toml::Value {
    toml::from_str(text).expect("parse toml params")
}

fn packet(topic: &str, payload: impl Into<Vec<u8>>) -> MqttInboundPayload {
    MqttInboundPayload {
        topic: SmolStr::new(topic),
        payload: payload.into(),
    }
}

fn expect_config_error(result: Result<MqttIoDriver, RuntimeError>) -> String {
    match result {
        Ok(_) => panic!("expected MQTT config validation failure"),
        Err(err) => err.to_string(),
    }
}

fn tls_fixture_path(name: &str) -> String {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tls")
        .join(name)
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn contract_test_reads_and_writes_payloads() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![1, 0, 1])]),
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state: Arc::clone(&state),
        attempts: Arc::clone(&attempts),
        fail_first: false,
        always_fail: false,
    });

    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
topic_in = "line/in"
topic_out = "line/out"
"#,
        ),
        factory.clone(),
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 4];
    driver.read_inputs(&mut inputs).expect("read inputs");
    assert_eq!(&inputs[..3], &[1, 0, 1]);
    driver.write_outputs(&[9, 8, 7]).expect("write outputs");
    assert!(matches!(driver.health(), IoDriverHealth::Ok));

    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(
        guard.published,
        vec![(SmolStr::new("line/out"), vec![9, 8, 7])]
    );
}

#[test]
fn typed_point_map_reads_json_text_and_binary_payloads() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([
            packet("line/in/ready", b"true".to_vec()),
            packet("line/in/temp", b"100".to_vec()),
            packet("line/in/count", 42u32.to_be_bytes().to_vec()),
        ]),
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state: Arc::clone(&state),
        attempts,
        fail_first: false,
        always_fail: false,
    });

    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"

[[input_points]]
topic = "line/in/ready"
image_offset = 0
image_bit = 2
data_type = "bool"
payload_format = "json"

[[input_points]]
topic = "line/in/temp"
image_offset = 1
data_type = "i16"
payload_format = "text"
scale = 0.5
offset = -10.0

[[input_points]]
topic = "line/in/count"
image_offset = 3
data_type = "u32"
payload_format = "binary_be"
"#,
        ),
        factory,
    )
    .expect("construct mapped mqtt driver");

    let mut inputs = [0u8; 7];
    driver.read_inputs(&mut inputs).expect("read mapped inputs");

    assert_eq!(inputs[0] & 0b0000_0100, 0b0000_0100);
    assert_eq!(i16::from_le_bytes([inputs[1], inputs[2]]), 40);
    assert_eq!(
        u32::from_le_bytes([inputs[3], inputs[4], inputs[5], inputs[6]]),
        42
    );
    assert!(matches!(driver.health(), IoDriverHealth::Ok));
}

#[test]
fn typed_point_map_writes_json_text_and_binary_payloads() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state: Arc::clone(&state),
        attempts,
        fail_first: false,
        always_fail: false,
    });

    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"

[[output_points]]
topic = "line/out/enable"
image_offset = 0
image_bit = 1
data_type = "bool"
payload_format = "text"

[[output_points]]
topic = "line/out/speed"
image_offset = 1
data_type = "u16"
payload_format = "json"
scale = 0.5
offset = 10.0

[[output_points]]
topic = "line/out/count"
image_offset = 3
data_type = "i32"
payload_format = "binary_be"
"#,
        ),
        factory,
    )
    .expect("construct mapped mqtt driver");

    let mut outputs = [0u8; 7];
    outputs[0] = 0b0000_0010;
    outputs[1..3].copy_from_slice(&60u16.to_le_bytes());
    outputs[3..7].copy_from_slice(&(-42i32).to_le_bytes());
    driver.write_outputs(&outputs).expect("write mapped outputs");

    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(
        guard.published,
        vec![
            (SmolStr::new("line/out/enable"), b"true".to_vec()),
            (SmolStr::new("line/out/speed"), b"100".to_vec()),
            (
                SmolStr::new("line/out/count"),
                (-42i32).to_be_bytes().to_vec()
            ),
        ]
    );
}

#[test]
fn typed_point_map_rejects_invalid_config() {
    let missing_type = MqttIoDriver::from_params(&params(
        r#"
broker = "127.0.0.1:1883"

[[input_points]]
topic = "line/in/missing"
image_offset = 0
"#,
    ));
    let error = expect_config_error(missing_type);
    assert!(error.contains("input_points.data_type"));

    let numeric_bit = MqttIoDriver::from_params(&params(
        r#"
broker = "127.0.0.1:1883"

[[output_points]]
topic = "line/out/numeric"
image_offset = 0
image_bit = 1
data_type = "u16"
"#,
    ));
    let error = expect_config_error(numeric_bit);
    assert!(error.contains("numeric point maps"));

    let zero_scale = MqttIoDriver::from_params(&params(
        r#"
broker = "127.0.0.1:1883"

[[output_points]]
topic = "line/out/scaled"
image_offset = 0
data_type = "u16"
scale = 0.0
"#,
    ));
    let error = expect_config_error(zero_scale);
    assert!(error.contains("scale"));
}

#[test]
fn on_error_warn_connect_failure_degrades_without_runtime_error() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: true,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
reconnect_ms = 1
on_error = "warn"
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 1];
    driver
        .read_inputs(&mut inputs)
        .expect("warn policy should degrade health without faulting the scan");
    assert!(matches!(driver.health(), IoDriverHealth::Degraded { .. }));
}

#[test]
fn on_error_ignore_publish_failure_degrades_without_runtime_error() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        fail_publish_once: true,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
on_error = "ignore"
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    driver
        .write_outputs(&[1, 2, 3])
        .expect("ignore policy should degrade health without faulting the scan");
    assert!(matches!(driver.health(), IoDriverHealth::Degraded { .. }));
}

#[test]
fn sparkplug_profile_configures_namespace_topics_and_last_will() {
    let config = MqttIoConfig::from_params(&params(
        r#"
broker = "127.0.0.1:1883"
client_id = "sparkplug-test"

[sparkplug]
enabled = true
group_id = "trust"
edge_node_id = "runtime-a"
birth_death_seq = 7

[[output_points]]
topic = "legacy/out/speed"
metric_name = "drive/speed"
image_offset = 0
data_type = "u16"
payload_format = "json"
"#,
    ))
    .expect("parse sparkplug config");

    let sparkplug = config.sparkplug.as_ref().expect("sparkplug config");
    assert_eq!(sparkplug.nbirth_topic(), "spBv1.0/trust/NBIRTH/runtime-a");
    assert_eq!(sparkplug.ndata_topic(), "spBv1.0/trust/NDATA/runtime-a");
    assert_eq!(sparkplug.ndeath_topic(), "spBv1.0/trust/NDEATH/runtime-a");

    let options = build_mqtt_options(&config).expect("build mqtt options");
    assert!(!options.clean_session());
    let will = options.last_will().expect("sparkplug NDEATH will");
    assert_eq!(will.topic, "spBv1.0/trust/NDEATH/runtime-a");
    assert_eq!(will.qos, QoS::AtMostOnce);
    assert!(!will.retain);
    assert!(
        will.message.windows(b"bdSeq".len()).any(|window| window == b"bdSeq"),
        "NDEATH payload should contain bdSeq metric name"
    );
}

#[test]
fn sparkplug_payload_encoding_matches_tahu_wire_shape_for_scalar_metrics() {
    let config = SparkplugConfig {
        namespace: SmolStr::new("spBv1.0"),
        spec_version: SmolStr::new("3.0.0"),
        group_id: SmolStr::new("trust"),
        edge_node_id: SmolStr::new("runtime-a"),
        birth_death_seq: 3,
    };
    let point = MqttOutputPoint::from_toml(MqttPointToml {
        topic: "legacy/out/speed".to_owned(),
        image_offset: 0,
        image_bit: None,
        data_type: Some("u16".to_owned()),
        payload_format: Some("json".to_owned()),
        metric_name: Some("drive/speed".to_owned()),
        scale: Some(0.5),
        offset: Some(10.0),
    })
    .expect("point config");
    let mut image = [0u8; 2];
    image.copy_from_slice(&60u16.to_le_bytes());

    let payload = encode_sparkplug_ndata_at(&[point], &image, 5, 123).expect("encode NDATA");

    assert_eq!(&payload[..2], &[0x08, 0x7b], "payload timestamp field");
    assert!(
        payload.ends_with(&[0x18, 0x05]),
        "payload seq field should be encoded as field 3"
    );
    assert!(
        payload
            .windows(b"drive/speed".len())
            .any(|window| window == b"drive/speed"),
        "payload should contain metric name"
    );
    assert!(
        payload
            .windows(2)
            .any(|window| window == [0x20, SPARKPLUG_DATATYPE_UINT16 as u8].as_slice()),
        "metric datatype should be UInt16"
    );
    assert!(
        payload
            .windows(2)
            .any(|window| window == [0x50, 0x64].as_slice()),
        "scaled value should encode raw 100 in int_value field"
    );

    let birth = encode_sparkplug_nbirth_at(&config, &[], 0, 123);
    assert!(
        birth.windows(b"bdSeq".len()).any(|window| window == b"bdSeq"),
        "NBIRTH should include bdSeq"
    );
}

#[test]
fn sparkplug_profile_publishes_birth_then_data_payload() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state: Arc::clone(&state),
        attempts,
        fail_first: false,
        always_fail: false,
    });

    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
client_id = "sparkplug-runtime"

[sparkplug]
enabled = true
group_id = "trust"
edge_node_id = "runtime-a"
birth_death_seq = 11

[[output_points]]
topic = "legacy/out/enabled"
metric_name = "drive/enabled"
image_offset = 0
image_bit = 1
data_type = "bool"
payload_format = "json"

[[output_points]]
topic = "legacy/out/speed"
metric_name = "drive/speed"
image_offset = 1
data_type = "u16"
payload_format = "json"
scale = 0.5
offset = 10.0
"#,
        ),
        factory,
    )
    .expect("construct sparkplug mqtt driver");

    let mut outputs = [0u8; 3];
    outputs[0] = 0b0000_0010;
    outputs[1..3].copy_from_slice(&60u16.to_le_bytes());
    driver
        .write_outputs(&outputs)
        .expect("write sparkplug outputs");

    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(guard.published.len(), 2);
    assert_eq!(guard.published[0].0, "spBv1.0/trust/NBIRTH/runtime-a");
    assert_eq!(guard.published[1].0, "spBv1.0/trust/NDATA/runtime-a");
    assert!(
        guard.published[0]
            .1
            .windows(b"bdSeq".len())
            .any(|window| window == b"bdSeq"),
        "NBIRTH should include bdSeq"
    );
    assert!(
        guard.published[1]
            .1
            .windows(b"drive/enabled".len())
            .any(|window| window == b"drive/enabled"),
        "NDATA should include first metric"
    );
    assert!(
        guard.published[1]
            .1
            .windows(b"drive/speed".len())
            .any(|window| window == b"drive/speed"),
        "NDATA should include second metric"
    );
}

#[test]
fn sparkplug_profile_rejects_unsupported_shapes() {
    let missing_identity = MqttIoDriver::from_params(&params(
        r#"
broker = "127.0.0.1:1883"

[sparkplug]
enabled = true
edge_node_id = "runtime-a"

[[output_points]]
topic = "legacy/out/speed"
image_offset = 0
data_type = "u16"
"#,
    ));
    let error = expect_config_error(missing_identity);
    assert!(error.contains("group_id"));

    let inputs = MqttIoDriver::from_params(&params(
        r#"
broker = "127.0.0.1:1883"

[sparkplug]
enabled = true
group_id = "trust"
edge_node_id = "runtime-a"

[[input_points]]
topic = "legacy/in/ready"
image_offset = 0
image_bit = 0
data_type = "bool"

[[output_points]]
topic = "legacy/out/speed"
image_offset = 1
data_type = "u16"
"#,
    ));
    let error = expect_config_error(inputs);
    assert!(error.contains("outbound output_points only"));

    let no_outputs = MqttIoDriver::from_params(&params(
        r#"
broker = "127.0.0.1:1883"

[sparkplug]
enabled = true
group_id = "trust"
edge_node_id = "runtime-a"
"#,
    ));
    let error = expect_config_error(no_outputs);
    assert!(error.contains("requires at least one output_point"));
}

#[test]
fn reconnection_test_retries_after_connect_failure() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts: Arc::clone(&attempts),
        fail_first: true,
        always_fail: false,
    });

    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
reconnect_ms = 1
"#,
        ),
        factory.clone(),
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 1];
    driver
        .read_inputs(&mut inputs)
        .expect_err("first read should report connect failure");
    assert!(matches!(driver.health(), IoDriverHealth::Faulted { .. }));
    thread::sleep(StdDuration::from_millis(2));
    {
        let mut guard = factory.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.payloads.push_back(packet("trust/io/in", vec![1]));
    }
    driver.read_inputs(&mut inputs).expect("second read");
    assert!(
        attempts.load(Ordering::SeqCst) >= 2,
        "expected at least two connect attempts"
    );
    assert!(matches!(driver.health(), IoDriverHealth::Ok));
}

#[test]
fn security_test_rejects_remote_insecure_broker() {
    let result = MqttIoDriver::from_params(&params(
        r#"
broker = "10.10.0.9:1883"
"#,
    ));
    assert!(result.is_err(), "expected security validation failure");
    let error = match result {
        Ok(_) => panic!("expected insecure remote broker validation failure"),
        Err(err) => err.to_string(),
    };
    assert!(error.contains("allow_insecure_remote"));

    let ok = MqttIoDriver::from_params(&params(
        r#"
broker = "10.10.0.9:1883"
allow_insecure_remote = true
"#,
    ));
    assert!(ok.is_ok(), "explicit insecure override should be allowed");
}

#[test]
fn security_test_allows_remote_broker_when_tls_configured() {
    let ca_path = tls_fixture_path("server-cert.pem");
    let result = MqttIoDriver::from_params(&params(&format!(
        r#"
broker = "mqtt.example.test:8883"
tls = true
tls_ca_path = "{ca_path}"
"#
    )));

    assert!(
        result.is_ok(),
        "remote MQTT with explicit TLS trust should be accepted"
    );
}

#[test]
fn security_test_mqtts_scheme_implies_tls() {
    let ca_path = tls_fixture_path("server-cert.pem");
    let config = MqttIoConfig::from_params(&params(&format!(
        r#"
broker = "mqtts://mqtt.example.test:8883"
tls_ca_path = "{ca_path}"
"#
    )))
    .expect("mqtts broker should enable TLS");

    assert!(config.tls.is_some());
    assert_eq!(config.endpoint.host.as_str(), "mqtt.example.test");
    assert_eq!(config.endpoint.port, 8883);
}

#[test]
fn security_test_rejects_tls_without_ca_path() {
    let result = MqttIoDriver::from_params(&params(
        r#"
broker = "mqtt.example.test:8883"
tls = true
"#,
    ));
    assert!(result.is_err(), "TLS without trust roots must be rejected");
    let error = match result {
        Ok(_) => panic!("expected TLS without trust roots to fail"),
        Err(err) => err.to_string(),
    };
    assert!(error.contains("tls_ca_path"));
}

#[test]
fn security_test_rejects_tls_fields_when_tls_disabled() {
    let ca_path = tls_fixture_path("server-cert.pem");
    let result = MqttIoDriver::from_params(&params(&format!(
        r#"
broker = "127.0.0.1:1883"
tls = false
tls_ca_path = "{ca_path}"
"#
    )));
    assert!(
        result.is_err(),
        "TLS file paths with tls=false must be rejected"
    );
    let error = match result {
        Ok(_) => panic!("expected TLS paths with tls=false to fail"),
        Err(err) => err.to_string(),
    };
    assert!(error.contains("tls=true"));
}

#[test]
fn security_test_allows_empty_alpn_list_when_tls_disabled() {
    let result = MqttIoDriver::from_params(&params(
        r#"
broker = "127.0.0.1:1883"
tls = false
tls_alpn = []
"#,
    ));

    assert!(
        result.is_ok(),
        "empty TLS ALPN list is the schema default and must not block non-TLS MQTT"
    );
}

#[test]
fn security_test_rejects_partial_mtls_pair() {
    let ca_path = tls_fixture_path("server-cert.pem");
    let cert_path = tls_fixture_path("server-cert.pem");
    let result = MqttIoDriver::from_params(&params(&format!(
        r#"
broker = "mqtt.example.test:8883"
tls = true
tls_ca_path = "{ca_path}"
tls_client_cert_path = "{cert_path}"
"#
    )));
    assert!(result.is_err(), "mTLS requires cert and key together");
    let error = match result {
        Ok(_) => panic!("expected partial mTLS config to fail"),
        Err(err) => err.to_string(),
    };
    assert!(error.contains("mTLS"));
}

#[test]
fn tls_transport_test_builds_rumqttc_tls_transport() {
    let ca_path = tls_fixture_path("server-cert.pem");
    let cert_path = tls_fixture_path("server-cert.pem");
    let key_path = tls_fixture_path("server-key.pem");
    let config = MqttIoConfig::from_params(&params(&format!(
        r#"
broker = "mqtt.example.test:8883"
client_id = "tls-test"
keep_alive_s = 11
tls = true
tls_ca_path = "{ca_path}"
tls_client_cert_path = "{cert_path}"
tls_client_key_path = "{key_path}"
tls_alpn = ["mqtt"]
"#
    )))
    .expect("parse TLS config");

    let options = build_mqtt_options(&config).expect("build TLS mqtt options");
    assert_eq!(options.keep_alive(), StdDuration::from_secs(11));
    match options.transport() {
        Transport::Tls(TlsConfiguration::NativeConnector(_)) => {
            assert_eq!(
                config.tls.as_ref().expect("tls config").ca,
                std::fs::read(ca_path).expect("read ca")
            );
            let (cert, key) = config
                .tls
                .as_ref()
                .expect("tls config")
                .client_auth
                .as_ref()
                .expect("mTLS client auth");
            assert_eq!(cert, &std::fs::read(cert_path).expect("read cert"));
            assert_eq!(key, &std::fs::read(key_path).expect("read key"));
            assert_eq!(
                config.tls.as_ref().expect("tls config").alpn,
                Some(vec!["mqtt".to_owned()])
            );
        }
        _ => panic!("expected rumqttc TLS transport"),
    }
}

#[test]
fn cycle_impact_test_driver_calls_are_non_blocking_without_session() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: true,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
reconnect_ms = 1
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 8];
    let mut max_elapsed = StdDuration::ZERO;
    for _ in 0..400 {
        let started = Instant::now();
        let _ = driver.read_inputs(&mut inputs);
        max_elapsed = max_elapsed.max(started.elapsed());

        let started = Instant::now();
        let _ = driver.write_outputs(&[1, 2, 3, 4]);
        max_elapsed = max_elapsed.max(started.elapsed());
    }
    assert!(
        max_elapsed < MQTT_WORKER_SCAN_WAIT_BOUND + MQTT_CI_TIMING_SLACK,
        "each driver call should stay within the scan-thread bound, max_elapsed={max_elapsed:?}"
    );
}

#[test]
fn mqtt_read_inputs_returns_within_scan_bound_without_fresh_payload() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
on_error = "warn"
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 8];
    let started = Instant::now();
    driver
        .read_inputs(&mut inputs)
        .expect("warn policy should degrade without faulting");
    let elapsed = started.elapsed();

    assert!(
        elapsed < MQTT_WORKER_SCAN_WAIT_BOUND + MQTT_CI_TIMING_SLACK,
        "MQTT read_inputs must return within the scan-thread bound when no fresh broker payload is available, elapsed={elapsed:?}"
    );
    assert!(matches!(driver.health(), IoDriverHealth::Degraded { .. }));
}

#[test]
fn mqtt_write_outputs_returns_within_scan_bound_while_session_connecting() {
    let state = Arc::new(Mutex::new(MockState {
        connected: false,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
on_error = "warn"
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let started = Instant::now();
    driver
        .write_outputs(&[1, 2, 3])
        .expect("warn policy should degrade without faulting");
    let elapsed = started.elapsed();

    assert!(
        elapsed < MQTT_WORKER_SCAN_WAIT_BOUND + MQTT_CI_TIMING_SLACK,
        "MQTT write_outputs must return within the scan-thread bound while session readiness is pending, elapsed={elapsed:?}"
    );
    assert!(matches!(driver.health(), IoDriverHealth::Degraded { .. }));
}

#[test]
fn mqtt_stale_snapshot_is_returned_without_fresh_payload() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        payloads: VecDeque::from([packet("line/in", vec![7, 8, 9])]),
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
topic_in = "line/in"
on_error = "warn"
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let mut first = [0u8; 4];
    driver.read_inputs(&mut first).expect("initial payload");
    assert_eq!(&first[..3], &[7, 8, 9]);

    let mut stale = [0u8; 4];
    let started = Instant::now();
    driver
        .read_inputs(&mut stale)
        .expect("warn policy should return stale snapshot without fresh payload");
    let elapsed = started.elapsed();

    assert!(
        elapsed < StdDuration::from_millis(50),
        "stale MQTT snapshot must be returned without waiting for broker freshness, elapsed={elapsed:?}"
    );
    assert_eq!(
        &stale[..3],
        &[7, 8, 9],
        "worker snapshot should preserve the last known MQTT input image"
    );
    assert!(matches!(driver.health(), IoDriverHealth::Degraded { .. }));
}

#[test]
fn mqtt_no_fresh_payload_follows_on_error_policy() {
    for (policy, should_error) in [("fault", true), ("warn", false), ("ignore", false)] {
        let state = Arc::new(Mutex::new(MockState {
            connected: true,
            ..MockState::default()
        }));
        let attempts = Arc::new(AtomicUsize::new(0));
        let factory = Arc::new(MockFactory {
            state,
            attempts,
            fail_first: false,
            always_fail: false,
        });
        let mut driver = MqttIoDriver::from_params_with_factory(
            &params(&format!(
                r#"
broker = "127.0.0.1:1883"
on_error = "{policy}"
"#
            )),
            factory,
        )
        .expect("construct mqtt driver");
        let mut inputs = [0u8; 4];

        let started = Instant::now();
        let result = driver.read_inputs(&mut inputs);
        let elapsed = started.elapsed();

        assert!(
            elapsed < StdDuration::from_millis(50),
            "{policy} no-fresh-payload handling must be scan-bounded, elapsed={elapsed:?}"
        );
        assert_eq!(
            result.is_err(),
            should_error,
            "{policy} result should match on_error policy for no-fresh-payload"
        );
        match (policy, driver.health()) {
            ("fault", IoDriverHealth::Faulted { .. }) => {}
            ("warn" | "ignore", IoDriverHealth::Degraded { .. }) => {}
            (_, other) => panic!("unexpected {policy} health for no-fresh-payload: {other:?}"),
        }
    }
}

#[test]
fn mqtt_reconnect_backoff_is_bounded_and_non_spinning() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts: Arc::clone(&attempts),
        fail_first: false,
        always_fail: true,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
reconnect_ms = 100
on_error = "warn"
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 1];
    for _ in 0..50 {
        driver
            .read_inputs(&mut inputs)
            .expect("warn policy should degrade without faulting");
    }

    let attempts = attempts.load(Ordering::SeqCst);
    assert!(
        attempts <= 3,
        "MQTT reconnect backoff must avoid scan-thread driven reconnect spinning, attempts={attempts}"
    );
}

#[test]
fn mqtt_output_handoff_is_bounded_when_scan_outpaces_worker() {
    let state = Arc::new(Mutex::new(MockState {
        connected: false,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
on_error = "warn"
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let started = Instant::now();
    for value in 0..32u8 {
        driver
            .write_outputs(&[value, value.wrapping_add(1)])
            .expect("warn policy should not fault when worker handoff is backpressured");
    }
    let elapsed = started.elapsed();

    assert!(
        elapsed < StdDuration::from_millis(50),
        "output command handoff must stay bounded when the scan thread outpaces the MQTT worker, elapsed={elapsed:?}"
    );
}

#[test]
fn mqtt_driver_drop_is_bounded_while_session_is_connecting() {
    let state = Arc::new(Mutex::new(MockState {
        connected: false,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: false,
    });

    let started = Instant::now();
    let driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
on_error = "warn"
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");
    drop(driver);
    let elapsed = started.elapsed();

    assert!(
        elapsed < StdDuration::from_millis(50),
        "dropping the MQTT driver must not wait for a worker blocked on session readiness, elapsed={elapsed:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn fail_closed_disconnected_read_returns_freshness_error() {
    let state = Arc::new(Mutex::new(MockState {
        connected: false,
        last_error: Some(SmolStr::new("broker disconnected")),
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
reconnect_ms = 1
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 1];
    let err = driver
        .read_inputs(&mut inputs)
        .expect_err("disconnected MQTT read must fail closed");
    assert!(
        err.to_string().contains("fresh") || err.to_string().contains("disconnect"),
        "expected freshness/disconnect error, got {err}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn fail_closed_publish_failure_returns_output_error() {
    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        fail_publish_once: true,
        ..MockState::default()
    }));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let err = driver
        .write_outputs(&[1, 2, 3])
        .expect_err("MQTT publish failure must fail closed");
    assert!(
        err.to_string().contains("publish"),
        "expected publish error, got {err}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn fail_closed_connect_failure_is_observable() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(MockFactory {
        state,
        attempts,
        fail_first: false,
        always_fail: true,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
reconnect_ms = 1
"#,
        ),
        factory,
    )
    .expect("construct mqtt driver");

    let mut inputs = [0u8; 1];
    let err = driver
        .read_inputs(&mut inputs)
        .expect_err("MQTT connect failure must be observable");
    assert!(
        err.to_string().contains("connect"),
        "expected connect error, got {err}"
    );
}
