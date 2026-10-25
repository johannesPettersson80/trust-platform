//! The safe-state handoff through the computer's driver (specification 11, safe-state handoff
//! confirmation): confirmed when the write succeeds and the output path is `Ok` right after it,
//! unconfirmed while the session is not running. The tests build a program, so they need the
//! toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use super::broker::{Answer, Broker};
use super::{at, start, MqttIoDriver};
use crate::io::{IoAddress, IoSafeState};
use crate::value::Value;
use crate::CompiledRuntime;

/// A program with a one-byte output image whose safe state is `0x5A` at `%QB0`, handed off through
/// `driver`, named `name`.
pub(super) fn runtime_with_safe_state(driver: MqttIoDriver, name: &str) -> CompiledRuntime {
    let mut runtime = crate::harness::TestHarness::from_source(
        "VAR_GLOBAL out AT %QB0 : BYTE; END_VAR PROGRAM Main END_PROGRAM",
    )
    .expect("build the MQTT output program")
    .into_runtime();
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((
        IoAddress::parse("%QB0").expect("safe-state output byte"),
        Value::Byte(0x5A),
    ));
    runtime.set_io_safe_state(safe_state);
    runtime.add_io_driver(name, Box::new(driver));
    runtime
}

/// With the session running, the safe state's image is handed off and confirmed, and the broker
/// receives it on `topic_out`. Catches a running session whose handoff is not confirmed, which
/// would fault every deliberate stop.
#[test]
fn mqtt_safe_state_handoff_succeeds_when_worker_confirms_publish() {
    let broker = Broker::start();
    let driver = start(&at(
        &broker,
        "topic_out = \"line/out\"\non_error = \"warn\"",
    ));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    let mut runtime = runtime_with_safe_state(driver, "mqtt-safe-state");
    runtime
        .apply_io_safe_state()
        .expect("a running session confirms the safe state");
    broker.wait_for("safe state on line/out", |record| {
        record.published("line/out") == [vec![0x5A]]
    });
}

/// While the session waits for its CONNACK, a safe-state handoff is unconfirmed even under `warn`,
/// whose write succeeds: the output path is degraded with `mqtt connecting`, and the error names
/// the driver (specification 11: a handoff that is queued, reconnecting or pending is unconfirmed).
/// Catches a safe state claimed delivered before any session.
#[test]
fn mqtt_safe_state_handoff_reports_unconfirmed_connecting_worker() {
    let broker = Broker::start();
    broker.answer_always(Some(Answer::Silence));
    let driver = start(&at(
        &broker,
        "topic_out = \"line/out\"\non_error = \"warn\"",
    ));
    broker.wait_for("CONNECT", |record| record.connects.len() == 1);
    let mut runtime = runtime_with_safe_state(driver, "mqtt-safe-state");
    let error = runtime
        .apply_io_safe_state()
        .expect_err("a session that waits for its CONNACK does not confirm the safe state");
    assert_eq!(
        error.to_string(),
        "i/o driver error 'safe-state output unconfirmed for driver 'mqtt-safe-state': mqtt \
         connecting'"
    );
}
