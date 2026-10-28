use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON31: &str = "09-industrial-control-structures/58-valve-characterization";

fn numbers(values: &[Value]) -> Vec<f64> {
    values
        .iter()
        .map(|value| match value {
            Value::LReal(number) => *number,
            other => panic!("{other:?}"),
        })
        .collect()
}

fn flags(values: &[Value]) -> Vec<bool> {
    values
        .iter()
        .map(|value| match value {
            Value::Bool(flag) => *flag,
            other => panic!("{other:?}"),
        })
        .collect()
}

/// A 5 s baseline at 75 %, then the program's staircase; returns the runtime after `seconds` of experiment
/// and the highest stem position reached.
fn lesson31_after(seconds: u64) -> (trust_runtime::CompiledRuntime, &'static str, f64) {
    let (mut runtime, mut simulation) = setup(LESSON31);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..50 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    // ExperimentEnabled is bound to %MX64.0: Control Studio's activation writes the address.
    debug.enqueue_io_write(
        trust_runtime::io::IoAddress::parse("%MX64.0").unwrap(),
        Value::Bool(true),
    );
    let mut highest_stem = f64::MIN;
    for step in 50..(50 + seconds * 10) {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
        highest_stem = highest_stem.max(number(&runtime, "%IL16"));
    }
    (runtime, program, highest_stem)
}

#[test]
fn lesson31_the_staircase_requests_each_target_in_turn() {
    let (runtime, _, _) = lesson31_after(10);
    assert_eq!(
        number(&runtime, "%QL0"),
        70.0,
        "the first target of the staircase is 70 %"
    );
    let stem = number(&runtime, "%IL16");
    assert!(
        stem < 72.0 && stem > 69.9,
        "the stem travels towards 70 % at 0.5 % per second: {stem}"
    );
    let (runtime, _, _) = lesson31_after(100);
    assert_eq!(number(&runtime, "%QL0"), 85.0, "the fourth target is 85 %");
}

#[test]
fn lesson31_records_qualified_steady_flows_for_both_pressure_arrangements() {
    let (runtime, program, highest_stem) = lesson31_after(151);
    assert!(
        highest_stem > 84.9,
        "the stem reached the 85 % target: {highest_stem}"
    );
    assert!(
        flags(&program_array(&runtime, program, "SteadyQualified"))
            .iter()
            .all(|qualified| *qualified),
        "every target is sampled after arrival and a quiet final 3 s"
    );
    let regulated = numbers(&program_array(&runtime, program, "SteadyFlowRegulated"));
    let fixed = numbers(&program_array(&runtime, program, "SteadyFlowFixedSupply"));
    for (index, (expected_regulated, expected_fixed)) in [
        (1.4, 1.4202),
        (1.5, 1.5),
        (1.6, 1.5763),
        (1.7, 1.6493),
        (1.5, 1.5),
    ]
    .iter()
    .enumerate()
    {
        assert!(
            (regulated[index] - expected_regulated).abs() < 0.004,
            "regulated step {index}: {}",
            regulated[index]
        );
        assert!(
            (fixed[index] - expected_fixed).abs() < 0.004,
            "fixed supply step {index}: {}",
            fixed[index]
        );
    }
    let pressure = numbers(&program_array(
        &runtime,
        program,
        "SteadyPressureFixedSupply",
    ));
    assert!(
        (pressure[3] - 94.12).abs() < 0.1,
        "the fixed supply leaves 94.1 kPa across the valve at 85 %: {}",
        pressure[3]
    );
    let regulated_slope = program_number(&runtime, program, "SlopeRegulatedKgSPerPct");
    let fixed_slope = program_number(&runtime, program, "SlopeFixedSupplyKgSPerPct");
    assert!(
        (regulated_slope - 0.02).abs() < 0.002,
        "regulated 75 → 80 % slope: {regulated_slope}"
    );
    assert!(
        (fixed_slope - 0.0153).abs() < 0.002,
        "fixed-supply 75 → 80 % slope: {fixed_slope}"
    );
    assert_eq!(
        value(&runtime, &format!("{program}.Active")),
        Some(Value::Bool(false)),
        "the staircase has finished"
    );
    assert_eq!(number(&runtime, "%QL0"), 75.0, "the valve returns to 75 %");
}

const LESSON32: &str = "09-industrial-control-structures/59-valve-stiction";

