//! Composed drivers that fill the same input bytes (specification 11 §6.5): the runtime warns at
//! start, and the multi_driver example, whose drivers fill separate bytes, starts without a warning.

use super::*;

/// The example's MQTT mappings, as `io.toml` writes them.
const MAPPINGS: &str = r#"mappings = [
  { tag = "P1.RemoteStart", topic = "trust/examples/multi/start", direction = "read" },
  { tag = "P1.Run", topic = "trust/examples/multi/running", direction = "write" },
]"#;

/// The example's safe state, as `io.toml` writes it.
const SAFE_STATE: &str = "[[io.safe_state]]\naddress = \"%QX0.0\"\nvalue = \"FALSE\"\n";

/// The raw topics that the example had in their place until 27 November 2026.
const RAW_TOPICS: &str = r#"topic_in = "trust/examples/multi/start"
topic_out = "trust/examples/multi/running""#;

/// The example's Modbus/TCP drive, as `io.toml` writes it after its `[[io.drivers]]` header.
const MODBUS_DRIVER: &str = r#"name = "modbus-tcp"

[io.drivers.params]
address = "127.0.0.1:1502"
unit_id = 1
timeout_ms = 500
on_error = "fault"

[[io.drivers.params.input_points]]
image_offset = 0
image_bit = 0
address = 0
function = "read_discrete_inputs"
data_type = "bool"

[[io.drivers.params.output_points]]
image_offset = 0
image_bit = 0
address = 0
function = "write_single_coil"
data_type = "bool"
"#;

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

