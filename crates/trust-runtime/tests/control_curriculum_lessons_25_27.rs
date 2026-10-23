use trust_runtime::value::{Duration, Value};

mod control_heater_support;
use control_heater_support::*;

const LESSON25: &str = "03-production-quality-digital-pid/39-controller-mode-initialization";

fn lreal(value: f64) -> Value {
    Value::LWord(value.to_bits())
}

#[test]
fn lesson25_initializes_every_retained_value_from_the_running_heater() {
    let (mut runtime, mut simulation) = setup(LESSON25);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    // Hot reset: the heater holds 35 °C at the 75 % manual command. Check the initializing scan itself.
    scan(&mut runtime, &mut simulation, &debug, 0, &[]);
    assert_eq!(
        number(&runtime, "%QL0"),
        75.0,
        "Manual applies its own command"
    );
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        75.0,
        "the output state starts at the applied command"
    );
    assert_eq!(controller_state(&runtime, program, "SaturatedOutput"), 75.0);
    let temperature = program_number(&runtime, program, "SafeProcessValueC");
    assert_eq!(
        controller_state(&runtime, program, "Setpoint"),
        program_number(&runtime, program, "SetpointC"),
        "the stored setpoint is the current setpoint"
    );
    assert_eq!(
        controller_state(&runtime, program, "ProcessValue"),
        temperature,
        "the stored measurement is the measured temperature"
    );
    assert_eq!(
        program_number(&runtime, program, "FilteredC"),
        temperature,
        "the filter starts at the measurement"
    );
    assert_eq!(controller_state(&runtime, program, "SetpointRate"), 0.0);
    assert_eq!(controller_state(&runtime, program, "ProcessValueRate"), 0.0);
    // Positional view: P ≈ 0 at SP = PV = 35 °C, so I = u − P ≈ 75 %.
    let proportional = program_number(&runtime, program, "ProportionalPct");
    assert!(
        proportional.abs() < 0.6,
        "P is only sensor noise at the operating point: {proportional}"
    );
    assert_eq!(
        program_number(&runtime, program, "IntegralPct"),
        75.0 - proportional,
        "I = u − P"
    );
    assert_eq!(
        runtime.storage().get_instance_var(program, "Initialized"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn lesson25_manual_leaves_the_controller_state_until_it_is_initialized_again() {
    let (mut runtime, mut simulation) = setup(LESSON25);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..3 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        75.0,
        "initialized at the 75 % hot-reset command"
    );
    // The operator raises the manual command: the controller is not updated in Manual.
    for step in 3..10 {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            &[("%ML72", lreal(85.0))],
        );
    }
    assert_eq!(number(&runtime, "%QL0"), 85.0, "Manual applies 85 %");
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        75.0,
        "the retained output still holds the 75 % from initialization"
    );
    // One explicit initialization request re-aligns every retained value, once.
    debug.enqueue_instance_write(program, "InitializeRequest", Value::Bool(true));
    scan(&mut runtime, &mut simulation, &debug, 1000, &[]);
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        85.0,
        "re-initialized at the applied 85 %"
    );
    for step in 11..15 {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            &[("%ML72", lreal(90.0))],
        );
    }
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        85.0,
        "a request that stays TRUE initializes only once"
    );
    // A new request takes the command actually applied: 120 % is limited to 100 %.
    debug.enqueue_instance_write(program, "InitializeRequest", Value::Bool(false));
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        1500,
        &[("%ML72", lreal(120.0))],
    );
    debug.enqueue_instance_write(program, "InitializeRequest", Value::Bool(true));
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        1600,
        &[("%ML72", lreal(120.0))],
    );
    assert_eq!(number(&runtime, "%QL0"), 100.0);
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        100.0,
        "initialized at the applied 100 %, not the requested 120 %"
    );
}

#[test]
fn lesson25_first_automatic_command_continues_the_applied_command() {
    let (mut runtime, mut simulation) = setup(LESSON25);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..5 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    scan(&mut runtime, &mut simulation, &debug, 500, &[]);
    let first = number(&runtime, "%QL0");
    // Only sensor noise (±0.02 °C) separates the stored and the fresh measurement: |ΔP| ≤ 13·0.04.
    assert!(
        (first - 75.0).abs() < 0.6,
        "the first automatic command continues the applied 75 %, got {first}"
    );
}

