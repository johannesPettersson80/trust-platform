//! The hardware watchdog timeout rule of the ESP32-C6 adapter (design §2.5 and specification 34,
//! TIME-04). Every chip shares the rule, which lives in `trust_runtime_services::chip::watchdog`;
//! this module re-exports it for the adapter's MWDT0 driver in `hardware/reset.rs`, which keeps only
//! the peripheral.

pub use trust_runtime_services::chip::watchdog::{
    watchdog_timeout_ms, CYCLE_MARGIN_MS, FEED_INTERVAL_MS, MIN_TIMEOUT_MS,
};
