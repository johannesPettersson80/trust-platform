//! Lesson 44, Level control: `Control.PID` sets the inlet pump of the buffer tank that the outlet pump draws from.
use trust_runtime::debug::DebugControl;
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::SimulationController;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

mod control_heater_support;
use control_heater_support::*;

const PROJECT: &str = "01-feedback-foundations-and-temperature-control/87-level-control";

fn step(
    runtime: &mut Runtime,
    simulation: &mut SimulationController,
    debug: &DebugControl,
    update: u64,
) {
    scan_every(runtime, simulation, debug, update * 100, 100, &[]);
}

fn lreal(value: f64) -> Value {
    Value::LWord(value.to_bits())
}

#[test]
fn level_control_holds_the_tank_at_the_hot_reset() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    for update in 0..100 {
        step(&mut runtime, &mut simulation, &debug, update);
        assert_eq!(
            number(&runtime, "%QL0"),
            30.0,
            "Manual runs the inlet pump at 30 %"
        );
        assert_eq!(number(&runtime, "%QL8"), 30.0, "the outlet pump draws 30 %");
        assert!(
            (number(&runtime, "%IL0") - 1.0).abs() <= 0.0005 + 1e-12,
            "equal flows hold 1.0 m within the sensor's ±0.0005 m"
        );
    }
}

#[test]
fn level_control_executes_the_pi_control_studio_applies() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for update in 0..10 {
        step(&mut runtime, &mut simulation, &debug, update);
    }
    // SIMC's more robust PI for the tank: Kp = 125 %/m, Ti = 40 s.
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[("ProportionalGain", 125.0), ("IntegralGain", 3.125)],
    );
    for update in 10..13 {
        step(&mut runtime, &mut simulation, &debug, update);
        assert_eq!(
            program_record_number(&runtime, program, "Parameters", "ProportionalGain"),
            125.0,
            "cyclic code keeps the gains Control Studio applied"
        );
    }
    let (setpoint_before, pv_before) = (
        controller_state(&runtime, program, "Setpoint"),
        controller_state(&runtime, program, "ProcessValue"),
    );
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    scan_every(
        &mut runtime,
        &mut simulation,
        &debug,
        13 * 100,
        100,
        &[("%ML0", lreal(1.1))],
    );
    let pv = number(&runtime, "%IL0");
    let expected =
        30.0 + 125.0 * ((1.1 - setpoint_before) - (pv - pv_before)) + 3.125 * 0.1 * (1.1 - pv);
    assert!(
        (number(&runtime, "%QL0") - expected).abs() < 1e-9,
        "the first Auto request uses the applied PI: {} (expected {expected})",
        number(&runtime, "%QL0")
    );
}

#[test]
fn level_control_stops_both_pumps_outside_the_level_range() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    step(&mut runtime, &mut simulation, &debug, 0);
    // Manual runs the inlet pump flat out: the level rises 0.0008 · 70 = 0.056 m/s toward the 1.8 m limit.
    debug.enqueue_io_write(IoAddress::parse("%ML8").unwrap(), lreal(100.0));
    let mut aborted = false;
    for update in 1..300 {
        step(&mut runtime, &mut simulation, &debug, update);
        if runtime.storage().get_instance_var(program, "SafetyAbort") == Some(&Value::Bool(true)) {
            aborted = true;
            assert_eq!(
                number(&runtime, "%QL0"),
                0.0,
                "the abort stops the inlet pump"
            );
            assert_eq!(number(&runtime, "%QL8"), 0.0, "and the outlet pump");
        }
    }
    assert!(aborted, "a level above 1.8 m aborts");
}