#[test]
fn lesson25_a_zeroed_controller_state_switches_the_heater_off() {
    let (mut runtime, mut simulation) = setup(LESSON25);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..5 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    // Stored values that do not describe the running heater: output, setpoint and measurement all 0.
    set_controller_state(
        &mut runtime,
        program,
        &[
            ("Setpoint", 0.0),
            ("SetpointRate", 0.0),
            ("ProcessValue", 0.0),
            ("ProcessValueRate", 0.0),
            ("FeedForward", 0.0),
            ("NominalOutput", 0.0),
            ("SaturatedOutput", 0.0),
        ],
    );
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    scan(&mut runtime, &mut simulation, &debug, 500, &[]);
    // ΔP = 13·(0.3·35 − 35) = −318.5 points from the zero references: the request is far below 0 %.
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "the first automatic command switches the heater off"
    );
}

#[test]
fn lesson25_verifies_from_the_state_control_studio_presets() {
    let (mut runtime, mut simulation) = setup(LESSON25);
    let program = program(&runtime);
    // control-loop.json pidInitialization: Control Studio presets the seven retained values and
    // P1.Initialized, then starts its own clock at 0.
    set_controller_state(
        &mut runtime,
        program,
        &[
            ("Setpoint", 35.0),
            ("SetpointRate", 0.0),
            ("ProcessValue", 35.0),
            ("ProcessValueRate", 0.0),
            ("FeedForward", 0.0),
            ("NominalOutput", 75.0),
            ("SaturatedOutput", 75.0),
        ],
    );
    assert!(runtime
        .write_instance(program, "Initialized", Value::Bool(true))
        .is_ok());
    assert!(runtime
        .write_instance(program, "AutoMode", Value::Bool(true))
        .is_ok());
    runtime
        .write_io(
            &trust_runtime::io::IoAddress::parse("%ML0").unwrap(),
            lreal(38.0),
        )
        .unwrap();
    assert!(runtime
        .write_instance(program, "SetpointC", Value::LReal(38.0))
        .is_ok());
    let mut outputs = Vec::new();
    for sequence in 0..30_i64 {
        let now = Duration::from_millis(sequence * 100);
        runtime.set_current_time(now);
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("cycle {sequence}: {error:?}"));
        outputs.push(number(&runtime, "%QL0"));
        simulation.apply_post_cycle(now, &runtime).unwrap();
    }
    assert_eq!(
        outputs[1], 75.0,
        "the first executed scan has no interval: the preset command is held"
    );
    assert!(
        outputs[2] > 75.0,
        "the controller acts on the 3 °C setpoint step from the next scan: {outputs:?}"
    );
}

const LESSON26: &str = "03-production-quality-digital-pid/40-bumpless-transfer";

/// Starts at the 75 % hot reset, then holds the manual command at 60 % until `until_step`.
fn lesson26_manual_hold(
    until_step: u64,
    track_manual: bool,
) -> (
    trust_runtime::Runtime,
    trust_runtime::simulation::SimulationController,
    trust_runtime::debug::DebugControl,
    trust_runtime::memory::InstanceId,
) {
    let (mut runtime, mut simulation) = setup(LESSON26);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    if !track_manual {
        assert!(
            runtime
                .write_instance(program, "TrackManual", Value::Bool(false))
                .is_ok(),
            "TrackManual selects the untracked counterexample"
        );
    }
    for step in 0..3 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    for step in 3..until_step {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            &[("%ML72", lreal(60.0))],
        );
    }
    (runtime, simulation, debug, program)
}

#[test]
fn lesson26_manual_tracks_the_applied_command() {
    let (runtime, _, _, program) = lesson26_manual_hold(30, true);
    assert_eq!(number(&runtime, "%QL0"), 60.0);
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        60.0,
        "the stored output follows the applied manual command"
    );
    assert_eq!(controller_state(&runtime, program, "SaturatedOutput"), 60.0);
    assert_eq!(
        controller_state(&runtime, program, "ProcessValue"),
        program_number(&runtime, program, "SafeProcessValueC"),
        "the stored measurement follows the temperature"
    );
    assert_eq!(
        controller_state(&runtime, program, "Setpoint"),
        program_number(&runtime, program, "SetpointC")
    );
}

#[test]
fn lesson26_transfer_continues_the_manual_command_without_an_integral_step() {
    let (mut runtime, mut simulation, debug, program) = lesson26_manual_hold(30, true);
    let error = program_number(&runtime, program, "SetpointC")
        - program_number(&runtime, program, "SafeProcessValueC");
    assert!(
        error > 0.3,
        "the 60 % hold cooled the water below the 35 °C setpoint: e = {error}"
    );
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    scan(&mut runtime, &mut simulation, &debug, 3000, &[]);
    let first = number(&runtime, "%QL0");
    assert!(
        (first - 60.0).abs() < 1e-9,
        "the transfer scan reconstructs the applied 60 % with no integral step: got {first}"
    );
    scan(&mut runtime, &mut simulation, &debug, 3100, &[]);
    let second = number(&runtime, "%QL0");
    assert!(
        second > first + 0.2,
        "ordinary PI resumes on the next scan and raises the command: {first} → {second}"
    );
}

