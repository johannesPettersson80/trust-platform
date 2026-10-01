use trust_runtime::value::{Duration, Value};

mod control_heater_support;
use control_heater_support::*;

const LESSON28: &str = "03-production-quality-digital-pid/42-invalid-stale-data";

fn lreal(value: f64) -> Value {
    Value::LWord(value.to_bits())
}

fn flag(
    runtime: &trust_runtime::Runtime,
    program: trust_runtime::memory::InstanceId,
    name: &str,
) -> bool {
    match runtime.storage().get_instance_var(program, name) {
        Some(Value::Bool(value)) => *value,
        other => panic!("{name}: {other:?}"),
    }
}

/// Hot reset, then Auto from 0.5 s at the 35 °C setpoint until `until_step`.
fn lesson28_in_auto(
    until_step: u64,
) -> (
    trust_runtime::Runtime,
    trust_runtime::simulation::SimulationController,
    trust_runtime::debug::DebugControl,
    trust_runtime::memory::InstanceId,
) {
    let (mut runtime, mut simulation) = setup(LESSON28);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..5 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    for step in 5..until_step {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    assert!(flag(&runtime, program, "AutoMode"));
    (runtime, simulation, debug, program)
}

#[test]
fn lesson28_bad_quality_removes_heat_at_once_and_freezes_the_state() {
    let (mut runtime, mut simulation, debug, program) = lesson28_in_auto(100);
    let stored_output = controller_state(&runtime, program, "NominalOutput");
    let filtered = program_number(&runtime, program, "FilteredC");
    assert!(runtime.storage_mut().set_instance_var(
        program,
        "SensorQualityGood",
        Value::Bool(false)
    ));
    scan(&mut runtime, &mut simulation, &debug, 10_000, &[]);
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "the fault removes heat in the scan that detects it"
    );
    assert!(flag(&runtime, program, "MeasurementFault"));
    assert!(
        !flag(&runtime, program, "AutoMode"),
        "the Auto request is cleared"
    );
    assert_eq!(
        program_number(&runtime, program, "ManualOutput"),
        0.0,
        "the manual demand is cleared"
    );
    for step in 101..120 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        stored_output,
        "no controller update during the fault"
    );
    assert_eq!(
        program_number(&runtime, program, "FilteredC"),
        filtered,
        "the filter is frozen"
    );
    assert_eq!(number(&runtime, "%QL0"), 0.0);
}

#[test]
fn lesson28_a_frozen_sensor_fails_once_its_reading_is_older_than_half_a_second() {
    let (mut runtime, mut simulation, debug, program) = lesson28_in_auto(100);
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "SensorFrozen", Value::Bool(true)));
    for scans in 1..=5 {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            (100 + scans) * 100,
            &[],
        );
        assert!(
            number(&runtime, "%QL0") > 0.0,
            "{:.1} s old is within the 0.5 s limit",
            scans as f64 * 0.1
        );
    }
    scan(&mut runtime, &mut simulation, &debug, 10_600, &[]);
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "a 0.6 s old reading fails the 0.5 s limit and removes heat"
    );
    assert!(flag(&runtime, program, "MeasurementFault"));
}

