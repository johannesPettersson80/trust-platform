use super::*;
use std::collections::VecDeque;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
struct MockState {
    connected: bool,
    last_error: Option<SmolStr>,
    payloads: VecDeque<Vec<u8>>,
    published: Vec<Vec<u8>>,
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

    fn take_payload(&mut self) -> Option<Vec<u8>> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.payloads.pop_front()
    }

    fn publish(&mut self, _topic: &str, payload: &[u8]) -> Result<(), RuntimeError> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if guard.fail_publish_once {
            guard.fail_publish_once = false;
            guard.last_error = Some(SmolStr::new("publish failed"));
            return Err(RuntimeError::IoDriver("publish failed".into()));
        }
        guard.published.push(payload.to_vec());
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
        payloads: VecDeque::from([vec![1, 0, 1]]),
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
    assert_eq!(guard.published, vec![vec![9, 8, 7]]);
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
    assert!(matches!(driver.health(), IoDriverHealth::Degraded { .. }));
    thread::sleep(StdDuration::from_millis(2));
    {
        let mut guard = factory.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.payloads.push_back(vec![1]);
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

    let started = Instant::now();
    let mut inputs = [0u8; 8];
    for _ in 0..400 {
        let _ = driver.read_inputs(&mut inputs);
        let _ = driver.write_outputs(&[1, 2, 3, 4]);
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < StdDuration::from_millis(250),
        "driver calls should stay non-blocking, elapsed={elapsed:?}"
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
