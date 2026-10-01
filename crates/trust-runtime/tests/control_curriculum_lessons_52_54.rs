use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON52: &str = "07-classical-stability-and-frequency-design/49-loop-shaping";

/// One 0.1 s update of the nine heaters, ordered [target, load, noise] × [baseline, Tz = 3 s, Tz = 10 s].
#[derive(Clone, Copy, Debug)]
struct Shaping {
    temperature: [f64; 9],
    command: [f64; 9],
    requests: [f64; 4],
}

/// One update at rest, then Control Studio's project experiment (activation at %MX64.0) for `updates`.
fn lesson52_run(updates: u64) -> Vec<Shaping> {
    let (mut runtime, mut simulation) = setup(LESSON52);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX64.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        let read = |address: String| number(&runtime, &address);
        record.push(Shaping {
            temperature: std::array::from_fn(|k| read(format!("%IL{}", 8 * k))),
            command: std::array::from_fn(|k| read(format!("%QL{}", 16 * k))),
            requests: [
                read("%ML24".into()),
                read("%ML32".into()),
                read("%ML40".into()),
                read("%ML48".into()),
            ],
        });
    }
    record
}

#[test]
fn lesson52_shapes_the_request_before_limiting_and_scales_the_integral_tracking() {
    // First update of the 35 → 38 °C step: e = 3, P = 15, I = 75.3, w = 15.3. Tz = 3 s (a = 1.5) requests 97.95 %;
    // Tz = 10 s (a = 5) requests 151.5 %, is limited to 100 % and tracks I back to 75.3 + 0.1·(100 − 151.5)/5 = 74.27.
    let record = lesson52_run(1);
    let first = record[1];
    assert!(
        (first.requests[0] - 97.95).abs() < 1e-9 && (first.command[1] - 97.95).abs() < 1e-9,
        "Tz = 3 s: {first:?}"
    );
    assert!(
        (first.requests[1] - 75.3).abs() < 1e-9,
        "Tz = 3 s integral: {first:?}"
    );
    assert!(
        (first.requests[2] - 151.5).abs() < 1e-9 && first.command[2] == 100.0,
        "Tz = 10 s: {first:?}"
    );
    assert!(
        (first.requests[3] - 74.27).abs() < 1e-9,
        "Tz = 10 s integral: {first:?}"
    );
    assert!(
        (first.command[0] - 90.3).abs() < 1e-9,
        "baseline: 75.3 + 15 = 90.3 %: {first:?}"
    );
}

#[test]
fn lesson52_the_candidates_against_the_fixed_requirements() {
    // Target 35 → 38 °C: peak ≤ 39 °C and |error| ≤ 0.2 °C from 30 s. Load 30 kW: the water stays above 33.8 °C and
    // within 0.2 °C from 30 s. Noise 0.05·sin(t) °C: command half-amplitude ≤ 0.5 % between 12π and 18π s.
    let record = lesson52_run(600);
    let run = &record[1..];
    let late = |k: usize, target: f64| {
        run[300..]
            .iter()
            .map(|u| (u.temperature[k] - target).abs())
            .fold(0.0, f64::max)
    };
    for k in 0..3 {
        let peak = run
            .iter()
            .map(|u| u.temperature[k])
            .fold(f64::MIN, f64::max);
        assert!(
            peak <= 39.0 && late(k, 38.0) <= 0.2,
            "target, candidate {k}: peak {peak}, late {}",
            late(k, 38.0)
        );
        assert!(
            late(3 + k, 35.0) <= 0.2,
            "load, candidate {k}: late {}",
            late(3 + k, 35.0)
        );
    }
    let minimum = |k: usize| {
        run.iter()
            .map(|u| u.temperature[k])
            .fold(f64::MAX, f64::min)
    };
    assert!(
        minimum(3) < 33.8,
        "the baseline lets the load pull the water to {}",
        minimum(3)
    );
    assert!(
        minimum(4) >= 33.8 && minimum(5) >= 33.8,
        "the leads keep it above 33.8 °C: {} and {}",
        minimum(4),
        minimum(5)
    );
    let window: Vec<&Shaping> = run
        .iter()
        .enumerate()
        .filter(|(k, _)| {
            let t = *k as f64 * 0.1;
            (12.0 * std::f64::consts::PI..=18.0 * std::f64::consts::PI).contains(&t)
        })
        .map(|(_, u)| u)
        .collect();
    let half = |k: usize| {
        (window.iter().map(|u| u.command[k]).fold(f64::MIN, f64::max)
            - window.iter().map(|u| u.command[k]).fold(f64::MAX, f64::min))
            / 2.0
    };
    assert!(
        half(6) <= 0.5 && half(7) <= 0.5,
        "baseline and Tz = 3 s within 0.5 %: {} and {}",
        half(6),
        half(7)
    );
    assert!(
        half(8) > 0.5,
        "Tz = 10 s amplifies the noise to {} %",
        half(8)
    );
}

