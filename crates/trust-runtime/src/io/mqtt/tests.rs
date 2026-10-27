//! The computer's `mqtt` driver (specification 32; specification 11 §6.5, deadline integrity and
//! safe-state handoff confirmation). The driver's logic is the shared exchange of
//! `trust_runtime_services::io_drivers::mqtt`, whose own tests prove its rules byte for byte. These
//! tests prove that the computer reads `io.toml` by those rules, reads its TLS files and builds its
//! TLS client, and carries the exchange: its transport against a broker stand-in (`broker`) over
//! real connections, and its scan side, with the session, reconnection, freshness, the handoff,
//! the health of both paths and the safe state.
//!
//! These are the computer's earlier tests, moved onto the shared exchange. Where the shared rules
//! decide an expected text or an error kind otherwise than the computer did, the test names the
//! rule that decides it.
//!
//! The tests that run a program build it: they need the toolchain of specification 44 §4.6 at
//! `TRUST_TOOLCHAIN`.

mod broker;
mod incoming_overflow;
mod input_output_health;
mod nonfinite_case;
mod point_map_contract;
// The verification catalog runs these tests as `io::mqtt::tests::safe_state_cases::*`.
#[path = "tests/safe_state.rs"]
mod safe_state_cases;
mod security;
mod session;
mod sparkplug;
mod tag_mapping;

use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use smol_str::SmolStr;
use trust_runtime_services::io_drivers::mqtt::{MqttParams, SCAN_WAIT_MS};

use self::broker::{Broker, PATIENCE};
use super::MqttIoDriver;
use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// What a scan call may take beyond its bound on a loaded test machine.
const TIMING_SLACK: Duration = Duration::from_millis(100);

/// The longest that a write waits for the worker, [`SCAN_WAIT_MS`] (specification 11, deadline
/// integrity).
const SCAN_WAIT: Duration = Duration::from_millis(SCAN_WAIT_MS);

/// The `io.toml` table `text`.
fn table(text: &str) -> toml::Value {
    toml::from_str(text).expect("a TOML table")
}

/// The parameters of the table `text`, as the computer reads them by the shared rules.
fn read(text: &str) -> Result<MqttParams, RuntimeError> {
    MqttIoDriver::read_params(&table(text))
}

/// The refusal of `key` for `reason` by the shared rules, after the driver's name.
fn refused<T>(key: &str, reason: &str) -> Result<T, RuntimeError> {
    Err(RuntimeError::InvalidConfig(
        format!("mqtt: {key}: {reason}").into(),
    ))
}

/// What validating the table `text` gives, which starting its driver must give too: the computer
/// refuses a configuration before any worker starts (specification 32 §2). Only for a table that
/// is refused, since a driver that starts connects to its broker.
fn refusal(text: &str) -> Result<(), RuntimeError> {
    let value = table(text);
    let validated = MqttIoDriver::validate_params(&value);
    assert_eq!(
        MqttIoDriver::from_params(&value).map(drop),
        validated,
        "the start and the validation of {text}"
    );
    validated
}

/// The driver of the table `text`, started.
fn start(text: &str) -> MqttIoDriver {
    MqttIoDriver::from_params(&table(text)).expect("the driver starts")
}

/// The table of the broker stand-in `broker` with the keys `rest`.
fn at(broker: &Broker, rest: &str) -> String {
    format!("broker = \"{}\"\n{rest}", broker.endpoint())
}

/// A broker on the loopback at which nothing listens: the port of a listener closed again.
fn nothing_listens() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    listener.local_addr().expect("its address").to_string()
}

/// Reads the inputs into `image` until a read finds the session running and a message since the
/// last read. Under `on_error = "fault"` a read without one fails with `mqtt input not fresh`, or
/// with the link's condition, and leaves the image as it was (specification 32 §8), so the reads
/// before the broker's messages arrive change nothing. Gives the first other result.
fn read_fresh(driver: &mut MqttIoDriver, image: &mut [u8]) -> Result<(), RuntimeError> {
    let deadline = Instant::now() + PATIENCE;
    loop {
        match driver.read_inputs(image) {
            Err(RuntimeError::IoFreshness(_)) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(1));
            }
            result => return result,
        }
    }
}

/// Waits until `done` holds, at most [`PATIENCE`]; fails the test with `what` otherwise.
fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done() {
        assert!(Instant::now() < deadline, "no {what} within {PATIENCE:?}");
        thread::sleep(Duration::from_millis(1));
    }
}

/// A degraded condition with `text`.
fn degraded(text: &str) -> IoDriverHealth {
    IoDriverHealth::Degraded {
        error: SmolStr::new(text),
    }
}

/// A faulted condition with `text`.
fn faulted(text: &str) -> IoDriverHealth {
    IoDriverHealth::Faulted {
        error: SmolStr::new(text),
    }
}

/// Whether `health` is faulted with a text that starts with `start`.
fn faulted_with(health: &IoDriverHealth, start: &str) -> bool {
    matches!(health, IoDriverHealth::Faulted { error } if error.starts_with(start))
}

