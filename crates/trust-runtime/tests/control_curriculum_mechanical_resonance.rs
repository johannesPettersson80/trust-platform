//! Lesson 59, Mechanical resonance: Lesson 42's `Control.PID` on the drum axis whose load turns through a flexible
//! coupling, measuring the load angle through the notch Control Studio tunes.
use trust_runtime::debug::DebugControl;
use trust_runtime::simulation::SimulationController;
use trust_runtime::value::Value;
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::*;

const PROJECT: &str = "07-classical-stability-and-frequency-design/89-mechanical-resonance";
const CYCLE_S: f64 = 0.02;

fn step(
    runtime: &mut CompiledRuntime,
    simulation: &mut SimulationController,
    debug: &DebugControl,
    update: u64,
) {
    scan_every(runtime, simulation, debug, update * 20, 20, &[]);
}

fn lreal(value: f64) -> Value {
    Value::LWord(value.to_bits())
}

fn aborted(runtime: &CompiledRuntime, program: &str) -> bool {
    value(runtime, &format!("{program}.SafetyAbort")) == Some(Value::Bool(true))
}

/// Auto at the resting angle with the given notch, then a 1 rad setpoint step: the load angle over 8 s and whether the
/// travel limit aborted the run.
fn step_response(frequency_hz: f64, quality: f64) -> (Vec<(f64, f64)>, bool) {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for update in 0..10 {
        step(&mut runtime, &mut simulation, &debug, update);
    }
    enqueue_instance_write(
        &debug,
        program,
        "NotchFrequencyHz",
        Value::LReal(frequency_hz),
    );
    enqueue_instance_write(&debug, program, "NotchQuality", Value::LReal(quality));
    enqueue_instance_write(&debug, program, "AutoMode", Value::Bool(true));
    step(&mut runtime, &mut simulation, &debug, 10);
    scan_every(
        &mut runtime,
        &mut simulation,
        &debug,
        11 * 20,
        20,
        &[("%ML0", lreal(1.0))],
    );
    let mut samples = Vec::new();
    let mut stopped = false;
    for update in 12..412 {
        step(&mut runtime, &mut simulation, &debug, update);
        samples.push(((update - 11) as f64 * CYCLE_S, number(&runtime, "%IL0")));
        stopped |= aborted(&runtime, program);
    }
    (samples, stopped)
}

#[test]
fn mechanical_resonance_rests_at_the_hot_reset() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    for update in 0..100 {
        step(&mut runtime, &mut simulation, &debug, update);
        assert_eq!(number(&runtime, "%QL0"), 0.0, "Manual holds zero torque");
        assert!(
            number(&runtime, "%IL0").abs() <= 0.001 + 1e-12,
            "the load rests at 0 rad within the encoder's ±0.001 rad"
        );
    }
}

#[test]
fn mechanical_resonance_grows_into_an_oscillation_without_the_notch() {
    let (_, stopped) = step_response(0.0, 1.0);
    assert!(
        stopped,
        "without the notch the 3 Hz oscillation grows until the load leaves its travel"
    );
}

#[test]
fn mechanical_resonance_settles_with_the_notch_control_studio_applies() {
    let (samples, stopped) = step_response(3.0, 1.0);
    assert!(!stopped, "the notched loop stays inside the travel");
    for (time, angle) in samples.iter().filter(|(time, _)| *time >= 4.0) {
        assert!(
            (angle - 1.0).abs() <= 0.02,
            "within 0.02 rad of the 1 rad target from 4 s: {angle} at {time} s"
        );
    }
}

#[test]
fn mechanical_resonance_settles_only_with_a_centred_notch_wide_enough() {
    // The eight combinations Control Studio's Tune searches; each must be within 0.02 rad of the target from 4 s.
    for (frequency_hz, quality) in [
        (0.0, 1.0),
        (0.0, 3.0),
        (2.0, 1.0),
        (2.0, 3.0),
        (3.0, 1.0),
        (3.0, 3.0),
        (4.0, 1.0),
        (4.0, 3.0),
    ] {
        let (samples, stopped) = step_response(frequency_hz, quality);
        let settled = !stopped
            && samples
                .iter()
                .filter(|(time, _)| *time >= 4.0)
                .all(|(_, angle)| (angle - 1.0).abs() <= 0.02);
        assert_eq!(
            settled,
            (frequency_hz, quality) == (3.0, 1.0),
            "notch {frequency_hz} Hz, Q = {quality}: settled {settled}"
        );
    }
}

#[test]
fn mechanical_resonance_computes_the_notch_from_its_frequency_and_quality() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    step(&mut runtime, &mut simulation, &debug, 0);
    enqueue_instance_write(&debug, program, "NotchFrequencyHz", Value::LReal(3.0));
    enqueue_instance_write(&debug, program, "NotchQuality", Value::LReal(1.0));
    step(&mut runtime, &mut simulation, &debug, 1);
    let omega = 2.0 * std::f64::consts::PI * 3.0 * CYCLE_S;
    let alpha = omega.sin() / 2.0;
    let b0 = 1.0 / (1.0 + alpha);
    for (name, expected) in [
        ("B0", b0),
        ("B1", -2.0 * omega.cos() * b0),
        ("B2", b0),
        ("A1", -2.0 * omega.cos() * b0),
        ("A2", (1.0 - alpha) * b0),
    ] {
        assert!(
            (program_number(&runtime, program, name) - expected).abs() < 1e-12,
            "{name} = {} (expected {expected})",
            program_number(&runtime, program, name)
        );
    }
}

#[test]
fn mechanical_resonance_excites_seven_tones_in_the_experiment() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    for update in 0..10 {
        step(&mut runtime, &mut simulation, &debug, update);
    }
    for index in 0..100u64 {
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            (10 + index) * 20,
            20,
            &[("%MX96.1", Value::Bool(true))],
        );
        let time = index as f64 * CYCLE_S;
        let expected: f64 = [1.0, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0]
            .iter()
            .map(|hz: &f64| (2.0 * std::f64::consts::PI * hz * time).sin())
            .sum();
        assert!(
            (number(&runtime, "%QL0") - expected).abs() < 1e-9,
            "experiment torque at {time} s: {} (expected {expected})",
            number(&runtime, "%QL0")
        );
    }
}

#[test]
fn mechanical_resonance_removes_torque_outside_the_travel() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    step(&mut runtime, &mut simulation, &debug, 0);
    // Manual drives 5 N·m: the axis turns past the 1.8 rad travel limit within about 1.5 s.
    debug.enqueue_io_write(
        trust_runtime::io::IoAddress::parse("%ML8").unwrap(),
        lreal(5.0),
    );
    let mut stopped = false;
    for update in 1..200 {
        step(&mut runtime, &mut simulation, &debug, update);
        if aborted(&runtime, program) {
            stopped = true;
            assert_eq!(
                number(&runtime, "%QL0"),
                0.0,
                "the abort removes the torque"
            );
        }
    }
    assert!(stopped, "a load angle beyond 1.8 rad aborts");
}
