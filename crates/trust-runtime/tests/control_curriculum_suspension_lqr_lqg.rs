use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn evidence_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/11-state-space-estimation-and-predictive-control/67-suspension-lqr/figures")
}

fn plot_bounds<'a>(series: impl IntoIterator<Item = &'a [f64]>) -> (f64, f64) {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for values in series {
        for value in values {
            minimum = minimum.min(*value);
            maximum = maximum.max(*value);
        }
    }
    let padding = ((maximum - minimum) * 0.08).max(0.005);
    (minimum - padding, maximum + padding)
}

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/11-state-space-estimation-and-predictive-control/67-suspension-lqr")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 67 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 67 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 67 program before configuration")
        .replace("DesignMode : DINT :=", "DesignMode AT %MD0 : DINT :=")
        .replace(
            "ForceRequested : LREAL :=",
            "ForceRequested AT %ML8 : LREAL :=",
        )
        .replace(
            "EstimatedDeflection : LREAL :=",
            "EstimatedDeflection AT %ML16 : LREAL :=",
        )
        .replace(
            "EstimatedVelocity : LREAL :=",
            "EstimatedVelocity AT %ML24 : LREAL :=",
        )
        .replace("Innovation : LREAL :=", "Innovation AT %ML32 : LREAL :=")
        .replace(
            "RoadAccelerationRequested : LREAL :=",
            "RoadAccelerationRequested AT %ML40 : LREAL :=",
        )
        .replace(
            "PreviousForce : LREAL :=",
            "PreviousForce AT %ML48 : LREAL :=",
        )
        .replace(
            "PreviousRoad : LREAL :=",
            "PreviousRoad AT %ML56 : LREAL :=",
        )
        .replace("Enabled : BOOL :=", "Enabled AT %MX120.0 : BOOL :=")
        .replace(
            "ControllerValid : BOOL :=",
            "ControllerValid AT %MX120.1 : BOOL :=",
        )
        .replace("Saturated : BOOL :=", "Saturated AT %MX120.2 : BOOL :=")
        .replace("ForceApplied : LREAL :=", "ForceApplied AT %QL0 : LREAL :=")
        .replace(
            "RoadAccelerationApplied : LREAL :=",
            "RoadAccelerationApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "FullStateDeflectionMeasurement : LREAL :=",
            "FullStateDeflectionMeasurement AT %IL0 : LREAL :=",
        )
        .replace(
            "FullStateVelocityMeasurement : LREAL :=",
            "FullStateVelocityMeasurement AT %IL8 : LREAL :=",
        )
        .replace(
            "PositionOnlyMeasurement : LREAL :=",
            "PositionOnlyMeasurement AT %IL16 : LREAL :=",
        )
        .replace(
            "PositionOnlyVelocityTruth : LREAL :=",
            "PositionOnlyVelocityTruth AT %IL24 : LREAL :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![SourceFile::with_path(
        "suspension-lqr/src/Main.st",
        mapped_source(),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 67 Structured Text")
}

fn temporary_config_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-suspension-lqr-lqg-{label}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn simulation(file: &str) -> SimulationController {
    let path = temporary_config_path(file);
    std::fs::write(&path, project_file(file)).expect("write Lesson 67 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 67 simulation config");
    let _ = std::fs::remove_file(path);
    SimulationController::new(config)
}

fn write_lreal(runtime: &mut Runtime, address: &str, value: f64) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid LREAL address"),
            Value::LWord(value.to_bits()),
        )
        .expect("write LREAL lesson input");
}

fn write_dint(runtime: &mut Runtime, address: &str, value: i32) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid DINT address"),
            Value::DWord(value as u32),
        )
        .expect("write DINT lesson selector");
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid BOOL address"),
            Value::Bool(value),
        )
        .expect("write BOOL lesson selector");
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid LREAL address"))
        .expect("read LREAL lesson signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid BOOL address"))
        .expect("read BOOL lesson signal")
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

