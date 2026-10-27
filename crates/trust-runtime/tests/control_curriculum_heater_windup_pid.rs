//! Approved lessons 7/8: preserve the hot experiment while reusing Control.PID.
use std::io::Write;
use std::path::{Path, PathBuf};
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::{compile, exists, type_name, value, LessonRuntime};

const LESSONS: [(&str, &str, f64); 2] = [
    ("06-water-heater-saturation-windup", "%MX96.0", 200.0),
    ("07-water-heater-anti-windup", "%MX112.0", 100.0),
];

fn lesson(folder: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control")
        .join(folder)
}

fn setup(folder: &str) -> (CompiledRuntime, SimulationController) {
    let project = lesson(folder);
    let session = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "src/Main.st",
            std::fs::read_to_string(project.join("src/Main.st")).unwrap(),
        ),
        SourceFile::with_path(
            "Control/control.st",
            std::fs::read_to_string(project.join("libraries/control/src/control.st")).unwrap(),
        ),
    ]);
    let runtime = compile(session);
    (
        runtime,
        SimulationController::new(SimulationConfig::load(project.join("simulation.toml")).unwrap()),
    )
}

fn number(runtime: &CompiledRuntime, address: &str) -> f64 {
    match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        Value::LReal(value) => value,
        _ => panic!("expected LREAL at {address}"),
    }
}

fn variable(runtime: &CompiledRuntime, name: &str) -> Value {
    assert!(exists(runtime, "P1"), "P1 program");
    value(runtime, &format!("P1.{name}")).unwrap_or_else(|| panic!("missing {name}"))
}

fn near(actual: f64, expected: f64, context: &str) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "{context}: expected {expected:.12}, got {actual:.12}"
    );
}

// Values are observed from the delivered Main and actual plant. No second PI
// calculation or simulated process is used as the trajectory oracle.
fn hot_trial(folder: &str, activation: &str, preparation: i64) -> Vec<[f64; 5]> {
    let (mut runtime, mut simulation) = setup(folder);
    let debug = runtime.enable_debug();
    let mut samples = Vec::new();
    for scan in 0..preparation + 702 {
        let elapsed = scan - preparation;
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse(activation).unwrap(),
            Value::Bool((0..700).contains(&elapsed)),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        let pv = number(&runtime, "%IL0");
        let requested = number(&runtime, "%ML8");
        if elapsed < 0 {
            near(pv, 35.0, "hot preparation PV");
            near(number(&runtime, "%QL0"), 75.0, "hot preparation command");
        } else if elapsed < 700 {
            near(
                number(&runtime, "%ML0"),
                if elapsed < 100 { 45.0 } else { 35.0 },
                "setpoint timing",
            );
            if elapsed == 0 {
                near(requested, 95.5, "first hot PI request");
            }
        } else {
            near(number(&runtime, "%QL0"), 0.0, "terminal command");
        }
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
        let applied = simulation.status().plants[0].applied_input[0];
        assert!(
            (0.0..=100.0).contains(&applied),
            "actual heater input is bounded"
        );
        if (0..700).contains(&elapsed) {
            samples.push([
                pv,
                requested,
                number(&runtime, "%ML64"),
                number(&runtime, "%ML80"),
                applied,
            ]);
        } else if elapsed >= 700 {
            near(applied, 0.0, "terminal physical input");
        }
    }
    samples
}

fn reference(folder: &str, samples: &[[f64; 5]]) -> Vec<[f64; 5]> {
    let path = lesson(folder).join("tests/fixtures/hot-response-reference.csv");
    if std::env::var_os("TRUST_HEATER_CAPTURE_BASELINE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // The baseline is deliberately write-once: a later run cannot silently
        // replace the accepted pre-conversion physical response.
        if !path.exists() {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            writeln!(file, "# Pre-conversion delivered Main; 100 ms samples; PV,current request,retained I,correction,physical input after cycle").unwrap();
            for row in samples {
                writeln!(
                    file,
                    "{:.17},{:.17},{:.17},{:.17},{:.17}",
                    row[0], row[1], row[2], row[3], row[4]
                )
                .unwrap();
            }
        }
    }
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let values: Vec<f64> = line
                .split(',')
                .map(|value| value.parse().unwrap())
                .collect();
            values.try_into().unwrap()
        })
        .collect()
}

#[test]
fn hot_physical_behavior_matches_the_original_delivered_experiment() {
    for (folder, activation, _) in LESSONS {
        for preparation in [0, 100] {
            let actual = hot_trial(folder, activation, preparation);
            let expected = reference(folder, &actual);
            assert_eq!(actual.len(), 700);
            assert_eq!(expected.len(), actual.len());
            for (scan, (row, reference)) in actual.iter().zip(&expected).enumerate() {
                for (column, (actual, expected)) in row.iter().zip(reference).enumerate() {
                    near(
                        *actual,
                        *expected,
                        &format!("{folder} preparation {preparation} scan {scan} column {column}"),
                    );
                }
            }
            assert!(actual.iter().any(|row| row[1] > 100.0));
            let leaves_limit = actual
                .iter()
                .enumerate()
                .skip(100)
                .find(|(_, row)| row[4] < 100.0)
                .map(|(scan, _)| scan);
            let settling = (100..700).find(|start| {
                actual[*start..]
                    .iter()
                    .all(|row| (row[0] - 35.0).abs() <= 0.2)
            });
            let peak = actual
                .iter()
                .map(|row| row[0])
                .fold(f64::NEG_INFINITY, f64::max);
            eprintln!("{folder} preparation={preparation}: I_before_return={:.9}, exit_limit_scan={leaves_limit:?}, peak_PV={peak:.9}, settled_scan={settling:?}", actual[99][2]);
        }
    }
}

