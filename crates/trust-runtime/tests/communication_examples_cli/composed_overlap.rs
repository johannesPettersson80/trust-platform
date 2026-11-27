//! Composed drivers that fill the same input bytes (specification 11 §6.5): the runtime warns at
//! start, and the multi_driver example, whose drivers fill separate bytes, starts without a warning.

use super::*;

/// The example's MQTT mappings, as `io.toml` writes them.
const MAPPINGS: &str = r#"mappings = [
  { tag = "P1.RemoteStart", topic = "trust/examples/multi/start", direction = "read" },
  { tag = "P1.Run", topic = "trust/examples/multi/running", direction = "write" },
]"#;

/// The raw topics that the example had in their place until 27 November 2026.
const RAW_TOPICS: &str = r#"topic_in = "trust/examples/multi/start"
topic_out = "trust/examples/multi/running""#;

/// The data of each `io_inputs_overlap` event that `runtime` has logged, each at level `warn`.
fn overlap_warnings(runtime: &runtime_process::Runtime) -> Vec<Value> {
    runtime
        .logged_events()
        .into_iter()
        .filter(|line| line["event"] == "io_inputs_overlap")
        .map(|line| {
            assert_eq!(line["level"], "warn", "the overlap is a warning: {line}");
            line["data"].clone()
        })
        .collect()
}

/// The multi_driver example started against `broker` and a Modbus/TCP test server of its own,
/// whose discrete input 0, the drive's ready signal, is TRUE, with `edits` of its `io.toml`. The
/// test server takes one connection, so each runtime has its own.
fn start_multi_driver(
    broker: &MqttTestBroker,
    edits: &[(&'static str, &'static str)],
) -> runtime_process::Runtime {
    let modbus = Arc::new(Mutex::new(ModbusTestState {
        discrete_inputs: vec![true],
        coils: vec![false],
        ..ModbusTestState::default()
    }));
    let server = start_modbus_server(modbus, 1_000_000);
    let address: &'static str = Box::leak(format!("address = \"{server}\"").into_boxed_str());
    let endpoint: &'static str =
        Box::leak(format!("broker = \"{}\"", broker.endpoint()).into_boxed_str());
    let mut example = ExampleRuntime::new("communication/multi_driver")
        .edit_source("io.toml", "address = \"127.0.0.1:1502\"", address)
        .edit_source("io.toml", "broker = \"127.0.0.1:1883\"", endpoint);
    for (from, to) in edits {
        example = example.edit_source("io.toml", from, to);
    }
    example.start()
}

#[test]
fn a_raw_topic_composed_with_a_modbus_point_is_warned_about_at_start() {
    // examples/communication/multi_driver (specification 11 §6.5, composed drivers; specification
    // 42 §5 item 4, the bytes each driver declares): as shipped, its Modbus/TCP point fills
    // %IX0.0, byte 0, and its MQTT mapping %IX1.0, byte 1, so the runtime starts without a
    // warning. With the mappings turned back into the raw topics that the example had until 27
    // November 2026, the topic fills the whole input image after the point's driver, which is 2
    // bytes (%IX0.0 and %IX1.0): at start the runtime logs one `io_inputs_overlap` warning with
    // `modbus_tcp:0` as the earlier driver, `mqtt:1` as the later, and byte 0 as the first and the
    // last byte. The composition still runs, and a one-byte message 0x00 on the topic overwrites
    // the drive's TRUE: %IX0.0 reads FALSE. A reload from Stopped warns again. Catches no warning
    // for that composition, a warning for the shipped one, the drivers in the wrong order, the
    // topic's bytes not cut to the shared byte, a composition refused or whose later driver does
    // not overwrite the earlier, and a warning at the launch alone.
    let broker = MqttTestBroker::start();
    let shipped = start_multi_driver(&broker, &[]);
    // A scan has run, so the start, which logs any overlap before it, is complete.
    shipped.wait_for_snapshot(WAIT, "the drive's input read", |s| {
        s.text("%IX0.0") == "Bool(true)"
    });
    assert_eq!(
        overlap_warnings(&shipped),
        Vec::<Value>::new(),
        "the shipped drivers fill separate bytes"
    );
    drop(shipped);

    let raw = start_multi_driver(&broker, &[(MAPPINGS, RAW_TOPICS)]);
    raw.wait_until(WAIT, "a scan after the start", || raw.snapshot().is_some());
    assert_eq!(
        overlap_warnings(&raw),
        vec![serde_json::json!({
            "event_id": "TRUST-RT-IO-OVERLAP-001",
            "earlier": "modbus_tcp:0",
            "later": "mqtt:1",
            "first_byte": 0,
            "last_byte": 0,
        })],
        "one warning for the topic that fills the point's byte"
    );
    raw.wait_until(WAIT, "the subscription to the raw input topic", || {
        broker
            .subscriptions()
            .iter()
            .any(|topic| topic == "trust/examples/multi/start")
    });
    broker.deliver("trust/examples/multi/start", &[0x00]);
    raw.wait_for_snapshot(WAIT, "the drive's TRUE overwritten by the topic", |s| {
        s.text("%IX0.0") == "Bool(false)"
    });

    // A `restart` that finds the resource Stopped loads the program again, as a launch does
    // (specification 34, STATE-08), and that start warns again.
    raw.ctl_ok(&["shutdown"]);
    raw.wait_until(WAIT, "the stopped state", || {
        raw.status().state == "stopped"
    });
    raw.ctl_ok(&["restart", "cold"]);
    raw.wait_until(WAIT, "the reloaded program's warning", || {
        overlap_warnings(&raw).len() == 2
    });
    let warnings = overlap_warnings(&raw);
    assert_eq!(
        warnings[1], warnings[0],
        "the reload warns as the launch did"
    );
}
