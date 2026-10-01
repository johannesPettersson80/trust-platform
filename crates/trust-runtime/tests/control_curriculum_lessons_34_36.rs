use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON34: &str = "02-derivative-action-and-dynamic-response/33-derivative-motion-damping";

/// One sample per 20 ms task cycle: (seconds after the step, angle, velocity, torque commanded from that angle).
type Motion = Vec<(f64, f64, f64, f64)>;

/// The axis rests at 0 rad in Manual for 0.5 s; then Auto with the given parameter changes and the setpoint
/// stepped to 1 rad. No changes run the lesson's own configuration.
fn lesson34_step(changes: &[(&str, f64)], seconds: f64) -> Motion {
    let (mut runtime, mut simulation) = setup(LESSON34);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..25 {
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
    }
    if !changes.is_empty() {
        set_program_record(&mut runtime, program, "Parameters", changes);
    }
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    let mut motion = Vec::new();
    for step in 25..(25 + (seconds / 0.02).round() as u64) {
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 20,
            20,
            &[("%ML0", Value::LWord(1.0f64.to_bits()))],
        );
        motion.push((
            (step - 24) as f64 * 0.02,
            number(&runtime, "%IL0"),
            number(&runtime, "%IL8"),
            number(&runtime, "%QL0"),
        ));
    }
    motion
}

fn peak(motion: &Motion) -> f64 {
    motion
        .iter()
        .map(|sample| sample.1)
        .fold(f64::MIN, f64::max)
}

/// The first time after which the angle stays within ±0.01 rad of the 1 rad target.
fn settling(motion: &Motion) -> Option<f64> {
    (0..motion.len())
        .find(|&index| {
            motion[index..]
                .iter()
                .all(|sample| (1.0 - sample.1).abs() <= 0.01)
        })
        .map(|index| motion[index].0)
}

#[test]
fn lesson34_derivative_on_measurement_brakes_the_shaft() {
    let motion = lesson34_step(&[], 8.0);
    assert!(
        peak(&motion) < 1.005,
        "Kp 16 with Kd 7 is critically damped: no overshoot, peak {}",
        peak(&motion)
    );
    let settled = settling(&motion).expect("the shaft settles");
    assert!(
        (settled - 1.74).abs() < 0.1,
        "within ±0.01 rad after about 1.74 s: {settled}"
    );
    assert_eq!(
        motion[0].3, 16.0,
        "at the step only P acts: the measured angle has not moved"
    );
    for pair in motion.windows(2) {
        // u = Kp (r − θ) − Kd (θ_k − θ_k−1)/Δt: derivative of the measured angle, not of the error.
        let expected = 16.0 * (1.0 - pair[1].1) - 7.0 * (pair[1].1 - pair[0].1) / 0.02;
        assert!(
            (pair[1].3 - expected.clamp(-20.0, 20.0)).abs() < 1e-9,
            "PD torque at {} s: {} vs {expected}",
            pair[1].0,
            pair[1].3
        );
    }
}

#[test]
fn lesson34_without_derivative_the_same_gain_swings() {
    let motion = lesson34_step(&[("DerivativeGain", 0.0)], 8.0);
    assert!(
        (peak(&motion) - 1.72).abs() < 0.01,
        "Kp 16 alone overshoots to 1.72 rad: {}",
        peak(&motion)
    );
    assert!(
        settling(&motion).is_none(),
        "and has not settled within ±0.01 rad after 8 s"
    );
}

#[test]
fn lesson34_derivative_on_error_kicks_the_torque_to_the_limit() {
    let kicked = lesson34_step(&[("DerivativeSetpointWeight", 1.0)], 2.0);
    assert_eq!(
        kicked[0].3, 20.0,
        "Kd · Δr/Δt = 7 · 1/0.02 = 350 N·m on top of 16 N·m: clipped at the 20 N·m limit"
    );
    let measured = lesson34_step(&[], 2.0);
    assert_eq!(
        measured[0].3, 16.0,
        "differentiating the measurement adds nothing at the step"
    );
    assert!(
        (kicked[10].3 - measured[10].3).abs() < 0.5,
        "after the step both forms brake alike"
    );
}

