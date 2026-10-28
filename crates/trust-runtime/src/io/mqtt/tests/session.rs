//! The computer's transport and scan side against the broker stand-in (specification 32 §4 and §8;
//! specification 11, deadline integrity): the session it opens, the raw and the typed exchange,
//! reconnection and its backoff, freshness, the bounds of the scan's calls, and the failures that
//! fail closed.

use std::thread;
use std::time::{Duration, Instant};

use trust_runtime_services::io_drivers::mqtt::MAX_PACKET_BYTES;

use super::broker::{Answer, Broker, Connect};
use super::{
    at, degraded, eventually, faulted, faulted_with, nothing_listens, read_fresh, start, SCAN_WAIT,
    TIMING_SLACK,
};
use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverHealth};

/// Raw mode (specification 32 §4): a read copies the newest payload of `topic_in` into the input
/// image, zero past its end, and a write publishes the whole output image to `topic_out`, over a
/// real connection. Catches a transport that loses the broker's bytes or the handoff, and stale
/// bytes left past a short payload.
#[test]
fn contract_test_reads_and_writes_payloads() {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        "topic_in = \"line/in\"\ntopic_out = \"line/out\"",
    ));
    broker.wait_for("subscription to line/in", |record| {
        record.subscriptions == ["line/in"]
    });
    broker.deliver(&[("line/in", &[1, 0, 1])]);
    let mut inputs = [0xAA; 4];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [1, 0, 1, 0]);
    assert_eq!(driver.write_outputs(&[9, 8, 7]), Ok(()));
    broker.wait_for("image on line/out", |record| {
        record.published("line/out") == [vec![9, 8, 7]]
    });
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

/// The messages that arrived since the last read are one batch, and in raw mode its newest payload
/// is the snapshot (specification 32 §8). Catches the oldest payload of a batch taken.
#[test]
fn mqtt_raw_read_uses_newest_payload_from_drained_batch() {
    let broker = Broker::start();
    let mut driver = start(&at(&broker, "topic_in = \"line/in\""));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    broker.deliver(&[("line/in", &[1, 2, 3]), ("line/in", &[7, 8, 9])]);
    let mut inputs = [0_u8; 3];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [7, 8, 9]);
}

/// The session opens with the CONNECT of an MQTT 3.1.1 client (OASIS MQTT 3.1.1 §3.1): protocol
/// level 4, the flags of a clean session with a user name and a password (0x02, 0x80 and 0x40,
/// the reserved bit 0 clear, MQTT-3.1.2-3), the keep alive of `keep_alive_s`, and the computer's
/// client identifier `trust-runtime-<process id>` when `client_id` is absent; a given `client_id`
/// replaces it. Catches a client identifier that the computer does not give, and credentials that
/// never reach the broker.
#[test]
fn the_session_opens_with_the_computer_s_connect() {
    let broker = Broker::start();
    let _driver = start(&at(
        &broker,
        "keep_alive_s = 7\nusername = \"plc\"\npassword = \"s3cret\"",
    ));
    broker.wait_for("CONNECT", |record| record.connects.len() == 1);
    assert_eq!(
        broker.record(|record| record.connects[0].clone()),
        Connect {
            level: 4,
            flags: 0xC2,
            clean_session: true,
            keep_alive_s: 7,
            client_id: format!("trust-runtime-{}", std::process::id()),
            will: None,
            username: Some(String::from("plc")),
            password: Some(b"s3cret".to_vec()),
        }
    );
    let named = Broker::start();
    let _named_driver = start(&at(&named, "client_id = \"line-7\""));
    named.wait_for("CONNECT", |record| record.connects.len() == 1);
    assert_eq!(
        named.record(|record| record.connects[0].client_id.clone()),
        "line-7"
    );
}