fn assert_actual_pid_ownership(folder: &str, activation: &str, maximum: f64) {
    let (mut runtime, mut simulation) = setup(folder);
    let debug = runtime.enable_debug();
    let mut previous_request = 75.0;
    let mut previous_applied = 75.0;
    let mut saw_request_mismatch = false;
    for scan in 0..702 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse(activation).unwrap(),
            Value::Bool(scan < 700),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert!(exists(&runtime, "P1"), "P1 program");
        let controller = "P1.Controller";
        assert!(
            exists(&runtime, controller),
            "{folder}: the delivered Main must execute a real Control.PID instance"
        );
        assert_eq!(
            type_name(&runtime, controller).unwrap().as_str(),
            "Control.PID"
        );
        let Some(Value::Struct(state)) = value(&runtime, &format!("{controller}.StateValue"))
        else {
            panic!("PID retained state");
        };
        let Some(Value::LReal(output)) = state.field("SaturatedOutput") else {
            panic!("actual limited PID output");
        };
        let Some(Value::LReal(nominal)) = state.field("NominalOutput") else {
            panic!("actual nominal PID state");
        };
        near(
            number(&runtime, "%QL0"),
            *output,
            "actual Update return owns ordinary output",
        );
        near(
            number(&runtime, "%IL8"),
            previous_applied,
            "applied feedback belongs to the completed prior step",
        );
        near(
            number(&runtime, "%ML88"),
            previous_request,
            "paired request history",
        );
        near(
            number(&runtime, "%ML16"),
            previous_request - previous_applied,
            "phase-aligned physical mismatch",
        );
        assert_eq!(
            state.field("ProcessValue"),
            Some(&Value::LReal(number(&runtime, "%IL0")))
        );
        let Value::Struct(parameters) = variable(&runtime, "Parameters") else {
            panic!("PID parameters");
        };
        assert_eq!(
            parameters.field("OutputMaximum"),
            Some(&Value::LReal(maximum))
        );
        assert_eq!(parameters.field("IntegralGain"), Some(&Value::LReal(0.5)));
        assert_eq!(parameters.field("DerivativeGain"), Some(&Value::LReal(0.0)));
        assert_eq!(parameters.field("TrackingTime"), Some(&Value::LReal(1.0)));
        if scan < 700 {
            near(
                number(&runtime, "%ML64"),
                *nominal - number(&runtime, "%ML48"),
                "equivalent I comes only from actual nominal state",
            );
            if maximum == 200.0 {
                assert!(
                    *nominal > 0.0 && *nominal < maximum,
                    "lesson 7's deliberately wrong internal limits must never bind"
                );
                near(
                    *output,
                    number(&runtime, "%ML8"),
                    "unlimited real PID request",
                );
                near(
                    number(&runtime, "%ML80"),
                    0.0,
                    "no correction for an unseen external limit",
                );
            } else {
                near(
                    number(&runtime, "%ML80"),
                    0.1 * (*output - number(&runtime, "%ML8")),
                    "real back-calculation correction",
                );
            }
        }
        previous_request = number(&runtime, "%ML8");
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
        previous_applied = simulation.status().plants[0].applied_input[0];
        saw_request_mismatch |= previous_request > previous_applied + 1e-8;
    }
    assert!(
        saw_request_mismatch,
        "the infeasible target must expose unavailable demand"
    );
}

#[test]
fn lesson7_actual_pid_owns_output_and_pairs_applied_feedback() {
    let (folder, activation, maximum) = LESSONS[0];
    assert_actual_pid_ownership(folder, activation, maximum);
}

#[test]
fn lesson8_actual_pid_owns_output_and_pairs_applied_feedback() {
    let (folder, activation, maximum) = LESSONS[1];
    assert_actual_pid_ownership(folder, activation, maximum);
}

#[test]
fn abort_removes_command_immediately_and_applied_feedback_on_next_publication() {
    for (folder, activation, _) in LESSONS {
        let (mut runtime, mut simulation) = setup(folder);
        let debug = runtime.enable_debug();
        for scan in 0..20 {
            simulation
                .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
                .unwrap();
            debug.enqueue_io_write(IoAddress::parse(activation).unwrap(), Value::Bool(true));
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            simulation
                .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
                .unwrap();
        }
        let previous_applied = simulation.status().plants[0].applied_input[0];
        assert!(previous_applied > 75.0);
        simulation
            .apply_pre_cycle(Duration::from_millis(2000), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord(90.0_f64.to_bits()),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(variable(&runtime, "SafetyAbort"), Value::Bool(true));
        near(
            number(&runtime, "%QL0"),
            0.0,
            "abort zero command in the current PLC cycle",
        );
        near(
            number(&runtime, "%IL8"),
            previous_applied,
            "abort cannot invent same-scan measured zero",
        );
        simulation
            .apply_post_cycle(Duration::from_millis(2100), &runtime)
            .unwrap();
        near(
            simulation.status().plants[0].applied_input[0],
            0.0,
            "zero reaches the actuator after the PLC cycle",
        );
        near(
            number(&runtime, "%IL8"),
            previous_applied,
            "applied input remains previous until publication",
        );
        simulation
            .apply_pre_cycle(Duration::from_millis(2100), &mut runtime)
            .unwrap();
        near(
            number(&runtime, "%IL8"),
            0.0,
            "next completed input publication reports zero",
        );
        assert!(
            number(&runtime, "%IL0") > 35.0,
            "removing command does not erase stored/delayed heat"
        );
    }
}
