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
        .join("../../examples/control/11-state-space-estimation-and-predictive-control/64-two-tank-observability/figures")
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
    let padding = ((maximum - minimum) * 0.08).max(0.05);
    (minimum - padding, maximum + padding)
}

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/11-state-space-estimation-and-predictive-control/64-two-tank-observability")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 64 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 64 file {}: {error}", path.display()))
}

fn temporary_config_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-two-tank-feasibility-{label}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL lesson input");
}

fn write_int(runtime: &mut Runtime, address: &IoAddress, value: i16) {
    runtime
        .io_mut()
        .write(address, Value::Word(value as u16))
        .expect("write INT lesson selector");
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime
        .io()
        .read(address)
        .expect("read LREAL lesson signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

#[derive(Default)]
struct Trace {
    tank1: Vec<f64>,
    tank2: Vec<f64>,
    selected: Vec<f64>,
    upstream_flow: Vec<f64>,
    bypass_flow: Vec<f64>,
}

fn run(model_file: &str, actuator_layout: i16, sensor_layout: i16, step: bool) -> Trace {
    let source = project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("two-tank program before configuration")
        .replace("ActuatorLayout : INT :=", "ActuatorLayout AT %MW0 : INT :=")
        .replace("SensorLayout : INT :=", "SensorLayout AT %MW2 : INT :=")
        .replace(
            "FlowRequested : LREAL :=",
            "FlowRequested AT %ML8 : LREAL :=",
        )
        .replace(
            "SelectedMeasurement : LREAL :=",
            "SelectedMeasurement AT %ML16 : LREAL :=",
        )
        .replace(
            "UpstreamPumpApplied : LREAL :=",
            "UpstreamPumpApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "BypassPumpApplied : LREAL :=",
            "BypassPumpApplied AT %QL8 : LREAL :=",
        )
        .replace("Tank1Truth : LREAL :=", "Tank1Truth AT %IL0 : LREAL :=")
        .replace("Tank2Truth : LREAL :=", "Tank2Truth AT %IL8 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "two-tank-observability/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped two-tank feasibility lesson");
    let path = temporary_config_path(model_file);
    std::fs::write(&path, project_file(model_file)).expect("write two-tank simulation config");
    let config = SimulationConfig::load(&path).expect("load two-tank simulation config");
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial tank truth");
    let _ = std::fs::remove_file(path);

    let actuator = IoAddress::parse("%MW0").unwrap();
    let sensor = IoAddress::parse("%MW2").unwrap();
    let request = IoAddress::parse("%ML8").unwrap();
    let selected = IoAddress::parse("%ML16").unwrap();
    let upstream = IoAddress::parse("%QL0").unwrap();
    let bypass = IoAddress::parse("%QL8").unwrap();
    let tank1 = IoAddress::parse("%IL0").unwrap();
    let tank2 = IoAddress::parse("%IL8").unwrap();
    let sample_count = if step { 3_000 } else { 1_800 };
    let mut trace = Trace::default();
    for cycle in 0..sample_count {
        let flow = if step && (200..1_200).contains(&cycle) {
            1.0
        } else {
            0.0
        };
        write_int(&mut runtime, &actuator, actuator_layout);
        write_int(&mut runtime, &sensor, sensor_layout);
        write_lreal(&mut runtime, &request, flow);
        runtime
            .execute_cycle()
            .expect("execute manual tank routing");
        // Capture the selected sensor with the same published truth sample that
        // the ST program used. The simulator advances only after this record.
        trace.tank1.push(read_lreal(&runtime, &tank1));
        trace.tank2.push(read_lreal(&runtime, &tank2));
        trace.selected.push(read_lreal(&runtime, &selected));
        trace.upstream_flow.push(read_lreal(&runtime, &upstream));
        trace.bypass_flow.push(read_lreal(&runtime, &bypass));
        let now = Duration::from_millis((cycle as i64 + 1) * 100);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance two-tank plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish two-tank truth");
    }
    trace
}

fn rank_2x2(matrix: [[f64; 2]; 2]) -> usize {
    let determinant = matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
    if determinant.abs() > 1e-12 {
        2
    } else if matrix.iter().flatten().any(|value| value.abs() > 1e-12) {
        1
    } else {
        0
    }
}

fn condition_2x2(matrix: [[f64; 2]; 2]) -> f64 {
    let trace = matrix
        .iter()
        .flatten()
        .map(|value| value * value)
        .sum::<f64>();
    let determinant = matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
    let difference = (trace * trace - 4.0 * determinant * determinant)
        .max(0.0)
        .sqrt();
    let maximum = ((trace + difference) / 2.0).sqrt();
    let minimum = ((trace - difference) / 2.0).sqrt();
    maximum / minimum
}

fn max_abs(values: &[f64]) -> f64 {
    values.iter().copied().map(f64::abs).fold(0.0, f64::max)
}

fn least_squares_slope(values: &[f64], sample_interval_seconds: f64) -> f64 {
    let count = values.len() as f64;
    let mean_time = sample_interval_seconds * (count - 1.0) / 2.0;
    let mean_value = values.iter().sum::<f64>() / count;
    let (covariance, time_variance) = values.iter().enumerate().fold(
        (0.0, 0.0),
        |(covariance, time_variance), (index, value)| {
            let centered_time = index as f64 * sample_interval_seconds - mean_time;
            (
                covariance + centered_time * (value - mean_value),
                time_variance + centered_time * centered_time,
            )
        },
    );
    covariance / time_variance
}

#[test]
fn control_curriculum_two_tank_feasibility_proves_reachable_and_hidden_states() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 64 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 64 must use schema 4");
    assert_eq!(lesson["sequence"], 64);
    assert_eq!(
        lesson["prerequisites"][0],
        "control-distillation-robustness"
    );
    assert_eq!(
        lesson["nextLesson"],
        "control-magnetic-bearing-state-feedback"
    );
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_two_tank_feasibility_proves_reachable_and_hidden_states"
    );
    let source = project_file("src/Main.st");
    assert!(!source.contains("Control.PID;"));
    for forbidden in [
        "ControllabilityRank",
        "ObservabilityRank",
        "Eigenvalue",
        "AcceptedLayout",
    ] {
        assert!(
            !source.contains(forbidden),
            "ST embeds analytical verdict '{forbidden}'"
        );
    }

    let controllable = [[0.20, -0.010], [0.0, 0.006]];
    let uncontrollable = [[0.0, 0.0], [0.20, -0.008]];
    let observable = [[0.0, 1.0], [0.03, -0.04]];
    let unobservable = [[1.0, 0.0], [-0.05, 0.0]];
    assert_eq!(rank_2x2(controllable), 2);
    assert_eq!(rank_2x2(uncontrollable), 1);
    assert_eq!(rank_2x2(observable), 2);
    assert_eq!(rank_2x2(unobservable), 1);
    assert!((controllable[0][0] * controllable[1][1] - 0.0012).abs() <= 1e-12);
    assert!(
        (observable[0][0] * observable[1][1] - observable[0][1] * observable[1][0] + 0.03).abs()
            <= 1e-12
    );
    assert!(condition_2x2(controllable) <= 50.0);
    assert!(condition_2x2(observable) <= 50.0);
    assert!([-0.05_f64, -0.04]
        .iter()
        .all(|eigenvalue| *eigenvalue < 0.0));

    let upstream_step = run("simulation.toml", 0, 0, true);
    let bypass_step = run("simulation.toml", 1, 0, true);
    let upstream_initial = run("simulation-initial-upstream.toml", 0, 0, false);
    let downstream_initial = run("simulation-initial-downstream.toml", 0, 1, false);
    for trace in [
        &upstream_step,
        &bypass_step,
        &upstream_initial,
        &downstream_initial,
    ] {
        assert!(trace
            .tank1
            .iter()
            .chain(&trace.tank2)
            .chain(&trace.selected)
            .chain(&trace.upstream_flow)
            .chain(&trace.bypass_flow)
            .all(|value| value.is_finite()));
        assert!(trace
            .tank1
            .iter()
            .chain(&trace.tank2)
            .all(|value| value.abs() <= 10.0));
        assert!(trace
            .upstream_flow
            .iter()
            .chain(&trace.bypass_flow)
            .all(|value| value.abs() <= 2.0));
    }

    let upstream_first_second_slope = least_squares_slope(&upstream_step.tank1[200..=210], 0.1);
    assert!((upstream_first_second_slope - 0.20).abs() <= 0.02);
    let tank1_first_move = upstream_step.tank1[200..]
        .iter()
        .position(|value| value.abs() >= 0.001)
        .expect("upstream tank responds");
    let tank2_first_move = upstream_step.tank2[200..]
        .iter()
        .position(|value| value.abs() >= 0.001)
        .expect("downstream tank responds through coupling");
    assert!(tank2_first_move > tank1_first_move);
    assert!(max_abs(&upstream_step.tank2[200..1_200]) >= 1.0);
    assert!(max_abs(&bypass_step.tank1) <= 0.001);
    assert!(max_abs(&bypass_step.tank2[200..1_200]) >= 1.0);
    let maximum_residual = |trace: &Trace, truth: &[f64]| {
        trace
            .selected
            .iter()
            .zip(truth)
            .map(|(selected, truth)| (selected - truth).abs())
            .fold(0.0, f64::max)
    };
    assert!(maximum_residual(&upstream_step, &upstream_step.tank2) <= 0.001);
    assert!(maximum_residual(&bypass_step, &bypass_step.tank2) <= 0.001);

    let downstream_response_to_upstream_initial = max_abs(&upstream_initial.selected);
    assert!(downstream_response_to_upstream_initial >= 0.10);
    assert!(maximum_residual(&upstream_initial, &upstream_initial.tank2) <= 0.001);
    assert!(downstream_initial.tank2[0] >= 4.9);
    assert!(max_abs(&downstream_initial.selected) <= 0.001);
    assert!(maximum_residual(&downstream_initial, &downstream_initial.tank1) <= 0.001);

    let restoration = |trace: &Trace| {
        let start = trace.tank1.len() - 100;
        (start..trace.tank1.len())
            .map(|index| trace.tank1[index].abs() + trace.tank2[index].abs())
            .sum::<f64>()
            / 100.0
    };
    let upstream_restoration = restoration(&upstream_step);
    let bypass_restoration = restoration(&bypass_step);
    assert!(upstream_restoration <= 0.02);
    assert!(bypass_restoration <= 0.02);
    eprintln!(
        "two-tank feasibility: rank Qc upstream/bypass=2/1, rank Qo downstream/upstream=2/1, conditions={:.3}/{:.3}, upstream slope={upstream_first_second_slope:.4} cm/s, first responses={:.1}/{:.1} s, peaks upstream-step={:.3}/{:.3} cm, bypass={:.3}/{:.3} cm, downstream sensor from upstream initial={downstream_response_to_upstream_initial:.3} cm, hidden downstream initial={:.3} cm, selected residual=0.000 cm, restoration={upstream_restoration:.4}/{bypass_restoration:.4} cm",
        condition_2x2(controllable),
        condition_2x2(observable),
        tank1_first_move as f64 * 0.1,
        tank2_first_move as f64 * 0.1,
        max_abs(&upstream_step.tank1[200..1_200]),
        max_abs(&upstream_step.tank2[200..1_200]),
        max_abs(&bypass_step.tank1),
        max_abs(&bypass_step.tank2[200..1_200]),
        downstream_initial.tank2[0],
    );

    let step_time = (0..upstream_step.tank1.len())
        .map(|sample| sample as f64 * 0.1)
        .collect::<Vec<_>>();
    let initial_time = (0..upstream_initial.tank1.len())
        .map(|sample| sample as f64 * 0.1)
        .collect::<Vec<_>>();
    let (step_min, step_max) = plot_bounds([
        upstream_step.tank1.as_slice(),
        upstream_step.tank2.as_slice(),
    ]);
    let (hidden_min, hidden_max) = plot_bounds([
        upstream_initial.tank2.as_slice(),
        upstream_initial.selected.as_slice(),
        downstream_initial.tank2.as_slice(),
        downstream_initial.selected.as_slice(),
    ]);
    let summary = format!(
        "layout,status,controllability_rank,observability_rank,condition_number,first_response_s,peak_tank1_cm,peak_tank2_cm,restoration_error_cm\nupstream_pump,qualified,2,,{:.6},{:.3},{:.6},{:.6},{:.6}\ndownstream_bypass,rejected_for_full_state_control,1,,,{:.3},{:.6},{:.6},{:.6}\ndownstream_sensor,qualified,,2,{:.6},,,,,\nupstream_sensor,rejected_for_full_state_observation,,1,,,,,\n",
        condition_2x2(controllable),
        tank1_first_move as f64 * 0.1,
        max_abs(&upstream_step.tank1[200..1_200]),
        max_abs(&upstream_step.tank2[200..1_200]),
        upstream_restoration,
        0.0,
        max_abs(&bypass_step.tank1),
        max_abs(&bypass_step.tank2[200..1_200]),
        bypass_restoration,
        condition_2x2(observable),
    );
    evidence::retain(
        &evidence_root(),
        "two-tank-feasibility-evidence",
        "Actuator and sensor placement determine reachable knowledge",
        "The upstream pump reaches both tanks, while an upstream-only sensor hides a downstream initial condition.",
        &[
            evidence::Panel {
                title: "Upstream-pump step",
                x_unit: "time (s)",
                y_unit: "level deviation (cm)",
                x_minimum: 0.0,
                x_maximum: *step_time.last().unwrap(),
                y_minimum: step_min,
                y_maximum: step_max,
                series: vec![
                    evidence::Series { name: "Tank 1", color: "#2563eb", dashed: false, x: &step_time, y: &upstream_step.tank1 },
                    evidence::Series { name: "Tank 2", color: "#059669", dashed: false, x: &step_time, y: &upstream_step.tank2 },
                ],
            },
            evidence::Panel {
                title: "Hidden downstream initial state",
                x_unit: "time (s)",
                y_unit: "level deviation (cm)",
                x_minimum: 0.0,
                x_maximum: *initial_time.last().unwrap(),
                y_minimum: hidden_min,
                y_maximum: hidden_max,
                series: vec![
                    evidence::Series { name: "Downstream truth", color: "#dc2626", dashed: false, x: &initial_time, y: &downstream_initial.tank2 },
                    evidence::Series { name: "Upstream sensor", color: "#6b7280", dashed: true, x: &initial_time, y: &downstream_initial.selected },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Upstream-pump step", name: "Applied upstream flow", x: &step_time, y: &upstream_step.upstream_flow, x_unit: "time (s)", y_unit: "flow (L/s)" },
            evidence::RetainedSeries { panel: "Bypass-pump step", name: "Tank 1", x: &step_time, y: &bypass_step.tank1, x_unit: "time (s)", y_unit: "level deviation (cm)" },
            evidence::RetainedSeries { panel: "Bypass-pump step", name: "Tank 2", x: &step_time, y: &bypass_step.tank2, x_unit: "time (s)", y_unit: "level deviation (cm)" },
            evidence::RetainedSeries { panel: "Bypass-pump step", name: "Applied bypass flow", x: &step_time, y: &bypass_step.bypass_flow, x_unit: "time (s)", y_unit: "flow (L/s)" },
            evidence::RetainedSeries { panel: "Observable initial state", name: "Downstream truth", x: &initial_time, y: &upstream_initial.tank2, x_unit: "time (s)", y_unit: "level deviation (cm)" },
            evidence::RetainedSeries { panel: "Observable initial state", name: "Downstream sensor", x: &initial_time, y: &upstream_initial.selected, x_unit: "time (s)", y_unit: "level deviation (cm)" },
        ],
        &summary,
    );
}