/// A broker that refuses the session with return code 5, not authorized, and then closes the
/// connection (MQTT-3.2.2-5) leaves the refusal in words in the driver's health (MQTT 3.1.1 Table
/// 3.1), not the closed connection that came with it. Catches a transport that reports the close
/// before the CONNACK it read with it, which would hide why the broker refused.
#[test]
fn a_refused_session_reports_the_broker_s_reason() {
    let broker = Broker::start();
    broker.answer_next(&[Answer::ConnAck(5)]);
    let driver = start(&at(&broker, "reconnect_ms = 60000"));
    eventually("refusal in the health", || {
        driver.health()
            == faulted(
                "mqtt connect failed: the broker refused the connection: not authorized (return \
                 code 5)",
            )
    });
}

/// The session sends PINGREQ once a keep alive, here 1 s, and stays while the broker answers with
/// PINGRESP (MQTT 3.1.1 §3.1.2.10): the worker runs the exchange's timers while it waits. Catches a
/// worker that sleeps through the keep alive, and a PINGRESP that the transport does not hand on,
/// after which the session would be dropped and opened again.
#[test]
fn the_session_pings_the_broker_each_keep_alive() {
    let broker = Broker::start();
    let _driver = start(&at(&broker, "keep_alive_s = 1"));
    broker.wait_for("two PINGREQs", |record| record.pings >= 2);
    assert_eq!(broker.record(|record| record.accepted), 1);
}

/// A subscription that the broker refuses (SUBACK return code 0x80, MQTT 3.1.1 §3.9.3) is named in
/// the text of a stale read, which the transport proves by handing the SUBACK to the exchange.
/// Catches a broker's refusal hidden behind a bare `mqtt input not fresh`, which would send the
/// operator looking for a silent publisher instead of the broker's access rules.
#[test]
fn a_refused_subscription_is_named_in_a_stale_read() {
    let broker = Broker::start();
    broker.refuse_subscriptions();
    let mut driver = start(&at(&broker, "topic_in = \"line/in\""));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    let refused = RuntimeError::IoFreshness(
        "mqtt input not fresh; the broker refused the subscription to \"line/in\"".into(),
    );
    let mut inputs = [0x55];
    eventually("refusal in a stale read", || {
        driver.read_inputs(&mut inputs) == Err(refused.clone())
    });
    assert_eq!(inputs, [0x55]);
}

/// Typed input points (specification 32 §7) read a JSON Boolean into bit 2 of byte 0, a text
/// `i16` scaled by 0.5 with offset −10 (100 × 0.5 − 10 = 40) into bytes 1 and 2, and a big-endian
/// binary `u32` 42 into bytes 3 to 6, little-endian in the image. Catches a typed payload that the
/// transport does not carry to its point, and a point written to the wrong storage.
#[test]
fn typed_point_map_reads_json_text_and_binary_payloads() {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        r#"
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
    ));
    broker.wait_for("three subscriptions", |record| {
        record.subscriptions == ["line/in/ready", "line/in/temp", "line/in/count"]
    });
    broker.deliver(&[
        ("line/in/ready", b"true"),
        ("line/in/temp", b"100"),
        ("line/in/count", &42_u32.to_be_bytes()),
    ]);
    let mut inputs = [0_u8; 7];
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [0b0000_0100, 40, 0, 42, 0, 0, 0]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

/// Typed output points (specification 32 §7) publish bit 1 of byte 0 as text `true`, a `u16` of 60
/// scaled back by 0.5 with offset 10 ((60 − 10) ÷ 0.5 = 100) as JSON `100`, and an `i32` of −42
/// big-endian (`0xFFFFFFD6`), each to its topic in the order of the points. Catches a point
/// published to another topic or in another format.
#[test]
fn typed_point_map_writes_json_text_and_binary_payloads() {
    let broker = Broker::start();
    let mut driver = start(&at(
        &broker,
        r#"
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
    ));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    let mut outputs = [0_u8; 7];
    outputs[0] = 0b0000_0010;
    outputs[1..3].copy_from_slice(&60_u16.to_le_bytes());
    outputs[3..7].copy_from_slice(&(-42_i32).to_le_bytes());
    assert_eq!(driver.write_outputs(&outputs), Ok(()));
    broker.wait_for("three publications", |record| record.publishes.len() == 3);
    broker.record(|record| {
        assert_eq!(
            record.topics(),
            ["line/out/enable", "line/out/speed", "line/out/count"]
        );
        assert_eq!(record.publishes[0].payload, b"true");
        assert_eq!(record.publishes[1].payload, b"100");
        assert_eq!(record.publishes[2].payload, [0xFF, 0xFF, 0xFF, 0xD6]);
        // Quality of service 0, without DUP and RETAIN (§3.3.1).
        assert!(record.publishes.iter().all(|publish| publish.flags == 0));
    });
}

