use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON76: &str = "12-nonlinear-adaptive-and-robust-control/74-lyapunov-stability";

fn flag(runtime: &trust_runtime::Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Bool(value) => value,
        other => panic!("expected BOOL at {address}, got {other:?}"),
    }
}

/// One 20 ms update of the upright arm: angle from upright, speed, applied torque, the energy function V and the flags.
#[derive(Clone, Copy, Debug)]
struct Arm {
    angle: f64,
    speed: f64,
    torque: f64,
    energy: f64,
    released: bool,
    inside: bool,
    saturated: bool,
    fault: bool,
}

/// Sets the start angle as Control Studio's declared experiment setting does and runs one update at rest, then the
/// project experiment (activation at %MX24.0), one update per entry of `periods_ms`. Each `(update, address, value)`
/// replaces that input for one update, counted from 0.
fn lesson76_run(start_rad: f64, periods_ms: &[u64], readings: &[(usize, &str, f64)]) -> Vec<Arm> {
    let (mut runtime, mut simulation) = setup(LESSON76);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    assert!(runtime
        .write_instance(p1, "InitialAngleRad", Value::LReal(start_rad))
        .is_ok());
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::once(&20).chain(periods_ms.iter()).enumerate() {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, value)| (*address, Value::LWord(value.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Arm {
            angle: number(&runtime, "%ML0"),
            speed: number(&runtime, "%IL8"),
            torque: number(&runtime, "%QL0"),
            energy: number(&runtime, "%ML8"),
            released: flag(&runtime, "%MX24.1"),
            inside: flag(&runtime, "%MX24.2"),
            saturated: flag(&runtime, "%MX24.3"),
            fault: flag(&runtime, "%MX24.4"),
        });
    }
    record
}

#[test]
fn lesson76_the_hold_sets_the_start_and_the_release_applies_the_pd_law() {
    // For 3 s the experiment holds the arm at the start angle with a gravity-compensated hold; then it releases the arm to
    // the tested law τ = −16φ − 7ω. The arm starts at rest at 0.1 rad, and every released update applies the law.
    let record = lesson76_run(0.1, &[20; 550], &[]);
    let run = &record[1..];
    let release = run.iter().position(|u| u.released).expect("released");
    assert_eq!(release, 150, "released after 3 s: {:?}", &run[148..152]);
    assert!(
        (run[release].angle - 0.1).abs() < 1e-3 && run[release].speed.abs() < 1e-3,
        "start at rest: {:?}",
        run[release]
    );
    for (i, u) in run.iter().enumerate().skip(release) {
        assert!(
            (u.torque - (-16.0 * u.angle - 7.0 * u.speed)).abs() < 1e-12 && !u.saturated,
            "law at {i}: {u:?}"
        );
    }
}

#[test]
fn lesson76_the_energy_function_is_published_for_every_update() {
    // V = ½Jω² + ½Kpφ² + G(cos φ − 1) with J = 1 kg·m², Kp = 16 N·m/rad and G = 4 N·m; the 0.2 rad start has V ≈ 0.24.
    let record = lesson76_run(0.2, &[20; 300], &[]);
    assert!(
        record.iter().any(|u| u.energy > 0.2),
        "the start angle: {:?}",
        record[record.len() - 1]
    );
    for (i, u) in record.iter().enumerate() {
        let energy =
            0.5 * u.speed * u.speed + 8.0 * u.angle * u.angle + 4.0 * (u.angle.cos() - 1.0);
        assert!(
            (u.energy - energy).abs() < 1e-12,
            "energy at {i}: {u:?} against {energy}"
        );
    }
}

#[test]
fn lesson76_inside_the_certificate_the_arm_returns_with_v_decreasing() {
    // From 0.1 rad at rest, V = 0.0600 ≤ 0.1: the state stays inside the certificate, V does not grow by more than
    // rounding, the torque stays within the 5.2 N·m bound, and the arm is within 0.001 rad of upright 4 s after release.
    let record = lesson76_run(0.1, &[20; 550], &[]);
    let run = &record[1..];
    let release = run.iter().position(|u| u.released).unwrap();
    let after = &run[release..];
    assert!(
        (after[0].energy - 0.0600).abs() < 1e-4
            && after.iter().all(|u| u.inside && u.torque.abs() <= 5.2),
        "inside: {:?}",
        after[0]
    );
    assert!(
        after.windows(2).all(|w| w[1].energy <= w[0].energy + 1e-6),
        "V non-increasing: {after:?}"
    );
    assert!(
        after[200..].iter().all(|u| u.angle.abs() < 1e-3),
        "returned: {:?}",
        after[200]
    );
}

