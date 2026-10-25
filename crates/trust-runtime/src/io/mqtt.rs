//! MQTT I/O driver (protocol ecosystem expansion baseline).

#![allow(missing_docs)]

use std::collections::VecDeque;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use native_tls::{Certificate, Identity, TlsConnector};
use rumqttc::{Client, Event, LastWill, MqttOptions, Packet, QoS, TlsConfiguration, Transport};
use serde::Deserialize;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoDriver, IoDriverErrorPolicy, IoDriverHealth};

include!("mqtt/point_map.rs");
include!("mqtt/sparkplug.rs");
include!("mqtt/config.rs");
include!("mqtt/session.rs");
include!("mqtt/worker.rs");
include!("mqtt/driver.rs");
include!("mqtt/parsing.rs");

#[cfg(test)]
mod tests {
    //! The tests that run a program build it: they need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

    include!("mqtt/tests.rs");
}