/// A connect that fails under `on_error = "warn"` degrades the health with the transport's reason
/// and a read succeeds with the image as it was (specification 32 §8: `warn` keeps its degraded
/// success). Catches a warned failure that faults the scan.
#[test]
fn on_error_warn_connect_failure_degrades_without_runtime_error() {
    let mut driver = start(&format!(
        "broker = \"{}\"\nreconnect_ms = 60000\non_error = \"warn\"",
        nothing_listens()
    ));
    eventually("failed connect in the health", || {
        matches!(driver.health(), IoDriverHealth::Degraded { error }
            if error.starts_with("mqtt connect failed: "))
    });
    let mut inputs = [0x55];
    assert_eq!(driver.read_inputs(&mut inputs), Ok(()));
    assert_eq!(inputs, [0x55]);
    assert!(
        matches!(driver.health(), IoDriverHealth::Degraded { error }
            if error.starts_with("mqtt reconnect backoff active (mqtt connect failed: ")),
        "{:?}",
        driver.health()
    );
}

/// A session that the broker drops after a publication is a lost connection: under `on_error =
/// "ignore"` the next write succeeds and the output path is degraded with the backoff and its
/// cause (specification 32 §8: a lost session starts the backoff). The computer's earlier driver
/// saw the failed publication itself; the shared exchange completes a handoff when its PUBLISH is
/// queued, so the loss shows on the writes after it. Catches an ignored failure that faults the
/// scan, and a lost session reported healthy.
#[test]
fn on_error_ignore_publish_failure_degrades_without_runtime_error() {
    let broker = Broker::start();
    broker.close_on_publish();
    let mut driver = start(&at(&broker, "on_error = \"ignore\"\nreconnect_ms = 60000"));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    assert_eq!(driver.write_outputs(&[1, 2, 3]), Ok(()));
    let lost = "mqtt disconnected: the broker closed the connection";
    eventually("lost session in the health", || {
        driver.health() == degraded(lost)
    });
    assert_eq!(driver.write_outputs(&[4, 5, 6]), Ok(()));
    assert_eq!(
        driver.output_health(),
        degraded(&format!("mqtt reconnect backoff active ({lost})"))
    );
    assert_eq!(broker.record(|record| record.publishes.len()), 1);
}

/// A connect that fails is tried again after the backoff of `reconnect_ms` (specification 32 §8):
/// the broker closes the first connection before its CONNACK, which faults the health with the
/// transport's reason and a read with the backoff and its cause ("Connection errors retain their
/// context"); the second connection opens the session and subscribes again, and the next message
/// is read fresh, which clears the health. Catches a driver that gives up after one failure, and a
/// reconnection that loses the subscription.
#[test]
fn reconnection_test_retries_after_connect_failure() {
    let broker = Broker::start();
    broker.answer_next(&[Answer::Close]);
    let mut driver = start(&at(&broker, "reconnect_ms = 1500"));
    let failed = "mqtt connect failed: the broker closed the connection";
    eventually("failed connect in the health", || {
        driver.health() == faulted(failed)
    });
    let mut inputs = [0_u8; 1];
    assert_eq!(
        driver.read_inputs(&mut inputs),
        Err(RuntimeError::IoFreshness(
            format!("mqtt reconnect backoff active ({failed})").into()
        ))
    );
    broker.wait_for("subscription of the second connection", |record| {
        record.accepted == 2 && record.subscriptions.len() == 1
    });
    broker.deliver(&[("trust/io/in", &[1])]);
    assert_eq!(read_fresh(&mut driver, &mut inputs), Ok(()));
    assert_eq!(inputs, [1]);
    assert_eq!(driver.health(), IoDriverHealth::Ok);
}

