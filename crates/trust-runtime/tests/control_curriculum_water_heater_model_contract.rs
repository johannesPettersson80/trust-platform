use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/src/Main.st");
const SEED: u64 = 20260824;
const SAMPLE_TIME_S: f64 = 0.1;
const END_TIME_S: f64 = 30.0;
const EQUILIBRIUM_DEGC: f64 = 24.0;
const SIX_SECOND_PREDICTION_DEGC: f64 = 22.5284822352;
const ASSUMPTIONS: &str =
    "well_mixed|constant_properties|constant_heater_efficiency|fixed_flow|linear_effective_loss";
const VALIDITY_REGION: &str = "pv_20_to_24_degc|applied_power_0_to_20_percent";
const MODEL_PROVENANCE: &str = "first_principles_energy_balance";
const SIMULATION_PROVENANCE: &str = "configured_simulation_truth";

#[derive(Clone, Copy)]
struct Sample {
    index: usize,
    time_s: f64,
    event: &'static str,
    pv_degc: f64,
    model_prediction_degc: f64,
    requested_power_percent: f64,
    applied_power_percent: f64,
    model_valid: bool,
    static_gain_degc_per_percent: f64,
    time_constant_s: f64,
    predicted_equilibrium_degc: f64,
    thermal_capacitance_kj_per_degc: f64,
    effective_heat_loss_kw_per_degc: f64,
    full_scale_heater_power_kw: f64,
    ambient_temperature_degc: f64,
    inputs_valid: bool,
    experiment_enabled: bool,
    manual_mode: bool,
}

fn addressed_source() -> String {
    let source = if LESSON_SOURCE.contains("ExperimentEnabled : BOOL") {
        LESSON_SOURCE.to_owned()
    } else {
        LESSON_SOURCE.replace(
            "    InputsValid : BOOL := FALSE;",
            "    InputsValid : BOOL := FALSE;\n    ExperimentEnabled : BOOL := FALSE;\n    ManualMode : BOOL := FALSE;",
        )
    };
    source
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 15 program before configuration")
        .replace(
            "    RequestedHeaterPower : LREAL :=",
            "    RequestedHeaterPower AT %ML32 : LREAL :=",
        )
        .replace(
            "    ProcessValue : LREAL :=",
            "    ProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "    AppliedHeaterPower : LREAL :=",
            "    AppliedHeaterPower AT %QL0 : LREAL :=",
        )
        .replace(
            "    StaticGain : LREAL :=",
            "    StaticGain AT %ML0 : LREAL :=",
        )
        .replace(
            "    TimeConstant : LREAL :=",
            "    TimeConstant AT %ML8 : LREAL :=",
        )
        .replace(
            "    PredictedEquilibrium : LREAL :=",
            "    PredictedEquilibrium AT %ML16 : LREAL :=",
        )
        .replace(
            "    ThermalCapacitance : LREAL :=",
            "    ThermalCapacitance AT %ML40 : LREAL :=",
        )
        .replace(
            "    EffectiveHeatLoss : LREAL :=",
            "    EffectiveHeatLoss AT %ML48 : LREAL :=",
        )
        .replace(
            "    FullScaleHeaterPower : LREAL :=",
            "    FullScaleHeaterPower AT %ML56 : LREAL :=",
        )
        .replace(
            "    AmbientTemperature : LREAL :=",
            "    AmbientTemperature AT %ML64 : LREAL :=",
        )
        .replace(
            "    ModelValid : BOOL :=",
            "    ModelValid AT %MX80.0 : BOOL :=",
        )
        .replace(
            "    InputsValid : BOOL :=",
            "    InputsValid AT %MX80.1 : BOOL :=",
        )
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %MX81.0 : BOOL :=",
        )
        .replace(
            "    ManualMode : BOOL :=",
            "    ManualMode AT %MX82.0 : BOOL :=",
        )
}

