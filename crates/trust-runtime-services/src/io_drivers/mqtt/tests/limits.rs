//! A chip's bounds on the exchange (specification 34 §11.1): the incoming queue by messages and
//! bytes, the packets by size and the outgoing bytes, and the time of the exchange's next timer,
//! which a chip's idle work waits for.

use trust_runtime_core::error::RuntimeError;

use super::{connect, degraded, exchange_of, faulted, publish, take, CONNACK};
use crate::io_drivers::mqtt::{MqttLimits, MqttPoll};
use crate::status::IoDriverHealth;

/// Bounds with room for `outgoing` bytes to send, and a computer's otherwise.
fn outgoing(outgoing: usize) -> MqttLimits {
    MqttLimits {
        outgoing_bytes: outgoing,
        ..MqttLimits::COMPUTER
    }
}

/// A chip bounds its queue by bytes as well (specification 34 §11.1; the chip's appendix names its
/// bounds): with room for 5 payload bytes, three 2-byte messages before a scan keep the newest two
/// (4 bytes) and drop the oldest, counted as an overflow, and a 6-byte message, larger than the
/// whole queue, is dropped too while the older ones stay. A packet above the chip's packet bound is
/// a malformed packet that ends the session. Catches a chip whose queue grows with the payloads past
/// its memory, and a message larger than the queue that empties it.
#[test]
fn a_chip_bounds_its_queue_by_bytes_and_its_packets_by_size() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","topic_in":"line/in"}"#)
        .with_limits(MqttLimits {
            max_packet_bytes: 64,
            queue_messages: 256,
            queue_bytes: 5,
            outgoing_bytes: 256,
        });
    connect(&mut exchange, 0);
    for index in 0..3_u16 {
        exchange.received(1, &publish("line/in", &index.to_be_bytes()));
    }
    let mut inputs = [0_u8; 2];
    assert_eq!(exchange.read_inputs(2, &mut inputs), Ok(()));
    assert_eq!(inputs, [0x00, 0x02]);
    assert_eq!(
        exchange.health(),
        degraded("incoming queue overflow: 1 messages dropped")
    );
    exchange.received(3, &publish("line/in", &[7, 7]));
    exchange.received(3, &publish("line/in", &[9; 6]));
    assert_eq!(exchange.read_inputs(4, &mut inputs), Ok(()));
    assert_eq!(inputs, [7, 7]);
    assert_eq!(
        exchange.health(),
        degraded("incoming queue overflow: 2 messages dropped")
    );
    exchange.received(5, &publish("line/in", &[1; 80]));
    assert!(
        !exchange.is_connected(),
        "a packet above 64 bytes ends the session"
    );
}

/// With room for 30 outgoing bytes, the raw driver's CONNECT (29 bytes: a 2-byte fixed header,
/// the 10-byte variable header and the 17-byte client identifier `trust-runtime-7` with its length)
/// and its SUBSCRIBE to `trust/io/in` (18 bytes: 2 + packet identifier 2 + topic 2 + 11 + the
/// requested quality of service 1)
/// fit. While the transport has not taken the SUBSCRIBE, the handoff of a 2-byte image, an 18-byte
/// PUBLISH to `trust/io/out` (2 + topic 2 + 12 + payload 2), does not fit beside it: the image waits
/// as `mqtt output handoff pending`, faulted under the default `on_error`, and goes out once the
/// transport took the SUBSCRIBE. Catches outgoing bytes that grow past a chip's bound, and a waiting
/// image that is never handed off.
#[test]
fn a_handoff_waits_for_room_in_the_outgoing_bytes() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883"}"#).with_limits(outgoing(30));
    assert_eq!(exchange.poll(0), MqttPoll::Open);
    exchange.opened(0);
    assert_eq!(take(&mut exchange, 0).len(), 29);
    exchange.received(1, &CONNACK);
    assert_eq!(exchange.outgoing().len(), 18, "the SUBSCRIBE waits");

    assert_eq!(
        exchange.write_outputs(2, &[0x12, 0x34]),
        Err(RuntimeError::IoTransport(
            "mqtt output handoff pending".into()
        ))
    );
    assert_eq!(
        exchange.output_health(),
        faulted("mqtt output handoff pending")
    );
    assert_eq!(
        exchange.outgoing().len(),
        18,
        "nothing joined the SUBSCRIBE"
    );

    exchange.sent(3, 18);
    assert_eq!(
        exchange.outgoing(),
        publish("trust/io/out", &[0x12, 0x34]).as_slice()
    );
    assert_eq!(exchange.output_health(), IoDriverHealth::Ok);
}