#[test]
fn lesson28_recovers_after_acknowledgement_and_three_fresh_valid_samples() {
    let (mut runtime, mut simulation, debug, program) = lesson28_in_auto(100);
    let set = |runtime: &mut trust_runtime::Runtime, name: &str, value: Value| {
        assert!(
            runtime.storage_mut().set_instance_var(program, name, value),
            "{name}"
        );
    };
    let mut step = 100;
    let mut next =
        |runtime: &mut trust_runtime::Runtime,
         simulation: &mut trust_runtime::simulation::SimulationController| {
            scan(runtime, simulation, &debug, step * 100, &[]);
            step += 1;
        };
    // An out-of-range reading (about 85 °C) latches the fault.
    set(&mut runtime, "InjectedOffsetC", Value::LReal(50.0));
    next(&mut runtime, &mut simulation);
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "an out-of-range reading removes heat"
    );
    assert!(flag(&runtime, program, "MeasurementFault"));
    set(&mut runtime, "InjectedOffsetC", Value::LReal(0.0));
    for _ in 0..5 {
        next(&mut runtime, &mut simulation);
    }
    assert!(
        flag(&runtime, program, "MeasurementFault"),
        "valid readings alone do not clear the fault"
    );
    // After the acknowledgement, repeated readings of a frozen sensor do not count.
    set(&mut runtime, "FaultAcknowledge", Value::Bool(true));
    set(&mut runtime, "SensorFrozen", Value::Bool(true));
    for _ in 0..3 {
        next(&mut runtime, &mut simulation);
    }
    assert_eq!(
        program_dint(&runtime, program, "RecoveryCount"),
        0,
        "old readings are not fresh acquisitions"
    );
    set(&mut runtime, "SensorFrozen", Value::Bool(false));
    next(&mut runtime, &mut simulation);
    next(&mut runtime, &mut simulation);
    assert_eq!(program_dint(&runtime, program, "RecoveryCount"), 2);
    set(&mut runtime, "SensorQualityGood", Value::Bool(false));
    next(&mut runtime, &mut simulation);
    assert_eq!(
        program_dint(&runtime, program, "RecoveryCount"),
        0,
        "a bad sample restarts the count"
    );
    set(&mut runtime, "SensorQualityGood", Value::Bool(true));
    next(&mut runtime, &mut simulation);
    next(&mut runtime, &mut simulation);
    assert!(
        flag(&runtime, program, "MeasurementFault"),
        "two fresh samples are not enough"
    );
    next(&mut runtime, &mut simulation);
    assert!(
        !flag(&runtime, program, "MeasurementFault"),
        "the third consecutive fresh valid sample recovers"
    );
    // Recovery re-initializes from the actual condition: no heat applied, Manual, no Auto request.
    assert!(flag(&runtime, program, "Initialized") && !flag(&runtime, program, "AutoMode"));
    assert_eq!(number(&runtime, "%QL0"), 0.0);
    assert_eq!(
        program_number(&runtime, program, "ManualOutput"),
        0.0,
        "the old manual demand is not restored"
    );
    assert_eq!(
        controller_state(&runtime, program, "NominalOutput"),
        0.0,
        "the stored output is the applied 0 %"
    );
    assert_eq!(
        program_number(&runtime, program, "FilteredC"),
        program_number(&runtime, program, "SafeProcessValueC"),
        "the filter restarts from the fresh measurement"
    );
    // Only a new Auto request resumes control, from the applied 0 % without a jump.
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    next(&mut runtime, &mut simulation);
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "the transfer scan continues the applied 0 %"
    );
    next(&mut runtime, &mut simulation);
    assert!(
        number(&runtime, "%QL0") > 0.0,
        "PI raises the command for the cooled water"
    );
}

#[test]
fn lesson28_verifies_from_the_state_control_studio_presets() {
    let (mut runtime, mut simulation) = setup(LESSON28);
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
    for name in ["Initialized", "AutoMode"] {
        assert!(
            runtime
                .storage_mut()
                .set_instance_var(program, name, Value::Bool(true)),
            "{name}"
        );
    }
    runtime
        .io_mut()
        .write(
            &trust_runtime::io::IoAddress::parse("%ML0").unwrap(),
            lreal(38.0),
        )
        .unwrap();
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "SetpointC", Value::LReal(38.0)));
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

#[test]
fn lesson28_hmi_only_binds_signals_of_the_executed_heater() {
    let (runtime, _) = setup(LESSON28);
    let diagnostics = hmi_diagnostics(LESSON28, &runtime);
    assert!(
        diagnostics.is_empty(),
        "The Lesson 29 HMI must bind the executed heater project: {diagnostics:?}"
    );
}

const LESSON29: &str = "14-integrated-capstones/81-water-heater";

