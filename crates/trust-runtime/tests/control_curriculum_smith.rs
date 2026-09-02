use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const ADVANCED_CONTROL_SOURCE: &str =
    include_str!("../../../libraries/control/src/advanced_control.st");

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn evidence_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/59-heated-air-duct-smith/figures")
}

fn time_axis(samples: usize) -> Vec<f64> {
    (1..=samples).map(|sample| sample as f64 * 0.1).collect()
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
        .join("../../examples/control/09-industrial-control-structures/59-heated-air-duct-smith")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 59 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 59 file {}: {error}", path.display()))
}

fn temporary_config_path(case: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-smith-{case}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL input");
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL input");
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

fn read_bool(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL lesson signal") {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

#[derive(Debug)]
struct SmithRun {
    feedback_used: Vec<f64>,
    process_value: Vec<f64>,
    manipulated_value: Vec<f64>,
    predicted_value: Vec<f64>,
    correction: Vec<f64>,
    active: Vec<bool>,
    fallback: Vec<bool>,
    enabled: Vec<bool>,
    model_valid: Vec<bool>,
}

fn run_smith(
    case: &str,
    predictor_enabled: bool,
    aggressive_feedback: bool,
    plant_gain: f64,
    plant_time_constant: f64,
    plant_dead_time: f64,
) -> SmithRun {
    let mut source = project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Smith lesson program before configuration")
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX90.0 : BOOL :=")
        .replace(
            "PredictorEnabled : BOOL :=",
            "PredictorEnabled AT %MX90.1 : BOOL :=",
        )
        .replace(
            "PredictorActive : BOOL :=",
            "PredictorActive AT %MX90.2 : BOOL :=",
        )
        .replace(
            "PredictorFallback : BOOL :=",
            "PredictorFallback AT %MX90.3 : BOOL :=",
        )
        .replace("ModelValid : BOOL :=", "ModelValid AT %MX90.4 : BOOL :=")
        .replace("Setpoint : LREAL :=", "Setpoint AT %ML272 : LREAL :=")
        .replace(
            "PredictedProcessValue : LREAL :=",
            "PredictedProcessValue AT %ML280 : LREAL :=",
        )
        .replace(
            "DelayFreeModel : LREAL :=",
            "DelayFreeModel AT %ML288 : LREAL :=",
        )
        .replace(
            "DelayedModel : LREAL :=",
            "DelayedModel AT %ML296 : LREAL :=",
        )
        .replace(
            "ModelCorrection : LREAL :=",
            "ModelCorrection AT %ML304 : LREAL :=",
        )
        .replace(
            "ProcessValue : LREAL :=",
            "ProcessValue AT %IL160 : LREAL :=",
        )
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL176 : LREAL :=",
        );
    if aggressive_feedback {
        source = source
            .replace(
                "FeedbackProportionalGain : LREAL := 0.46875;",
                "FeedbackProportionalGain : LREAL := 0.625;",
            )
            .replace(
                "FeedbackIntegralGain : LREAL := 0.0390625;",
                "FeedbackIntegralGain : LREAL := 0.052083333333;",
            );
    }
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("Control/advanced_control.st", ADVANCED_CONTROL_SOURCE),
        SourceFile::with_path("heated-air-duct-smith/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Smith-predictor lesson");

    let simulation = project_file("simulation.toml")
        .replace("gain = 0.8", &format!("gain = {plant_gain}"))
        .replace(
            "time_constant_s = 12.0",
            &format!("time_constant_s = {plant_time_constant}"),
        )
        .replace(
            "dead_time_s = 16.0",
            &format!("dead_time_s = {plant_dead_time}"),
        );
    let config_path = temporary_config_path(case);
    std::fs::write(&config_path, simulation).expect("write Smith simulation case");
    let config = SimulationConfig::load(&config_path).expect("load delayed conveyor-oven plant");
    assert_eq!(config.plant_system.plants.len(), 1);
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial product temperature");

    let auto_address = IoAddress::parse("%MX90.0").unwrap();
    let enabled_address = IoAddress::parse("%MX90.1").unwrap();
    let active_address = IoAddress::parse("%MX90.2").unwrap();
    let fallback_address = IoAddress::parse("%MX90.3").unwrap();
    let model_valid_address = IoAddress::parse("%MX90.4").unwrap();
    let setpoint_address = IoAddress::parse("%ML272").unwrap();
    let predicted_address = IoAddress::parse("%ML280").unwrap();
    let correction_address = IoAddress::parse("%ML304").unwrap();
    let pv_address = IoAddress::parse("%IL160").unwrap();
    let mv_address = IoAddress::parse("%QL176").unwrap();
    let mut result = SmithRun {
        feedback_used: Vec::with_capacity(1_800),
        process_value: Vec::with_capacity(1_800),
        manipulated_value: Vec::with_capacity(1_800),
        predicted_value: Vec::with_capacity(1_800),
        correction: Vec::with_capacity(1_800),
        active: Vec::with_capacity(1_800),
        fallback: Vec::with_capacity(1_800),
        enabled: Vec::with_capacity(1_800),
        model_valid: Vec::with_capacity(1_800),
    };
    for cycle in 1usize..=1_800 {
        let setpoint = if cycle <= 100 { 20.0 } else { 35.0 };
        write_bool(&mut runtime, &auto_address, true);
        write_bool(&mut runtime, &enabled_address, predictor_enabled);
        write_lreal(&mut runtime, &setpoint_address, setpoint);
        result.feedback_used.push(read_lreal(&runtime, &pv_address));
        runtime
            .execute_cycle()
            .expect("execute oven controller and predictor");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance delayed oven");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish downstream temperature");
        result.process_value.push(read_lreal(&runtime, &pv_address));
        result
            .manipulated_value
            .push(read_lreal(&runtime, &mv_address));
        result
            .predicted_value
            .push(read_lreal(&runtime, &predicted_address));
        result
            .correction
            .push(read_lreal(&runtime, &correction_address));
        result.active.push(read_bool(&runtime, &active_address));
        result.fallback.push(read_bool(&runtime, &fallback_address));
        result.enabled.push(predictor_enabled);
        result
            .model_valid
            .push(read_bool(&runtime, &model_valid_address));
    }
    let _ = std::fs::remove_file(config_path);
    result
}

