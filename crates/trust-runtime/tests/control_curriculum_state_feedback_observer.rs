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
        .join("../../examples/control/11-state-space-estimation-and-predictive-control/65-magnetic-bearing-state-feedback/figures")
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
    let padding = ((maximum - minimum) * 0.08).max(0.01);
    (minimum - padding, maximum + padding)
}

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/11-state-space-estimation-and-predictive-control/65-magnetic-bearing-state-feedback")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 65 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 65 file {}: {error}", path.display()))
}

fn temporary_config_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-state-feedback-observer-{label}-{}-{stamp}.toml",
        std::process::id()
    ))
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

fn build_runtime() -> Runtime {
    let source = project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 65 program before configuration")
        .replace(
            "BearingEnabled : BOOL :=",
            "BearingEnabled AT %MX120.0 : BOOL :=",
        )
        .replace(
            "MotorEnabled : BOOL :=",
            "MotorEnabled AT %MX120.1 : BOOL :=",
        )
        .replace(
            "MotorObserverEnabled : BOOL :=",
            "MotorObserverEnabled AT %MX120.2 : BOOL :=",
        )
        .replace(
            "BearingArchitecture : INT :=",
            "BearingArchitecture AT %MW0 : INT :=",
        )
        .replace(
            "BearingSetpoint : LREAL :=",
            "BearingSetpoint AT %ML8 : LREAL :=",
        )
        .replace(
            "MotorSetpoint : LREAL :=",
            "MotorSetpoint AT %ML16 : LREAL :=",
        )
        .replace(
            "BearingCurrentRequested : LREAL :=",
            "BearingCurrentRequested AT %ML24 : LREAL :=",
        )
        .replace(
            "MotorTorqueRequested : LREAL :=",
            "MotorTorqueRequested AT %ML32 : LREAL :=",
        )
        .replace(
            "EstimatedMotorSpeed : LREAL :=",
            "EstimatedMotorSpeed AT %ML40 : LREAL :=",
        )
        .replace(
            "EstimatedLoadTorque : LREAL :=",
            "EstimatedLoadTorque AT %ML48 : LREAL :=",
        )
        .replace(
            "MotorInnovation : LREAL :=",
            "MotorInnovation AT %ML56 : LREAL :=",
        )
        .replace(
            "BearingCurrentApplied : LREAL :=",
            "BearingCurrentApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "MotorTorqueApplied : LREAL :=",
            "MotorTorqueApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "BearingDisplacement : LREAL :=",
            "BearingDisplacement AT %IL0 : LREAL :=",
        )
        .replace(
            "BearingVelocity : LREAL :=",
            "BearingVelocity AT %IL8 : LREAL :=",
        )
        .replace("MotorSpeed : LREAL :=", "MotorSpeed AT %IL16 : LREAL :=")
        .replace(
            "MotorLoadTruth : LREAL :=",
            "MotorLoadTruth AT %IL24 : LREAL :=",
        );
    CompileSession::from_sources(vec![SourceFile::with_path(
        "magnetic-bearing-state-feedback/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 65 Structured Text")
}

fn simulation(file: &str) -> SimulationController {
    let path = temporary_config_path(file);
    std::fs::write(&path, project_file(file)).expect("write Lesson 65 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 65 simulation config");
    let _ = std::fs::remove_file(path);
    SimulationController::new(config)
}

#[derive(Default)]
struct BearingTrace {
    displacement: Vec<f64>,
    velocity: Vec<f64>,
    requested: Vec<f64>,
    applied: Vec<f64>,
}

fn run_bearing(config_file: &str, architecture: i16, reference: f64) -> BearingTrace {
    let mut runtime = build_runtime();
    let mut plant = simulation(config_file);
    plant
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial bearing state");
    let mut trace = BearingTrace::default();
    for cycle in 0..120 {
        write_bool(&mut runtime, "%MX120.0", true);
        write_bool(&mut runtime, "%MX120.1", false);
        write_int(&mut runtime, "%MW0", architecture);
        write_lreal(&mut runtime, "%ML8", reference);
        runtime.execute_cycle().expect("execute bearing controller");
        trace.displacement.push(read_lreal(&runtime, "%IL0"));
        trace.velocity.push(read_lreal(&runtime, "%IL8"));
        trace.requested.push(read_lreal(&runtime, "%ML24"));
        trace.applied.push(read_lreal(&runtime, "%QL0"));
        let now = Duration::from_millis((cycle as i64 + 1) * 100);
        plant
            .apply_post_cycle(now, &runtime)
            .expect("advance bearing plant");
        plant
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish bearing state");
    }
    write_bool(&mut runtime, "%MX120.0", false);
    runtime.execute_cycle().expect("disable bearing output");
    assert_eq!(
        read_lreal(&runtime, "%QL0"),
        0.0,
        "disabled bearing current"
    );
    trace
}

#[derive(Default)]
struct MotorTrace {
    speed: Vec<f64>,
    load_truth: Vec<f64>,
    estimated_speed: Vec<f64>,
    estimated_load: Vec<f64>,
    innovation: Vec<f64>,
    requested: Vec<f64>,
    applied: Vec<f64>,
}

fn run_motor(config_file: &str, observer_enabled: bool) -> MotorTrace {
    let mut runtime = build_runtime();
    let mut plant = simulation(config_file);
    plant
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial motor state");
    let mut trace = MotorTrace::default();
    for cycle in 0..200 {
        write_bool(&mut runtime, "%MX120.0", false);
        write_bool(&mut runtime, "%MX120.1", true);
        write_bool(&mut runtime, "%MX120.2", observer_enabled);
        write_lreal(&mut runtime, "%ML16", 1.0);
        runtime
            .execute_cycle()
            .expect("execute motor observer and controller");
        trace.speed.push(read_lreal(&runtime, "%IL16"));
        trace.load_truth.push(read_lreal(&runtime, "%IL24"));
        trace.estimated_speed.push(read_lreal(&runtime, "%ML40"));
        trace.estimated_load.push(read_lreal(&runtime, "%ML48"));
        trace.innovation.push(read_lreal(&runtime, "%ML56"));
        trace.requested.push(read_lreal(&runtime, "%ML32"));
        trace.applied.push(read_lreal(&runtime, "%QL16"));
        let now = Duration::from_millis((cycle as i64 + 1) * 100);
        plant
            .apply_post_cycle(now, &runtime)
            .expect("advance motor plant");
        plant
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish motor state");
    }
    write_bool(&mut runtime, "%MX120.1", false);
    runtime.execute_cycle().expect("disable motor output");
    assert_eq!(read_lreal(&runtime, "%QL16"), 0.0, "disabled motor torque");
    trace
}

fn first_settled_sample(
    first: &[f64],
    second: &[f64],
    first_limit: f64,
    second_limit: f64,
    window_samples: usize,
) -> Option<usize> {
    (0..=first.len().saturating_sub(window_samples)).find(|&start| {
        (start..start + window_samples)
            .all(|index| first[index].abs() <= first_limit && second[index].abs() <= second_limit)
    })
}

fn mean_abs(values: impl Iterator<Item = f64>, count: usize) -> f64 {
    values.map(f64::abs).sum::<f64>() / count as f64
}

fn max_abs(values: &[f64]) -> f64 {
    values.iter().copied().map(f64::abs).fold(0.0, f64::max)
}

fn total_variation(values: &[f64]) -> f64 {
    values
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum()
}

#[test]
fn control_curriculum_state_feedback_and_observer_match_executed_modes() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 65 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 65 must use schema 4");
    assert_eq!(lesson["sequence"], 65);
    assert_eq!(lesson["prerequisites"][0], "control-two-tank-observability");
    assert_eq!(lesson["nextLesson"], "control-weigh-feeder-kalman");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_state_feedback_and_observer_match_executed_modes"
    );

    let source = project_file("src/Main.st");
    assert!(!source.contains("Control.PID;"));
    for forbidden in [
        "AcceptedDesign",
        "BearingPole",
        "ObserverPole",
        "SettlingVerdict",
    ] {
        assert!(
            !source.contains(forbidden),
            "ST embeds analytical verdict '{forbidden}'"
        );
    }
    assert_eq!(
        source.matches("MotorLoadTruth").count(),
        2,
        "truth load may appear only in its declaration and diagnostic I/O binding"
    );

    let position_only_real = 0.0_f64;
    let position_only_imaginary = 1.0_f64;
    assert_eq!((position_only_real, position_only_imaginary), (0.0, 1.0));
    let feedback_discriminant = 7.0_f64.powi(2) - 4.0 * 12.0;
    let feedback_poles = (
        (-7.0 + feedback_discriminant.sqrt()) / 2.0,
        (-7.0 - feedback_discriminant.sqrt()) / 2.0,
    );
    assert_eq!(feedback_poles, (-3.0, -4.0));
    assert_eq!((16.0_f64, 7.0_f64, 12.0_f64), (16.0, 7.0, 12.0));
    let observer_discriminant = 9.0_f64.powi(2) - 4.0 * 20.0;
    let observer_poles = (
        (-9.0 + observer_discriminant.sqrt()) / 2.0,
        (-9.0 - observer_discriminant.sqrt()) / 2.0,
    );
    assert_eq!(observer_poles, (-4.0, -5.0));
    assert_eq!(
        (1.0 + 0.1 * observer_poles.0, 1.0 + 0.1 * observer_poles.1),
        (0.6, 0.5)
    );

    let bearing_baseline = run_bearing("simulation.toml", 0, 0.0);
    let bearing_feedback = run_bearing("simulation.toml", 1, 0.0);
    let bearing_reference = run_bearing("simulation-bearing-reference.toml", 1, 0.1);
    let motor_baseline = run_motor("simulation-motor-nominal.toml", false);
    let motor_observer = run_motor("simulation-motor-nominal.toml", true);
    let motor_mismatch = run_motor("simulation-motor-damping-mismatch.toml", true);

    for value in bearing_baseline
        .displacement
        .iter()
        .chain(&bearing_baseline.velocity)
        .chain(&bearing_baseline.requested)
        .chain(&bearing_baseline.applied)
        .chain(&bearing_feedback.displacement)
        .chain(&bearing_feedback.velocity)
        .chain(&bearing_feedback.requested)
        .chain(&bearing_feedback.applied)
        .chain(&bearing_reference.displacement)
        .chain(&bearing_reference.velocity)
        .chain(&bearing_reference.requested)
        .chain(&bearing_reference.applied)
    {
        assert!(value.is_finite());
    }
    for (name, trace) in [
        ("position-only", &bearing_baseline),
        ("state-feedback", &bearing_feedback),
        ("reference", &bearing_reference),
    ] {
        assert!(
            trace.displacement.iter().all(|value| value.abs() <= 0.5),
            "{name} displacement peak={:.6} mm",
            max_abs(&trace.displacement)
        );
        assert!(
            trace.velocity.iter().all(|value| value.abs() <= 2.0),
            "{name} velocity peak={:.6} mm/s",
            max_abs(&trace.velocity)
        );
        assert!(
            trace.applied.iter().all(|value| value.abs() <= 2.0),
            "{name} current peak={:.6} A",
            max_abs(&trace.applied)
        );
    }
    assert!(first_settled_sample(
        &bearing_baseline.displacement,
        &bearing_baseline.velocity,
        0.002,
        0.005,
        20,
    )
    .is_none());
    let feedback_settling = first_settled_sample(
        &bearing_feedback.displacement,
        &bearing_feedback.velocity,
        0.002,
        0.005,
        20,
    )
    .expect("measured-state bearing feedback settles") as f64
        * 0.1;
    let reference_error: Vec<f64> = bearing_reference
        .displacement
        .iter()
        .map(|value| value - 0.1)
        .collect();
    let reference_settling = first_settled_sample(
        &reference_error,
        &bearing_reference.velocity,
        0.002,
        0.005,
        20,
    )
    .expect("bearing reference response settles") as f64
        * 0.1;
    assert!(feedback_settling <= 4.0);
    assert!(reference_settling <= 4.0);
    let bearing_final_error = mean_abs(bearing_feedback.displacement[100..].iter().copied(), 20);
    let reference_final_error = mean_abs(reference_error[100..].iter().copied(), 20);
    assert!(bearing_final_error <= 0.002);
    assert!(reference_final_error <= 0.002);
    assert!(
        bearing_feedback
            .applied
            .iter()
            .filter(|value| value.abs() >= 1.999)
            .count()
            <= 2
    );
    assert!(
        bearing_reference
            .applied
            .iter()
            .filter(|value| value.abs() >= 1.999)
            .count()
            <= 2
    );

    for trace in [&motor_baseline, &motor_observer, &motor_mismatch] {
        assert!(trace
            .speed
            .iter()
            .chain(&trace.load_truth)
            .chain(&trace.estimated_speed)
            .chain(&trace.estimated_load)
            .chain(&trace.innovation)
            .chain(&trace.requested)
            .chain(&trace.applied)
            .all(|value| value.is_finite()));
        assert!(trace.speed.iter().all(|value| value.abs() <= 2.0));
        assert!(trace.applied.iter().all(|value| value.abs() <= 3.0));
    }
    let final_window = 150..200;
    let motor_speed_error = |trace: &MotorTrace| {
        mean_abs(
            final_window.clone().map(|index| 1.0 - trace.speed[index]),
            50,
        )
    };
    let motor_load_error = |trace: &MotorTrace| {
        mean_abs(
            final_window
                .clone()
                .map(|index| trace.estimated_load[index] - trace.load_truth[index]),
            50,
        )
    };
    let baseline_speed_error = motor_speed_error(&motor_baseline);
    let observer_speed_error = motor_speed_error(&motor_observer);
    let observer_load_error = motor_load_error(&motor_observer);
    let mismatch_speed_error = motor_speed_error(&motor_mismatch);
    let mismatch_load_error = motor_load_error(&motor_mismatch);
    assert!(baseline_speed_error >= 0.10);
    assert!(observer_speed_error <= 0.02);
    assert!(observer_load_error <= 0.02);
    assert!(mismatch_speed_error <= 0.02);
    assert!(mismatch_load_error >= 0.10);
    let speed_estimation_error: Vec<f64> = motor_observer
        .estimated_speed
        .iter()
        .zip(&motor_observer.speed)
        .map(|(estimate, truth)| estimate - truth)
        .collect();
    let load_estimation_error: Vec<f64> = motor_observer
        .estimated_load
        .iter()
        .zip(&motor_observer.load_truth)
        .map(|(estimate, truth)| estimate - truth)
        .collect();
    let observer_settling = first_settled_sample(
        &speed_estimation_error,
        &load_estimation_error,
        0.01,
        0.02,
        20,
    )
    .expect("nominal observer estimates settle") as f64
        * 0.1;
    assert!(observer_settling <= 3.0);
    assert!(
        motor_observer
            .applied
            .iter()
            .filter(|value| value.abs() >= 2.999)
            .count()
            <= 2
    );

    eprintln!(
        "state feedback/observer: poles position-only=+/-j1, feedback={:.1}/{:.1}, observer={:.1}/{:.1}, bearing settle regulation/reference={feedback_settling:.2}/{reference_settling:.2}s, final errors={bearing_final_error:.5}/{reference_final_error:.5}mm, bearing peaks position/current={:.4}mm/{:.4}A, current travel={:.4}A, motor baseline/observer/mismatch speed errors={baseline_speed_error:.4}/{observer_speed_error:.4}/{mismatch_speed_error:.4}rad/s, nominal/mismatch load errors={observer_load_error:.4}/{mismatch_load_error:.4}Nm, observer settle={observer_settling:.2}s, innovation peak={:.4}rad/s, torque peak/travel={:.4}/{:.4}Nm",
        feedback_poles.0,
        feedback_poles.1,
        observer_poles.0,
        observer_poles.1,
        max_abs(&bearing_feedback.displacement),
        max_abs(&bearing_feedback.applied),
        total_variation(&bearing_feedback.applied),
        max_abs(&motor_observer.innovation),
        max_abs(&motor_observer.applied),
        total_variation(&motor_observer.applied),
    );

    let bearing_time = (0..bearing_feedback.displacement.len())
        .map(|sample| sample as f64 * 0.1)
        .collect::<Vec<_>>();
    let motor_time = (0..motor_observer.speed.len())
        .map(|sample| sample as f64 * 0.1)
        .collect::<Vec<_>>();
    let (bearing_min, bearing_max) = plot_bounds([
        bearing_baseline.displacement.as_slice(),
        bearing_feedback.displacement.as_slice(),
        bearing_reference.displacement.as_slice(),
    ]);
    let (motor_min, motor_max) = plot_bounds([
        motor_baseline.speed.as_slice(),
        motor_observer.speed.as_slice(),
        motor_mismatch.speed.as_slice(),
    ]);
    let summary = format!(
        "case,status,settling_s,final_error,peak_applied,applied_travel,load_estimation_error\nbearing_position_only,not_settled,,,{:.6},{:.6},\nbearing_state_feedback,qualified,{:.6},{:.6},{:.6},{:.6},\nbearing_reference,qualified,{:.6},{:.6},{:.6},{:.6},\nmotor_without_observer,rejected,,{:.6},{:.6},{:.6},\nmotor_nominal_observer,qualified,{:.6},{:.6},{:.6},{:.6},{:.6}\nmotor_damping_mismatch,model_mismatch_visible,,{:.6},{:.6},{:.6},{:.6}\n",
        max_abs(&bearing_baseline.applied),
        total_variation(&bearing_baseline.applied),
        feedback_settling,
        bearing_final_error,
        max_abs(&bearing_feedback.applied),
        total_variation(&bearing_feedback.applied),
        reference_settling,
        reference_final_error,
        max_abs(&bearing_reference.applied),
        total_variation(&bearing_reference.applied),
        baseline_speed_error,
        max_abs(&motor_baseline.applied),
        total_variation(&motor_baseline.applied),
        observer_settling,
        observer_speed_error,
        max_abs(&motor_observer.applied),
        total_variation(&motor_observer.applied),
        observer_load_error,
        mismatch_speed_error,
        max_abs(&motor_mismatch.applied),
        total_variation(&motor_mismatch.applied),
        mismatch_load_error,
    );
    evidence::retain(
        &evidence_root(),
        "magnetic-bearing-state-feedback-evidence",
        "State feedback stabilizes; observation remains model-qualified",
        "Measured velocity stabilizes the bearing, while motor-load estimation exposes the declared damping mismatch.",
        &[
            evidence::Panel {
                title: "Magnetic-bearing displacement",
                x_unit: "time (s)",
                y_unit: "displacement (mm)",
                x_minimum: 0.0,
                x_maximum: *bearing_time.last().unwrap(),
                y_minimum: bearing_min,
                y_maximum: bearing_max,
                series: vec![
                    evidence::Series { name: "Position only", color: "#dc2626", dashed: false, x: &bearing_time, y: &bearing_baseline.displacement },
                    evidence::Series { name: "State feedback", color: "#2563eb", dashed: false, x: &bearing_time, y: &bearing_feedback.displacement },
                    evidence::Series { name: "0.1 mm reference", color: "#059669", dashed: false, x: &bearing_time, y: &bearing_reference.displacement },
                ],
            },
            evidence::Panel {
                title: "Loaded-motor speed",
                x_unit: "time (s)",
                y_unit: "speed (rad/s)",
                x_minimum: 0.0,
                x_maximum: *motor_time.last().unwrap(),
                y_minimum: motor_min,
                y_maximum: motor_max,
                series: vec![
                    evidence::Series { name: "No observer", color: "#6b7280", dashed: false, x: &motor_time, y: &motor_baseline.speed },
                    evidence::Series { name: "Nominal observer", color: "#2563eb", dashed: false, x: &motor_time, y: &motor_observer.speed },
                    evidence::Series { name: "Damping mismatch", color: "#b45309", dashed: true, x: &motor_time, y: &motor_mismatch.speed },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Bearing state", name: "State-feedback velocity", x: &bearing_time, y: &bearing_feedback.velocity, x_unit: "time (s)", y_unit: "velocity (mm/s)" },
            evidence::RetainedSeries { panel: "Bearing current", name: "Requested", x: &bearing_time, y: &bearing_feedback.requested, x_unit: "time (s)", y_unit: "current (A)" },
            evidence::RetainedSeries { panel: "Bearing current", name: "Applied", x: &bearing_time, y: &bearing_feedback.applied, x_unit: "time (s)", y_unit: "current (A)" },
            evidence::RetainedSeries { panel: "Motor estimate", name: "Measured speed", x: &motor_time, y: &motor_observer.speed, x_unit: "time (s)", y_unit: "speed (rad/s)" },
            evidence::RetainedSeries { panel: "Motor estimate", name: "Estimated speed", x: &motor_time, y: &motor_observer.estimated_speed, x_unit: "time (s)", y_unit: "speed (rad/s)" },
            evidence::RetainedSeries { panel: "Motor estimate", name: "Load truth", x: &motor_time, y: &motor_observer.load_truth, x_unit: "time (s)", y_unit: "torque (N m)" },
            evidence::RetainedSeries { panel: "Motor estimate", name: "Estimated load", x: &motor_time, y: &motor_observer.estimated_load, x_unit: "time (s)", y_unit: "torque (N m)" },
            evidence::RetainedSeries { panel: "Motor estimate", name: "Innovation", x: &motor_time, y: &motor_observer.innovation, x_unit: "time (s)", y_unit: "speed (rad/s)" },
            evidence::RetainedSeries { panel: "Motor torque", name: "Requested", x: &motor_time, y: &motor_observer.requested, x_unit: "time (s)", y_unit: "torque (N m)" },
            evidence::RetainedSeries { panel: "Motor torque", name: "Applied", x: &motor_time, y: &motor_observer.applied, x_unit: "time (s)", y_unit: "torque (N m)" },
            evidence::RetainedSeries { panel: "Mismatch estimate", name: "Estimated load", x: &motor_time, y: &motor_mismatch.estimated_load, x_unit: "time (s)", y_unit: "torque (N m)" },
        ],
        &summary,
    );
}