fn care_gain(q_position: f64, q_velocity: f64, r: f64) -> [f64; 2] {
    // The two-state CARE has a closed positive-root solution. Deriving it here
    // keeps the oracle independent of the gain literals executed by the lesson.
    let p12 = -16.0 * r + 2.0 * (64.0 * r * r + r * q_position).sqrt();
    let p22 = (-9.6 * r + (92.16 * r * r + 16.0 * r * (2.0 * p12 + q_velocity)).sqrt()) / 2.0;
    [0.5 * p12 / r, 0.5 * p22 / r]
}

fn steady_state_kalman_gain() -> [f64; 2] {
    let (a00, a01, a10, a11) = (0.980_840_04, 0.093_606_16, -0.374_424_62, 0.868_512_65);
    let (mut p00, mut p01, mut p11): (f64, f64, f64) = (1.0, 0.0, 1.0);
    let mut gain = [0.0; 2];
    for _ in 0..10_000 {
        let r00 = a00 * p00 + a01 * p01;
        let r01 = a00 * p01 + a01 * p11;
        let r10 = a10 * p00 + a11 * p01;
        let r11 = a10 * p01 + a11 * p11;
        let prior00 = r00 * a00 + r01 * a01 + 1e-7;
        let prior01 = r00 * a10 + r01 * a11;
        let prior11 = r10 * a10 + r11 * a11 + 1e-4;
        let innovation_variance = prior00 + 4e-6;
        gain = [prior00 / innovation_variance, prior01 / innovation_variance];
        let next00 = (1.0 - gain[0]) * prior00;
        let next01 = (1.0 - gain[0]) * prior01;
        let next11 = prior11 - gain[1] * prior01;
        let change = (next00 - p00)
            .abs()
            .max((next01 - p01).abs())
            .max((next11 - p11).abs());
        (p00, p01, p11) = (next00, next01, next11);
        if change <= 1e-15 {
            break;
        }
    }
    gain
}

#[derive(Default)]
struct Trace {
    position: Vec<f64>,
    velocity: Vec<f64>,
    position_measurement: Vec<f64>,
    estimated_position: Vec<f64>,
    estimated_velocity: Vec<f64>,
    innovation: Vec<f64>,
    force_requested: Vec<f64>,
    force_applied: Vec<f64>,
    road: Vec<f64>,
    saturated: Vec<bool>,
}

fn run(mode: i32, config: &str) -> Trace {
    let mut runtime = build_runtime();
    let mut plant = simulation(config);
    plant
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial suspension signals");
    let mut trace = Trace::default();
    for cycle in 0..120 {
        let road = if (10..15).contains(&cycle) { 0.5 } else { 0.0 };
        write_bool(&mut runtime, "%MX120.0", true);
        write_dint(&mut runtime, "%MD0", mode);
        write_lreal(&mut runtime, "%ML40", road);
        runtime
            .execute_cycle()
            .expect("execute suspension controller");
        assert!(
            read_bool(&runtime, "%MX120.1"),
            "valid lesson case rejected: mode={mode}, config={config}, cycle={cycle}, full-state={:.6}/{:.6}, measured-position={:.6}, estimate={:.6}/{:.6}",
            read_lreal(&runtime, "%IL0"),
            read_lreal(&runtime, "%IL8"),
            read_lreal(&runtime, "%IL16"),
            read_lreal(&runtime, "%ML16"),
            read_lreal(&runtime, "%ML24"),
        );
        trace.position.push(read_lreal(&runtime, "%IL0"));
        trace.velocity.push(read_lreal(&runtime, "%IL8"));
        trace
            .position_measurement
            .push(read_lreal(&runtime, "%IL16"));
        trace.estimated_position.push(read_lreal(&runtime, "%ML16"));
        trace.estimated_velocity.push(read_lreal(&runtime, "%ML24"));
        trace.innovation.push(read_lreal(&runtime, "%ML32"));
        trace.force_requested.push(read_lreal(&runtime, "%ML8"));
        trace.force_applied.push(read_lreal(&runtime, "%QL0"));
        trace.road.push(read_lreal(&runtime, "%QL8"));
        trace.saturated.push(read_bool(&runtime, "%MX120.2"));
        let now = Duration::from_millis((cycle as i64 + 1) * 100);
        plant
            .apply_post_cycle(now, &runtime)
            .expect("advance suspension copies");
        plant
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish suspension signals");
    }
    write_bool(&mut runtime, "%MX120.0", false);
    write_lreal(&mut runtime, "%ML40", 0.0);
    runtime
        .execute_cycle()
        .expect("disable suspension controller");
    assert_eq!(
        read_lreal(&runtime, "%QL0"),
        0.0,
        "disabled hydraulic force"
    );
    trace
}