const LESSON53: &str = "07-classical-stability-and-frequency-design/47-nyquist-stability";

/// One 0.1 s update of the two P-controlled heaters, ordered [1 s delay, 8 s delay].
#[derive(Clone, Copy, Debug)]
struct Delayed {
    temperature: [f64; 2],
    command: [f64; 2],
    abort: [bool; 2],
}

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

/// One update at rest, then Control Studio's project experiment (activation at %MX16.0) for `updates`.
fn lesson53_run(updates: u64) -> Vec<Delayed> {
    let (mut runtime, mut simulation) = setup(LESSON53);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX16.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Delayed {
            temperature: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            command: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
            abort: [flag(&runtime, "%MX16.1"), flag(&runtime, "%MX16.2")],
        });
    }
    record
}

#[test]
fn lesson53_steps_the_setpoint_and_applies_proportional_control_about_the_resting_command() {
    // At rest both heaters hold 35 °C with 75 %. The first update of the 35 → 35.1 °C step: u = 75 + 10·0.1 = 76 %.
    let record = lesson53_run(1);
    assert_eq!(record[0].command, [75.0, 75.0], "at rest: {:?}", record[0]);
    assert!(
        record[0]
            .temperature
            .iter()
            .all(|t| (t - 35.0).abs() < 1e-9),
        "at rest: {:?}",
        record[0]
    );
    assert!(
        record[1].command.iter().all(|u| (u - 76.0).abs() < 1e-9),
        "first update: {:?}",
        record[1]
    );
}

#[test]
fn lesson53_the_one_second_delay_settles_and_the_eight_second_delay_grows() {
    // P control leaves the water at (35 + 2·35.1)/3 = 35.0667 °C. With 1 s the deviation dies away; with 8 s the
    // oscillation grows: its swing over 120–150 s is more than three times its swing over 30–60 s.
    let record = lesson53_run(1500);
    let run = &record[1..];
    let swing = |k: usize, from: usize, to: usize| {
        let window = &run[from * 10..to * 10];
        window
            .iter()
            .map(|u| u.temperature[k])
            .fold(f64::MIN, f64::max)
            - window
                .iter()
                .map(|u| u.temperature[k])
                .fold(f64::MAX, f64::min)
    };
    let last = run.last().unwrap();
    assert!(
        swing(0, 30, 150) < 1e-3 && (last.temperature[0] - 35.0 - 0.2 / 3.0).abs() < 1e-3,
        "1 s: swing {} and final {}",
        swing(0, 30, 150),
        last.temperature[0]
    );
    assert!(
        swing(1, 120, 150) > 3.0 * swing(1, 30, 60),
        "8 s: swings {} then {}",
        swing(1, 30, 60),
        swing(1, 120, 150)
    );
    assert!(
        run.iter().all(|u| !u.abort[0] && !u.abort[1]),
        "no abort within 150 s"
    );
}

#[test]
fn lesson53_an_abort_returns_the_growing_loop_to_the_resting_command() {
    // Left running, the 8 s loop leaves 34–36 °C: from that update on its abort stays set and its command stays 75 %.
    let record = lesson53_run(2200);
    let run = &record[1..];
    let first = run
        .iter()
        .position(|u| !(34.0..=36.0).contains(&u.temperature[1]))
        .expect("the 8 s loop leaves 34–36 °C");
    assert!(
        run[first..]
            .iter()
            .all(|u| u.abort[1] && u.command[1] == 75.0),
        "after {:.1} s: {:?}",
        first as f64 * 0.1,
        run[first]
    );
    assert!(
        run[..first].iter().all(|u| !u.abort[1]),
        "no abort before the limit"
    );
    assert!(run.iter().all(|u| !u.abort[0]), "the 1 s loop never aborts");
}

