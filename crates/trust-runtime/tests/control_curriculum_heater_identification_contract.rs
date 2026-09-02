use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/16-heater-step-identification/src/Main.st");
const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const SAMPLE_TIME_S: f64 = 0.1;
const SEED: u64 = 20260827;

#[derive(Clone, Copy)]
struct Sample {
    index: usize,
    time_s: f64,
    event: &'static str,
    requested_power_percent: f64,
    applied_power_percent: f64,
    pv_degc: f64,
    inputs_valid: bool,
    auto_mode: bool,
}

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 16 program before configuration")
        .replace(
            "    ExperimentInput : LREAL :=",
            "    ExperimentInput AT %ML32 : LREAL :=",
        )
        .replace("    Setpoint : LREAL :=", "    Setpoint AT %ML40 : LREAL :=")
        .replace(
            "    ProcessValue : LREAL :=",
            "    ProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "    ManipulatedValue : LREAL :=",
            "    ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %MX81.0 : BOOL :=",
        )
        .replace(
            "    SafetyAbort : BOOL :=",
            "    SafetyAbort AT %MX81.1 : BOOL :=",
        )
        .replace(
            "    InputsValid : BOOL :=",
            "    InputsValid AT %MX81.2 : BOOL :=",
        )
        .replace("    AutoMode : BOOL :=", "    AutoMode AT %MX81.3 : BOOL :=")
}

#[test]
fn lesson_16_establishes_its_declared_manual_operating_point() {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 16 heater state");
    runtime
        .execute_cycle()
        .expect("execute Lesson 16 initialization scan");

    assert!((read_lreal(&runtime, "%ML32") - 20.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, "%ML40") - 30.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, "%IL0") - 30.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, "%QL0") - 20.0).abs() <= 1e-12);
    assert!(!read_bool(&runtime, "%MX81.3"));
}

fn runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("heater-step-identification/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 16 Structured Text")
}

fn simulation() -> SimulationController {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/16-heater-step-identification/simulation.toml");
    SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load shipped Lesson 16 simulation: {error}")),
    )
}

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).unwrap_or_else(|error| panic!("parse {text}: {error}"))
}

fn write_lreal(runtime: &mut Runtime, text: &str, value: f64) {
    runtime
        .io_mut()
        .write(&address(text), Value::LWord(value.to_bits()))
        .unwrap_or_else(|error| panic!("write {text}: {error}"));
}

fn write_bool(runtime: &mut Runtime, text: &str, value: bool) {
    runtime
        .io_mut()
        .write(&address(text), Value::Bool(value))
        .unwrap_or_else(|error| panic!("write {text}: {error}"));
}

fn read_lreal(runtime: &Runtime, text: &str) -> f64 {
    match runtime
        .io()
        .read(&address(text))
        .unwrap_or_else(|error| panic!("read {text}: {error}"))
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL at {text}, got {other:?}"),
    }
}

fn read_bool(runtime: &Runtime, text: &str) -> bool {
    match runtime
        .io()
        .read(&address(text))
        .unwrap_or_else(|error| panic!("read {text}: {error}"))
    {
        Value::Bool(value) => value,
        other => panic!("expected BOOL at {text}, got {other:?}"),
    }
}

fn capture(index: usize, time_s: f64, event: &'static str, runtime: &Runtime) -> Sample {
    Sample {
        index,
        time_s,
        event,
        requested_power_percent: read_lreal(runtime, "%ML32"),
        applied_power_percent: read_lreal(runtime, "%QL0"),
        pv_degc: read_lreal(runtime, "%IL0"),
        inputs_valid: read_bool(runtime, "%MX81.2"),
        auto_mode: read_bool(runtime, "%MX81.3"),
    }
}

