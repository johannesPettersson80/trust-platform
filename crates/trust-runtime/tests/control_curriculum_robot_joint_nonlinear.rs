use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const DT: f64 = 0.02;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/12-nonlinear-adaptive-and-robust-control/70-robot-joint-nonlinear")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 70 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 70 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 70 program before configuration")
        .replace("ControllerMode : INT :=", "ControllerMode AT %MW0 : INT :=")
        .replace(
            "ActiveArchitecture : INT :=",
            "ActiveArchitecture AT %MW2 : INT :=",
        )
        .replace("Enabled : BOOL :=", "Enabled AT %MX4.0 : BOOL :=")
        .replace(
            "ModelEligible : BOOL :=",
            "ModelEligible AT %MX4.1 : BOOL :=",
        )
        .replace(
            "FallbackActive : BOOL :=",
            "FallbackActive AT %MX4.2 : BOOL :=",
        )
        .replace(
            "TorqueSaturated : BOOL :=",
            "TorqueSaturated AT %MX4.3 : BOOL :=",
        )
        .replace(
            "JointSetpoint : LREAL :=",
            "JointSetpoint AT %ML8 : LREAL :=",
        )
        .replace(
            "FixedProportionalGain : LREAL :=",
            "FixedProportionalGain AT %ML16 : LREAL :=",
        )
        .replace(
            "ScheduledProportionalGain : LREAL :=",
            "ScheduledProportionalGain AT %ML24 : LREAL :=",
        )
        .replace(
            "ActiveProportionalGain : LREAL :=",
            "ActiveProportionalGain AT %ML32 : LREAL :=",
        )
        .replace(
            "EstimatedInertia : LREAL :=",
            "EstimatedInertia AT %ML40 : LREAL :=",
        )
        .replace(
            "EstimatedGravityCoefficient : LREAL :=",
            "EstimatedGravityCoefficient AT %ML48 : LREAL :=",
        )
        .replace(
            "EstimatedViscousCoefficient : LREAL :=",
            "EstimatedViscousCoefficient AT %ML56 : LREAL :=",
        )
        .replace(
            "GravityCompensation : LREAL :=",
            "GravityCompensation AT %ML64 : LREAL :=",
        )
        .replace(
            "TorqueRequested : LREAL :=",
            "TorqueRequested AT %ML72 : LREAL :=",
        )
        .replace(
            "SchedulingVariable : LREAL :=",
            "SchedulingVariable AT %ML80 : LREAL :=",
        )
        .replace(
            "ModelCompensation : LREAL :=",
            "ModelCompensation AT %ML88 : LREAL :=",
        )
        .replace(
            "TorqueApplied : LREAL :=",
            "TorqueApplied AT %QL0 : LREAL :=",
        )
        .replace("JointAngle : LREAL :=", "JointAngle AT %IL0 : LREAL :=")
        .replace("JointRate : LREAL :=", "JointRate AT %IL8 : LREAL :=")
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path("robot-joint-nonlinear/src/Main.st", mapped_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 70 Structured Text and Control library")
}

fn temporary_config_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-robot-joint-{label}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn simulation(file: &str, label: &str) -> SimulationController {
    let path = temporary_config_path(label);
    std::fs::write(&path, project_file(file)).expect("write Lesson 70 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 70 simulation config");
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

fn write_int(runtime: &mut Runtime, address: &str, value: i16) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid INT address"),
            Value::Word(value as u16),
        )
        .expect("write INT lesson selector");
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