fn runtime() -> Runtime {
    CompileSession::from_sources(vec![SourceFile::with_path(
        "water-heater-energy-balance/src/Main.st",
        addressed_source(),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 15 Structured Text")
}

fn simulation() -> SimulationController {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/simulation.toml");
    SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load shipped Lesson 15 simulation: {error}")),
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

fn write_bool(runtime: &mut Runtime, text: &str, value: bool) {
    runtime
        .io_mut()
        .write(&address(text), Value::Bool(value))
        .unwrap_or_else(|error| panic!("write {text}: {error}"));
}

fn prediction(time_s: f64) -> f64 {
    if time_s <= 1.0 {
        20.0
    } else {
        EQUILIBRIUM_DEGC - 4.0 * (-(time_s - 1.0) / 5.0).exp()
    }
}

fn capture(index: usize, time_s: f64, event: &'static str, runtime: &Runtime) -> Sample {
    Sample {
        index,
        time_s,
        event,
        pv_degc: read_lreal(runtime, "%IL0"),
        model_prediction_degc: prediction(time_s.min(END_TIME_S)),
        requested_power_percent: read_lreal(runtime, "%ML32"),
        applied_power_percent: read_lreal(runtime, "%QL0"),
        model_valid: read_bool(runtime, "%MX80.0"),
        static_gain_degc_per_percent: read_lreal(runtime, "%ML0"),
        time_constant_s: read_lreal(runtime, "%ML8"),
        predicted_equilibrium_degc: read_lreal(runtime, "%ML16"),
        thermal_capacitance_kj_per_degc: read_lreal(runtime, "%ML40"),
        effective_heat_loss_kw_per_degc: read_lreal(runtime, "%ML48"),
        full_scale_heater_power_kw: read_lreal(runtime, "%ML56"),
        ambient_temperature_degc: read_lreal(runtime, "%ML64"),
        inputs_valid: read_bool(runtime, "%MX80.1"),
        experiment_enabled: read_bool(runtime, "%MX81.0"),
        manual_mode: read_bool(runtime, "%MX82.0"),
    }
}

fn run() -> Vec<Sample> {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 15 heater state");

    write_lreal(&mut runtime, "%ML40", 50.0);
    write_lreal(&mut runtime, "%ML48", 10.0);
    write_lreal(&mut runtime, "%ML56", 200.0);
    write_lreal(&mut runtime, "%ML64", 20.0);
    write_bool(&mut runtime, "%MX81.0", true);
    write_lreal(&mut runtime, "%ML32", 20.0);
    runtime.execute_cycle().expect("execute Lesson 15 at t=0");
    let mut samples = vec![capture(0, 0.0, "active_step", &runtime)];

    for index in 1..=300 {
        let now = Duration::from_millis((index * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 15 heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 15 heater PV");
        runtime.execute_cycle().expect("execute Lesson 15 program");
        samples.push(capture(
            index,
            index as f64 * SAMPLE_TIME_S,
            "active_step",
            &runtime,
        ));
    }

    write_bool(&mut runtime, "%MX81.0", false);
    write_lreal(&mut runtime, "%ML32", 0.0);
    runtime
        .execute_cycle()
        .expect("execute Lesson 15 safe restoration");
    samples.push(capture(301, 30.1, "safe_restoration", &runtime));
    samples
}

fn artifact_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/figures")
        .join(name)
}

fn write_or_compare(path: &Path, expected: &str) {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("artifact parent"))
            .expect("create Lesson 15 artifact directory");
        std::fs::write(path, expected).expect("write Lesson 15 artifact");
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
fn lesson_15_requires_boolean_test_authority_and_owns_the_power_step() {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 15 heater state");

    write_lreal(&mut runtime, "%ML40", 50.0);
    write_lreal(&mut runtime, "%ML48", 10.0);
    write_lreal(&mut runtime, "%ML56", 200.0);
    write_lreal(&mut runtime, "%ML64", 20.0);
    write_lreal(&mut runtime, "%ML32", 20.0);
    runtime
        .execute_cycle()
        .expect("execute Lesson 15 with stale request but no authority");

    assert!(!read_bool(&runtime, "%MX81.0"));
    assert!(read_bool(&runtime, "%MX82.0"));
    assert_eq!(read_lreal(&runtime, "%ML32"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);

    write_bool(&mut runtime, "%MX81.0", true);
    runtime
        .execute_cycle()
        .expect("execute Lesson 15 with reviewed Boolean authority");

    assert!(read_bool(&runtime, "%MX81.0"));
    assert!(!read_bool(&runtime, "%MX82.0"));
    assert!((read_lreal(&runtime, "%ML32") - 20.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, "%QL0") - 20.0).abs() <= 1e-12);
}

#[test]
fn lesson_15_retains_reconstructible_prediction_and_equilibrium_evidence() {
    let samples = run();
    let active = &samples[..301];
    let final_window: Vec<_> = active
        .iter()
        .filter(|sample| (29.0..=30.0).contains(&sample.time_s))
        .collect();
    assert_eq!(final_window.len(), 11);

    let numerically_valid = active.iter().all(|sample| {
        sample.pv_degc.is_finite()
            && sample.model_prediction_degc.is_finite()
            && sample.static_gain_degc_per_percent.is_finite()
            && sample.time_constant_s.is_finite()
            && sample.predicted_equilibrium_degc.is_finite()
            && sample.model_valid
            && sample.inputs_valid
    });
    let power_valid = active.iter().all(|sample| {
        (sample.requested_power_percent - 20.0).abs() <= 1e-12
            && (sample.applied_power_percent - 20.0).abs() <= 1e-12
    });
    let hard_limits_valid = active.iter().all(|sample| {
        (0.0..=80.0).contains(&sample.pv_degc)
            && (0.0..=100.0).contains(&sample.applied_power_percent)
    });
    let final_min = final_window
        .iter()
        .map(|sample| sample.pv_degc)
        .fold(f64::INFINITY, f64::min);
    let final_max = final_window
        .iter()
        .map(|sample| sample.pv_degc)
        .fold(f64::NEG_INFINITY, f64::max);
    let final_range = final_max - final_min;
    let settled = numerically_valid && power_valid && hard_limits_valid && final_range <= 0.004;
    let final_mean_error = settled.then(|| {
        final_window
            .iter()
            .map(|sample| (sample.pv_degc - EQUILIBRIUM_DEGC).abs())
            .sum::<f64>()
            / final_window.len() as f64
    });
    let at_six = active
        .iter()
        .find(|sample| (sample.time_s - 6.0).abs() <= 1e-12)
        .expect("six-second retained sample");
    let six_second_error = (at_six.pv_degc - SIX_SECOND_PREDICTION_DEGC).abs();
    let restoration = samples.last().expect("restoration sample");
    let safe_restoration = restoration.event == "safe_restoration"
        && !restoration.experiment_enabled
        && restoration.manual_mode
        && restoration.requested_power_percent.abs() <= 1e-12
        && restoration.applied_power_percent.abs() <= 1e-12;

    assert!(numerically_valid);
    assert!(power_valid);
    assert!(hard_limits_valid);
    assert!(active
        .iter()
        .all(|sample| sample.experiment_enabled && !sample.manual_mode));
    assert!(settled, "final-window PV range={final_range:.12} degC");
    assert!(six_second_error <= 0.08 + 0.001);
    assert!(final_mean_error.expect("eligible equilibrium error") <= 0.02 + 0.001);
    assert!(safe_restoration);
    assert!(samples.iter().all(|sample| {
        (sample.thermal_capacitance_kj_per_degc - 50.0).abs() <= 1e-12
            && (sample.effective_heat_loss_kw_per_degc - 10.0).abs() <= 1e-12
            && (sample.full_scale_heater_power_kw - 200.0).abs() <= 1e-12
            && (sample.ambient_temperature_degc - 20.0).abs() <= 1e-12
    }));

    let mut trace_csv = String::from(
        "run_id,sample_index,time_s,event,pv_degc,model_prediction_degc,requested_power_percent,applied_power_percent,model_valid,inputs_valid,experiment_enabled,manual_mode,thermal_capacitance_kj_per_degc,thermal_capacitance_unit,effective_heat_loss_kw_per_degc,effective_heat_loss_unit,full_scale_heater_power_kw,heater_power_unit,ambient_temperature_degc,ambient_temperature_unit,static_gain_degc_per_percent,time_constant_s,dead_time_s,predicted_equilibrium_degc,pv_min_degc,pv_max_degc,power_min_percent,power_max_percent,sample_time_s,seed,plant_id,model_id,assumptions,validity_region,model_evidence_class,model_provenance,simulation_evidence_class,simulation_provenance\n",
    );
    for sample in &samples {
        writeln!(
            trace_csv,
            "lesson-15,{},{:.12},{},{:.12},{:.12},{:.12},{:.12},{},{},{},{},{:.12},kJ/degC,{:.12},kW/degC,{:.12},kW,{:.12},degC,{:.12},{:.12},1.000000000000,{:.12},0.000000000000,80.000000000000,0.000000000000,100.000000000000,0.100000000000,{SEED},recirculating-water-heater,energy-balance-local-fopdt-v1,{ASSUMPTIONS},{VALIDITY_REGION},model,{MODEL_PROVENANCE},configured_simulation,{SIMULATION_PROVENANCE}",
            sample.index,
            sample.time_s,
            sample.event,
            sample.pv_degc,
            sample.model_prediction_degc,
            sample.requested_power_percent,
            sample.applied_power_percent,
            sample.model_valid,
            sample.inputs_valid,
            sample.experiment_enabled,
            sample.manual_mode,
            sample.thermal_capacitance_kj_per_degc,
            sample.effective_heat_loss_kw_per_degc,
            sample.full_scale_heater_power_kw,
            sample.ambient_temperature_degc,
            sample.static_gain_degc_per_percent,
            sample.time_constant_s,
            sample.predicted_equilibrium_degc,
        )
        .expect("format Lesson 15 trace");
    }

    let final_error_text = final_mean_error
        .map(|value| format!("{value:.12}"))
        .unwrap_or_else(|| "Not settled".to_owned());
    let summary_csv = format!(
        "run_id,numerically_valid,active_power_valid,hard_limits_valid,final_window_start_s,final_window_end_s,final_window_pv_range_degc,settling_range_limit_degc,settled,thermal_capacitance_kj_per_degc,thermal_capacitance_unit,effective_heat_loss_kw_per_degc,effective_heat_loss_unit,full_scale_heater_power_kw,heater_power_unit,ambient_temperature_degc,ambient_temperature_unit,derived_gain_degc_per_percent,gain_expected,gain_tolerance,time_constant_s,time_constant_expected_s,time_constant_tolerance_s,dead_time_s,predicted_equilibrium_degc,equilibrium_expected_degc,equilibrium_tolerance_degc,six_second_prediction_degc,six_second_simulation_degc,six_second_absolute_error_degc,six_second_error_limit_degc,six_second_tolerance_degc,six_second_pass,final_window_mean_absolute_equilibrium_error_degc,final_error_limit_degc,final_error_tolerance_degc,final_error_pass,restoration_time_s,restoration_event,restoration_experiment_enabled,restoration_manual_mode,restoration_requested_power_percent,restoration_applied_power_percent,safe_restoration,assumptions,validity_region,model_evidence_class,model_provenance,simulation_evidence_class,simulation_provenance,decision\nlesson-15,{numerically_valid},{power_valid},{hard_limits_valid},29.000000000000,30.000000000000,{final_range:.12},0.004000000000,{settled},50.000000000000,kJ/degC,10.000000000000,kW/degC,200.000000000000,kW,20.000000000000,degC,{:.12},0.200000000000,0.000000001000,{:.12},5.000000000000,0.000000001000,1.000000000000,{:.12},24.000000000000,0.000000001000,{SIX_SECOND_PREDICTION_DEGC:.12},{:.12},{six_second_error:.12},0.080000000000,0.001000000000,{},{final_error_text},0.020000000000,0.001000000000,{},{:.12},{},{},{},{:.12},{:.12},{safe_restoration},{ASSUMPTIONS},{VALIDITY_REGION},model,{MODEL_PROVENANCE},configured_simulation,{SIMULATION_PROVENANCE},Model and configured simulation consistent\n",
        active[0].static_gain_degc_per_percent,
        active[0].time_constant_s,
        active[0].predicted_equilibrium_degc,
        at_six.pv_degc,
        six_second_error <= 0.08 + 0.001,
        final_mean_error.is_some_and(|value| value <= 0.02 + 0.001),
        restoration.time_s,
        restoration.event,
        restoration.experiment_enabled,
        restoration.manual_mode,
        restoration.requested_power_percent,
        restoration.applied_power_percent,
    );

    write_or_compare(&artifact_path("heater-model-trace.csv"), &trace_csv);
    write_or_compare(&artifact_path("heater-model-summary.csv"), &summary_csv);
}