/// One probe run: (seconds after activation, request, deadband accepted target, deadband stem, sticky stem, sticky force).
fn lesson32_probe_trace() -> Vec<(f64, f64, f64, f64, f64, f64)> {
    let (mut runtime, mut simulation) = setup(LESSON32);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..50 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    debug.enqueue_io_write(
        trust_runtime::io::IoAddress::parse("%MX64.0").unwrap(),
        Value::Bool(true),
    );
    let mut trace = Vec::new();
    for step in 50..(50 + 1300) {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
        let force = value(&runtime, &format!("{program}.StickyForceN"))
            .and_then(|value| match value {
                Value::LReal(number) => Some(number),
                _ => None,
            })
            .unwrap_or(f64::NAN);
        trace.push((
            (step - 49) as f64 * 0.1,
            number(&runtime, "%QL0"),
            number(&runtime, "%QL8"),
            number(&runtime, "%IL16"),
            number(&runtime, "%IL24"),
            force,
        ));
    }
    trace
}

fn at(trace: &[(f64, f64, f64, f64, f64, f64)], seconds: f64) -> (f64, f64, f64, f64, f64, f64) {
    *trace
        .iter()
        .min_by(|a, b| (a.0 - seconds).abs().total_cmp(&(b.0 - seconds).abs()))
        .unwrap()
}

#[test]
fn lesson32_the_command_deadband_ignores_requests_below_one_point() {
    let trace = lesson32_probe_trace();
    let (_, request, accepted, stem, _, _) = at(&trace, 2.0);
    assert_eq!(request, 75.5, "the first probe requests 75.5 %");
    assert_eq!(
        accepted, 75.0,
        "a 0.5 point change is below the deadband: the accepted target stays"
    );
    assert_eq!(
        stem, 75.0,
        "the stem follows the accepted target, not the request"
    );
    let (_, request, accepted, stem, _, _) = at(&trace, 7.0);
    assert_eq!(
        (request, accepted),
        (76.0, 76.0),
        "a 1 point change is accepted"
    );
    assert!(
        (stem - 76.0).abs() < 1e-6,
        "and the stem follows it: {stem}"
    );
}

#[test]
fn lesson32_stiction_holds_until_breakaway_and_stops_where_moving_friction_wins() {
    let trace = lesson32_probe_trace();
    let (_, request, _, _, stem, force) = at(&trace, 39.0);
    assert_eq!(request, 80.0);
    assert_eq!(stem, 75.0, "the stem is held at 50 N");
    assert!(
        (force - 50.0).abs() < 1e-6,
        "F = 10 N/point · 5 points: {force}"
    );
    let (_, request, _, _, stem, force) = at(&trace, 51.0);
    assert_eq!(request, 80.5, "55 N exceeds the 50 N breakaway force");
    assert!(
        (stem - 78.5).abs() < 0.05,
        "the stem slides until F falls to 20 N: {stem}"
    );
    assert!(
        (force - 20.0).abs() < 0.5,
        "moving friction holds 20 N: {force}"
    );
}

#[test]
fn lesson32_the_sticky_stem_ignores_the_decrease_and_stops_short_after_reversal() {
    let trace = lesson32_probe_trace();
    let (_, request, _, deadband_stem, sticky_stem, force) = at(&trace, 95.0);
    assert_eq!(request, 75.0);
    assert!(
        (deadband_stem - 75.0).abs() < 1e-6,
        "the deadband valve follows the decrease: {deadband_stem}"
    );
    assert!(
        (sticky_stem - 78.5).abs() < 0.05,
        "the sticky stem does not move at -35 N: {sticky_stem}"
    );
    assert!((force + 35.0).abs() < 0.6, "{force}");
    let (_, request, _, _, sticky_stem, _) = at(&trace, 111.0);
    assert_eq!(request, 72.0, "the reversal probe");
    assert!(
        (sticky_stem - 74.0).abs() < 0.05,
        "-65 N breaks away; the stem stops 2 points short at 74 %: {sticky_stem}"
    );
    let (_, request, _, deadband_stem, sticky_stem, _) = at(&trace, 127.0);
    assert_eq!(request, 75.0);
    assert!((deadband_stem - 75.0).abs() < 1e-6, "{deadband_stem}");
    assert!(
        (sticky_stem - 74.0).abs() < 0.05,
        "back at 75 % the sticky stem stays at 74 % (10 N): {sticky_stem}"
    );
}

const LESSON33: &str =
    "02-derivative-action-and-dynamic-response/32-proportional-position-stiffness";

/// One sample per 20 ms task cycle: (seconds after the step, angle, velocity, torque applied during the cycle).
type Motion = Vec<(f64, f64, f64, f64)>;