fn mean(values: impl Iterator<Item = f64>, count: usize) -> f64 {
    values.sum::<f64>() / count as f64
}

fn rms(values: &[f64], range: std::ops::Range<usize>) -> f64 {
    mean(
        range.clone().map(|index| values[index] * values[index]),
        range.len(),
    )
    .sqrt()
}

fn rms_error(left: &[f64], right: &[f64], range: std::ops::Range<usize>) -> f64 {
    mean(
        range
            .clone()
            .map(|index| (left[index] - right[index]).powi(2)),
        range.len(),
    )
    .sqrt()
}

fn force_energy(trace: &Trace, range: std::ops::Range<usize>) -> f64 {
    range
        .map(|index| trace.force_applied[index].powi(2) * 0.1)
        .sum()
}

fn acceleration_rms(trace: &Trace, range: std::ops::Range<usize>) -> f64 {
    mean(
        range.clone().map(|index| {
            (-4.0 * trace.position[index] - 1.2 * trace.velocity[index]
                + 0.5 * trace.force_applied[index]
                + trace.road[index])
                .powi(2)
        }),
        range.len(),
    )
    .sqrt()
}

fn total_variation(values: &[f64], range: std::ops::Range<usize>) -> f64 {
    values[range]
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum()
}

fn peak_absolute(values: &[f64], range: std::ops::Range<usize>) -> f64 {
    values[range]
        .iter()
        .fold(0.0_f64, |peak, value| peak.max(value.abs()))
}

fn return_time(trace: &Trace) -> Option<f64> {
    (15..=trace.position.len() - 10)
        .find(|&candidate| {
            (candidate..candidate + 10).all(|index| {
                trace.position[index].abs() <= 0.002 && trace.velocity[index].abs() <= 0.01
            })
        })
        .map(|index| (index - 15) as f64 * 0.1)
}

