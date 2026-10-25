//! The `mqtt` I/O driver that every runtime shares (specification 32; specification 11 §6.5 and
//! its MQTT runtime integration integrity; specification 34 ARCH-11 and §11.1): one
//! implementation of its logic, with or without an operating system. It holds the parameters with
//! the keys, defaults and refusals of `io.toml`, the MQTT 3.1.1 packets of the driver's client,
//! the typed points and their payloads, the Sparkplug B outbound profile, and the session with the
//! broker as a state machine without a transport.
//!
//! [`MqttParams::from_json`] reads the `params` table: a chip from its record's section 8
//! (specification 34 §5.1.1), a computer from its `io.toml`, so both refuse and accept the same
//! files. Symbolic `mappings` arrive lowered to typed points: the runtime that holds the program's
//! symbols lowers them before the driver starts (specification 32 §5), and their presence selects
//! mapped mode (§4).
//!
//! [`MqttExchange`] holds no socket and reads no clock: the runtime's transport carries its bytes
//! over TCP, with TLS when the parameters ask for it, and gives it the time of a monotonic clock.
//! A runtime whose transport runs on another thread gives a write up to [`SCAN_WAIT_MS`] while
//! [`MqttExchange::output_settling`] holds, as trust-runtime's scan waits for its worker; a
//! runtime that drives the transport itself sends the outgoing bytes after each write. A
//! safe-state handoff is confirmed when [`MqttExchange::write_outputs`] succeeds and
//! [`MqttExchange::output_health`] is `Ok` right after it (specification 11, safe-state handoff
//! confirmation).

pub mod codec;
mod exchange;
mod params;
mod payload;
mod points;
mod sparkplug;

pub use exchange::{
    MqttExchange, MqttLimits, MqttPoll, CONNECT_TIMEOUT_MS, FLUSH_TIMEOUT_MS, INCOMING_QUEUE_LIMIT,
    MAX_PACKET_BYTES, SCAN_WAIT_MS,
};
pub use params::{
    BrokerEndpoint, ClientAuth, Credentials, Mapping, MappingDirection, MqttParams, MqttTls,
};
pub use points::{InputPoint, OutputPoint, PayloadFormat, PointType};
pub use sparkplug::SparkplugParams;

#[cfg(test)]
mod tests;
