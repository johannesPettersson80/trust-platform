use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON46: &str = "07-classical-stability-and-frequency-design/43-characteristic-equation";

/// One 20 ms update of the PD shaft: the measured state, the torques and the prediction from the modes.
#[derive(Clone, Copy, Debug)]
struct Knock {
    angle: f64,
    velocity: f64,
    torque: f64,
    knock: f64,
    mode_angle: f64,
    mode_velocity: f64,
    sampled_angle: f64,
    sampled_velocity: f64,
}

/// `rest` updates at rest on target 0 rad, then Control Studio's project experiment (activation at %MX24.0), which
/// knocks the shaft, for `updates` more.
fn lesson46_run(rest: u64, updates: u64) -> Vec<Knock> {
    let (mut runtime, mut simulation) = setup(LESSON46);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..rest + updates {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
        record.push(Knock {
            angle: number(&runtime, "%IL0"),
            velocity: number(&runtime, "%IL8"),
            torque: number(&runtime, "%QL0"),
            knock: number(&runtime, "%QL8"),
            mode_angle: number(&runtime, "%ML8"),
            mode_velocity: number(&runtime, "%ML16"),
            sampled_angle: number(&runtime, "%ML56"),
            sampled_velocity: number(&runtime, "%ML64"),
        });
    }
    record
}

#[test]
fn lesson46_the_program_forms_the_characteristic_equation_of_the_pd_shaft() {
    // J·s² + (B + Kd)·s + Kp = 0 divided by J: s² + 8s + 16 = (s + 4)², the repeated root −4 s⁻¹.
    let (mut runtime, mut simulation) = setup(LESSON46);
    let debug = runtime.enable_debug();
    scan_every(&mut runtime, &mut simulation, &debug, 0, 20, &[]);
    assert_eq!(number(&runtime, "%ML32"), 8.0, "(B + Kd)/J");
    assert_eq!(number(&runtime, "%ML40"), 16.0, "Kp/J");
    assert_eq!(number(&runtime, "%ML48"), -4.0, "the repeated root");
}

#[test]
fn lesson46_the_knock_pushes_for_a_tenth_of_a_second() {
    let record = lesson46_run(25, 20);
    let run = &record[25..];
    assert!(
        record[..25]
            .iter()
            .all(|update| update.knock == 0.0 && update.angle == 0.0),
        "at rest before the start"
    );
    assert!(
        run[..5].iter().all(|update| update.knock == 10.0),
        "10 N·m for five updates: {:?}",
        &run[..5]
    );
    assert!(
        run[5..].iter().all(|update| update.knock == 0.0),
        "then nothing: {:?}",
        &run[5..]
    );
}

#[test]
fn lesson46_after_the_knock_the_shaft_moves_in_its_natural_modes() {
    // Without external input, z = (z1 + (v1 + 4·z1)·t)·e^(−4t) from the state (z1, v1) at the knock's end.
    let record = lesson46_run(25, 150);
    let run = &record[25..];
    let end = run[5];
    assert!(
        end.angle > 0.03 && end.velocity > 0.5,
        "the knock has moved the shaft: {end:?}"
    );
    assert_eq!(
        (end.mode_angle, end.mode_velocity),
        (end.angle, end.velocity),
        "the prediction starts from the measured state"
    );
    for update in &run[5..] {
        assert!(
            (update.angle - update.mode_angle).abs() <= 0.012,
            "angle within 0.012 rad of the modes: {update:?}"
        );
        assert!(
            (update.velocity - update.mode_velocity).abs() <= 0.05,
            "velocity within 0.05 rad/s of the modes: {update:?}"
        );
    }
    let peak = (0..run.len())
        .max_by(|&a, &b| run[a].angle.total_cmp(&run[b].angle))
        .unwrap();
    assert!(
        (12..=16).contains(&peak) && (0.09..=0.10).contains(&run[peak].angle),
        "one peak near 0.28 s: {peak} {:?}",
        run[peak]
    );
    assert!(
        run.iter().all(|update| update.angle >= -0.002),
        "the repeated real root does not swing back through the target"
    );
    assert!(
        run.iter().all(|update| update.torque.abs() < 20.0),
        "the torque stays inside its limits"
    );
}