/// Without a broker the scan's calls stay bounded (specification 32 §8: an outage cannot block the
/// scheduler): a read returns at once, and a write waits at most [`SCAN_WAIT`] for a connection
/// that is being opened. Catches a scan that waits for the network.
#[test]
fn cycle_impact_test_driver_calls_are_non_blocking_without_session() {
    let mut driver = start(&format!(
        "broker = \"{}\"\nreconnect_ms = 1",
        nothing_listens()
    ));
    let mut inputs = [0_u8; 8];
    let mut longest = Duration::ZERO;
    for _ in 0..50 {
        let started = Instant::now();
        let _ = driver.read_inputs(&mut inputs);
        longest = longest.max(started.elapsed());
        let started = Instant::now();
        let _ = driver.write_outputs(&[1, 2, 3, 4]);
        longest = longest.max(started.elapsed());
    }
    assert!(
        longest < SCAN_WAIT + TIMING_SLACK,
        "a scan call took {longest:?}"
    );
}

/// A read with no message since the last one returns at once, under `warn` with the health
/// degraded: before any snapshot, `mqtt input snapshot unavailable` (specification 32 §8).
/// Catches a read that waits for the broker's next message.
#[test]
fn mqtt_read_inputs_returns_within_scan_bound_without_fresh_payload() {
    let broker = Broker::start();
    let mut driver = start(&at(&broker, "on_error = \"warn\""));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    let mut inputs = [0_u8; 8];
    let started = Instant::now();
    assert_eq!(driver.read_inputs(&mut inputs), Ok(()));
    let elapsed = started.elapsed();
    assert!(
        elapsed < SCAN_WAIT + TIMING_SLACK,
        "the read took {elapsed:?}"
    );
    assert_eq!(driver.health(), degraded("mqtt input snapshot unavailable"));
}

/// While the session waits for its CONNACK a write waits for it at most [`SCAN_WAIT`] (the shared
/// exchange's `output_settling`), then returns under `warn` with the output path degraded with
/// `mqtt connecting`. Catches a write that waits for the broker without a bound, and one that does
/// not give a session being opened its chance.
#[test]
fn mqtt_write_outputs_returns_within_scan_bound_while_session_connecting() {
    let broker = Broker::start();
    broker.answer_always(Some(Answer::Silence));
    let mut driver = start(&at(&broker, "on_error = \"warn\""));
    broker.wait_for("CONNECT", |record| record.connects.len() == 1);
    let started = Instant::now();
    assert_eq!(driver.write_outputs(&[1, 2, 3]), Ok(()));
    let elapsed = started.elapsed();
    // The exchange's clock counts whole milliseconds, so the wait may end up to 1 ms early.
    assert!(
        elapsed + Duration::from_millis(1) >= SCAN_WAIT && elapsed < SCAN_WAIT + TIMING_SLACK,
        "the write took {elapsed:?}"
    );
    assert_eq!(driver.output_health(), degraded("mqtt connecting"));
}

/// Under `warn` a read without a message since the last one returns the last snapshot, at once,
/// with the health degraded by `mqtt input not fresh` (specification 32 §8). Catches a warned
/// stale read that loses the last known input image.
#[test]
fn mqtt_stale_snapshot_is_returned_without_fresh_payload() {
    let broker = Broker::start();
    let mut driver = start(&at(&broker, "topic_in = \"line/in\"\non_error = \"warn\""));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    broker.deliver(&[("line/in", &[7, 8, 9])]);
    let mut first = [0_u8; 4];
    eventually("first message in the image", || {
        driver.read_inputs(&mut first).is_ok() && first == [7, 8, 9, 0]
    });
    let mut stale = [0_u8; 4];
    let started = Instant::now();
    assert_eq!(driver.read_inputs(&mut stale), Ok(()));
    let elapsed = started.elapsed();
    assert!(
        elapsed < SCAN_WAIT + TIMING_SLACK,
        "the read took {elapsed:?}"
    );
    assert_eq!(stale, [7, 8, 9, 0]);
    assert_eq!(driver.health(), degraded("mqtt input not fresh"));
}