/// The verification case `RT_SAFE_NAN_001` through the driver and the broker stand-in
/// (`nonfinite_case`), which the verification catalog runs as
/// `io::mqtt::tests::nonfinite_mapped_payload_batch_cannot_leak_partial_values_after_recovery`.
#[test]
fn nonfinite_mapped_payload_batch_cannot_leak_partial_values_after_recovery() {
    nonfinite_case::run_nonfinite_mqtt_case();
}

/// The scalar types, as a refusal lists them (specification 32 §6).
const TYPES: &str = "bool, u16, i16, u32, i32 or f32, or an alias: boolean, uint16, word, int16, \
                     int, uint32, dword, int32, dint, float or real";

/// A typed point without its type, a numeric point with a bit, and a scale of 0, which cannot be
/// inverted for an output, are refused with the key and the rule (specification 32 §7), when the
/// driver is validated and when it starts. The shared rules' texts replace the computer's
/// (`mqtt input_points.data_type is required ...`), since the rules are one implementation on
/// every runtime (specification 34 ARCH-11). Catches a typed point taken with a guessed type, and
/// a scale that would divide by zero.
#[test]
fn typed_point_map_rejects_invalid_config() {
    assert_eq!(
        refusal(
            "broker = \"127.0.0.1:1883\"\n[[input_points]]\ntopic = \"line/in/missing\"\n\
             image_offset = 0\n"
        ),
        refused(
            "input_points[0].data_type",
            &format!("is required, {TYPES}")
        )
    );
    assert_eq!(
        refusal(
            "broker = \"127.0.0.1:1883\"\n[[output_points]]\ntopic = \"line/out/numeric\"\n\
             image_offset = 0\nimage_bit = 1\ndata_type = \"u16\"\n"
        ),
        refused(
            "output_points[0].image_bit",
            "is only for bool points, not for a u16 point"
        )
    );
    assert_eq!(
        refusal(
            "broker = \"127.0.0.1:1883\"\n[[output_points]]\ntopic = \"line/out/scaled\"\n\
             image_offset = 0\ndata_type = \"u16\"\nscale = 0.0\n"
        ),
        refused(
            "output_points[0].scale",
            "must be a finite number other than 0, not 0.0"
        )
    );
}

/// The Sparkplug B profile needs its `group_id`, takes no input points and needs an output point
/// for a metric (specification 32 §9). The shared rules' texts replace the computer's (`outbound
/// output_points only`, `requires at least one output_point metric`). Catches a node published
/// under a topic without its group, and a profile that would subscribe to inputs it never claims.
#[test]
fn sparkplug_profile_rejects_unsupported_shapes() {
    assert_eq!(
        refusal(
            "broker = \"127.0.0.1:1883\"\n[sparkplug]\nenabled = true\n\
             edge_node_id = \"runtime-a\"\n[[output_points]]\ntopic = \"legacy/out/speed\"\n\
             image_offset = 0\ndata_type = \"u16\"\n"
        ),
        refused(
            "sparkplug.group_id",
            "is required, a non-empty MQTT topic level without /, +, # and control characters"
        )
    );
    assert_eq!(
        refusal(
            "broker = \"127.0.0.1:1883\"\n[sparkplug]\nenabled = true\ngroup_id = \"trust\"\n\
             edge_node_id = \"runtime-a\"\n[[input_points]]\ntopic = \"legacy/in/ready\"\n\
             image_offset = 0\nimage_bit = 0\ndata_type = \"bool\"\n[[output_points]]\n\
             topic = \"legacy/out/speed\"\nimage_offset = 1\ndata_type = \"u16\"\n"
        ),
        refused(
            "input_points",
            "is not taken with Sparkplug, whose profile publishes output points only"
        )
    );
    assert_eq!(
        refusal(
            "broker = \"127.0.0.1:1883\"\n[sparkplug]\nenabled = true\ngroup_id = \"trust\"\n\
             edge_node_id = \"runtime-a\"\n"
        ),
        refused(
            "output_points",
            "is required with Sparkplug, at least one output point for a metric"
        )
    );
}

/// Specification 42, §4 item 9: the driver's own words name the broker and whether the session
/// is connected, or why it is not. The broker stand-in accepts the session, then closes the
/// connection, and the backoff of `reconnect_ms` keeps the session down. Catches a detail without
/// the broker, one that reads connected after the broker went away, and one that hides the cause.
#[test]
fn mqtt_detail_names_the_broker_and_the_sessions_state() {
    let broker = Broker::start();
    let mut driver = start(&at(&broker, "on_error = \"warn\"\nreconnect_ms = 60000"));
    let connected = format!("{}, connected", broker.endpoint());
    let lost = format!(
        "{}, disconnected: the broker closed the connection",
        broker.endpoint()
    );
    let mut inputs = [0_u8; 4];
    let mut detail = || {
        let _ = driver.read_inputs(&mut inputs);
        driver
            .detail()
            .map(|text| text.to_string())
            .unwrap_or_default()
    };
    eventually("a connected session in the detail", || {
        detail() == connected
    });
    broker.close_connection();
    eventually("the lost session in the detail", || detail() == lost);
}