fn iae(run: &SmithRun) -> f64 {
    run.process_value[100..]
        .iter()
        .map(|value| (35.0 - value).abs() * 0.1)
        .sum()
}

fn window_iae(run: &SmithRun, start: usize, end: usize) -> f64 {
    run.process_value[start..end]
        .iter()
        .map(|value| (35.0 - value).abs() * 0.1)
        .sum()
}

fn overshoot(run: &SmithRun) -> f64 {
    run.process_value[100..]
        .iter()
        .copied()
        .fold(35.0, f64::max)
        - 35.0
}

fn final_error(run: &SmithRun) -> f64 {
    run.process_value[1_700..]
        .iter()
        .map(|value| (35.0 - value).abs())
        .sum::<f64>()
        / 100.0
}

fn peak_correction(run: &SmithRun) -> f64 {
    run.correction
        .iter()
        .map(|value| value.abs())
        .fold(0.0, f64::max)
}

fn ninety_percent_rise_time(run: &SmithRun) -> f64 {
    let index = run.process_value[100..700]
        .iter()
        .position(|value| *value >= 33.5)
        .expect("response must reach 90 percent before the disturbance");
    index as f64 * 0.1
}

fn assert_bounded(case: &str, run: &SmithRun) {
    assert!(run
        .process_value
        .iter()
        .chain(&run.predicted_value)
        .chain(&run.correction)
        .all(|value| value.is_finite()));
    assert!(
        run.process_value
            .iter()
            .all(|value| (15.0..=60.0).contains(value)),
        "{case} temperature range was {:?}..{:?}",
        run.process_value
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min),
        run.process_value
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max),
    );
    assert!(run
        .manipulated_value
        .iter()
        .all(|value| (0.0..=100.0).contains(value)));
}

