//! The driver's two paths (specification 32 §8; specification 11, safe-state handoff
//! confirmation), through the broker stand-in: stale or missing input stays off the output path,
//! a lost link marks both and starts the backoff, a typed-output preflight failure is a publish
//! condition that only a publication clears, and the reported health is the more severe path.

use std::time::Instant;

use super::broker::{Answer, Broker};
use super::safe_state_cases::runtime_with_safe_state;
use super::{at, degraded, eventually, faulted, faulted_with, read_fresh, start, SCAN_WAIT};
use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// The stale read's error.
fn not_fresh() -> Result<(), RuntimeError> {
    Err(RuntimeError::IoFreshness("mqtt input not fresh".into()))
}

/// A broker and a driver on `line/in` and `line/out` under `on_error`, with a backoff of 60 s, so
/// that a backoff started during a test cannot end inside it, its session running and its first
/// message, 7, read.
fn split_health_driver(on_error: &str) -> (Broker, super::MqttIoDriver) {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        &format!(
            "topic_in = \"line/in\"\ntopic_out = \"line/out\"\non_error = \"{on_error}\"\n\
             reconnect_ms = 60000"
        ),
    ));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    broker.deliver(&[("line/in", &[7])]);
    let mut inputs = [0_u8; 1];
    eventually("first message", || {
        driver.read_inputs(&mut inputs).is_ok() && inputs == [7]
    });
    (broker, driver)
}

/// Stale input on a connected link faults the reported health under `fault`, but starts no backoff
/// and leaves the output path `Ok`: the next write publishes at once (specification 32 §8: "a
/// connected session stays connected, starts no backoff and keeps publishing outputs"). Catches a
/// stale read that holds the outputs back or that unconfirms a safe state.
#[test]
fn fault_policy_stale_input_on_connected_link_leaves_output_path_healthy() {
    let (broker, mut driver) = split_health_driver("fault");
    assert_eq!(driver.write_outputs(&[1]), Ok(()));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    let mut inputs = [7_u8];
    assert_eq!(driver.read_inputs(&mut inputs), not_fresh());
    assert_eq!(driver.health(), faulted("mqtt input not fresh"));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    assert_eq!(driver.write_outputs(&[2]), Ok(()));
    broker.wait_for("both images", |record| record.publishes.len() == 2);
    assert_eq!(
        broker.record(|record| record.published("line/out")),
        [vec![1], vec![2]]
    );
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// The same stale input under `warn` degrades the reported health only, with `mqtt input not
/// fresh`; the output path stays `Ok` and the next write publishes. Catches a warned stale read
/// that reaches the output path.
#[test]
fn warn_policy_stale_input_on_connected_link_degrades_reported_health_only() {
    let (broker, mut driver) = split_health_driver("warn");
    assert_eq!(driver.write_outputs(&[1]), Ok(()));
    let mut inputs = [0_u8];
    assert_eq!(driver.read_inputs(&mut inputs), Ok(()));
    assert_eq!(inputs, [7]);
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    assert_eq!(driver.health(), degraded("mqtt input not fresh"));
    assert_eq!(driver.write_outputs(&[2]), Ok(()));
    broker.wait_for("both images", |record| record.publishes.len() == 2);
    assert_eq!(
        broker.record(|record| record.published("line/out")),
        [vec![1], vec![2]]
    );
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// A lost link starts the backoff and marks the output path: a broker that closes the connection
/// before its CONNACK fails the first write with the backoff and its cause, the next write with
/// the backoff too, and no second connection is tried within the 60 s of `reconnect_ms`
/// (specification 32 §8). Catches a write that reconnects at once, and a failure that leaves the
/// output path `Ok`.
#[test]
fn lost_link_still_starts_backoff_and_marks_the_output_path() {
    let broker = Broker::start();
    broker.answer_always(Some(Answer::Close));
    let mut driver = start(&at(
        &broker,
        "topic_out = \"line/out\"\nreconnect_ms = 60000",
    ));
    eventually("failed connect", || {
        faulted_with(&driver.health(), "mqtt connect failed: ")
    });
    let backoff = "mqtt reconnect backoff active (mqtt connect failed: the broker closed the \
                   connection)";
    assert_eq!(
        driver.write_outputs(&[1]),
        Err(RuntimeError::IoTransport(backoff.into()))
    );
    assert_eq!(
        driver.write_outputs(&[2]),
        Err(RuntimeError::IoTransport(backoff.into()))
    );
    assert_eq!(driver.output_health(), faulted(backoff));
    assert_eq!(broker.record(|record| record.accepted), 1);
}

/// A typed-output preflight failure on a connected link follows the policy but starts no backoff,
/// so the next valid write publishes at once (specification 32 §8). Catches a preflight failure
/// treated as a lost connection.
#[test]
fn typed_output_preflight_failure_on_connected_link_does_not_start_backoff() {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        "reconnect_ms = 60000\n[[output_points]]\ntopic = \"line/out\"\nimage_offset = 4\n\
         data_type = \"u16\"\npayload_format = \"binary_le\"\n",
    ));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    let text = "mqtt point image range 4..6 exceeds process image size 1";
    assert_eq!(
        driver.write_outputs(&[1]),
        Err(RuntimeError::IoTransport(text.into()))
    );
    assert_eq!(driver.output_health(), faulted(text));
    assert_eq!(driver.write_outputs(&[1, 2, 3, 4, 5, 6]), Ok(()));
    broker.wait_for("valid image", |record| !record.publishes.is_empty());
    assert_eq!(
        broker.record(|record| record.published("line/out")),
        [vec![5, 6]]
    );
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// A driver that only reads, a lowered read mapping, has nothing to deliver, so its output path is
/// always `Ok`: a lost link is an input condition alone, and a write succeeds (specification 32 §8:
/// a direction the configuration disables is always healthy). Catches a safe state unconfirmed by a
/// driver without outputs.
#[test]
fn input_only_driver_output_path_stays_healthy_while_its_link_is_down() {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        "reconnect_ms = 60000\nmappings = [\n  { tag = \"MainInstance.Level\", topic = \
         \"line/in\", direction = \"read\" }\n]\n[[input_points]]\ntopic = \"line/in\"\n\
         image_offset = 0\ndata_type = \"u16\"\n",
    ));
    broker.wait_for("subscription", |record| record.subscriptions == ["line/in"]);
    broker.deliver(&[("line/in", b"7")]);
    let mut inputs = [0_u8; 2];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [7, 0]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
    broker.close_connection();
    let lost =
        "mqtt reconnect backoff active (mqtt disconnected: the broker closed the connection)";
    eventually("lost link in a read", || {
        driver.read_inputs(&mut inputs) == Err(RuntimeError::IoFreshness(lost.into()))
    });
    assert_eq!(driver.health(), faulted(lost));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    assert_eq!(driver.write_outputs(&[0]), Ok(()));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// After a stale read the safe state is confirmed when its image is handed off, because stale input