/// A handoff larger than the whole outgoing bound can never go: a 20-byte image is a 36-byte
/// PUBLISH (2 + 2 + 12 + 20), above 30, so nothing is published, the write fails under the default
/// `on_error` with the sizes, and the session stays. Catches a handoff that waits for ever for room
/// that cannot come, and one that is cut to fit.
#[test]
fn a_handoff_larger_than_the_outgoing_bound_is_not_published() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883"}"#).with_limits(outgoing(30));
    connect(&mut exchange, 0);
    let text = "the handoff of 36 bytes exceeds the 30-byte outgoing buffer";
    assert_eq!(
        exchange.write_outputs(1, &[0; 20]),
        Err(RuntimeError::IoTransport(text.into()))
    );
    assert_eq!(exchange.outgoing(), &[] as &[u8]);
    assert!(exchange.is_connected());
    assert_eq!(exchange.output_health(), faulted(text));
}

/// A session whose first packets do not fit fails to connect, and says why: with room for 40
/// bytes, a driver with typed inputs on two 20-character topics sends a 29-byte CONNECT, and its
/// two SUBSCRIBE packets after CONNACK take 27 bytes each (2 + 2 + 2 + 20 + 1), 54 together; the
/// read in the backoff names the cause. Catches a session that starts without its subscriptions.
#[test]
fn a_session_whose_first_packets_do_not_fit_fails_to_connect() {
    let mut exchange = exchange_of(concat!(
        r#"{"broker":"127.0.0.1:1883","input_points":["#,
        r#"{"data_type":"u16","image_offset":0,"topic":"plant/line/level/a01"},"#,
        r#"{"data_type":"u16","image_offset":2,"topic":"plant/line/level/a02"}]}"#
    ))
    .with_limits(outgoing(40));
    assert_eq!(exchange.poll(0), MqttPoll::Open);
    exchange.opened(0);
    assert_eq!(take(&mut exchange, 0).len(), 29);
    exchange.received(1, &CONNACK);
    assert!(!exchange.is_connected());
    assert_eq!(exchange.poll(1), MqttPoll::Close);
    let mut inputs = [0_u8; 4];
    assert_eq!(
        exchange.read_inputs(2, &mut inputs),
        Err(RuntimeError::IoFreshness(
            "mqtt reconnect backoff active (mqtt connect failed: the session start of 54 bytes \
             exceeds the 40-byte outgoing buffer)"
                .into()
        ))
    );
}

/// The exchange names the time of its next timer, so that a chip's idle work wakes for it: at
/// once before the first connection, the connect timeout 5,000 ms after the stream opened, the
/// flush timeout 5,000 ms after the SUBSCRIBE began to wait at CONNACK (10 ms), earlier than the
/// keep alive of 30 s, then that keep alive, and the end of the default 500 ms backoff after a lost
/// connection. Catches a chip whose session misses its keep alive while no scan runs.
#[test]
fn the_exchange_names_its_next_timer() {
    let mut exchange = exchange_of(r#"{"broker":"127.0.0.1:1883","keep_alive_s":30}"#);
    assert_eq!(exchange.due_ms(), Some(0));
    assert_eq!(exchange.poll(0), MqttPoll::Open);
    assert_eq!(exchange.due_ms(), Some(5_000));
    exchange.opened(0);
    take(&mut exchange, 0);
    exchange.received(10, &CONNACK);
    assert_eq!(
        exchange.due_ms(),
        Some(5_010),
        "the SUBSCRIBE waits since 10 ms"
    );
    take(&mut exchange, 20);
    assert_eq!(
        exchange.due_ms(),
        Some(30_010),
        "the keep alive from CONNACK"
    );
    exchange.closed(100, "the server closed the connection");
    assert_eq!(exchange.due_ms(), Some(600));
}
