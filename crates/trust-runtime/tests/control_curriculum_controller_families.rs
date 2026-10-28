//! Lesson 42, Controller families: the heater (P1) and the drum axis (P2) under `Control.PID` in one project.
use trust_runtime::debug::DebugControl;
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::SimulationController;
use trust_runtime::value::Value;
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::*;

const PROJECT: &str = "04-controller-selection-and-plant-models/27-controller-selection";

fn instance(runtime: &CompiledRuntime, name: &'static str) -> &'static str {
    assert!(
        exists(runtime, name),
        "the lesson must execute its {name} program"
    );
    name
}

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

#[test]
fn controller_families_hold_both_processes_at_the_hot_reset() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    for update in 0..100 {
        step(&mut runtime, &mut simulation, &debug, update);
        assert_eq!(
            number(&runtime, "%QL0"),
            75.0,
            "Manual holds the heater at 75 %"
        );
        assert!(
            (number(&runtime, "%IL8") - 35.0).abs() <= 0.020001,
            "the heater stays at 35 °C within its noise"
        );
        assert_eq!(
            number(&runtime, "%QL16"),
            0.0,
            "the axis rests without torque"
        );
        assert!(
            number(&runtime, "%IL16").abs() <= 0.001 + 1e-12,
            "the axis stays at 0 rad within its noise"
        );
    }
}

#[test]
fn controller_families_execute_the_gains_control_studio_applies_to_each_loop() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let (heater, axis) = (instance(&runtime, "P1"), instance(&runtime, "P2"));
    for update in 0..10 {
        step(&mut runtime, &mut simulation, &debug, update);
    }
    // Lesson 22's more robust PI for the heater and the Automatic PID for the axis.
    set_program_record(
        &mut runtime,
        heater,
        "Parameters",
        &[("ProportionalGain", 3.27), ("IntegralGain", 0.6407)],
    );
    set_program_record(
        &mut runtime,
        axis,
        "Parameters",
        &[
            ("ProportionalGain", 18.77),
            ("IntegralGain", 17.46),
            ("DerivativeGain", 7.27),
        ],
    );
    for update in 10..13 {
        step(&mut runtime, &mut simulation, &debug, update);
        assert_eq!(
            program_record_number(&runtime, heater, "Parameters", "ProportionalGain"),
            3.27
        );
        assert_eq!(
            program_record_number(&runtime, axis, "Parameters", "DerivativeGain"),
            7.27,
            "cyclic code keeps the gains Control Studio applied"
        );
    }
    let (setpoint_before, pv_before) = (
        controller_state(&runtime, heater, "Setpoint"),
        controller_state(&runtime, heater, "ProcessValue"),
    );
    enqueue_instance_write(&debug, heater, "AutoMode", Value::Bool(true));
    enqueue_instance_write(&debug, axis, "AutoMode", Value::Bool(true));
    scan_every(
        &mut runtime,
        &mut simulation,
        &debug,
        13 * 20,
        20,
        &[("%ML0", lreal(38.0)), ("%ML200", lreal(1.0))],
    );
    // The incremental PI adds Kp·Δ(SP − PV) + Ki·e·Δt to the 75 % it holds.
    let pv = number(&runtime, "%IL8");
    let expected =
        75.0 + 3.27 * ((38.0 - setpoint_before) - (pv - pv_before)) + 0.6407 * 0.02 * (38.0 - pv);
    assert!(
        (number(&runtime, "%QL0") - expected).abs() < 1e-9,
        "the heater's first Auto request uses the applied PI: {} (expected {expected})",
        number(&runtime, "%QL0")
    );
    assert!(
        number(&runtime, "%QL16") > 15.0,
        "the axis's first Auto request pushes toward the 1 rad target with the applied Kp: {} N·m",
        number(&runtime, "%QL16")
    );
}

#[test]
fn controller_families_remove_torque_and_heat_on_their_aborts() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let axis = instance(&runtime, "P2");
    step(&mut runtime, &mut simulation, &debug, 0);
    debug.enqueue_io_write(IoAddress::parse("%ML208").unwrap(), lreal(10.0));
    let mut aborted = false;
    for update in 1..200 {
        step(&mut runtime, &mut simulation, &debug, update);
        if value(&runtime, &format!("{axis}.SafetyAbort")) == Some(Value::Bool(true)) {
            aborted = true;
            assert_eq!(
                number(&runtime, "%QL16"),
                0.0,
                "the axis abort removes the torque"
            );
        }
    }
    assert!(aborted, "leaving the ±2 rad travel range aborts the axis");
    assert_eq!(
        number(&runtime, "%QL0"),
        75.0,
        "the heater keeps its own holding command"
    );
    debug.enqueue_io_write(IoAddress::parse("%IL8").unwrap(), lreal(1000.0));
    step(&mut runtime, &mut simulation, &debug, 200);
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "an unusable temperature removes the heat"
    );
}