/// The axis rests at 0 rad in Manual for 0.5 s; then Auto with `gain` and the setpoint stepped to `setpoint`.
/// Returns the motion, the program's latched arrival velocity and whether the safety abort tripped.
fn lesson33_step(gain: f64, setpoint: f64, seconds: f64) -> (Motion, f64, bool) {
    let (mut runtime, mut simulation) = setup(LESSON33);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..25 {
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
        assert_eq!(
            number(&runtime, "%QL0"),
            0.0,
            "Manual holds zero torque at rest"
        );
    }
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[("ProportionalGain", gain)],
    );
    enqueue_instance_write(&debug, program, "AutoMode", Value::Bool(true));
    let mut motion = Vec::new();
    for step in 25..(25 + (seconds / 0.02).round() as u64) {
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 20,
            20,
            &[("%ML0", Value::LWord(setpoint.to_bits()))],
        );
        motion.push((
            (step - 24) as f64 * 0.02,
            number(&runtime, "%IL0"),
            number(&runtime, "%IL8"),
            number(&runtime, "%QL0"),
        ));
    }
    let aborted = matches!(
        runtime.read_io(&trust_runtime::io::IoAddress::parse("%MX24.0").unwrap()),
        Ok(Value::Bool(true))
    );
    (motion, number(&runtime, "%ML16"), aborted)
}

/// Time and velocity of the first sample at or past the 1 rad target, and the peak angle.
fn lesson33_crossing_and_peak(motion: &Motion) -> (f64, f64, f64) {
    let crossing = motion
        .iter()
        .find(|sample| sample.1 >= 1.0)
        .expect("the shaft reaches the target");
    (
        crossing.0,
        crossing.2,
        motion
            .iter()
            .map(|sample| sample.1)
            .fold(f64::MIN, f64::max),
    )
}

#[test]
fn lesson33_proportional_torque_follows_the_position_error() {
    let (motion, _, _) = lesson33_step(4.0, 1.0, 3.0);
    assert_eq!(
        motion[0].3, 4.0,
        "at rest, 1 rad of error requests Kp · e = 4 N·m"
    );
    for (seconds, angle, _, torque) in &motion {
        // Each sample holds the angle the cycle read and the torque it commanded from it.
        assert!(
            (torque - 4.0 * (1.0 - angle)).abs() < 1e-9,
            "u = Kp (r − θ) at {seconds} s: {torque} N·m at θ {angle} rad"
        );
    }
    assert!(
        motion.iter().any(|sample| sample.3 < -0.5),
        "past the target the torque reverses and pulls back"
    );
}

#[test]
fn lesson33_the_shaft_reaches_the_target_still_moving() {
    let (low, low_arrival, low_abort) = lesson33_step(4.0, 1.0, 8.0);
    let (high, high_arrival, high_abort) = lesson33_step(16.0, 1.0, 8.0);
    let (low_time, low_velocity, low_peak) = lesson33_crossing_and_peak(&low);
    let (high_time, high_velocity, high_peak) = lesson33_crossing_and_peak(&high);
    assert!(
        (low_time - 0.94).abs() < 0.05 && (high_time - 0.43).abs() < 0.05,
        "first crossings: {low_time} s and {high_time} s"
    );
    assert!(
        (low_velocity - 1.25).abs() < 0.08,
        "Kp 4 arrives at 1.25 rad/s: {low_velocity}"
    );
    assert!(
        (high_velocity - 3.23).abs() < 0.15,
        "Kp 16 arrives at 3.23 rad/s: {high_velocity}"
    );
    assert!((low_arrival - low_velocity).abs() < 1e-9 && (high_arrival - high_velocity).abs() < 1e-9,
        "the program latches the velocity of the first sample at the target: {low_arrival}, {high_arrival}");
    // Continuous peaks are 1.44 and 1.67 rad; holding the torque for 20 ms adds a little overshoot.
    assert!(
        (low_peak - 1.46).abs() < 0.01 && (high_peak - 1.72).abs() < 0.01,
        "sampled peaks: {low_peak}, {high_peak}"
    );
    for (motion, gain) in [(&low, 4.0), (&high, 16.0)] {
        let late = motion
            .iter()
            .filter(|sample| sample.0 >= 4.0)
            .map(|sample| (1.0 - sample.1).abs())
            .fold(0.0, f64::max);
        assert!(
            late > 0.05 && late < 0.2,
            "Kp {gain}: friction alone damps the swing, e^(-t/2); late error {late}"
        );
    }
    assert!(
        !low_abort && !high_abort,
        "both runs stay inside the travel and speed limits"
    );
}

#[test]
fn lesson33_overtravel_removes_the_torque() {
    let (motion, _, aborted) = lesson33_step(16.0, 1.5, 4.0);
    assert!(
        aborted,
        "a 1.5 rad step with Kp 16 swings past the 2 rad travel limit"
    );
    let limit = motion
        .iter()
        .position(|sample| sample.1.abs() > 2.0)
        .expect("the angle passes 2 rad");
    assert!(
        motion[limit + 1..].iter().all(|sample| sample.3 == 0.0),
        "after the limit the drive commands zero torque"
    );
}