/// never reaches the output path (specification 11: input-path conditions never unconfirm a
/// safe-state handoff). The test builds a program, so it needs the toolchain of specification 44
/// §4.6 at `TRUST_TOOLCHAIN`. Catches a safe state unconfirmed by stale input.
#[test]
fn safe_state_after_stale_input_is_confirmed_when_the_safe_output_publishes() {
    let (broker, mut driver) = split_health_driver("fault");
    let mut inputs = [0_u8];
    assert_eq!(driver.read_inputs(&mut inputs), not_fresh());
    let mut runtime = runtime_with_safe_state(driver, "mqtt-stale-input");
    runtime
        .apply_io_safe_state()
        .expect("stale input must not unconfirm the safe state");
    broker.wait_for("safe state", |record| !record.publishes.is_empty());
    assert_eq!(
        broker.record(|record| record.published("line/out")),
        [vec![0x5A]]
    );
}

/// The output path follows the link, not the reported health: under `warn` a stale read degrades
/// the health and leaves the output path `Ok`, and a lost link degrades the output path too. The
/// computer's earlier driver proved this without its worker; with the shared exchange the worker
/// is the transport, and the rule holds for the one driver there is. Catches an output path that
/// follows input conditions, or that misses a lost link.
#[test]
fn the_output_path_follows_the_link_not_the_reported_health() {
    let (broker, mut driver) = split_health_driver("warn");
    let mut inputs = [0_u8];
    assert_eq!(driver.read_inputs(&mut inputs), Ok(()));
    assert_eq!(driver.health(), degraded("mqtt input not fresh"));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    broker.close_connection();
    eventually("lost link in the output path", || {
        driver.output_health() == degraded("mqtt disconnected: the broker closed the connection")
    });
}