#[test]
fn lesson76_outside_the_certificate_the_result_is_unproved_but_observed() {
    // From 0.2 rad, V = 0.2403 > 0.1: the release starts outside the certificate, which therefore proves nothing about
    // this start. The arm still returns, and it enters the certificate later.
    let record = lesson76_run(0.2, &[20; 550], &[]);
    let run = &record[1..];
    let release = run.iter().position(|u| u.released).unwrap();
    let after = &run[release..];
    assert!(
        (after[0].energy - 0.2403).abs() < 1e-3 && !after[0].inside,
        "outside: {:?}",
        after[0]
    );
    assert!(
        after.iter().any(|u| u.inside) && after[200..].iter().all(|u| u.angle.abs() < 1e-3),
        "returned: {:?}",
        after[200]
    );
}

#[test]
fn lesson76_leaving_the_bounds_or_a_wrong_interval_latches_zero_torque() {
    // An angle beyond 0.3 rad from upright or an update interval outside 20 ms ± 5 ms latches zero torque.
    let angle = lesson76_run(0.1, &[20; 10], &[(6, "%IL0", std::f64::consts::PI + 0.35)]);
    assert!(
        !angle[5].fault && angle[6..].iter().all(|u| u.fault && u.torque == 0.0),
        "angle: {angle:?}"
    );
    let gap = lesson76_run(0.1, &[20, 20, 40, 20], &[]);
    assert!(
        !gap[2].fault && gap[3..].iter().all(|u| u.fault && u.torque == 0.0),
        "interval: {gap:?}"
    );
}

const LESSON77: &str = "12-nonlinear-adaptive-and-robust-control/75-feedback-linearization";

/// One 20 ms update of the arm with a gravity-compensated PD law: angle from upright, setpoint, speed, applied torque, the
/// peak angle and the largest error from 4 s to 8 s, and the flags.
#[derive(Clone, Copy, Debug)]
struct Compensated {
    angle: f64,
    setpoint: f64,
    speed: f64,
    torque: f64,
    peak: f64,
    late_error: f64,
    saturated: bool,
    fault: bool,
}

/// Sets the gravity estimate as Control Studio's declared experiment setting does, runs one update at rest, then the
/// project experiment (activation at %MX48.0), one update per entry of `periods_ms`.
fn lesson77_run(
    estimate_nm: f64,
    periods_ms: &[u64],
    readings: &[(usize, &str, f64)],
) -> Vec<Compensated> {
    let (mut runtime, mut simulation) = setup(LESSON77);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    assert!(runtime
        .write_instance(p1, "GravityEstimateNm", Value::LReal(estimate_nm))
        .is_ok());
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::once(&20).chain(periods_ms.iter()).enumerate() {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX48.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, value)| (*address, Value::LWord(value.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Compensated {
            angle: number(&runtime, "%ML0"),
            setpoint: number(&runtime, "%ML8"),
            speed: number(&runtime, "%IL8"),
            torque: number(&runtime, "%QL0"),
            peak: number(&runtime, "%ML16"),
            late_error: number(&runtime, "%ML24"),
            saturated: flag(&runtime, "%MX48.1"),
            fault: flag(&runtime, "%MX48.2"),
        });
    }
    record
}

#[test]
fn lesson77_every_update_applies_the_compensated_law() {
    // τ = 16(r − φ) − 7ω − Ĝ·sin φ with the target r = 0.1 rad from the start of the experiment.
    let record = lesson77_run(3.5, &[20; 450], &[]);
    for (i, u) in record.iter().enumerate().skip(1) {
        let law = 16.0 * (u.setpoint - u.angle) - 7.0 * u.speed - 3.5 * u.angle.sin();
        assert!(
            u.setpoint == 0.1 && (u.torque - law).abs() < 1e-12 && !u.saturated,
            "law at {i}: {u:?}"
        );
    }
}

#[test]
fn lesson77_nominal_compensation_holds_the_target_exactly() {
    // With Ĝ = G = 4 N·m gravity cancels: the arm settles at 0.1 rad, the error from 4 s to 8 s stays within 0.01 rad, the
    // peak within 0.12 rad, and the holding torque is −4·sin 0.1 = −0.399 N·m.
    let record = lesson77_run(4.0, &[20; 450], &[]);
    let end = record[record.len() - 1];
    assert!(
        end.late_error <= 0.01 && end.peak <= 0.12 && (end.angle - 0.1).abs() < 1e-4,
        "nominal: {end:?}"
    );
    assert!(
        (end.torque + 4.0 * 0.1f64.sin()).abs() < 1e-3,
        "holding torque: {end:?}"
    );
}

#[test]
fn lesson77_without_compensation_position_error_holds_the_arm() {
    // With Ĝ = 0 the P term must supply the holding torque: 16(0.1 − φ) = −4·sin φ, so the arm settles near 0.133 rad and
    // misses both the 0.12 rad peak and the 0.01 rad error requirements.
    let record = lesson77_run(0.0, &[20; 450], &[]);
    let end = record[record.len() - 1];
    assert!(
        (end.angle - 0.1332).abs() < 1e-3 && end.peak > 0.12 && end.late_error > 0.01,
        "none: {end:?}"
    );
}