#[test]
fn control_curriculum_smith_predictor_improves_nominal_delay_response_and_exposes_mismatch() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 59 descriptor");
    assert_eq!(
        lesson["schemaVersion"], 4,
        "Lesson 59 must use the pedagogical schema"
    );
    assert_eq!(lesson["sequence"], 59);
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_smith_predictor_improves_nominal_delay_response_and_exposes_mismatch"
    );
    assert_eq!(
        project_file("src/Main.st").matches("Control.PID;").count(),
        1
    );
    assert_eq!(
        project_file("src/Main.st")
            .matches("Control.SmithPredictorFOPDT;")
            .count(),
        1
    );
    assert_eq!(
        project_file("simulation.toml")
            .matches(r#"model = "first_order_dead_time""#)
            .count(),
        1
    );

    let ordinary = run_smith("ordinary", false, false, 0.8, 12.0, 16.0);
    let aggressive = run_smith("aggressive", false, true, 0.8, 12.0, 16.0);
    let nominal = run_smith("nominal", true, false, 0.8, 12.0, 16.0);
    let moderate = run_smith("moderate", true, false, 0.76, 13.0, 17.0);
    let severe = run_smith("severe", true, false, 0.60, 18.0, 22.0);
    for (case, run) in [
        ("ordinary", &ordinary),
        ("aggressive", &aggressive),
        ("nominal", &nominal),
        ("moderate", &moderate),
        ("severe", &severe),
    ] {
        assert_bounded(case, run);
    }

    let ordinary_iae = iae(&ordinary);
    let aggressive_iae = iae(&aggressive);
    let nominal_iae = iae(&nominal);
    let moderate_iae = iae(&moderate);
    let ordinary_setpoint_iae = window_iae(&ordinary, 100, 700);
    let nominal_setpoint_iae = window_iae(&nominal, 100, 700);
    let ordinary_regulatory_iae = window_iae(&ordinary, 700, 1_800);
    let nominal_regulatory_iae = window_iae(&nominal, 700, 1_800);
    let ordinary_rise_time = ninety_percent_rise_time(&ordinary);
    let nominal_rise_time = ninety_percent_rise_time(&nominal);
    let ordinary_overshoot = overshoot(&ordinary);
    let aggressive_overshoot = overshoot(&aggressive);
    let nominal_correction = peak_correction(&nominal);
    let moderate_correction = peak_correction(&moderate);
    let nominal_fallbacks = nominal.fallback[100..]
        .iter()
        .filter(|value| **value)
        .count();
    let moderate_fallbacks = moderate.fallback[100..]
        .iter()
        .filter(|value| **value)
        .count();
    let severe_fallbacks = severe.fallback[100..]
        .iter()
        .filter(|value| **value)
        .count();
    println!(
        "oven ordinary total/setpoint/regulatory IAE={ordinary_iae:.4}/{ordinary_setpoint_iae:.4}/{ordinary_regulatory_iae:.4}, rise={ordinary_rise_time:.1}s, overshoot={ordinary_overshoot:.4}, final={:.4}; aggressive IAE={aggressive_iae:.4}, overshoot={aggressive_overshoot:.4}, final={:.4}; nominal Smith total/setpoint/regulatory IAE={nominal_iae:.4}/{nominal_setpoint_iae:.4}/{nominal_regulatory_iae:.4}, rise={nominal_rise_time:.1}s, correction={nominal_correction:.4}, final={:.4}; moderate IAE={moderate_iae:.4}, correction={moderate_correction:.4}; fallback nominal/moderate/severe={nominal_fallbacks}/{moderate_fallbacks}/{severe_fallbacks}",
        final_error(&ordinary),
        final_error(&aggressive),
        final_error(&nominal),
    );

    assert!(ordinary_overshoot <= 2.0 && final_error(&ordinary) <= 0.5);
    assert!(
        aggressive_overshoot > 2.0 || final_error(&aggressive) > 0.5,
        "the next-more-aggressive ordinary PI must expose the selection boundary"
    );
    assert!(nominal_rise_time <= ordinary_rise_time * 0.85);
    assert!(nominal_setpoint_iae < ordinary_setpoint_iae);
    assert!(final_error(&nominal) <= 0.5);
    assert!(moderate_iae > nominal_iae || moderate_correction > nominal_correction * 1.25);
    assert_eq!(moderate_fallbacks, 0);
    assert!(nominal.active[100..].iter().all(|value| *value));
    assert!(severe_fallbacks > nominal_fallbacks);
    for (index, fallback) in severe.fallback.iter().enumerate() {
        if *fallback {
            assert!((severe.predicted_value[index] - severe.feedback_used[index]).abs() <= 1e-12);
        }
    }

    let time = time_axis(ordinary.process_value.len());
    let (temperature_min, temperature_max) = plot_bounds([
        ordinary.process_value.as_slice(),
        nominal.process_value.as_slice(),
        moderate.process_value.as_slice(),
        severe.process_value.as_slice(),
    ]);
    let (output_min, output_max) = plot_bounds([
        ordinary.manipulated_value.as_slice(),
        nominal.manipulated_value.as_slice(),
    ]);
    let summary = format!(
        "case,status,total_iae_degC_s,setpoint_iae_degC_s,rise_time_s,overshoot_degC,final_error_degC,peak_model_correction_degC,fallback_samples\nordinary_pi,qualified,{ordinary_iae:.6},{ordinary_setpoint_iae:.6},{ordinary_rise_time:.6},{ordinary_overshoot:.6},{:.6},0,0\naggressive_pi,rejected,{aggressive_iae:.6},,,{aggressive_overshoot:.6},{:.6},0,0\nnominal_smith,qualified,{nominal_iae:.6},{nominal_setpoint_iae:.6},{nominal_rise_time:.6},{:.6},{:.6},{nominal_correction:.6},{nominal_fallbacks}\nmoderate_mismatch,qualified_with_residual,{moderate_iae:.6},,,,,{moderate_correction:.6},{moderate_fallbacks}\nsevere_mismatch,fallback_required,,,,,,,{severe_fallbacks}\n",
        final_error(&ordinary), final_error(&aggressive), overshoot(&nominal), final_error(&nominal),
    );
    let nominal_enabled = nominal
        .enabled
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let nominal_active = nominal
        .active
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let nominal_valid = nominal
        .model_valid
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let nominal_fallback = nominal
        .fallback
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let severe_active = severe
        .active
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let severe_valid = severe
        .model_valid
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let severe_fallback = severe
        .fallback
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    evidence::retain(
        &evidence_root(),
        "heated-air-duct-smith-evidence",
        "Smith prediction handles transport delay",
        "The nominal model improves the delayed duct response; mismatch correction exposes when fallback is required.",
        &[
            evidence::Panel {
                title: "Outlet temperature", x_unit: "time (s)", y_unit: "temperature (degC)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(),
                y_minimum: temperature_min, y_maximum: temperature_max,
                series: vec![
                    evidence::Series { name: "Ordinary PI", color: "#6b7280", dashed: false, x: &time, y: &ordinary.process_value },
                    evidence::Series { name: "Nominal Smith", color: "#2563eb", dashed: false, x: &time, y: &nominal.process_value },
                    evidence::Series { name: "Moderate mismatch", color: "#b45309", dashed: true, x: &time, y: &moderate.process_value },
                    evidence::Series { name: "Severe mismatch", color: "#dc2626", dashed: true, x: &time, y: &severe.process_value },
                ],
            },
            evidence::Panel {
                title: "Heater command", x_unit: "time (s)", y_unit: "output (%)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(),
                y_minimum: output_min, y_maximum: output_max,
                series: vec![
                    evidence::Series { name: "Ordinary PI", color: "#6b7280", dashed: false, x: &time, y: &ordinary.manipulated_value },
                    evidence::Series { name: "Nominal Smith", color: "#2563eb", dashed: false, x: &time, y: &nominal.manipulated_value },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Heater commands", name: "Moderate mismatch applied", x: &time, y: &moderate.manipulated_value, x_unit: "time (s)", y_unit: "heater (%)" },
            evidence::RetainedSeries { panel: "Heater commands", name: "Severe mismatch applied", x: &time, y: &severe.manipulated_value, x_unit: "time (s)", y_unit: "heater (%)" },
            evidence::RetainedSeries { panel: "Model monitor", name: "Nominal correction", x: &time, y: &nominal.correction, x_unit: "time (s)", y_unit: "temperature (degC)" },
            evidence::RetainedSeries { panel: "Model monitor", name: "Severe correction", x: &time, y: &severe.correction, x_unit: "time (s)", y_unit: "temperature (degC)" },
            evidence::RetainedSeries { panel: "Predictor state", name: "Nominal enabled", x: &time, y: &nominal_enabled, x_unit: "time (s)", y_unit: "state (0/1)" },
            evidence::RetainedSeries { panel: "Predictor state", name: "Nominal active", x: &time, y: &nominal_active, x_unit: "time (s)", y_unit: "state (0/1)" },
            evidence::RetainedSeries { panel: "Predictor state", name: "Nominal model valid", x: &time, y: &nominal_valid, x_unit: "time (s)", y_unit: "state (0/1)" },
            evidence::RetainedSeries { panel: "Predictor state", name: "Nominal fallback", x: &time, y: &nominal_fallback, x_unit: "time (s)", y_unit: "state (0/1)" },
            evidence::RetainedSeries { panel: "Severe mismatch state", name: "Predictor active", x: &time, y: &severe_active, x_unit: "time (s)", y_unit: "state (0/1)" },
            evidence::RetainedSeries { panel: "Severe mismatch state", name: "Model valid", x: &time, y: &severe_valid, x_unit: "time (s)", y_unit: "state (0/1)" },
            evidence::RetainedSeries { panel: "Severe mismatch state", name: "Fallback", x: &time, y: &severe_fallback, x_unit: "time (s)", y_unit: "state (0/1)" },
        ],
        &summary,
    );
}