#[test]
fn lesson26_untracked_transfer_jumps_to_the_stale_output() {
    let (mut runtime, mut simulation, debug, program) = lesson26_manual_hold(30, false);
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        75.0,
        "without tracking the stored output stays at 75 %"
    );
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    scan(&mut runtime, &mut simulation, &debug, 3000, &[]);
    let first = number(&runtime, "%QL0");
    assert!(
        first - 60.0 > 10.0,
        "the stale stored values make the command jump from 60 %: got {first}"
    );
}

#[test]
fn lesson26_verifies_from_the_state_control_studio_presets() {
    let (mut runtime, mut simulation) = setup(LESSON26);
    let program = program(&runtime);
    // Control Studio presets the stored values and P1.Initialized and starts the program directly in Auto.
    set_controller_state(
        &mut runtime,
        program,
        &[
            ("Setpoint", 35.0),
            ("SetpointRate", 0.0),
            ("ProcessValue", 35.0),
            ("ProcessValueRate", 0.0),
            ("FeedForward", 0.0),
            ("NominalOutput", 75.0),
            ("SaturatedOutput", 75.0),
        ],
    );
    for flag in ["Initialized", "AutoMode"] {
        assert!(
            runtime
                .write_instance(program, flag, Value::Bool(true))
                .is_ok(),
            "{flag}"
        );
    }
    runtime
        .write_io(
            &trust_runtime::io::IoAddress::parse("%ML0").unwrap(),
            lreal(38.0),
        )
        .unwrap();
    assert!(runtime
        .write_instance(program, "SetpointC", Value::LReal(38.0))
        .is_ok());
    let mut outputs = Vec::new();
    for sequence in 0..10_i64 {
        let now = Duration::from_millis(sequence * 100);
        runtime.set_current_time(now);
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("cycle {sequence}: {error:?}"));
        outputs.push(number(&runtime, "%QL0"));
        simulation.apply_post_cycle(now, &runtime).unwrap();
    }
    assert_eq!(
        outputs[1], 75.0,
        "the first executed scan has no interval: the preset command is held"
    );
    // The setpoint step happens in Auto: 75 + 13·0.3·3 + 3.5·3·0.1 = 87.75 %, give or take the noise.
    assert!(
        (outputs[2] - 87.75).abs() < 0.6,
        "the setpoint kick is kept, not absorbed by a transfer: {outputs:?}"
    );
}

const LESSON27: &str = "03-production-quality-digital-pid/41-atomic-parameter-update";

/// Hot reset, Auto at 0.5 s and the setpoint at 38 °C until `until_step`: the heater then holds 38 °C at about 90 %.
fn lesson27_at_38c(
    until_step: u64,
) -> (
    trust_runtime::Runtime,
    trust_runtime::simulation::SimulationController,
    trust_runtime::debug::DebugControl,
    trust_runtime::memory::InstanceId,
) {
    let (mut runtime, mut simulation) = setup(LESSON27);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..5 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    for step in 5..until_step {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            &[("%ML0", lreal(38.0))],
        );
    }
    (runtime, simulation, debug, program)
}

#[test]
fn lesson27_rejects_an_invalid_parameter_set_and_keeps_the_active_one() {
    let (mut runtime, mut simulation, debug, program) = lesson27_at_38c(400);
    let revision = program_dint(&runtime, program, "ParameterRevision");
    let before = number(&runtime, "%QL0");
    // Each limit is inside its own 0–100 % bound, but the set is inconsistent: minimum above maximum.
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[("OutputMinimum", 80.0), ("OutputMaximum", 20.0)],
    );
    for step in 400..405 {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            &[("%ML0", lreal(38.0))],
        );
    }
    assert_eq!(
        program_dint(&runtime, program, "ParameterRevision"),
        revision,
        "an invalid set is not adopted"
    );
    let after = number(&runtime, "%QL0");
    assert!(
        (after - before).abs() < 1.0,
        "the controller keeps running on the active set: {before} → {after}"
    );
    assert_eq!(
        runtime
            .storage()
            .get_instance_var(program, "ParametersRejected"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn lesson27_adopts_a_valid_set_at_one_boundary_without_moving_the_command() {
    let (mut runtime, mut simulation, debug, program) = lesson27_at_38c(400);
    let revision = program_dint(&runtime, program, "ParameterRevision");
    let before = number(&runtime, "%QL0");
    let integral_before = program_number(&runtime, program, "IntegralPct");
    let temperature = program_number(&runtime, program, "FilteredC");
    // Same snapshot, both sets: P = Kp·(0.3·(38 − 35) − (PV − 35)).
    let p_old = 13.0 * (0.3 * 3.0 - (temperature - 35.0));
    let p_new = 9.0 * (0.3 * 3.0 - (temperature - 35.0));
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[("ProportionalGain", 9.0)],
    );
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        40_000,
        &[("%ML0", lreal(38.0))],
    );
    assert_eq!(
        program_dint(&runtime, program, "ParameterRevision"),
        revision + 1,
        "the complete set is adopted once"
    );
    let after = number(&runtime, "%QL0");
    assert!(
        (after - before).abs() < 0.8,
        "only the ordinary increment moves the command: {before} → {after}"
    );
    let rebased = program_number(&runtime, program, "IntegralPct") - integral_before;
    assert!(
        (rebased - (p_old - p_new)).abs() < 0.8,
        "I_new = I_old + P_old − P_new: {rebased} vs {}",
        p_old - p_new
    );
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        40_100,
        &[("%ML0", lreal(38.0))],
    );
    assert_eq!(
        program_dint(&runtime, program, "ParameterRevision"),
        revision + 1,
        "an unchanged set is not adopted again"
    );
}