#[test]
fn lesson46_the_equation_of_the_sampled_law_predicts_the_motion() {
    // The torque is held for 20 ms and D uses the angle change over the last update: P acts 10 ms and D 20 ms late.
    // J·s² + B·s + Kp·e^(−0.01s) + Kd·s·e^(−0.02s) ≈ 0.86·s² + 7.84·s + 16 = 0, the roots −3.084 and −6.032 s⁻¹.
    let (mut runtime, mut simulation) = setup(LESSON46);
    let debug = runtime.enable_debug();
    scan_every(&mut runtime, &mut simulation, &debug, 0, 20, &[]);
    assert!(
        (number(&runtime, "%ML72") + 3.0843).abs() < 1e-4,
        "slow root: {}",
        number(&runtime, "%ML72")
    );
    assert!(
        (number(&runtime, "%ML80") + 6.0319).abs() < 1e-4,
        "fast root: {}",
        number(&runtime, "%ML80")
    );
    let record = lesson46_run(25, 150);
    let run = &record[25..];
    assert!(
        (run[5].sampled_angle - run[5].angle).abs() < 1e-12
            && (run[5].sampled_velocity - run[5].velocity).abs() < 1e-12,
        "from the measured state: {:?}",
        run[5]
    );
    for update in &run[5..] {
        assert!(
            (update.angle - update.sampled_angle).abs() <= 0.002,
            "angle within 0.002 rad: {update:?}"
        );
        assert!(
            (update.velocity - update.sampled_velocity).abs() <= 0.02,
            "velocity within 0.02 rad/s: {update:?}"
        );
    }
}

const LESSON47: &str = "07-classical-stability-and-frequency-design/45-root-locus";

/// One 20 ms update of the two shafts: the candidate gain (angle, velocity, torque) and the alternative gain.
#[derive(Clone, Copy, Debug)]
struct Gains {
    angle: f64,
    velocity: f64,
    torque: f64,
    alternative_angle: f64,
    alternative_velocity: f64,
    alternative_torque: f64,
}

/// `rest` updates at rest, then Control Studio's project experiment (activation at %MX96.0) steps both targets to 1 rad.
fn lesson47_run(rest: u64, updates: u64) -> Vec<Gains> {
    let (mut runtime, mut simulation) = setup(LESSON47);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..rest + updates {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX96.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
        record.push(Gains {
            angle: number(&runtime, "%IL0"),
            velocity: number(&runtime, "%IL8"),
            torque: number(&runtime, "%QL0"),
            alternative_angle: number(&runtime, "%IL16"),
            alternative_velocity: number(&runtime, "%IL24"),
            alternative_torque: number(&runtime, "%QL8"),
        });
    }
    record
}

#[test]
fn lesson47_both_shafts_run_the_same_law_with_their_own_gain() {
    // u = Kp·(r − θ) − 7·ω within ±20 N·m: Kp = 16 for the candidate, 12 for the alternative.
    let record = lesson47_run(10, 150);
    for update in &record[10..] {
        let law = |kp: f64, angle: f64, velocity: f64| {
            (kp * (1.0 - angle) - 7.0 * velocity).clamp(-20.0, 20.0)
        };
        assert!(
            (update.torque - law(16.0, update.angle, update.velocity)).abs() < 1e-12,
            "candidate: {update:?}"
        );
        assert!(
            (update.alternative_torque
                - law(12.0, update.alternative_angle, update.alternative_velocity))
            .abs()
                < 1e-12,
            "alternative: {update:?}"
        );
    }
    assert!(
        record[..10]
            .iter()
            .all(|update| update.torque == 0.0 && update.alternative_torque == 0.0),
        "no torque before the start"
    );
}

#[test]
fn lesson47_the_repeated_root_meets_the_requirement_and_the_slow_mode_does_not() {
    // Within ±0.02 rad of the 1 rad target from 2 s on, overshoot at most 0.05 rad. Kp = 16 (s + 4)² settles in about
    // 1.46 s; Kp = 12 keeps the slow mode e^(−2t): 1 − 1.5·e^(−4) = 0.9725 rad at 2 s.
    let record = lesson47_run(10, 405);
    let run = &record[10..];
    assert!(
        run[100..]
            .iter()
            .all(|update| (update.angle - 1.0).abs() <= 0.02),
        "the candidate from 2 s on"
    );
    assert!(
        run.iter().all(|update| update.angle <= 1.05),
        "no overshoot beyond 0.05 rad"
    );
    assert!(
        (run[100].alternative_angle - 0.9725).abs() < 0.003,
        "the alternative at 2 s: {:?}",
        run[100]
    );
    let settled = (0..run.len())
        .find(|&k| {
            run[k..]
                .iter()
                .all(|update| (update.alternative_angle - 1.0).abs() <= 0.02)
        })
        .unwrap();
    assert!(
        (settled as f64 * 0.02 - 2.16).abs() <= 0.04,
        "the alternative settles at about 2.16 s: {}",
        settled as f64 * 0.02
    );
}