#[test]
fn control_curriculum_suspension_lqr_lqg_qualifies_cost_and_estimation() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 67 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 67 must use schema 4");
    assert_eq!(lesson["sequence"], 67);
    assert_eq!(lesson["prerequisites"][0], "control-weigh-feeder-kalman");
    assert_eq!(lesson["nextLesson"], "control-building-hvac-mpc");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_suspension_lqr_lqg_qualifies_cost_and_estimation"
    );

    let source = project_file("src/Main.st");
    assert_eq!(source.matches("PositionOnlyVelocityTruth").count(), 2);
    for forbidden in [
        "AcceptedCandidate",
        "DeflectionRms",
        "ForceEnergy",
        "RANDOM",
    ] {
        assert!(
            !source.contains(forbidden),
            "ST embeds result '{forbidden}'"
        );
    }

    let expected = [
        ([20.0, 20.0, 1.0], [1.165_151_39, 3.115_487_79]),
        ([120.0, 2.0, 1.0], [5.564_659_97, 3.078_926_89]),
        ([40.0, 4.0, 1.0], [2.198_039_03, 1.907_221_39]),
        ([40.0, 4.0, 5.0], [0.485_281_37, 0.515_668_96]),
    ];
    for ([q_position, q_velocity, r], gain) in expected {
        let oracle = care_gain(q_position, q_velocity, r);
        assert!((oracle[0] - gain[0]).abs() <= 1e-8);
        assert!((oracle[1] - gain[1]).abs() <= 1e-8);
        assert!(1.2 + 0.5 * oracle[1] > 0.0);
        assert!(4.0 + 0.5 * oracle[0] > 0.0);
    }
    let observer_gain = steady_state_kalman_gain();
    // The shipped sampled matrices are rounded to eight decimal places.
    assert!((observer_gain[0] - 0.584_882_59).abs() <= 5e-8);
    assert!((observer_gain[1] - 2.246_828_47).abs() <= 5e-8);

    let pole = run(0, "simulation.toml");
    let body = run(1, "simulation.toml");
    let travel = run(2, "simulation.toml");
    let balanced = run(3, "simulation.toml");
    let effort = run(4, "simulation.toml");
    let lqg = run(5, "simulation.toml");
    let lqg_noisy = run(5, "simulation-noise-doubled.toml");
    let lqg_mismatch = run(5, "simulation-model-mismatch.toml");
    for trace in [
        &pole,
        &body,
        &travel,
        &balanced,
        &effort,
        &lqg,
        &lqg_noisy,
        &lqg_mismatch,
    ] {
        assert!(trace
            .position
            .iter()
            .chain(&trace.velocity)
            .chain(&trace.position_measurement)
            .chain(&trace.estimated_position)
            .chain(&trace.estimated_velocity)
            .chain(&trace.innovation)
            .chain(&trace.force_requested)
            .chain(&trace.force_applied)
            .chain(&trace.road)
            .all(|value| value.is_finite()));
        assert!(trace.position.iter().all(|value| value.abs() <= 0.08));
        assert!(trace.velocity.iter().all(|value| value.abs() <= 0.50));
        assert!(trace.force_applied.iter().all(|value| value.abs() <= 5.0));
        assert!(trace.road.iter().all(|value| value.abs() <= 2.0));
    }

    let window = 10..120;
    let body_velocity_rms = rms(&body.velocity, window.clone());
    let travel_position_rms = rms(&travel.position, window.clone());
    let effort_position_rms = rms(&effort.position, window.clone());
    let effort_velocity_rms = rms(&effort.velocity, window.clone());
    let body_energy = force_energy(&body, window.clone());
    let travel_energy = force_energy(&travel, window.clone());
    let balanced_energy = force_energy(&balanced, window.clone());
    let effort_energy = force_energy(&effort, window.clone());
    assert!(travel_position_rms < effort_position_rms);
    assert!(body_velocity_rms < effort_velocity_rms);
    assert!(effort_energy < body_energy && effort_energy < travel_energy);
    assert!(!balanced.saturated.iter().any(|value| *value));

    let lqg_position_error = rms_error(&lqg.estimated_position, &lqg.position, window.clone());
    let lqg_velocity_error = rms_error(&lqg.estimated_velocity, &lqg.velocity, window.clone());
    let lqg_innovation_mean = mean(lqg.innovation[window.clone()].iter().copied(), window.len());
    assert!(lqg_position_error <= 0.01);
    assert!(lqg_velocity_error <= 0.10);
    assert!(lqg_innovation_mean.abs() <= 0.002);
    let balanced_position_rms = rms(&balanced.position, window.clone());
    let lqg_position_rms = rms(&lqg.position, window.clone());
    assert!(lqg_position_rms <= 1.5 * balanced_position_rms);
    assert!(force_energy(&lqg, window.clone()) <= 2.0 * balanced_energy);
    assert!(
        rms_error(
            &lqg_noisy.estimated_position,
            &lqg_noisy.position,
            window.clone()
        ) >= lqg_position_error
    );
    assert!(
        rms_error(
            &lqg_mismatch.estimated_velocity,
            &lqg_mismatch.velocity,
            window.clone()
        ) >= lqg_velocity_error
    );

    let mut left = build_runtime();
    let mut right = build_runtime();
    for runtime in [&mut left, &mut right] {
        write_bool(runtime, "%MX120.0", true);
        write_dint(runtime, "%MD0", 5);
        write_lreal(runtime, "%IL16", 0.01);
    }
    write_lreal(&mut left, "%IL8", -100.0);
    write_lreal(&mut right, "%IL8", 100.0);
    left.execute_cycle()
        .expect("execute first LQG ownership case");
    right
        .execute_cycle()
        .expect("execute second LQG ownership case");
    assert_eq!(read_lreal(&left, "%QL0"), read_lreal(&right, "%QL0"));

    let mut invalid = build_runtime();
    write_bool(&mut invalid, "%MX120.0", true);
    write_dint(&mut invalid, "%MD0", 99);
    invalid
        .execute_cycle()
        .expect("execute invalid selector case");
    assert!(!read_bool(&invalid, "%MX120.1"));
    assert_eq!(read_lreal(&invalid, "%QL0"), 0.0);

    for (label, trace) in [
        ("pole", &pole),
        ("body", &body),
        ("travel", &travel),
        ("balanced", &balanced),
        ("effort", &effort),
        ("lqg", &lqg),
    ] {
        eprintln!(
            "suspension {label}: z_rms={:.6}m v_rms={:.6}m/s a_rms={:.6}m/s2 force_energy={:.6}kN2s force_movement={:.6}kN peak_force={:.6}kN return={:?}s",
            rms(&trace.position, window.clone()),
            rms(&trace.velocity, window.clone()),
            acceleration_rms(trace, window.clone()),
            force_energy(trace, window.clone()),
            total_variation(&trace.force_applied, window.clone()),
            peak_absolute(&trace.force_applied, window.clone()),
            return_time(trace),
        );
    }
    eprintln!(
        "suspension LQG estimator: nominal z/v RMS={lqg_position_error:.6}/{lqg_velocity_error:.6}, innovation mean={lqg_innovation_mean:.6}; doubled-noise z/v RMS={:.6}/{:.6}; mismatch z/v RMS={:.6}/{:.6}, innovation mean={:.6}",
        rms_error(&lqg_noisy.estimated_position, &lqg_noisy.position, window.clone()),
        rms_error(&lqg_noisy.estimated_velocity, &lqg_noisy.velocity, window.clone()),
        rms_error(&lqg_mismatch.estimated_position, &lqg_mismatch.position, window.clone()),
        rms_error(&lqg_mismatch.estimated_velocity, &lqg_mismatch.velocity, window.clone()),
        mean(lqg_mismatch.innovation[window.clone()].iter().copied(), window.len()),
    );

    let time = (0..balanced.position.len())
        .map(|sample| sample as f64 * 0.1)
        .collect::<Vec<_>>();
    let (position_min, position_max) = plot_bounds([
        body.position.as_slice(),
        travel.position.as_slice(),
        balanced.position.as_slice(),
        effort.position.as_slice(),
    ]);
    let (force_min, force_max) = plot_bounds([
        body.force_applied.as_slice(),
        travel.force_applied.as_slice(),
        balanced.force_applied.as_slice(),
        effort.force_applied.as_slice(),
    ]);
    let lqg_saturation = lqg
        .saturated
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let summary = [
        ("pole_placement", &pole),
        ("body_motion_lqr", &body),
        ("travel_lqr", &travel),
        ("balanced_lqr", &balanced),
        ("effort_lqr", &effort),
        ("balanced_lqg", &lqg),
    ]
    .iter()
    .fold(
        String::from("case,position_rms_m,velocity_rms_m_s,acceleration_rms_m_s2,force_energy_kN2_s,force_movement_kN,peak_force_kN,return_time_s\n"),
        |mut summary, (name, trace)| {
            use std::fmt::Write as _;
            writeln!(
                summary,
                "{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.3}",
                name,
                rms(&trace.position, window.clone()),
                rms(&trace.velocity, window.clone()),
                acceleration_rms(trace, window.clone()),
                force_energy(trace, window.clone()),
                total_variation(&trace.force_applied, window.clone()),
                peak_absolute(&trace.force_applied, window.clone()),
                return_time(trace).unwrap_or(f64::INFINITY),
            )
            .unwrap();
            summary
        },
    );
    evidence::retain(
        &evidence_root(),
        "suspension-lqr-evidence",
        "LQR weights expose comfort, travel, and effort tradeoffs",
        "No candidate is best on every physical measure; position-only LQG reproduces the balanced design with additional estimator activity.",
        &[
            evidence::Panel {
                title: "Suspension deflection",
                x_unit: "time (s)",
                y_unit: "deflection (m)",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: position_min,
                y_maximum: position_max,
                series: vec![
                    evidence::Series { name: "Body motion", color: "#2563eb", dashed: false, x: &time, y: &body.position },
                    evidence::Series { name: "Travel", color: "#059669", dashed: false, x: &time, y: &travel.position },
                    evidence::Series { name: "Balanced", color: "#7c3aed", dashed: false, x: &time, y: &balanced.position },
                    evidence::Series { name: "Effort", color: "#b45309", dashed: false, x: &time, y: &effort.position },
                ],
            },
            evidence::Panel {
                title: "Applied hydraulic force",
                x_unit: "time (s)",
                y_unit: "force (kN)",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: force_min,
                y_maximum: force_max,
                series: vec![
                    evidence::Series { name: "Body motion", color: "#2563eb", dashed: false, x: &time, y: &body.force_applied },
                    evidence::Series { name: "Travel", color: "#059669", dashed: false, x: &time, y: &travel.force_applied },
                    evidence::Series { name: "Balanced", color: "#7c3aed", dashed: false, x: &time, y: &balanced.force_applied },
                    evidence::Series { name: "Effort", color: "#b45309", dashed: false, x: &time, y: &effort.force_applied },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Road input", name: "Applied road acceleration", x: &time, y: &balanced.road, x_unit: "time (s)", y_unit: "acceleration (m/s2)" },
            evidence::RetainedSeries { panel: "Balanced actuator", name: "Requested force", x: &time, y: &balanced.force_requested, x_unit: "time (s)", y_unit: "force (kN)" },
            evidence::RetainedSeries { panel: "LQG response", name: "Deflection", x: &time, y: &lqg.position, x_unit: "time (s)", y_unit: "deflection (m)" },
            evidence::RetainedSeries { panel: "LQG estimate", name: "Measured position", x: &time, y: &lqg.position_measurement, x_unit: "time (s)", y_unit: "deflection (m)" },
            evidence::RetainedSeries { panel: "LQG estimate", name: "Estimated position", x: &time, y: &lqg.estimated_position, x_unit: "time (s)", y_unit: "deflection (m)" },
            evidence::RetainedSeries { panel: "LQG estimate", name: "Estimated velocity", x: &time, y: &lqg.estimated_velocity, x_unit: "time (s)", y_unit: "velocity (m/s)" },
            evidence::RetainedSeries { panel: "LQG estimate", name: "Innovation", x: &time, y: &lqg.innovation, x_unit: "time (s)", y_unit: "deflection (m)" },
            evidence::RetainedSeries { panel: "LQG actuator", name: "Requested force", x: &time, y: &lqg.force_requested, x_unit: "time (s)", y_unit: "force (kN)" },
            evidence::RetainedSeries { panel: "LQG actuator", name: "Applied force", x: &time, y: &lqg.force_applied, x_unit: "time (s)", y_unit: "force (kN)" },
            evidence::RetainedSeries { panel: "LQG actuator", name: "Saturated", x: &time, y: &lqg_saturation, x_unit: "time (s)", y_unit: "active (0/1)" },
            evidence::RetainedSeries { panel: "Noise qualification", name: "Doubled-noise estimate", x: &time, y: &lqg_noisy.estimated_position, x_unit: "time (s)", y_unit: "deflection (m)" },
            evidence::RetainedSeries { panel: "Mismatch qualification", name: "Mismatched estimate", x: &time, y: &lqg_mismatch.estimated_position, x_unit: "time (s)", y_unit: "deflection (m)" },
        ],
        &summary,
    );
}