/// Under the default `fault` a read without a message since the last one fails with `mqtt input not
/// fresh` and leaves the image as it was, even with a snapshot held (specification 32 §8: "A
/// faulted stale read leaves the caller's input image unchanged"). The computer's earlier driver
/// waited for a refresh round trip and reported `mqtt input refresh pending`; the shared exchange
/// reads what the worker received without waiting ("the worker checks for new input without
/// waiting"), so a stale read is `mqtt input not fresh`. Catches a faulted read that exposes old
/// bytes as fresh input.
#[test]
fn mqtt_default_fault_rejects_stale_snapshot() {
    let broker = Broker::start();
    let mut driver = start(&at(&broker, "topic_in = \"line/in\""));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    broker.deliver(&[("line/in", &[7, 8, 9])]);
    let mut first = [0_u8; 4];
    assert_eq!(read_fresh(&mut driver, &mut first), Ok(()));
    let mut stale = [0xA5; 4];
    assert_eq!(
        driver.read_inputs(&mut stale),
        Err(RuntimeError::IoFreshness("mqtt input not fresh".into()))
    );
    assert_eq!(stale, [0xA5; 4]);
    assert_eq!(driver.health(), faulted("mqtt input not fresh"));
}

/// A read with no message follows `on_error`: an error and a faulted health under `fault`, success
/// and a degraded health under `warn` and `ignore`, each at once (specification 32 §8). Catches a
/// policy that is not applied to the freshness of input.
#[test]
fn mqtt_no_fresh_payload_follows_on_error_policy() {
    for (policy, fails) in [("fault", true), ("warn", false), ("ignore", false)] {
        let broker = Broker::start();
        let mut driver = start(&at(&broker, &format!("on_error = \"{policy}\"")));
        broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
        let mut inputs = [0_u8; 4];
        let started = Instant::now();
        let result = driver.read_inputs(&mut inputs);
        let elapsed = started.elapsed();
        assert!(
            elapsed < SCAN_WAIT + TIMING_SLACK,
            "{policy}: the read took {elapsed:?}"
        );
        assert_eq!(result.is_err(), fails, "{policy}: {result:?}");
        match (policy, driver.health()) {
            ("fault", IoDriverHealth::Faulted { .. })
            | ("warn" | "ignore", IoDriverHealth::Degraded { .. }) => {}
            (_, other) => panic!("{policy}: {other:?}"),
        }
    }
}

/// The backoff of `reconnect_ms`, here a second, bounds the connection attempts to a broker that
/// closes each connection: at most one more than the whole backoffs in the time elapsed, however
/// often the scan reads (specification 32 §8: a nonzero, bounded backoff that cannot busy-loop),
/// and a read never waits for the reconnection, which would take the whole second. Catches
/// reconnection driven by the scan's calls.
#[test]
fn mqtt_reconnect_backoff_is_bounded_and_non_spinning() {
    let broker = Broker::start();
    broker.answer_always(Some(Answer::Close));
    let started = Instant::now();
    let mut driver = start(&at(&broker, "reconnect_ms = 1000\non_error = \"warn\""));
    let mut inputs = [0_u8; 1];
    for _ in 0..50 {
        let read = Instant::now();
        assert_eq!(driver.read_inputs(&mut inputs), Ok(()));
        assert!(read.elapsed() < SCAN_WAIT + TIMING_SLACK);
        thread::sleep(Duration::from_millis(5));
    }
    let attempts = broker.record(|record| record.accepted);
    let elapsed = started.elapsed();
    let most = usize::try_from(elapsed.as_millis() / 1000).expect("a short test") + 1;
    assert!(
        (1..=most).contains(&attempts),
        "{attempts} attempts in {elapsed:?}"
    );
}