#[test]
fn lesson27_a_lowered_ceiling_deliberately_limits_the_command() {
    let (mut runtime, mut simulation, debug, program) = lesson27_at_38c(400);
    assert!(
        number(&runtime, "%QL0") > 85.0,
        "holding 38 °C needs about 90 %"
    );
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[("OutputMaximum", 80.0)],
    );
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        40_000,
        &[("%ML0", lreal(38.0))],
    );
    assert_eq!(
        number(&runtime, "%QL0"),
        80.0,
        "the new ceiling limits the command at once: a deliberate change"
    );
}

#[test]
fn lesson27_adopts_the_filter_time_with_the_set_and_rejects_a_negative_one() {
    let (mut runtime, mut simulation, debug, program) = lesson27_at_38c(400);
    let revision = program_dint(&runtime, program, "ParameterRevision");
    assert!(runtime
        .write_instance(program, "FilterTimeS", Value::LReal(-1.0))
        .is_ok());
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        40_000,
        &[("%ML0", lreal(38.0))],
    );
    assert_eq!(
        program_dint(&runtime, program, "ParameterRevision"),
        revision,
        "a negative filter time is rejected"
    );
    assert!(
        (program_number(&runtime, program, "FilterAlpha") - 0.1 / 1.1).abs() < 1e-9,
        "the filter keeps Tf = 1 s"
    );
    assert!(runtime
        .write_instance(program, "FilterTimeS", Value::LReal(2.0))
        .is_ok());
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        40_100,
        &[("%ML0", lreal(38.0))],
    );
    assert_eq!(
        program_dint(&runtime, program, "ParameterRevision"),
        revision + 1,
        "Tf belongs to the adopted set"
    );
    assert!(
        (program_number(&runtime, program, "FilterAlpha") - 0.1 / 2.1).abs() < 1e-9,
        "the filter uses Tf = 2 s"
    );
}

#[test]
fn lesson27_verifies_from_the_state_control_studio_presets() {
    let (mut runtime, mut simulation) = setup(LESSON27);
    let program = program(&runtime);
    set_controller_state(
        &mut runtime,
        program,
        &[
            ("Setpoint", 35.0),
            ("SetpointRate", 0.0),
            ("ProcessValue", 35.0),
            ("ProcessValueRate", 0.0),
            ("FeedForward", 0.0),
            ("NominalOutput", 75.0),
            ("SaturatedOutput", 75.0),
        ],
    );
    for flag in ["Initialized", "AutoMode"] {
        assert!(
            runtime
                .write_instance(program, flag, Value::Bool(true))
                .is_ok(),
            "{flag}"
        );
    }
    runtime
        .write_io(
            &trust_runtime::io::IoAddress::parse("%ML0").unwrap(),
            lreal(38.0),
        )
        .unwrap();
    assert!(runtime
        .write_instance(program, "SetpointC", Value::LReal(38.0))
        .is_ok());
    let mut outputs = Vec::new();
    for sequence in 0..10_i64 {
        let now = Duration::from_millis(sequence * 100);
        runtime.set_current_time(now);
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("cycle {sequence}: {error:?}"));
        outputs.push(number(&runtime, "%QL0"));
        simulation.apply_post_cycle(now, &runtime).unwrap();
    }
    assert_eq!(outputs[1], 75.0);
    assert!(
        (outputs[2] - 87.75).abs() < 0.6,
        "the setpoint kick of the Auto run: {outputs:?}"
    );
}