#[test]
fn lesson_16_boolean_test_authority_runs_the_project_owned_step_sequence() {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 16 heater state");

    write_lreal(&mut runtime, "%ML32", 77.0);
    write_bool(&mut runtime, "%MX81.0", true);
    for index in 1..=101 {
        runtime.execute_cycle().expect("execute Lesson 16 Test scan");
        if index == 1 || index == 100 {
            assert!((read_lreal(&runtime, "%ML32") - 20.0).abs() <= 1e-12);
        }
    }
    assert!((read_lreal(&runtime, "%ML32") - 30.0).abs() <= 1e-12);

    write_bool(&mut runtime, "%MX81.0", false);
    runtime
        .execute_cycle()
        .expect("execute Lesson 16 authority restoration");
    assert!((read_lreal(&runtime, "%ML32") - 20.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, "%QL0") - 20.0).abs() <= 1e-12);
}

fn run() -> Vec<Sample> {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 16 heater state");
    let mut samples = Vec::with_capacity(501);
    for index in 1..=500 {
        write_bool(&mut runtime, "%MX81.0", true);
        runtime.execute_cycle().expect("execute Lesson 16 program");
        let now = Duration::from_millis((index * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 16 heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 16 heater PV");
        samples.push(capture(
            index,
            index as f64 * SAMPLE_TIME_S,
            if index <= 100 {
                "baseline"
            } else {
                "identification_step"
            },
            &runtime,
        ));
    }

    write_bool(&mut runtime, "%MX81.0", false);
    runtime
        .execute_cycle()
        .expect("execute Lesson 16 Manual restoration");
    samples.push(capture(501, 50.1, "manual_restoration", &runtime));
    samples
}

fn artifact_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/16-heater-step-identification/figures")
        .join(name)
}

fn write_or_compare(path: &Path, expected: &str) {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("artifact parent"))
            .expect("create Lesson 16 artifact directory");
        std::fs::write(path, expected).expect("write Lesson 16 artifact");
        return;
    }
    let actual = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read retained artifact {}: {error}", path.display()));
    assert_eq!(
        actual,
        expected,
        "retained artifact drift: {}",
        path.display()
    );
}