const LESSON54: &str = "07-classical-stability-and-frequency-design/50-position-z-plane";

/// One 20 ms scan of the two shafts, ordered [0.02 s updates, 0.6 s updates].
#[derive(Clone, Copy, Debug)]
struct Sampled {
    angle: [f64; 2],
    velocity: [f64; 2],
    torque: [f64; 2],
    pending: [f64; 2],
    abort: [bool; 2],
}

/// One scan at rest, then Control Studio's project experiment (activation at %MX24.0) for `scans`.
fn lesson54_run(scans: u64) -> Vec<Sampled> {
    let (mut runtime, mut simulation) = setup(LESSON54);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + scans {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
        record.push(Sampled {
            angle: [number(&runtime, "%IL0"), number(&runtime, "%IL16")],
            velocity: [number(&runtime, "%IL8"), number(&runtime, "%IL24")],
            torque: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
            pending: [number(&runtime, "%ML8"), number(&runtime, "%ML16")],
            abort: [flag(&runtime, "%MX24.1"), flag(&runtime, "%MX24.2")],
        });
    }
    record
}

#[test]
fn lesson54_applies_each_torque_one_update_after_it_is_calculated() {
    // At the first update both controllers calculate 16·(0.01 − 0) − 7·0 = 0.16 N·m and keep applying the old 0 N·m.
    // The 0.02 s controller applies it at its next update, one scan later; the 0.6 s controller 30 scans later.
    let record = lesson54_run(40);
    let first = record[1];
    assert!(
        first.torque == [0.0, 0.0] && first.pending.iter().all(|u| (u - 0.16).abs() < 1e-12),
        "first update: {first:?}"
    );
    assert!(
        (record[2].torque[0] - 0.16).abs() < 1e-12,
        "0.02 s, next update: {:?}",
        record[2]
    );
    assert!(
        record[2..31]
            .iter()
            .all(|u| u.torque[1] == 0.0 && (u.pending[1] - 0.16).abs() < 1e-12),
        "0.6 s holds its torque between updates"
    );
    assert!(
        (record[31].torque[1] - 0.16).abs() < 1e-12,
        "0.6 s, next update: {:?}",
        record[31]
    );
}

#[test]
fn lesson54_the_short_period_settles_and_the_long_period_grows_until_the_abort() {
    // 0.02 s: the angle settles at the 0.01 rad target without overshoot. 0.6 s: the oscillation grows until the angle
    // leaves ±0.2 rad, between 2 and 4 s.
    let record = lesson54_run(500);
    let run = &record[1..];
    assert!(
        run.iter().all(|u| !u.abort[0] && u.angle[0] <= 0.01 + 1e-6),
        "0.02 s: no abort and no overshoot"
    );
    assert!(
        run[250..].iter().all(|u| (u.angle[0] - 0.01).abs() < 1e-4),
        "0.02 s settles within 5 s"
    );
    let abort = run
        .iter()
        .position(|u| u.abort[1])
        .expect("the 0.6 s loop aborts") as f64
        * 0.02;
    assert!((2.0..4.0).contains(&abort), "0.6 s aborts at {abort} s");
}

#[test]
fn lesson54_an_abort_clears_the_pending_torque_and_brakes_the_shaft() {
    // After the abort nothing calculated earlier is applied: the pending torque is 0 and the shaft is braked by −7·ω
    // until it rests, well inside ±0.4 rad.
    let record = lesson54_run(500);
    let run = &record[1..];
    let abort = run
        .iter()
        .position(|u| u.abort[1])
        .expect("the 0.6 s loop aborts");
    assert!(
        run[abort..]
            .iter()
            .all(|u| u.abort[1] && u.pending[1] == 0.0),
        "the abort latches and clears the pending torque"
    );
    assert!(
        run[abort..]
            .iter()
            .all(|u| (u.torque[1] + 7.0 * u.velocity[1]).abs() < 1e-12),
        "braking torque −7·ω from the measured velocity"
    );
    assert!(
        run[abort + 100..]
            .iter()
            .all(|u| u.velocity[1].abs() < 1e-3 && u.angle[1].abs() < 0.4),
        "at rest: {:?}",
        run.last()
    );
}