const LESSON35: &str = "02-derivative-action-and-dynamic-response/34-servo-pid-holding-torque";

/// The axis rests at 0 rad in Manual for 0.5 s; then Auto with the given parameter changes, the setpoint at
/// `setpoint` and the drum load `load` coupled from the first Auto cycle.
fn lesson35_run(changes: &[(&str, f64)], setpoint: f64, load: f64, seconds: f64) -> Motion {
    let (mut runtime, mut simulation) = setup(LESSON35);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..25 {
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
    }
    if !changes.is_empty() {
        set_program_record(&mut runtime, program, "Parameters", changes);
    }
    // As in Control Studio's trials, the controller starts from the axis at rest: without this, the
    // incremental derivative would keep the noisy angle rate of the last Manual cycle as its reference.
    set_controller_state(
        &mut runtime,
        program,
        &[("SetpointRate", 0.0), ("ProcessValueRate", 0.0)],
    );
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    debug.enqueue_instance_write(program, "LoadTorqueNm", Value::LReal(load));
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
    motion
}

fn window_mean(motion: &Motion, from: f64, value: impl Fn(&(f64, f64, f64, f64)) -> f64) -> f64 {
    let samples: Vec<f64> = motion
        .iter()
        .filter(|sample| sample.0 >= from)
        .map(value)
        .collect();
    samples.iter().sum::<f64>() / samples.len() as f64
}

#[test]
fn lesson35_pd_holds_the_drum_below_its_target() {
    let motion = lesson35_run(
        &[("IntegralGain", 0.0), ("ProportionalSetpointWeight", 1.0)],
        0.0,
        1.962,
        8.0,
    );
    let angle = window_mean(&motion, 6.0, |sample| sample.1);
    let torque = window_mean(&motion, 6.0, |sample| sample.3);
    assert!(
        (angle + 0.1226).abs() < 0.002,
        "PD needs e = τL/Kp = 1.962/16 = 0.1226 rad of error to hold the load: {angle}"
    );
    assert!(
        (torque - 1.962).abs() < 0.02,
        "the holding torque comes from P alone: {torque}"
    );
}

#[test]
fn lesson35_the_integral_holds_the_drum_at_its_target() {
    let motion = lesson35_run(&[], 0.0, 1.962, 8.0);
    let late = motion
        .iter()
        .filter(|sample| sample.0 >= 4.0)
        .map(|sample| sample.1.abs())
        .fold(0.0, f64::max);
    assert!(
        late <= 0.02,
        "PID returns to the target and stays within 0.02 rad from 4 s: {late}"
    );
    let droop = motion
        .iter()
        .map(|sample| sample.1)
        .fold(f64::MAX, f64::min);
    assert!(
        (droop + 0.094).abs() < 0.01,
        "the load first pulls the shaft down about 0.094 rad: {droop}"
    );
    let torque = window_mean(&motion, 6.0, |sample| sample.3);
    assert!(
        (torque - 1.962).abs() < 0.05,
        "the integral supplies the 1.962 N·m holding torque at zero error: {torque}"
    );
}

#[test]
fn lesson35_the_setpoint_weight_keeps_the_integral_from_overshooting_a_step() {
    let weighted = lesson35_run(&[], 1.0, 0.0, 8.0);
    let peak = weighted
        .iter()
        .map(|sample| sample.1)
        .fold(f64::MIN, f64::max);
    assert!(
        peak < 1.015,
        "β = 0.3 keeps the unloaded step below 1.015 rad: {peak}"
    );
    let settled = (0..weighted.len())
        .find(|&index| {
            weighted[index..]
                .iter()
                .all(|sample| (1.0 - sample.1).abs() <= 0.01)
        })
        .map(|index| weighted[index].0);
    assert!(
        settled.is_some_and(|seconds| seconds < 4.0),
        "and within ±0.01 rad before 4 s: {settled:?}"
    );
    let full = lesson35_run(&[("ProportionalSetpointWeight", 1.0)], 1.0, 0.0, 8.0);
    let peak = full.iter().map(|sample| sample.1).fold(f64::MIN, f64::max);
    assert!(
        (peak - 1.23).abs() < 0.02,
        "with β = 1 the integral grows during the approach: peak {peak}"
    );
}