/// `trust-runtime validate` of `runtime`'s project: its plain answer, and the warnings of its
/// `--ci` report. Both succeed, as an overlap is a warning.
fn validated(runtime: &runtime_process::Runtime) -> (String, Vec<Value>) {
    let validate = |args: &[&str]| {
        let output = runtime_process::trust_runtime_command()
            .arg("validate")
            .args(args)
            .arg("--project")
            .arg(runtime.project())
            .output()
            .expect("run trust-runtime validate");
        assert!(
            output.status.success(),
            "validate succeeds: {}",
            runtime_process::describe(&output)
        );
        output
    };
    let plain = validate(&[]);
    let report: Value =
        serde_json::from_slice(&validate(&["--ci"]).stdout).expect("the CI report's JSON");
    (
        String::from_utf8_lossy(&plain.stdout).into_owned(),
        report["warnings"].as_array().cloned().unwrap_or_default(),
    )
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

/// The multi_driver example with the `loopback` driver in place of its Modbus/TCP drive, started
/// against `broker` with `edits` of its `io.toml`.
fn start_with_loopback(
    broker: &MqttTestBroker,
    edits: &[(&'static str, &'static str)],
) -> runtime_process::Runtime {
    let endpoint: &'static str =
        Box::leak(format!("broker = \"{}\"", broker.endpoint()).into_boxed_str());
    let mut example = ExampleRuntime::new("communication/multi_driver")
        .edit_source("io.toml", MODBUS_DRIVER, "name = \"loopback\"\n")
        .edit_source("io.toml", "broker = \"127.0.0.1:1883\"", endpoint);
    for (from, to) in edits {
        example = example.edit_source("io.toml", from, to);
    }
    example.start()
}

#[test]
fn a_loopback_is_warned_about_only_where_its_copy_of_the_outputs_reaches() {
    // examples/communication/multi_driver with the `loopback` driver in place of its Modbus/TCP
    // drive (specification 11 §6.5; specification 42 §5 item 4: the loopback driver copies the
    // output image over the input image, from byte 0 as far as both reach). The output image is
    // 1 byte (%QX0.0) and the input image 2 (%IX0.0 and %IX1.0), so the loopback fills byte 0
    // and the MQTT mapping of P1.RemoteStart byte 1: the runtime starts without a warning. With
    // the read mapping on P1.DriveReady, %IX0.0, instead, the mapping fills byte 0 after the
    // loopback, and the start logs one `io_inputs_overlap` warning with `loopback:0` as the
    // earlier driver, `mqtt:1` as the later, and byte 0 as the first and the last byte.
    // `trust-runtime validate` reads the same from the parameters: nothing for the first, and for
    // the second the warning line before `Project ok` and the same data in its `--ci` report.
    // Catches a loopback that declares the whole input image, which warns of byte 1 that it never
    // writes, one that declares nothing, which warns of no byte, and a `validate` that tells
    // otherwise than the start.
    let broker = MqttTestBroker::start();
    let separate = start_with_loopback(&broker, &[]);
    separate.wait_until(WAIT, "a scan after the start", || {
        separate.snapshot().is_some()
    });
    assert_eq!(
        overlap_warnings(&separate),
        Vec::<Value>::new(),
        "the loopback's 1-byte copy stops before the mapping's byte"
    );
    assert_eq!(
        validated(&separate),
        ("Project ok\n".to_string(), Vec::new()),
        "validate reads the same from the parameters"
    );
    drop(separate);

    let shared = start_with_loopback(
        &broker,
        &[("tag = \"P1.RemoteStart\"", "tag = \"P1.DriveReady\"")],
    );
    shared.wait_until(WAIT, "a scan after the start", || {
        shared.snapshot().is_some()
    });
    assert_eq!(
        overlap_warnings(&shared),
        vec![serde_json::json!({
            "event_id": "TRUST-RT-IO-OVERLAP-001",
            "earlier": "loopback:0",
            "later": "mqtt:1",
            "first_byte": 0,
            "last_byte": 0,
        })],
        "one warning for the mapping on the loopback's byte"
    );
    assert_eq!(
        validated(&shared),
        (
            "warning: loopback:0 and mqtt:1 both fill input byte 0; mqtt:1, the later, overwrites \
             what loopback:0 read there at every cycle\nProject ok\n"
                .to_string(),
            overlap_warnings(&shared)
        ),
        "validate warns as the start did"
    );
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
    // the drive's TRUE: %IX0.0 reads FALSE. `trust-runtime validate` finds the same from the
    // parameters, before a start: nothing for the shipped example, and for the raw topics the
    // warning line before `Project ok` and the logged event's data in its `--ci` report. A reload
    // from Stopped warns again. Catches a `validate` that tells otherwise than the start, no warning
    // for that composition, a warning for the shipped one, the drivers in the wrong order, the
    // topic's bytes not cut to the shared byte, a composition refused or whose later driver does
    // not overwrite the earlier, a warning at the launch alone, and a reload's warning at the
    // project file's log level instead of the live one.
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
    assert_eq!(
        validated(&shipped),
        ("Project ok\n".to_string(), Vec::new()),
        "validate finds no overlap in the shipped example"
    );
    drop(shipped);

    // The Modbus/TCP test server takes one connection, which a reload cannot open again: with
    // `on_error = "warn"` and no safe state to confirm at a stop, the reloaded program runs on
    // without its drive and can be stopped and reloaded once more.
    let raw = start_multi_driver(
        &broker,
        &[
            (MAPPINGS, RAW_TOPICS),
            ("on_error = \"fault\"", "on_error = \"warn\""),
            (SAFE_STATE, ""),
        ],
    );
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
    assert_eq!(
        validated(&raw),
        (
            "warning: modbus_tcp:0 and mqtt:1 both fill input byte 0; mqtt:1, the later, \
             overwrites what modbus_tcp:0 read there at every cycle\nProject ok\n"
                .to_string(),
            overlap_warnings(&raw)
        ),
        "validate warns as the start did, before the project starts"
    );
    let readme = std::fs::read_to_string(
        runtime_process::repo_root().join("examples/communication/multi_driver/README.md"),
    )
    .expect("the example's README");
    assert!(
        readme.contains(
            "`warning: modbus_tcp:0 and mqtt:1 both fill input byte 0; mqtt:1, the later, \
             overwrites what modbus_tcp:0 read there at every cycle`"
        ),
        "the README quotes validate's warning"
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

    // The warning is logged at the live log level: with `log.level` set to `error` while the
    // runtime runs, the next reload's start warns of nothing.
    raw.ctl_ok(&["config-set", "log.level", "error"]);
    raw.ctl_ok(&["shutdown"]);
    raw.wait_until(WAIT, "the stopped state", || {
        raw.status().state == "stopped"
    });
    raw.ctl_ok(&["restart", "cold"]);
    raw.wait_until(WAIT, "the reloaded program's start", || {
        raw.status().state != "stopped"
    });
    assert_eq!(
        overlap_warnings(&raw).len(),
        2,
        "the live level `error` holds the reload's warning back"
    );
}
