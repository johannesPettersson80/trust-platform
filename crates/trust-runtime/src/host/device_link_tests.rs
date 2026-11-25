//! The record a program for stepped scans is installed with (specification 11 §6.13.3,
//! specification 34 MEM-12).

use super::{STEPPING_EVENTS, STEPPING_RUNTIME};
use crate::config::RuntimeConfig;

#[test]
fn a_program_for_stepped_scans_keeps_16_events_with_its_debugger_on() {
    // Specification 11 §6.13.3 with specification 34 MEM-12: a chip keeps the debugger's
    // buffer of stops beside a debug-enabled program, one stop for each event its record's
    // event store keeps. Stepped scans need the debugger and read no event, so their record
    // keeps 16 events, not the chip's default: the ESP32-C6's 200 took 17,860 bytes of stops
    // and left a debug-enabled program on a C6 with Wi-Fi on about 4.7 KB (checklist G100).
    // Catches the events section dropped (the chip's default again), a capacity apart from
    // STEPPING_EVENTS, and the debugger off, which `cycle.step` needs.
    let folder = trust_test_support::unique_temp_path("stepping-runtime");
    std::fs::create_dir_all(&folder).expect("a folder for the runtime.toml");
    let path = folder.join("runtime.toml");
    std::fs::write(&path, STEPPING_RUNTIME).expect("the stepping runtime.toml is written");
    let config = RuntimeConfig::load(&path).expect("the stepping runtime.toml loads");
    let _ = std::fs::remove_dir_all(&folder);
    assert_eq!(STEPPING_EVENTS, 16);
    assert_eq!(config.event_capacity, Some(16));
    assert!(config.control_debug_enabled);
}