/// A scan that writes faster than the session can take its images, here while the session waits
/// for its CONNACK, finds each write bounded by [`SCAN_WAIT`] (specification 11: worker-backed
/// drivers keep a bounded handoff deadline). The computer's earlier driver waited only for the
/// first of such writes; the shared exchange gives every write its wait while the session is being
/// opened (`output_settling`), so the bound holds per write. Catches a handoff that queues the
/// scan behind the broker.
#[test]
fn mqtt_output_handoff_is_bounded_when_scan_outpaces_worker() {
    // The broker never answers, so a write queued behind the session would wait for its 5 s
    // connect timeout, far beyond the bound.
    let bound = SCAN_WAIT + TIMING_SLACK;
    let broker = Broker::start();
    broker.answer_always(Some(Answer::Silence));
    let mut driver = start(&at(&broker, "on_error = \"warn\""));
    broker.wait_for("CONNECT", |record| record.connects.len() == 1);
    for value in 0..32_u8 {
        let started = Instant::now();
        assert_eq!(
            driver.write_outputs(&[value, value.wrapping_add(1)]),
            Ok(())
        );
        let elapsed = started.elapsed();
        assert!(elapsed < bound, "write {value} took {elapsed:?}");
    }
}

/// Under the default `fault` a write while the session waits for its CONNACK fails with the link's
/// condition, `mqtt connecting`, and faults the output path. The computer's earlier driver reported
/// `mqtt output handoff pending`; the shared exchange reports the condition of the link that holds
/// the handoff back (specification 32 §8: "Connection errors retain their context"). Catches a
/// write confirmed before any session.
#[test]
fn mqtt_default_fault_rejects_output_handoff_timeout() {
    let broker = Broker::start();
    broker.answer_always(Some(Answer::Silence));
    let mut driver = start(&at(&broker, ""));
    broker.wait_for("CONNECT", |record| record.connects.len() == 1);
    assert_eq!(
        driver.write_outputs(&[1, 2, 3]),
        Err(RuntimeError::IoTransport("mqtt connecting".into()))
    );
    assert_eq!(driver.health(), faulted("mqtt connecting"));
}

/// A typed output whose point lies beyond the image (a `u16` at byte 4 of a 1-byte image) follows
/// `on_error` with the conversion's text, publishes nothing and keeps the session: the next valid
/// image is the only publication (specification 32 §8: a typed-output preflight failure is a
/// publish condition and starts no backoff). Catches a failed conversion that publishes part of the
/// image, and a policy that hides the conversion's text.
#[test]
fn mqtt_policy_preserves_typed_output_preflight_failure() {
    let text = "mqtt point image range 4..6 exceeds process image size 1";
    for (policy, fails) in [("fault", true), ("warn", false), ("ignore", false)] {
        let broker = Broker::start();
        let mut driver = start(&at(
            &broker,
            &format!(
                "on_error = \"{policy}\"\n[[output_points]]\ntopic = \"line/out\"\n\
                 image_offset = 4\ndata_type = \"u16\"\npayload_format = \"binary_le\"\n"
            ),
        ));
        broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
        let result = driver.write_outputs(&[1]);
        assert_eq!(result.is_err(), fails, "{policy}: {result:?}");
        if fails {
            assert_eq!(result, Err(RuntimeError::IoTransport(text.into())));
            assert_eq!(driver.health(), faulted(text), "{policy}");
        } else {
            assert_eq!(driver.health(), degraded(text), "{policy}");
        }
        assert_eq!(
            driver.write_outputs(&[1, 2, 3, 4, 5, 6]),
            Ok(()),
            "{policy}"
        );
        broker.wait_for("valid image", |record| !record.publishes.is_empty());
        assert_eq!(
            broker.record(|record| record.published("line/out")),
            [vec![5, 6]],
            "{policy}"
        );
    }
}

/// Dropping the driver returns at once while the session waits for a CONNACK that never comes
/// (specification 32 §8: the drop returns within its bounded deadline even while a first
/// connection is pending), as does starting it. Catches a drop that joins a worker blocked on the
/// broker.
#[test]
fn mqtt_driver_drop_is_bounded_while_session_is_connecting() {
    let broker = Broker::start();
    broker.answer_always(Some(Answer::Silence));
    let started = Instant::now();
    let driver = start(&at(&broker, "on_error = \"warn\""));
    let start_took = started.elapsed();
    broker.wait_for("CONNECT", |record| record.connects.len() == 1);
    let dropped = Instant::now();
    drop(driver);
    let drop_took = dropped.elapsed();
    assert!(
        start_took < SCAN_WAIT + TIMING_SLACK && drop_took < SCAN_WAIT + TIMING_SLACK,
        "the start took {start_took:?} and the drop {drop_took:?}"
    );
}