/// Hot reset, then Auto from 0.5 s until `until_step`, with the given setpoint and extraction.
fn lesson29_in_auto(
    until_step: u64,
    setpoint: f64,
    load_kw: f64,
) -> (
    trust_runtime::Runtime,
    trust_runtime::simulation::SimulationController,
    trust_runtime::debug::DebugControl,
    trust_runtime::memory::InstanceId,
) {
    let (mut runtime, mut simulation) = setup(LESSON29);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..5 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    assert!(runtime.storage_mut().set_instance_var(
        program,
        "DisturbanceKw",
        Value::LReal(load_kw)
    ));
    for step in 5..until_step {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            &[("%ML0", lreal(setpoint))],
        );
    }
    (runtime, simulation, debug, program)
}

#[test]
fn lesson29_the_installed_heater_runs_the_introductory_pi() {
    let (runtime, _, _, program) = lesson29_in_auto(10, 35.0, 0.0);
    for (field, expected) in [
        ("ProportionalGain", 2.0),
        ("IntegralGain", 0.5),
        ("ProportionalSetpointWeight", 1.0),
    ] {
        assert_eq!(
            program_record_number(&runtime, program, "ConfiguredParameters", field),
            expected,
            "{field}"
        );
    }
    assert_eq!(
        program_number(&runtime, program, "ConfiguredFilterTimeS"),
        0.0,
        "no measurement filter yet"
    );
}

#[test]
fn lesson29_commissions_the_selected_controller_at_one_boundary() {
    let (mut runtime, mut simulation, debug, program) = lesson29_in_auto(300, 35.0, 0.0);
    let revision = program_dint(&runtime, program, "ParameterRevision");
    let before = number(&runtime, "%QL0");
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[
            ("ProportionalGain", 13.0),
            ("IntegralGain", 3.5),
            ("ProportionalSetpointWeight", 0.3),
        ],
    );
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "FilterTimeS", Value::LReal(1.0)));
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        30_000,
        &[("%ML0", lreal(35.0))],
    );
    assert_eq!(
        program_dint(&runtime, program, "ParameterRevision"),
        revision + 1,
        "the selected set is adopted at one boundary"
    );
    assert_eq!(
        program_record_number(
            &runtime,
            program,
            "ConfiguredParameters",
            "ProportionalGain"
        ),
        13.0
    );
    assert_eq!(
        program_number(&runtime, program, "ConfiguredFilterTimeS"),
        1.0
    );
    let after = number(&runtime, "%QL0");
    assert!(
        (after - before).abs() < 1.0,
        "the command continues across the update: {before} → {after}"
    );
}

#[test]
fn lesson29_capacity_holds_38c_under_30kw_only_to_37c() {
    // u_hold = (H(T − Ta) + D)/a = (10·(38 − 20) + 30)/2 = 105 %: beyond the heater. At 100 %, T = 20 + (200 − 30)/10 = 37 °C.
    let (runtime, _, _, program) = lesson29_in_auto(1200, 38.0, 30.0);
    assert_eq!(number(&runtime, "%QL0"), 100.0, "the command saturates");
    let temperature = program_number(&runtime, program, "WaterTemperatureC");
    assert!(
        (temperature - 37.0).abs() < 0.1,
        "the water settles at the 37 °C capacity limit: {temperature}"
    );
}

#[test]
fn lesson29_verifies_from_the_state_control_studio_presets() {
    let (mut runtime, mut simulation) = setup(LESSON29);
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
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[
            ("ProportionalGain", 13.0),
            ("IntegralGain", 3.5),
            ("ProportionalSetpointWeight", 0.3),
        ],
    );
    for name in ["Initialized", "AutoMode"] {
        assert!(
            runtime
                .storage_mut()
                .set_instance_var(program, name, Value::Bool(true)),
            "{name}"
        );
    }
    runtime
        .io_mut()
        .write(
            &trust_runtime::io::IoAddress::parse("%ML0").unwrap(),
            lreal(38.0),
        )
        .unwrap();
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "SetpointC", Value::LReal(38.0)));
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
        "the candidate's setpoint kick: {outputs:?}"
    );
}

