//! Specification 42, §3: every state of an ADS and an OPC UA client connection as a link's state;
//! §4 items 1 and 5: the runtime finds every `Comm.Link` of a compiled program, at any depth and of
//! a derived block too, and finds them again at once in a changed program.

use std::sync::{Arc, Mutex};

use smol_str::SmolStr;

use super::super::ads_subsystem::AdsSubsystem;
use super::super::io_subsystem::IoDriverEntry;
use super::super::opcua_client_subsystem::OpcUaClientSubsystem;
use super::{ads_reading, opcua_reading, LinkSources, LinkState, LinkSubsystem};
use crate::ads::diagnostics::{AdsConnectionStatus, AdsConnectionStatusState};
use crate::error::RuntimeError;
use crate::harness::TestHarness;
use crate::io::{IoDriver, IoDriverHealth};
use crate::opcua::{OpcUaClientConnectionState, OpcUaClientConnectionStatus};
use crate::value::{Duration, Value};

const ALARM: &str = include_str!("../../../../libraries/alarm/src/alarm.st");
const COMM: &str = include_str!("../../../../libraries/comm/src/comm.st");

fn ads(state: AdsConnectionStatusState, degraded_points: usize) -> AdsConnectionStatus {
    AdsConnectionStatus {
        name: "line".to_string(),
        target: None,
        state,
        point_count: 4,
        degraded_points,
        last_good_value_ms: None,
        symbol_version: None,
        summary: "route refused".to_string(),
    }
}

fn opcua(state: OpcUaClientConnectionState, degraded_points: usize) -> OpcUaClientConnectionStatus {
    OpcUaClientConnectionStatus {
        name: SmolStr::new("plant"),
        endpoint_url: "opc.tcp://127.0.0.1:4840".to_string(),
        state,
        point_count: 4,
        degraded_points,
        last_seen_ms: None,
        detail: "session closed".to_string(),
        points: Vec::new(),
    }
}

#[test]
fn every_ads_connection_state_maps_to_a_link_state() {
    use AdsConnectionStatusState as Ads;
    let cases = [
        (ads(Ads::Connected, 0), LinkState::Connected),
        (ads(Ads::Connected, 1), LinkState::Degraded),
        (ads(Ads::Stale, 0), LinkState::Degraded),
        (ads(Ads::NotReady, 0), LinkState::Connecting),
        (ads(Ads::Reconnecting, 0), LinkState::Failed),
        (ads(Ads::Faulted, 0), LinkState::Failed),
        (ads(Ads::Disabled, 0), LinkState::Disabled),
        (ads(Ads::Unknown, 0), LinkState::Unknown),
    ];
    for (connection, state) in cases {
        let reading = ads_reading(&connection);
        assert_eq!(
            (reading.name.as_str(), reading.state),
            ("ads:line", state),
            "{:?}",
            connection.state
        );
    }
    assert_eq!(
        ads_reading(&ads(Ads::Faulted, 0)).error.as_deref(),
        Some("route refused")
    );
    assert_eq!(
        ads_reading(&ads(Ads::Connected, 0)).error,
        None,
        "a healthy link reports no error"
    );
}

#[test]
fn every_opcua_client_connection_state_maps_to_a_link_state() {
    use OpcUaClientConnectionState as Ua;
    let cases = [
        (opcua(Ua::Connected, 0), LinkState::Connected),
        (opcua(Ua::Connected, 2), LinkState::Degraded),
        (opcua(Ua::Configured, 0), LinkState::Connecting),
        (opcua(Ua::Connecting, 0), LinkState::Connecting),
        (opcua(Ua::Stale, 0), LinkState::Degraded),
        (opcua(Ua::Reconnecting, 0), LinkState::Failed),
        (opcua(Ua::Faulted, 0), LinkState::Failed),
        (opcua(Ua::Disabled, 0), LinkState::Disabled),
    ];
    for (connection, state) in cases {
        let reading = opcua_reading(&connection);
        assert_eq!(
            (reading.name.as_str(), reading.state),
            ("opcua_client:plant", state),
            "{:?}",
            connection.state
        );
    }
    assert_eq!(
        opcua_reading(&opcua(Ua::Faulted, 0)).error.as_deref(),
        Some("session closed")
    );
}

/// A driver whose health the test sets.
struct TestDriver(Arc<Mutex<IoDriverHealth>>);

impl IoDriver for TestDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        self.0.lock().expect("health").clone()
    }
}

/// A compiled program with the Alarm and Comm libraries, run for one scan from time zero, so
/// that its blocks hold the names its calls gave them.
fn program(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_sources(&[ALARM, COMM, source]).expect("compiles");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness
}

/// The link `modbus_tcp:0` of a driver whose health the test sets.
fn modbus_link(health: &Arc<Mutex<IoDriverHealth>>) -> Vec<IoDriverEntry> {
    vec![IoDriverEntry::new(
        "modbus-tcp".into(),
        Some("modbus_tcp:0".into()),
        Box::new(TestDriver(Arc::clone(health))),
    )]
}

