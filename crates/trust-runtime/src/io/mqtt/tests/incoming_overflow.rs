//! The bound of the received messages and the health it reports (specification 34 §11.1), through
//! the broker stand-in: the driver keeps at most 256 messages for the scan; at the bound a new
//! message drops the oldest and is counted, and from the first drop until the driver restarts the
//! health is `degraded` with `incoming queue overflow: <n> messages dropped`, unless it is faulted;
//! the overflow concerns received messages only, so the output path stays `Ok`. Each message's
//! payload is its number as two big-endian bytes, so that the image shows which message the read
//! took; the messages of one delivery reach the driver together, as one batch.

use super::broker::Broker;
use super::{at, degraded, faulted, read_fresh, start, MqttIoDriver};
use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// A broker and a driver on `line/in` and `line/out`, its session running.
fn overflow_driver() -> (Broker, MqttIoDriver) {
    let broker = Broker::start();
    let driver = start(&at(
        &broker,
        "topic_in = \"line/in\"\ntopic_out = \"line/out\"",
    ));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    (broker, driver)
}

/// Delivers the messages numbered `numbers` on `line/in`, in one batch.
fn deliver(broker: &Broker, numbers: std::ops::Range<u16>) {
    let payloads: Vec<[u8; 2]> = numbers.map(u16::to_be_bytes).collect();
    let messages: Vec<(&str, &[u8])> = payloads
        .iter()
        .map(|payload| ("line/in", payload.as_slice()))
        .collect();
    broker.deliver(&messages);
}

/// 256 messages, the bound, drop nothing: the read takes the newest, 255 (0x00 0xFF), and the
/// health is `Ok`. Catches a bound below 256.
#[test]
fn driver_with_256_queued_messages_stays_healthy() {
    let (broker, mut driver) = overflow_driver();
    deliver(&broker, 0..256);
    let mut inputs = [0_u8; 2];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x00, 0xFF]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

/// The 257th message drops the oldest: the newest, 256 (0x01 0x00), is kept and read, and the
/// health is degraded with one message dropped. Catches an unbounded queue, and the newest message
/// dropped instead of the oldest.
#[test]
fn incoming_queue_257th_message_keeps_newest_256_and_counts_one_drop() {
    let (broker, mut driver) = overflow_driver();
    deliver(&broker, 0..257);
    let mut inputs = [0_u8; 2];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x01, 0x00]);
    assert_eq!(
        driver.health(),
        degraded("incoming queue overflow: 1 messages dropped")
    );
}

/// Every dropped message is counted: 300 messages drop 44, and the newest, 299 (0x01 0x2B), is
/// read. Catches a count that stops at one.
#[test]
fn incoming_queue_counts_every_dropped_message() {
    let (broker, mut driver) = overflow_driver();
    deliver(&broker, 0..300);
    let mut inputs = [0_u8; 2];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x01, 0x2B]);
    assert_eq!(
        driver.health(),
        degraded("incoming queue overflow: 44 messages dropped")
    );
}

/// Degraded by the overflow alone, the driver keeps its inputs fresh: the next message, 257 (0x01
/// 0x01), reaches the image, and the health keeps the count. Catches an overflow that stops the
/// input.
#[test]
fn driver_after_257th_message_reports_exact_detail_and_keeps_inputs_fresh() {
    let (broker, mut driver) = overflow_driver();
    deliver(&broker, 0..257);
    let mut inputs = [0_u8; 2];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    deliver(&broker, 257..258);
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x01, 0x01]);
    assert_eq!(
        driver.health(),
        degraded("incoming queue overflow: 1 messages dropped")
    );
}

/// The overflow leaves the output path `Ok`, so a safe-state handoff through the driver is
/// confirmed while the health is degraded (specification 11: incoming-queue overflow is an
/// input-path condition). Catches an overflow that unconfirms a safe state.
#[test]
fn overflowed_driver_reports_degraded_health_with_ok_output_path() {
    let (broker, mut driver) = overflow_driver();
    deliver(&broker, 0..257);
    let mut inputs = [0_u8; 2];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(driver.write_outputs(&[0x5A]), Ok(()));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    assert_eq!(
        driver.health(),
        degraded("incoming queue overflow: 1 messages dropped")
    );
}

/// The overflow never hides a fault: after it, a stale read under the default `fault` faults the
/// health with `mqtt input not fresh`, not the overflow's degraded detail. Catches a degraded
/// overflow reported over a faulted driver.
#[test]
fn incoming_overflow_never_overrides_a_faulted_driver() {
    let (broker, mut driver) = overflow_driver();
    deliver(&broker, 0..257);
    let mut inputs = [0_u8; 2];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(
        driver.read_inputs(&mut inputs),
        Err(RuntimeError::IoFreshness("mqtt input not fresh".into()))
    );
    assert_eq!(driver.health(), faulted("mqtt input not fresh"));
}