#[test]
fn lesson77_an_imperfect_estimate_leaves_a_small_residual_and_a_wrong_sign_a_larger_one() {
    // Ĝ = 3.5 N·m leaves (G − Ĝ)·sin φ: the arm settles near 0.1032 rad, within both requirements. Ĝ = −4 N·m doubles the
    // gravity the P term must hold: near 0.199 rad, worse than no compensation.
    let imperfect = lesson77_run(3.5, &[20; 450], &[]);
    let end = imperfect[imperfect.len() - 1];
    assert!(
        (end.angle - 0.1032).abs() < 1e-3 && end.peak <= 0.12 && end.late_error <= 0.01,
        "imperfect: {end:?}"
    );
    let wrong = lesson77_run(-4.0, &[20; 450], &[]);
    let end = wrong[wrong.len() - 1];
    assert!(
        (end.angle - 0.1987).abs() < 2e-3 && end.late_error > 0.05,
        "wrong sign: {end:?}"
    );
}

#[test]
fn lesson77_leaving_the_bounds_or_a_wrong_interval_latches_zero_torque() {
    // An angle beyond 0.3 rad or an update interval outside 20 ms ± 5 ms latches zero torque.
    let angle = lesson77_run(4.0, &[20; 10], &[(6, "%IL0", std::f64::consts::PI + 0.35)]);
    assert!(
        !angle[5].fault && angle[6..].iter().all(|u| u.fault && u.torque == 0.0),
        "angle: {angle:?}"
    );
    let gap = lesson77_run(4.0, &[20, 20, 40, 20], &[]);
    assert!(
        !gap[2].fault && gap[3..].iter().all(|u| u.fault && u.torque == 0.0),
        "interval: {gap:?}"
    );
}

const LESSON78: &str = "12-nonlinear-adaptive-and-robust-control/76-adaptive-control";

/// One 100 ms update of the two heaters whose heat loss changed to 8 kW/°C: temperatures and commands [fixed A, adaptive
/// B], the setpoint, the two windows' heat-loss estimates and temperature spans, B's integral gain and the flags.
#[derive(Clone, Copy, Debug)]
struct Adaptive {
    temperature: [f64; 2],
    command: [f64; 2],
    nominal: [f64; 2],
    setpoint: f64,
    estimate: [f64; 2],
    span: [f64; 2],
    integral_gain: f64,
    accepted: [bool; 2],
    fault: bool,
}

/// The retained nominal output of one of the program's Control.PID instances.
fn pid_nominal(runtime: &trust_runtime::Runtime, name: &str) -> f64 {
    let Some(Value::Instance(pid)) = runtime
        .storage()
        .get_instance_var(program(runtime), name)
        .cloned()
    else {
        panic!("the lesson must execute Control.PID as {name}");
    };
    let Some(Value::Struct(state)) = runtime.storage().get_instance_var(pid, "StateValue") else {
        panic!("Control.PID must keep its state record");
    };
    match state.field("NominalOutput") {
        Some(Value::LReal(value)) => *value,
        other => panic!("NominalOutput: {other:?}"),
    }
}

/// Runs one update at rest, then the project experiment (activation at %MX56.0), one update per entry of `periods_ms`.
fn lesson78_run(periods_ms: &[u64], readings: &[(usize, &str, f64)]) -> Vec<Adaptive> {
    let (mut runtime, mut simulation) = setup(LESSON78);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::once(&100).chain(periods_ms.iter()).enumerate() {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX56.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, value)| (*address, Value::LWord(value.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Adaptive {
            temperature: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            command: [number(&runtime, "%QL0"), number(&runtime, "%QL16")],
            nominal: [
                pid_nominal(&runtime, "FixedLoop"),
                pid_nominal(&runtime, "AdaptiveLoop"),
            ],
            setpoint: number(&runtime, "%ML0"),
            estimate: [number(&runtime, "%ML8"), number(&runtime, "%ML16")],
            span: [number(&runtime, "%ML24"), number(&runtime, "%ML32")],
            integral_gain: number(&runtime, "%ML40"),
            accepted: [flag(&runtime, "%MX56.1"), flag(&runtime, "%MX56.2")],
            fault: flag(&runtime, "%MX56.3"),
        });
    }
    record
}

