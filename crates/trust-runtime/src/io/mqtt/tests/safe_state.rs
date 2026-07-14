mod safe_state_cases {
    use super::*;
    use crate::io::{IoAddress, IoSafeState};
    use crate::value::Value;
    use crate::Runtime;

    fn runtime_with_mqtt_safe_state(
        state: Arc<Mutex<MockState>>,
        connected: bool,
    ) -> Runtime {
        state.lock().unwrap_or_else(|e| e.into_inner()).connected = connected;
        let factory = Arc::new(MockFactory {
            state,
            attempts: Arc::new(AtomicUsize::new(0)),
            fail_first: false,
            always_fail: false,
        });
        let driver = MqttIoDriver::from_params_with_factory(
            &params(
                r#"
broker = "127.0.0.1:1883"
topic_out = "line/out"
on_error = "warn"
"#,
            ),
            factory,
        )
        .expect("construct MQTT driver");

        let mut runtime = Runtime::new();
        runtime.io_mut().resize(0, 1, 0);
        let mut safe_state = IoSafeState::default();
        safe_state.outputs.push((
            IoAddress::parse("%QB0").expect("safe-state output byte"),
            Value::Byte(0x5a),
        ));
        runtime.set_io_safe_state(safe_state);
        runtime.add_io_driver("mqtt-safe-state", Box::new(driver));
        runtime
    }

    #[test]
    fn mqtt_safe_state_handoff_succeeds_when_worker_confirms_publish() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mut runtime = runtime_with_mqtt_safe_state(Arc::clone(&state), true);

        runtime
            .apply_io_safe_state()
            .expect("completed MQTT publish should confirm safe state");

        let published = &state.lock().unwrap_or_else(|e| e.into_inner()).published;
        assert_eq!(published, &[(SmolStr::new("line/out"), vec![0x5a])]);
    }

    #[test]
    fn mqtt_safe_state_handoff_reports_unconfirmed_connecting_worker() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mut runtime = runtime_with_mqtt_safe_state(state, false);

        let error = runtime
            .apply_io_safe_state()
            .expect_err("connecting worker must not confirm physical safe state");

        assert!(
            error.to_string().contains("mqtt-safe-state")
                && error.to_string().contains("unconfirmed"),
            "expected named unconfirmed MQTT safe-state error, got {error}"
        );
    }
}
