//! Tests of the shared `mqtt` driver. Every expected value is worked out by hand from
//! specification 32, specification 11, specification 34 §11.1, the OASIS MQTT 3.1.1 standard and
//! Sparkplug B, and each test says which wrong behaviour it catches.

mod codec;
mod exchange;
mod health;
mod limits;
mod params;
mod payload;
mod points;
mod sparkplug;

use alloc::vec::Vec;

use smol_str::SmolStr;

use super::codec::encode_publish;
use super::{MqttExchange, MqttParams, MqttPoll};
use crate::status::IoDriverHealth;

/// The default client identifier the tests give the exchange, as trust-runtime gives
/// `trust-runtime-<process id>`.
const CLIENT_ID: &str = "trust-runtime-7";

/// A CONNACK that accepts the connection, without a session present.
const CONNACK: [u8; 4] = [0x20, 0x02, 0x00, 0x00];

/// The exchange of the JSON parameters `json`, started at 0 ms.
fn exchange_of(json: &str) -> MqttExchange {
    MqttExchange::new(
        MqttParams::from_json(json).expect("valid parameters"),
        CLIENT_ID,
        0,
    )
}

/// Takes every outgoing byte at `now_ms`, as a transport that sends them all at once.
fn take(exchange: &mut MqttExchange, now_ms: u64) -> Vec<u8> {
    let bytes = exchange.outgoing().to_vec();
    exchange.sent(now_ms, bytes.len());
    bytes
}

/// Opens the stream at `now_ms` and accepts the session with a CONNACK. Gives the CONNECT and the
/// bytes the session sent after the CONNACK, each taken by the transport.
fn connect(exchange: &mut MqttExchange, now_ms: u64) -> (Vec<u8>, Vec<u8>) {
    assert_eq!(exchange.poll(now_ms), MqttPoll::Open);
    exchange.opened(now_ms);
    let connect = take(exchange, now_ms);
    assert!(!exchange.is_connected(), "no session before CONNACK");
    exchange.received(now_ms, &CONNACK);
    assert!(exchange.is_connected(), "CONNACK accepted the session");
    (connect, take(exchange, now_ms))
}

/// A PUBLISH from the broker of `payload` on `topic`.
fn publish(topic: &str, payload: &[u8]) -> Vec<u8> {
    encode_publish(topic, payload).expect("a PUBLISH")
}

/// Hands the exchange a PUBLISH for each of `messages`, a topic and its payload, at `now_ms`.
fn deliver(exchange: &mut MqttExchange, now_ms: u64, messages: &[(&str, &[u8])]) {
    for (topic, payload) in messages {
        exchange.received(now_ms, &publish(topic, payload));
    }
}

/// A faulted condition with `text`.
fn faulted(text: &str) -> IoDriverHealth {
    IoDriverHealth::Faulted {
        error: SmolStr::new(text),
    }
}

/// A degraded condition with `text`.
fn degraded(text: &str) -> IoDriverHealth {
    IoDriverHealth::Degraded {
        error: SmolStr::new(text),
    }
}