/// Dropping the driver ends a running session in order: the worker sends DISCONNECT (MQTT 3.1.1
/// §3.14) and closes the connection. Catches a drop that leaves the session for the broker's keep
/// alive to find.
#[test]
fn dropping_the_driver_disconnects_its_session() {
    let broker = Broker::start();
    let driver = start(&at(&broker, ""));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    drop(driver);
    broker.wait_for("DISCONNECT", |record| record.disconnects == 1);
}

/// When the broker goes away, a read under the default `fault` fails with the backoff and the
/// lost session's cause, and leaves the image as it was (specification 32 §8: "disconnected ...
/// input ... return an error"; "Connection errors retain their context"). Catches a lost session
/// read as healthy, and a generic error that hides the cause.
#[test]
fn fail_closed_disconnected_read_returns_freshness_error() {
    let broker = Broker::start();
    let mut driver = start(&at(&broker, "reconnect_ms = 60000"));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    broker.close_connection();
    let lost =
        "mqtt reconnect backoff active (mqtt disconnected: the broker closed the connection)";
    let mut inputs = [0x55];
    eventually("read in the backoff", || {
        driver.read_inputs(&mut inputs) == Err(RuntimeError::IoFreshness(lost.into()))
    });
    assert_eq!(inputs, [0x55]);
}

/// A publication that the session cannot carry fails the write under the default `fault` with
/// `mqtt publish:` and the reason, and keeps the session for the next image (specification 32 §8:
/// a publish failure returns an error and sets faulted health). An image of `MAX_PACKET_BYTES`
/// bytes to `trust/io/out` makes a PUBLISH of 17 bytes more, the fixed header's 1, the remaining
/// length's 2 and the topic's 2 + 12, beyond the client's packet limit. The computer's earlier
/// driver saw failed publications from its client library; with the shared exchange a lost
/// connection is the session's loss, and a PUBLISH that cannot be made fails the write. Catches a
/// failed publication reported as a success.
#[test]
fn fail_closed_publish_failure_returns_output_error() {
    let broker = Broker::start();
    let mut driver = start(&at(&broker, ""));
    broker.wait_for("subscription", |record| record.subscriptions.len() == 1);
    let text = format!(
        "mqtt publish: the PUBLISH to \"trust/io/out\" of {} bytes exceeds the \
         {MAX_PACKET_BYTES}-byte packet limit",
        MAX_PACKET_BYTES + 17
    );
    assert_eq!(
        driver.write_outputs(&[0_u8; MAX_PACKET_BYTES]),
        Err(RuntimeError::IoTransport(text.as_str().into()))
    );
    assert_eq!(driver.output_health(), faulted(&text));
    assert_eq!(driver.write_outputs(&[1]), Ok(()));
    broker.wait_for("next image", |record| !record.publishes.is_empty());
    assert_eq!(driver.output_health(), IoDriverHealth::Ok);
}

/// A connect that fails is observable: under the default `fault` the health and a read in the
/// backoff carry `mqtt connect failed:` and the transport's reason (specification 32 §8). Catches
/// a failed connect reported as a stale input.
#[test]
fn fail_closed_connect_failure_is_observable() {
    let mut driver = start(&format!(
        "broker = \"{}\"\nreconnect_ms = 60000",
        nothing_listens()
    ));
    eventually("failed connect in the health", || {
        faulted_with(&driver.health(), "mqtt connect failed: ")
    });
    let mut inputs = [0_u8; 1];
    let error = driver
        .read_inputs(&mut inputs)
        .expect_err("a failed connect fails the read under fault");
    assert!(
        matches!(&error, RuntimeError::IoFreshness(text)
            if text.starts_with("mqtt reconnect backoff active (mqtt connect failed: ")),
        "{error}"
    );
}