/// A stale read under `fault` returns at once with `mqtt input not fresh`: the read takes what has
/// arrived and waits for nothing (specification 32 §8). Catches a read that waits for a message.
#[test]
fn stale_read_under_fault_returns_at_once_with_not_fresh() {
    let (_broker, mut driver) = split_health_driver("fault");
    let mut inputs = [0_u8];
    let started = Instant::now();
    let result = driver.read_inputs(&mut inputs);
    let elapsed = started.elapsed();
    assert!(elapsed < SCAN_WAIT / 2, "the stale read took {elapsed:?}");
    assert_eq!(result, not_fresh());
}

/// A publication after a stale read clears only the output path: the reported health stays
/// faulted with `mqtt input not fresh` until a fresh read (specification 32 §8). Catches a publish
/// that hides stale input.
#[test]
fn reported_health_keeps_stale_input_after_a_publish_until_a_fresh_read() {
    let (broker, mut driver) = split_health_driver("fault");
    let mut inputs = [0_u8];
    assert_eq!(driver.read_inputs(&mut inputs), not_fresh());
    assert_eq!(driver.write_outputs(&[1]), Ok(()));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    assert_eq!(driver.health(), faulted("mqtt input not fresh"));
    broker.deliver(&[("line/in", &[8])]);
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [8]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

/// A write right after a stale read is handed off at once and is not reported as pending: the
/// stale read holds no worker time (specification 32 §8). Catches a handoff held behind a read.
#[test]
fn handoff_right_after_a_stale_read_is_not_held_as_pending() {
    let (broker, mut driver) = split_health_driver("fault");
    let mut inputs = [0_u8];
    assert_eq!(driver.read_inputs(&mut inputs), not_fresh());
    let started = Instant::now();
    assert_eq!(driver.write_outputs(&[2]), Ok(()));
    let elapsed = started.elapsed();
    assert!(elapsed < SCAN_WAIT, "the write took {elapsed:?}");
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    broker.wait_for("image", |record| !record.publishes.is_empty());
    assert_eq!(
        broker.record(|record| record.published("line/out")),
        [vec![2]]
    );
}

/// A link loss that a read sees faults the output path; once the session runs again the link
/// condition is gone, without a publication ("a connection condition clears as soon as any
/// operation finds the link up"). A typed-output preflight failure is a publish condition: the
/// reads after it leave it, and only a publication clears it (specification 32 §8). While the link
/// is down the broker closes each connection, so that the driver, with its backoff of 1 ms, cannot
/// reconnect before the read sees the loss. Catches a link condition that outlives the link, and a
/// preflight failure that a read clears.
#[test]
fn fresh_read_clears_a_recovered_link_but_a_preflight_failure_waits_for_a_publish() {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        "topic_in = \"line/in\"\nreconnect_ms = 1\n[[output_points]]\ntopic = \"line/out\"\n\
         image_offset = 4\ndata_type = \"u16\"\npayload_format = \"binary_le\"\n",
    ));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    broker.deliver(&[("line/in", &[1])]);
    let mut inputs = [0_u8];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);

    broker.answer_always(Some(Answer::Close));
    broker.close_connection();
    eventually("lost link in a read", || {
        driver.read_inputs(&mut inputs).is_err()
            && matches!(driver.output_health(), IoDriverHealth::Faulted { .. })
    });

    broker.answer_always(None);
    broker.wait_for("subscription of a new session", |record| {
        record.subscriptions.len() == 2
    });
    broker.deliver(&[("line/in", &[2])]);
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [2]);
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);

    let text = "mqtt point image range 4..6 exceeds process image size 1";
    assert_eq!(
        driver.write_outputs(&[1]),
        Err(RuntimeError::IoTransport(text.into()))
    );
    assert_eq!(driver.output_health(), faulted(text));
    broker.deliver(&[("line/in", &[3])]);
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(driver.output_health(), faulted(text));

    assert_eq!(driver.write_outputs(&[1, 2, 3, 4, 5, 6]), Ok(()));
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
    broker.wait_for("valid image", |record| !record.publishes.is_empty());
    assert_eq!(
        broker.record(|record| record.published("line/out")),
        [vec![5, 6]]
    );
}