#[test]
fn lesson35_the_derivative_turns_angle_noise_into_torque_noise() {
    let motion = lesson35_run(&[], 0.0, 1.962, 8.0);
    let torques: Vec<f64> = motion
        .iter()
        .filter(|sample| sample.0 >= 6.0)
        .map(|sample| sample.3)
        .collect();
    let spread = torques.iter().cloned().fold(f64::MIN, f64::max)
        - torques.iter().cloned().fold(f64::MAX, f64::min);
    assert!(
        spread > 1.0 && spread < 1.6,
        "±0.001 rad of angle noise makes Kd·Δθ/Δt vary by up to ±0.7 N·m: spread {spread}"
    );
}

const LESSON36: &str = "02-derivative-action-and-dynamic-response/36-filtered-derivative";

/// A 0 → 1 rad step from rest with the given measurement filter time (None keeps the program's own).
fn lesson36_step(filter_time: Option<f64>, seconds: f64) -> Motion {
    let (mut runtime, mut simulation) = setup(LESSON36);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..25 {
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
    }
    if let Some(filter_time) = filter_time {
        assert!(runtime.storage_mut().set_instance_var(
            program,
            "FilterTimeS",
            Value::LReal(filter_time)
        ));
    }
    set_controller_state(
        &mut runtime,
        program,
        &[("SetpointRate", 0.0), ("ProcessValueRate", 0.0)],
    );
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    let mut motion = Vec::new();
    for step in 25..(25 + (seconds / 0.02).round() as u64) {
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 20,
            20,
            &[("%ML0", Value::LWord(1.0f64.to_bits()))],
        );
        motion.push((
            (step - 24) as f64 * 0.02,
            number(&runtime, "%IL0"),
            number(&runtime, "%IL8"),
            number(&runtime, "%QL0"),
        ));
    }
    motion
}

/// Torque activity: the sum of all torque changes, in N·m.
fn activity(motion: &Motion) -> f64 {
    motion
        .windows(2)
        .map(|pair| (pair[1].3 - pair[0].3).abs())
        .sum()
}

#[test]
fn lesson36_the_filter_cuts_the_torque_activity_without_changing_the_motion() {
    let raw = lesson36_step(Some(0.0), 8.0);
    let filtered = lesson36_step(None, 8.0);
    assert!(
        activity(&raw) > 150.0,
        "unfiltered, D turns the angle noise into {} N·m of torque changes",
        activity(&raw)
    );
    assert!(
        activity(&filtered) < 30.0,
        "Tf = 0.1 s leaves {} N·m",
        activity(&filtered)
    );
    let peak = filtered
        .iter()
        .map(|sample| sample.1)
        .fold(f64::MIN, f64::max);
    let settled = (0..filtered.len())
        .find(|&index| {
            filtered[index..]
                .iter()
                .all(|sample| (1.0 - sample.1).abs() <= 0.01)
        })
        .map(|index| filtered[index].0);
    assert!(
        peak < 1.015 && settled.is_some_and(|seconds| seconds < 3.0),
        "the step is unchanged: peak {peak}, settled {settled:?}"
    );
}

#[test]
fn lesson36_too_much_filtering_weakens_the_braking() {
    let slow = lesson36_step(Some(0.5), 8.0);
    let peak = slow.iter().map(|sample| sample.1).fold(f64::MIN, f64::max);
    assert!(
        peak > 1.2,
        "Tf = 0.5 s delays the velocity estimate so much that the shaft overshoots: peak {peak}"
    );
}