#[test]
fn lesson29_hmi_only_binds_signals_of_the_executed_heater() {
    let (runtime, _) = setup(LESSON29);
    let diagnostics = hmi_diagnostics(LESSON29, &runtime);
    assert!(
        diagnostics.is_empty(),
        "The Lesson 30 HMI must bind the executed heater project: {diagnostics:?}"
    );
}

const LESSON30: &str =
    "01-feedback-foundations-and-temperature-control/08-heating-valve-rate-tracking";

/// Hot reset in Manual at 75 %, then Auto and a 35 → 37 °C setpoint step, recorded for `seconds`.
/// Returns (valve request, stem position, temperature) per scan.
fn lesson30_setpoint_step(track_measured: bool, seconds: u64) -> Vec<(f64, f64, f64)> {
    let (mut runtime, mut simulation) = setup(LESSON30);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    if !track_measured {
        assert!(runtime.storage_mut().set_instance_var(
            program,
            "TrackMeasuredPosition",
            Value::Bool(false)
        ));
    }
    for step in 0..5 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    let mut trace = Vec::new();
    for step in 5..(5 + seconds * 10) {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            &[("%ML0", lreal(37.0))],
        );
        trace.push((
            number(&runtime, "%QL0"),
            number(&runtime, "%IL16"),
            number(&runtime, "%IL8"),
        ));
    }
    trace
}

#[test]
fn lesson30_the_valve_stem_moves_at_half_a_percent_per_second() {
    let (mut runtime, mut simulation) = setup(LESSON30);
    let debug = runtime.enable_debug();
    for step in 0..3 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    assert_eq!(
        number(&runtime, "%IL16"),
        75.0,
        "the stem starts at the 75 % hot-reset position"
    );
    for step in 3..103 {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            &[("%ML72", lreal(85.0))],
        );
    }
    assert_eq!(number(&runtime, "%QL0"), 85.0, "the request jumps to 85 %");
    let stem = number(&runtime, "%IL16");
    assert!(
        (stem - 80.0).abs() < 0.11,
        "after 10 s the stem has moved only 5 points: {stem}"
    );
    for step in 103..210 {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            &[("%ML72", lreal(85.0))],
        );
    }
    assert_eq!(
        number(&runtime, "%IL16"),
        85.0,
        "after 20 s the stem reaches 85 %"
    );
}

#[test]
fn lesson30_measured_tracking_keeps_the_request_near_the_stem() {
    let lead = |trace: &[(f64, f64, f64)]| {
        trace
            .iter()
            .map(|(request, stem, _)| request - stem)
            .fold(f64::MIN, f64::max)
    };
    let tracked = lesson30_setpoint_step(true, 40);
    let untracked = lesson30_setpoint_step(false, 40);
    assert!(
        lead(&untracked) > 15.0,
        "target tracking lets the request run ahead of the stem: {:.2}",
        lead(&untracked)
    );
    assert!(
        lead(&tracked) < 8.0,
        "measured tracking keeps the request near the stem: {:.2}",
        lead(&tracked)
    );
}

#[test]
fn lesson30_measured_tracking_reduces_the_overshoot() {
    let peak = |trace: &[(f64, f64, f64)]| {
        trace
            .iter()
            .map(|(_, _, temperature)| *temperature)
            .fold(f64::MIN, f64::max)
    };
    let tracked = peak(&lesson30_setpoint_step(true, 90));
    let untracked = peak(&lesson30_setpoint_step(false, 90));
    assert!(
        untracked - tracked > 0.3,
        "measured tracking lowers the peak: {tracked:.3} vs {untracked:.3} °C"
    );
}

#[test]
fn lesson30_hmi_only_binds_signals_of_the_executed_valve_loop() {
    let (runtime, _) = setup(LESSON30);
    let diagnostics = hmi_diagnostics(LESSON30, &runtime);
    assert!(
        diagnostics.is_empty(),
        "The Lesson 31 HMI must bind the executed valve project: {diagnostics:?}"
    );
}