#[test]
fn lesson_16_retains_qualified_runtime_recording_and_reaction_curve() {
    let samples = run();
    let active = &samples[..500];
    let baseline_window = &active[80..100];
    let final_window = &active[450..500];
    let baseline = baseline_window
        .iter()
        .map(|sample| sample.pv_degc)
        .sum::<f64>()
        / baseline_window.len() as f64;
    let final_pv = final_window
        .iter()
        .map(|sample| sample.pv_degc)
        .sum::<f64>()
        / final_window.len() as f64;
    let delta_u = 10.0;
    let delta_y = final_pv - baseline;
    let gain = delta_y / delta_u;
    let after_step = &active[100..];
    let delay_index = after_step
        .iter()
        .position(|sample| sample.pv_degc >= baseline + 0.02 * delta_y)
        .expect("2 percent response crossing");
    let response_start_s = delay_index as f64 * SAMPLE_TIME_S;
    let level_63 = baseline + 0.6321205588 * delta_y;
    let crossing_63_index = after_step
        .iter()
        .position(|sample| sample.pv_degc >= level_63)
        .expect("63.2 percent response crossing");
    let crossing_63_s = crossing_63_index as f64 * SAMPLE_TIME_S;
    let time_constant_s = crossing_63_s - response_start_s;
    let baseline_drift = (baseline_window.last().unwrap().pv_degc
        - baseline_window.first().unwrap().pv_degc)
        / (baseline_window.last().unwrap().time_s - baseline_window.first().unwrap().time_s);
    let input_span = active
        .iter()
        .map(|sample| sample.applied_power_percent)
        .fold(f64::NEG_INFINITY, f64::max)
        - active
            .iter()
            .map(|sample| sample.applied_power_percent)
            .fold(f64::INFINITY, f64::min);
    let output_span = active
        .iter()
        .map(|sample| sample.pv_degc)
        .fold(f64::NEG_INFINITY, f64::max)
        - active
            .iter()
            .map(|sample| sample.pv_degc)
            .fold(f64::INFINITY, f64::min);
    let restoration = samples.last().expect("Manual restoration sample");

    assert_eq!(active.len(), 500);
    assert!(active.iter().all(|sample| sample.pv_degc.is_finite()
        && sample.requested_power_percent.is_finite()
        && sample.applied_power_percent.is_finite()
        && sample.inputs_valid
        && !sample.auto_mode));
    assert!(
        baseline_drift.abs() <= 0.001,
        "baseline drift={baseline_drift}"
    );
    assert!(input_span >= 9.99, "applied input span={input_span}");
    assert!(output_span >= 1.90, "output span={output_span}");
    assert!((baseline - 30.0).abs() <= 1e-9, "baseline PV={baseline}");
    assert!((gain - 0.2).abs() <= 0.005, "estimated gain={gain}");
    assert!(
        (response_start_s - 1.0).abs() <= 0.2,
        "estimated delay={response_start_s}"
    );
    assert!(
        (time_constant_s - 5.0).abs() <= 0.3,
        "estimated time constant={time_constant_s}"
    );
    assert_eq!(restoration.event, "manual_restoration");
    assert!(!restoration.auto_mode && restoration.inputs_valid);
    assert!((restoration.requested_power_percent - 20.0).abs() <= 1e-12);
    assert!((restoration.applied_power_percent - 20.0).abs() <= 1e-12);

    let mut trace = String::from(
        "run_id,sample_index,time_s,event,requested_power_percent,applied_power_percent,pv_degc,quality,clipped,saturated,inputs_valid,manual_mode,sample_time_s,seed,evidence_class,plant_id\n",
    );
    for sample in &samples {
        writeln!(
            trace,
            "lesson-16,{},{:.12},{},{:.12},{:.12},{:.12},good,false,false,{},{},0.100000000000,{SEED},simulation,identification-water-heater",
            sample.index,
            sample.time_s,
            sample.event,
            sample.requested_power_percent,
            sample.applied_power_percent,
            sample.pv_degc,
            sample.inputs_valid,
            !sample.auto_mode,
        )
        .expect("format Lesson 16 trace");
    }

    let summary = format!(
        "run_id,active_sample_count,restoration_time_s,median_sample_time_s,maximum_interval_s,gap_count,rejected_quality_count,clipped_count,saturated_count,baseline_window_start_s,baseline_window_end_s,baseline_degc,baseline_drift_degc_per_s,input_span_percentage_points,output_span_degc,delta_u_percentage_points,delta_y_degc,gain_degc_per_percent,delay_lower_s,delay_upper_s,estimated_delay_s,level_63_degc,crossing_63_lower_s,crossing_63_upper_s,estimated_time_constant_s,restoration_requested_percent,restoration_applied_percent,restoration_manual_mode,qualification,model_state,simulation_provenance,identified_model_provenance\nlesson-16,500,50.100000000000,0.100000000000,0.100000000000,0,0,0,0,8.100000000000,10.000000000000,{baseline:.12},{baseline_drift:.12},{input_span:.12},{output_span:.12},{delta_u:.12},{delta_y:.12},{gain:.12},{:.12},{response_start_s:.12},{response_start_s:.12},{level_63:.12},{:.12},{crossing_63_s:.12},{time_constant_s:.12},{:.12},{:.12},{},Qualified,validation_required,configured_simulation_measurement,reaction_curve_identification\n",
        (response_start_s - SAMPLE_TIME_S).max(0.0),
        (crossing_63_s - SAMPLE_TIME_S).max(0.0),
        restoration.requested_power_percent,
        restoration.applied_power_percent,
        !restoration.auto_mode,
    );

    write_or_compare(&artifact_path("heater-identification-trace.csv"), &trace);
    write_or_compare(
        &artifact_path("heater-identification-run-summary.csv"),
        &summary,
    );
}