#[test]
fn lesson78_both_heaters_run_control_pid() {
    // Each heater is held by Control.PID as PI with β = 1, back-calculation with Tt = 1 s and limits 0–100 %, starting at rest:
    // setpoint 35 °C, 35 °C and 60 %. A stays in Auto with Kp = 2.5 and Ki = 0.5. B is in Manual for the windows, 60, 70 and
    // 60 % for 30 s each, and in Auto from 90 s with Kp = 2.5 and the integral gain the program holds.
    let record = lesson78_run(&[100; 1000], &[]);
    let mut fixed = MirrorPi::at_rest(60.0, 35.0);
    let mut adaptive = MirrorPi::at_rest(60.0, 35.0);
    for (i, u) in record.iter().enumerate() {
        let expected = fixed.update(2.5, 0.5, u.setpoint, u.temperature[0]);
        assert!(
            u.command[0] == expected && u.nominal[0] == fixed.nominal,
            "A at {i}: {u:?} against {expected}"
        );
        let manual = i >= 1 && i - 1 < 900;
        let expected = if manual {
            let pct = if (300..600).contains(&(i - 1)) {
                70.0
            } else {
                60.0
            };
            adaptive.manual(pct, u.setpoint, u.temperature[1])
        } else {
            adaptive.update(2.5, u.integral_gain, u.setpoint, u.temperature[1])
        };
        assert!(
            u.command[1] == expected && u.nominal[1] == adaptive.nominal,
            "B at {i}: {u:?} against {expected}"
        );
    }
}

#[test]
fn lesson78_the_steady_window_is_rejected_and_the_step_window_accepted() {
    // From 0 s to 30 s heater B holds 60 %: its temperature barely moves, so the window cannot tell a changed heat loss from
    // an unknown extraction and is rejected (span below 1 °C). From 30 s B steps to 70 %: the least-squares estimate of the
    // 8 kW/°C heat loss is accepted, and B's integral gain becomes Ĥ/(2λ) with λ = 10 s; its gains are 2.5 and 0.5 before.
    let record = lesson78_run(&[100; 1510], &[]);
    let run = &record[1..];
    let (before, after) = (run[590], run[610]);
    assert!(
        !before.accepted[0] && before.span[0] < 1.0 && before.integral_gain == 0.5,
        "steady window: {before:?}"
    );
    assert!(
        after.accepted[1] && after.span[1] >= 1.0 && (after.estimate[1] - 8.0).abs() < 0.1,
        "step window: {after:?}"
    );
    assert!(
        (after.integral_gain - after.estimate[1] / 20.0).abs() < 1e-12,
        "revised gain: {after:?}"
    );
}

#[test]
fn lesson78_the_revised_gains_restore_the_designed_step_response() {
    // At 90 s both heaters, at 35 °C in Auto, step to 38 °C. B's revised gains reproduce the response designed with λ = 10 s:
    // no overshoot and 63 % of the step after about 10 s. A's fixed gains, designed for 10 kW/°C, now respond faster and
    // overshoot. Both are within 0.2 °C from 30 s after the step.
    let record = lesson78_run(&[100; 1510], &[]);
    let run = &record[1..];
    let step = &run[900..];
    assert!(
        step[0].setpoint == 38.0 && (run[899].temperature[1] - 35.0).abs() < 0.05,
        "step start: {:?}",
        run[899]
    );
    let reach = |h: usize| {
        step.iter()
            .position(|u| u.temperature[h] >= 35.0 + 0.632 * 3.0)
            .unwrap() as f64
            * 0.1
    };
    let peak = |h: usize| step.iter().map(|u| u.temperature[h]).fold(0.0, f64::max);
    assert!(
        (reach(1) - 9.9).abs() < 0.3 && peak(1) <= 38.0,
        "revised B: 63 % at {} s, peak {}",
        reach(1),
        peak(1)
    );
    assert!(
        reach(0) < reach(1) - 0.5 && peak(0) > 38.0,
        "fixed A: 63 % at {} s, peak {}",
        reach(0),
        peak(0)
    );
    assert!(
        step[300..]
            .iter()
            .all(|u| (u.temperature[0] - 38.0).abs() <= 0.2
                && (u.temperature[1] - 38.0).abs() <= 0.2),
        "late: {:?}",
        step[300]
    );
}

#[test]
fn lesson78_an_invalid_temperature_or_interval_latches_zero_output() {
    // A temperature outside 0–80 °C or an update interval outside 100 ms ± 10 ms latches both commands at zero.
    let reading = lesson78_run(&[100; 10], &[(6, "%IL8", 95.0)]);
    assert!(
        !reading[5].fault
            && reading[6..]
                .iter()
                .all(|u| u.fault && u.command == [0.0, 0.0]),
        "temperature: {reading:?}"
    );
    let gap = lesson78_run(&[100, 100, 200, 100], &[]);
    assert!(
        !gap[2].fault && gap[3..].iter().all(|u| u.fault && u.command == [0.0, 0.0]),
        "interval: {gap:?}"
    );
}
