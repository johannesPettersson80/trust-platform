//! Specification 35, B9.2: the label of the selected target, from the speed words the runtime's status report carries. The speed
//! words themselves are the runtime's rule (`speed_tests.rs` in the runtime); these tests pin how the engine names the target.

use serde_json::json;

use super::*;

fn status(enabled: bool, only_io: bool, speed_text: Option<&str>) -> RuntimeStatus {
    let mut report = json!({
        "state": "running", "session_epoch": 1, "simulation_enabled": enabled,
        "simulation_only_io": only_io, "simulation_time_scale": 1,
        "program_revision": "sha256:program", "tasks": [], "cycle_interval_ms": 10
    });
    if let Some(text) = speed_text {
        report["simulation_speed_text"] = json!(text);
    }
    serde_json::from_value(report).expect("the status decodes")
}

#[test]
fn a_target_that_is_not_fully_simulated_is_physical_whatever_its_speed_words() {
    assert_eq!(
        target_label(&status(false, true, Some("\u{00D7}10"))),
        "Physical"
    );
    assert_eq!(
        target_label(&status(true, false, Some("\u{00D7}10"))),
        "Physical",
        "an attached I/O that is not simulated makes the target physical"
    );
}

#[test]
fn a_simulation_without_speed_words_is_named_without_a_speed() {
    assert_eq!(target_label(&status(true, true, Some(""))), "Simulation");
    assert_eq!(
        target_label(&status(true, true, None)),
        "Simulation",
        "an answer without the speed words names the simulation without a speed"
    );
}

#[test]
fn a_simulation_names_the_speed_words_the_runtime_gives() {
    assert_eq!(
        target_label(&status(true, true, Some("\u{00D7}10"))),
        "Simulation \u{00D7}10"
    );
    assert_eq!(
        target_label(&status(true, true, Some("\u{00D7}7 of \u{00D7}10"))),
        "Simulation \u{00D7}7 of \u{00D7}10"
    );
}
