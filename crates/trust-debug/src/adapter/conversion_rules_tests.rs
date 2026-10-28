//! The simulator builds the project with the conversion rules of its `vendor_profile`
//! (docs/specs/07-standard-functions.md 2.10), as `trust-runtime run` does. The test needs the
//! toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use trust_runtime::value::Value;

use super::simulation_speed_tests::{project_with, published_output, simulator};
use super::tests_part_02::reserve_loopback_addr;

/// Converts 2.5 to an `INT`, which it publishes at `%QW0`.
const ROUNDING: &str = "PROGRAM Main\nVAR\n    Half : REAL := 2.5;\n    Rounded AT %QW0 : INT;\nEND_VAR\nRounded := REAL_TO_INT(Half);\nEND_PROGRAM\n";

/// Specification 07 §2.10, worked from its table: `REAL_TO_INT(2.5)` is 3 under the CODESYS rules
/// (half away from zero), which `codesys` selects, and 2 under the IEC rules (ties to even), which
/// `siemens` selects. Catches a debug session whose program is built with the IEC rules whatever
/// its project names.
#[test]
fn the_simulator_builds_with_the_conversion_rules_of_the_project_vendor_profile() {
    for (vendor_profile, rounded) in [("codesys", 3), ("siemens", 2)] {
        let root = project_with(&format!("rules-{vendor_profile}"), ROUNDING, None, None);
        fs::write(
            root.join("trust-lsp.toml"),
            format!("[project]\nvendor_profile = \"{vendor_profile}\"\n"),
        )
        .unwrap();
        let (_session, simulator) = simulator(&root, reserve_loopback_addr(), "rules-token");
        let state = simulator.control_server().state();
        let deadline = Instant::now() + Duration::from_secs(10);
        let published = loop {
            let value = published_output(&state, "%QW0");
            if value.is_some() || Instant::now() >= deadline {
                break value;
            }
            thread::sleep(Duration::from_millis(10));
        };
        simulator.stop();
        let published = match published {
            Some(Value::Int(value)) => Some(value),
            Some(Value::Word(bits)) => Some(bits.cast_signed()),
            _ => None,
        };
        assert_eq!(published, Some(rounded), "{vendor_profile}");
        fs::remove_dir_all(root).ok();
    }
}
