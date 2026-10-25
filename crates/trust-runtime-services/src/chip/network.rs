//! A chip's network as the shared chip platform drives it (specification 34 ARCH-12): its work
//! between scans, when that work is next due, and what `capabilities` reports of it. The chip's
//! adapter owns the network, its radio or its Ethernet controller and its frames; the platform only
//! gives it the loop's idle time and lets the loop's wait end when the network's next timer is due
//! (a retransmission, an address lease, a connection attempt).

use alloc::string::String;

use serde_json::{json, Value};
use trust_runtime_core::ports::MonotonicInstant;

/// A chip's network, which the platform's idle work keeps.
pub trait ChipNetwork {
    /// The network's work between scans: its link, its address, the frames that arrived and what
    /// its sockets queued. It must not wait.
    fn poll(&mut self);

    /// When the network's work is next due, on the loop's clock; none while only an arriving frame
    /// or a change of the link, which wake the loop, give it work.
    fn poll_due(&mut self) -> Option<MonotonicInstant>;

    /// The network as it stands, for `capabilities`.
    fn facts(&self) -> NetworkFacts;

    /// The chip is about to reset: the network withdraws what it announced (specification 11,
    /// runtime discovery) and leaves its network, so that the access point or switch does not hold
    /// the chip's old connection (specification 34 PORT-06). The default does nothing.
    fn leave(&mut self) {}
}

/// What `capabilities` reports of a chip's network as `network` (specification 11, the runtime
/// device capabilities contract).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkFacts {
    /// The interface: `wifi` or `ethernet`.
    pub interface: &'static str,
    /// Whether the link is up: the chip has joined its Wi-Fi network, or its cable is plugged in.
    pub link_up: bool,
    /// The interface's IPv4 address with its prefix length, while it has one.
    pub address: Option<String>,
    /// The TCP port of the control connection, none while the chip serves none.
    pub control_port: Option<u16>,
    /// Why the interface did not start, or why its last attempt to join failed.
    pub error: Option<String>,
    /// The memory of the network's driver, where it has its own (the ESP32-C6's radio arena).
    pub driver_memory: Option<DriverMemory>,
    /// The time server of the device settings and what its last request gave, none without one
    /// (specification 34 TARGET-06).
    pub time: Option<TimeFacts>,
}

/// The time server of a chip's settings and what its last request gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeFacts {
    /// The server, a host name or an IPv4 address.
    pub server: String,
    /// How far behind the server the chip's clock was when the last request set it, in
    /// milliseconds, negative when ahead; none before a request set it.
    pub offset_ms: Option<i64>,
    /// Why the last request took no time, none when it took one or before the first.
    pub error: Option<String>,
}

/// The memory a network's driver has of its own: its size, the most it has held and the
/// allocations it refused, which tell whether the driver has room enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverMemory {
    /// The bytes it has.
    pub bytes: u64,
    /// The most bytes it has held at once since boot.
    pub peak_bytes: u64,
    /// The allocations it refused since boot.
    pub failures: u64,
}

impl NetworkFacts {
    /// The `network` member of `capabilities`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "interface": self.interface,
            "link": if self.link_up { "up" } else { "down" },
            "address": self.address,
            "control_port": self.control_port,
            "error": self.error,
            "driver_memory": self.driver_memory.map(|memory| json!({
                "bytes": memory.bytes,
                "peak_bytes": memory.peak_bytes,
                "failures": memory.failures,
            })),
            "time": self.time.as_ref().map(|time| json!({
                "server": time.server,
                "offset_ms": time.offset_ms,
                "error": time.error,
            })),
        })
    }
}