/// The links' states written into the program between its scans, as the access point after the
/// inputs writes them, at `now_ms`.
fn apply(
    harness: &mut TestHarness,
    links: &mut LinkSubsystem,
    drivers: &mut [IoDriverEntry],
    now_ms: i64,
) {
    let ads = AdsSubsystem::new();
    let opcua = OpcUaClientSubsystem::new();
    harness.runtime_mut().with_program(|program| {
        links.apply_link_states(
            program,
            LinkSources {
                drivers,
                ads: &ads,
                opcua: &opcua,
                input_len: 0,
            },
            now_ms,
        );
    });
}

/// The value at `path`, as the program holds it now.
fn value(harness: &TestHarness, path: &str) -> Value {
    harness
        .runtime()
        .with_program_view(|program| program.value(path))
        .unwrap_or_else(|| panic!("{path} reads"))
}

/// An enumerated value's name, or a text's content.
fn word(harness: &TestHarness, path: &str) -> String {
    match value(harness, path) {
        Value::Enum(value) => value.variant_name().to_string(),
        Value::String(text) => text.to_string(),
        other => panic!("{path} is a word: {other:?}"),
    }
}

/// Specification 42, §4 item 1: the runtime fills every `Comm.Link` instance at any depth of the
/// program: one inside a function block, the elements of an array of links and an instance of a
/// block derived from `Comm.Link`, each from the link its own `Name` names; a name no link has
/// reads `Unknown` with its error. Catches a search of the program's own variables only, one that
/// misses array elements or derived blocks, and a fill that writes one block's state into another.
#[test]
fn every_link_block_at_any_depth_and_of_a_derived_block_is_filled() {
    let mut harness = program(
        r#"
USING Comm;

FUNCTION_BLOCK Station
VAR
    Uplink : Link;
END_VAR
    Uplink(Name := 'modbus_tcp:0', AlarmDelay := T#1s);
END_FUNCTION_BLOCK

FUNCTION_BLOCK Gateway EXTENDS Link
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Line : Station;
    Own : Gateway;
    Spares : ARRAY[1..2] OF Link;
END_VAR
    Line();
    Own(Name := 'modbus_tcp:0', AlarmDelay := T#1s);
    Spares[1](Name := 'modbus_tcp:0', AlarmDelay := T#1s);
    Spares[2](Name := 'mqtt:7', AlarmDelay := T#1s);
END_PROGRAM
"#,
    );
    let health = Arc::new(Mutex::new(IoDriverHealth::Ok));
    let mut drivers = modbus_link(&health);
    let mut links = LinkSubsystem::new();
    apply(&mut harness, &mut links, &mut drivers, 100);
    for block in ["Main.Line.Uplink", "Main.Own", "Main.Spares[1]"] {
        assert_eq!(
            word(&harness, &format!("{block}.State")),
            "Connected",
            "{block}"
        );
        assert_eq!(
            value(&harness, &format!("{block}.Known")),
            Value::Bool(true),
            "{block}"
        );
    }
    assert_eq!(word(&harness, "Main.Spares[2].State"), "Unknown");
    assert_eq!(value(&harness, "Main.Spares[2].Known"), Value::Bool(false));
    assert_eq!(
        word(&harness, "Main.Spares[2].LastError"),
        "no link mqtt:7 in this runtime"
    );
}

/// Specification 42, §4 item 5: after an online change the blocks are found again at once, not
/// only at the next walk a second later, and written where the changed program keeps them. The
/// change puts eight `LREAL`s before the block, so the block's old place lies in them. Catches a
/// fill through the places found in the program before the change: the block would keep the state
/// it had and the eight values would be overwritten.
#[test]
fn a_changed_program_has_its_blocks_found_again_at_once() {
    let before = r#"
USING Comm;

PROGRAM Main
VAR
    Field : Link;
END_VAR
    Field(Name := 'modbus_tcp:0', AlarmDelay := T#1s);
END_PROGRAM
"#;
    let after = r#"
USING Comm;

PROGRAM Main
VAR
    Before : ARRAY[1..8] OF LREAL;
    Field : Link;
END_VAR
    Field(Name := 'modbus_tcp:0', AlarmDelay := T#1s);
END_PROGRAM
"#;
    let mut harness = program(before);
    let health = Arc::new(Mutex::new(IoDriverHealth::Ok));
    let mut drivers = modbus_link(&health);
    let mut links = LinkSubsystem::new();
    apply(&mut harness, &mut links, &mut drivers, 100);
    assert_eq!(word(&harness, "Main.Field.State"), "Connected");

    harness
        .reload_sources(&[ALARM, COMM, after])
        .expect("the changed program is taken online");
    harness.advance_time(Duration::from_millis(100));
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    *health.lock().expect("health") = IoDriverHealth::Faulted {
        error: "timeout after 500 ms".into(),
    };
    apply(&mut harness, &mut links, &mut drivers, 200);
    assert_eq!(
        word(&harness, "Main.Field.State"),
        "Failed",
        "the block of the changed program is written"
    );
    assert_eq!(
        word(&harness, "Main.Field.LastError"),
        "timeout after 500 ms"
    );
    for index in 1..=8 {
        assert_eq!(
            value(&harness, &format!("Main.Before[{index}]")),
            Value::LReal(0.0),
            "Main.Before[{index}] keeps its value"
        );
    }
}