fn read_int(runtime: &Runtime, address: &str) -> i16 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid INT address"))
        .expect("read INT lesson signal")
    {
        Value::Word(bits) => bits as i16,
        value => panic!("expected INT, got {value:?}"),
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

fn scheduled_gain(setpoint: f64) -> f64 {
    let angle = setpoint.abs().min(1.2);
    if angle <= 0.6 {
        15.19 + (16.903_457_618 - 15.19) * angle / 0.6
    } else {
        16.903_457_618 + (21.445_270_429 - 16.903_457_618) * (angle - 0.6) / 0.6
    }
}

fn setpoint(sample: usize) -> f64 {
    let time = sample as f64 * DT;
    if time < 1.0 {
        0.0
    } else if time < 11.0 {
        1.2
    } else if time < 16.0 {
        0.0
    } else if time < 26.0 {
        -1.2
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    setpoint: f64,
    angle: f64,
    rate: f64,
    requested: f64,
    applied: f64,
}

#[derive(Debug)]
struct Metrics {
    iae: f64,
    movement: f64,
    peak_requested: f64,
    peak_applied: f64,
    saturation_seconds: f64,
    overshoot_percent: [f64; 4],
    final_errors: [f64; 4],
    settling_times: [Option<f64>; 4],
}

fn segment_metrics(samples: &[Sample], start: usize, end: usize) -> (f64, Option<f64>, f64) {
    let window = 25;
    let final_start = end - window;
    let final_error = samples[final_start..end]
        .iter()
        .map(|sample| (sample.setpoint - sample.angle).abs())
        .sum::<f64>()
        / window as f64;
    let mut last_bad = None;
    for (offset, sample) in samples[start..end].iter().enumerate() {
        if (sample.setpoint - sample.angle).abs() > 0.02 || sample.rate.abs() > 0.05 {
            last_bad = Some(offset);
        }
    }
    let settling = match last_bad {
        Some(index) if end - start - index - 1 >= window => Some((index + 1) as f64 * DT),
        None => Some(0.0),
        _ => None,
    };
    let target = samples[start].setpoint;
    let preceding = samples[start - 1].setpoint;
    let step = target - preceding;
    let overshoot = samples[start..end]
        .iter()
        .map(|sample| {
            if step > 0.0 {
                (sample.angle - target).max(0.0)
            } else {
                (target - sample.angle).max(0.0)
            }
        })
        .fold(0.0_f64, f64::max)
        / step.abs()
        * 100.0;
    (final_error, settling, overshoot)
}

fn settling_spread(times: &[Option<f64>; 4]) -> f64 {
    let values: Vec<f64> = times.iter().copied().flatten().collect();
    assert_eq!(
        values.len(),
        4,
        "settling spread requires four settled plateaus"
    );
    values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - values.iter().copied().fold(f64::INFINITY, f64::min)
}

fn run(mode: i16, simulation_file: &str, label: &str) -> Metrics {
    let mut runtime = build_runtime();
    let mut plant = simulation(simulation_file, label);
    plant.apply_pre_cycle(Duration::ZERO, &mut runtime).unwrap();
    write_bool(&mut runtime, "%MX4.0", true);
    write_int(&mut runtime, "%MW0", mode);
    write_lreal(&mut runtime, "%ML16", 15.19);
    write_lreal(&mut runtime, "%ML40", 1.0);
    write_lreal(&mut runtime, "%ML48", 9.81);
    write_lreal(&mut runtime, "%ML56", 0.2);

    let mut samples = Vec::with_capacity(1550);
    let mut iae = 0.0;
    let mut movement = 0.0;
    let mut peak_requested: f64 = 0.0;
    let mut peak_applied: f64 = 0.0;
    let mut saturation_seconds = 0.0;
    let mut previous_applied = 0.0;
    for sample_index in 0..1550 {
        let reference = setpoint(sample_index);
        write_lreal(&mut runtime, "%ML8", reference);
        runtime
            .execute_cycle()
            .expect("execute robot-joint controller interval");
        let sample = Sample {
            setpoint: reference,
            angle: read_lreal(&runtime, "%IL0"),
            rate: read_lreal(&runtime, "%IL8"),
            requested: read_lreal(&runtime, "%ML72"),
            applied: read_lreal(&runtime, "%QL0"),
        };
        assert!(sample.applied.abs() <= 30.0 + 1e-12);
        assert!((read_lreal(&runtime, "%ML24") - scheduled_gain(reference)).abs() <= 1e-9);
        assert!((read_lreal(&runtime, "%ML64") - 9.81 * sample.angle.sin()).abs() <= 1e-9);
        assert!(
            (read_lreal(&runtime, "%ML88") - (9.81 * sample.angle.sin() + 0.2 * sample.rate)).abs()
                <= 1e-9
        );
        assert_eq!(read_bool(&runtime, "%MX4.1"), true);
        assert_eq!(read_bool(&runtime, "%MX4.2"), false);
        assert_eq!(read_int(&runtime, "%MW2"), mode);
        let expected_active_gain = if mode == 0 {
            15.19
        } else if mode == 1 {
            scheduled_gain(reference)
        } else {
            24.0
        };
        assert!(
            (read_lreal(&runtime, "%ML32") - expected_active_gain).abs() <= 1e-9,
            "active gain must match the selected architecture"
        );
        iae += (reference - sample.angle).abs() * DT;
        movement += (sample.applied - previous_applied).abs();
        peak_requested = peak_requested.max(sample.requested.abs());
        peak_applied = peak_applied.max(sample.applied.abs());
        if read_bool(&runtime, "%MX4.3") {
            saturation_seconds += DT;
        }
        previous_applied = sample.applied;
        samples.push(sample);
        let now = Duration::from_millis((sample_index as i64 + 1) * 20);
        plant.apply_post_cycle(now, &runtime).unwrap();
        plant.apply_pre_cycle(now, &mut runtime).unwrap();
    }
    let first = segment_metrics(&samples, 50, 550);
    let second = segment_metrics(&samples, 550, 800);
    let third = segment_metrics(&samples, 800, 1300);
    let fourth = segment_metrics(&samples, 1300, 1550);
    Metrics {
        iae,
        movement,
        peak_requested,
        peak_applied,
        saturation_seconds,
        overshoot_percent: [first.2, second.2, third.2, fourth.2],
        final_errors: [first.0, second.0, third.0, fourth.0],
        settling_times: [first.1, second.1, third.1, fourth.1],
    }
}

#[test]
fn control_curriculum_robot_joint_compares_scheduling_and_linearisation() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 70 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 70 must use schema 4");
    assert_eq!(lesson["sequence"], 70);
    assert_eq!(lesson["prerequisites"][0], "control-pendulum-lyapunov");
    assert_eq!(lesson["nextLesson"], "control-extruder-adaptive");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_robot_joint_compares_scheduling_and_linearisation"
    );

    assert!((scheduled_gain(0.0) - 15.19).abs() <= 1e-12);
    assert!((scheduled_gain(0.6) - 16.903_457_618).abs() <= 1e-12);
    assert!((scheduled_gain(1.2) - 21.445_270_429).abs() <= 1e-12);
    assert!((9.81 * 0.5_f64.sin() - 4.703_164_533_707_231).abs() <= 1e-12);
    assert!((9.81 * 1.2_f64.sin() - 9.143_303_433_338_49).abs() <= 1e-12);

    let fixed_nominal = run(0, "simulation.toml", "fixed-nominal");
    let scheduled_nominal = run(1, "simulation.toml", "scheduled-nominal");
    let linearised_nominal = run(2, "simulation.toml", "linearised-nominal");
    let fixed_heavy = run(0, "simulation-heavy-payload.toml", "fixed-heavy");
    let scheduled_heavy = run(1, "simulation-heavy-payload.toml", "scheduled-heavy");
    let linearised_heavy = run(2, "simulation-heavy-payload.toml", "linearised-heavy");

    eprintln!("robot joint nominal fixed/scheduled/linearised: IAE={:.4}/{:.4}/{:.4} rad*s, movement={:.2}/{:.2}/{:.2} N*m, peak={:.2}/{:.2}/{:.2} N*m, saturation={:.2}/{:.2}/{:.2} s, overshoot={:?}/{:?}/{:?}, final={:?}/{:?}/{:?}, settling={:?}/{:?}/{:?}", fixed_nominal.iae, scheduled_nominal.iae, linearised_nominal.iae, fixed_nominal.movement, scheduled_nominal.movement, linearised_nominal.movement, fixed_nominal.peak_applied, scheduled_nominal.peak_applied, linearised_nominal.peak_applied, fixed_nominal.saturation_seconds, scheduled_nominal.saturation_seconds, linearised_nominal.saturation_seconds, fixed_nominal.overshoot_percent, scheduled_nominal.overshoot_percent, linearised_nominal.overshoot_percent, fixed_nominal.final_errors, scheduled_nominal.final_errors, linearised_nominal.final_errors, fixed_nominal.settling_times, scheduled_nominal.settling_times, linearised_nominal.settling_times);
    eprintln!("robot joint heavy fixed/scheduled/linearised: IAE={:.4}/{:.4}/{:.4} rad*s, movement={:.2}/{:.2}/{:.2} N*m, peak requested={:.2}/{:.2}/{:.2} N*m, saturation={:.2}/{:.2}/{:.2} s, overshoot={:?}/{:?}/{:?}, final={:?}/{:?}/{:?}, settling={:?}/{:?}/{:?}", fixed_heavy.iae, scheduled_heavy.iae, linearised_heavy.iae, fixed_heavy.movement, scheduled_heavy.movement, linearised_heavy.movement, fixed_heavy.peak_requested, scheduled_heavy.peak_requested, linearised_heavy.peak_requested, fixed_heavy.saturation_seconds, scheduled_heavy.saturation_seconds, linearised_heavy.saturation_seconds, fixed_heavy.overshoot_percent, scheduled_heavy.overshoot_percent, linearised_heavy.overshoot_percent, fixed_heavy.final_errors, scheduled_heavy.final_errors, linearised_heavy.final_errors, fixed_heavy.settling_times, scheduled_heavy.settling_times, linearised_heavy.settling_times);

    assert_eq!(linearised_nominal.saturation_seconds, 0.0);
    assert!(linearised_nominal
        .settling_times
        .iter()
        .all(Option::is_some));
    assert!(linearised_nominal
        .settling_times
        .iter()
        .flatten()
        .all(|time| *time <= 4.0));
    assert!(
        settling_spread(&scheduled_nominal.settling_times)
            < settling_spread(&fixed_nominal.settling_times),
        "scheduled PID must reduce nominal settling-time variation"
    );
    assert!(scheduled_heavy.final_errors[0] <= 0.02);
    assert!(scheduled_heavy.final_errors[2] <= 0.02);
    assert!(linearised_heavy.final_errors[0] >= scheduled_heavy.final_errors[0] + 0.03);
    assert!(linearised_heavy.final_errors[2] >= scheduled_heavy.final_errors[2] + 0.03);

    let mut invalid = build_runtime();
    let mut invalid_plant = simulation("simulation.toml", "invalid-model");
    invalid_plant
        .apply_pre_cycle(Duration::ZERO, &mut invalid)
        .unwrap();
    write_bool(&mut invalid, "%MX4.0", true);
    write_int(&mut invalid, "%MW0", 2);
    write_lreal(&mut invalid, "%ML8", 1.2);
    write_lreal(&mut invalid, "%ML16", 15.19);
    write_lreal(&mut invalid, "%ML40", 0.0);
    write_lreal(&mut invalid, "%ML48", 9.81);
    write_lreal(&mut invalid, "%ML56", 0.2);
    invalid
        .execute_cycle()
        .expect("execute invalid-model fallback");
    assert!(!read_bool(&invalid, "%MX4.1"));
    assert!(read_bool(&invalid, "%MX4.2"));
    assert_eq!(read_int(&invalid, "%MW2"), 1);

    write_bool(&mut invalid, "%MX4.0", false);
    invalid
        .execute_cycle()
        .expect("disable robot-joint controller");
    assert_eq!(read_lreal(&invalid, "%QL0"), 0.0);

    let architecture = [0.0, 1.0, 2.0];
    let nominal_iae = [
        fixed_nominal.iae,
        scheduled_nominal.iae,
        linearised_nominal.iae,
    ];
    let heavy_iae = [fixed_heavy.iae, scheduled_heavy.iae, linearised_heavy.iae];
    let nominal_movement = [
        fixed_nominal.movement,
        scheduled_nominal.movement,
        linearised_nominal.movement,
    ];
    let heavy_movement = [
        fixed_heavy.movement,
        scheduled_heavy.movement,
        linearised_heavy.movement,
    ];
    let mut summary = String::from(
        "architecture,nominal_iae_rad_s,heavy_iae_rad_s,nominal_movement_nm,heavy_movement_nm\n",
    );
    for (index, name) in ["fixed", "scheduled", "feedback_linearised"]
        .iter()
        .enumerate()
    {
        writeln!(
            summary,
            "{name},{:.9},{:.9},{:.9},{:.9}",
            nominal_iae[index], heavy_iae[index], nominal_movement[index], heavy_movement[index]
        )
        .unwrap();
    }
    let figure_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/12-nonlinear-adaptive-and-robust-control/70-robot-joint-nonlinear/figures");
    evidence::retain(
        &figure_root,
        "robot-joint-nonlinear-evidence",
        "Wide-range nonlinear control must survive payload change",
        "Index 0 is fixed gain, 1 is gain scheduled, and 2 uses feedback linearisation.",
        &[
            evidence::Panel {
                title: "Integrated absolute position error",
                x_unit: "architecture index",
                y_unit: "rad s",
                x_minimum: -0.1,
                x_maximum: 2.1,
                y_minimum: 0.0,
                y_maximum: nominal_iae
                    .iter()
                    .chain(&heavy_iae)
                    .copied()
                    .fold(0.0_f64, f64::max)
                    * 1.1,
                series: vec![
                    evidence::Series {
                        name: "Nominal payload",
                        color: "#2563eb",
                        dashed: false,
                        x: &architecture,
                        y: &nominal_iae,
                    },
                    evidence::Series {
                        name: "Heavy payload",
                        color: "#dc2626",
                        dashed: true,
                        x: &architecture,
                        y: &heavy_iae,
                    },
                ],
            },
            evidence::Panel {
                title: "Cumulative motor-torque movement",
                x_unit: "architecture index",
                y_unit: "N m",
                x_minimum: -0.1,
                x_maximum: 2.1,
                y_minimum: 0.0,
                y_maximum: nominal_movement
                    .iter()
                    .chain(&heavy_movement)
                    .copied()
                    .fold(0.0_f64, f64::max)
                    * 1.1,
                series: vec![
                    evidence::Series {
                        name: "Nominal payload",
                        color: "#2563eb",
                        dashed: false,
                        x: &architecture,
                        y: &nominal_movement,
                    },
                    evidence::Series {
                        name: "Heavy payload",
                        color: "#dc2626",
                        dashed: true,
                        x: &architecture,
                        y: &heavy_movement,
                    },
                ],
            },
        ],
        &[],
        &summary,
    );
}
