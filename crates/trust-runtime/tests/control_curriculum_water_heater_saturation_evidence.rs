use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/08-water-heater-saturation/src/Main.st");
const RETAINED_TRACE: &str = include_str!(
    "../../../examples/control/01-feedback-foundations-and-temperature-control/08-water-heater-saturation/figures/saturation-trace.csv"
);
const RETAINED_SUMMARY: &str = include_str!(
    "../../../examples/control/01-feedback-foundations-and-temperature-control/08-water-heater-saturation/figures/saturation-summary.csv"
);
const RUN_ID: &str = "lesson-08-saturation-seed-20260828-kp8-ki4-dt0.1";

#[derive(Clone, Copy)]
struct Sample {
    time_s: f64,
    phase: &'static str,
    setpoint: f64,
    pv: f64,
    requested: f64,
    applied: f64,
    saturation_error: f64,
    proportional_increment: f64,
    integral_increment: f64,
    auto: bool,
}

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 8 program before configuration")
        .replace(
            "    AutoMode : BOOL :=",
            "    AutoMode AT %MX64.0 : BOOL :=",
        )
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %MX72.0 : BOOL :=",
        )
        .replace(
            "    Setpoint : LREAL :=",
            "    Setpoint AT %ML0 : LREAL :=",
        )
        .replace(
            "    ProcessValue : LREAL :=",
            "    ProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "    ManualHeaterPower : LREAL :=",
            "    ManualHeaterPower AT %ML16 : LREAL :=",
        )
        .replace(
            "    RequestedHeaterPower : LREAL :=",
            "    RequestedHeaterPower AT %ML24 : LREAL :=",
        )
        .replace(
            "    RequestedManipulatedValue : LREAL :=",
            "    RequestedManipulatedValue AT %ML24 : LREAL :=",
        )
        .replace(
            "    SaturationError : LREAL :=",
            "    SaturationError AT %ML32 : LREAL :=",
        )
        .replace(
            "    ProportionalIncrement : LREAL :=",
            "    ProportionalIncrement AT %ML40 : LREAL :=",
        )
        .replace(
            "    IntegralIncrement : LREAL :=",
            "    IntegralIncrement AT %ML48 : LREAL :=",
        )
        .replace(
            "    AppliedHeaterPower : LREAL :=",
            "    AppliedHeaterPower AT %QL0 : LREAL :=",
        )
        .replace(
            "    ManipulatedValue : LREAL :=",
            "    ManipulatedValue AT %QL0 : LREAL :=",
        )
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("BOOL address"),
            Value::Bool(value),
        )
        .expect("write BOOL process-image value");
}

fn write_lreal(runtime: &mut Runtime, address: &str, value: f64) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("LREAL address"),
            Value::LWord(value.to_bits()),
        )
        .expect("write LREAL process-image value");
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("LREAL address"))
        .expect("read LREAL process-image value")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("BOOL address"))
        .expect("read BOOL process-image value")
    {
        Value::Bool(value) => value,
        other => panic!("expected BOOL storage, got {other:?}"),
    }
}

fn lesson_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-saturation/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 8 Structured Text")
}

fn lesson_simulation() -> SimulationController {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/08-water-heater-saturation/simulation.toml");
    SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load shipped Lesson 8 simulation: {error}")),
    )
}

fn assert_close(actual: f64, expected: f64, message: &str) {
    assert!(
        (actual - expected).abs() <= 1.0e-9,
        "{message}: expected {expected}, got {actual}"
    );
}

fn run_saturation_experiment() -> Vec<Sample> {
    let mut runtime = lesson_runtime();
    let mut simulation = lesson_simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 8 plant state");
    let mut trace = Vec::with_capacity(951);
    write_bool(&mut runtime, "%MX72.0", true);

    for cycle in 1..=950 {
        let phase = match cycle {
            1 => "safe_start",
            2..=500 => "manual_preparation",
            501..=550 => "achievable_baseline",
            551..=650 => "unreachable_exposure",
            _ => "achievable_restoration",
        };
        runtime.execute_cycle().expect("execute Lesson 8 scan");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 8 heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 8 process value");
        trace.push(Sample {
            time_s: cycle as f64 * 0.1,
            phase,
            setpoint: read_lreal(&runtime, "%ML0"),
            pv: read_lreal(&runtime, "%IL0"),
            requested: read_lreal(&runtime, "%ML24"),
            applied: read_lreal(&runtime, "%QL0"),
            saturation_error: read_lreal(&runtime, "%ML32"),
            proportional_increment: read_lreal(&runtime, "%ML40"),
            integral_increment: read_lreal(&runtime, "%ML48"),
            auto: read_bool(&runtime, "%MX64.0"),
        });
    }

    runtime
        .execute_cycle()
        .expect("execute Lesson 8 project-owned safe restoration");
    trace.push(Sample {
        time_s: 95.1,
        phase: "safe_restoration",
        setpoint: 35.0,
        pv: read_lreal(&runtime, "%IL0"),
        requested: read_lreal(&runtime, "%ML24"),
        applied: read_lreal(&runtime, "%QL0"),
        saturation_error: read_lreal(&runtime, "%ML32"),
        proportional_increment: read_lreal(&runtime, "%ML40"),
        integral_increment: read_lreal(&runtime, "%ML48"),
        auto: read_bool(&runtime, "%MX64.0"),
    });
    trace
}

fn write_evidence(trace: &[Sample], post_restore_hold_s: f64, recovered_s: Option<f64>) {
    let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("create Lesson 8 evidence directory");
    let mut csv = String::from(
        "run_id,time_s,phase,setpoint_degC,pv_degC,error_degC,kp_percent_per_degC,ki_percent_per_degC_s,proportional_increment_pp,integral_increment_pp,requested_mv_percent,applied_mv_percent,saturation_error_pp,manual_output_percent,auto_mode,required_steady_mv_percent,reachability_margin_pp,sample_time_s,seed,plant_model,ambient_temperature_degC,steady_heater_gain_degC_per_percent,time_constant_s,dead_time_s,mv_min_percent,mv_max_percent,abort_state,evidence_class\n",
    );
    for sample in trace {
        let required = (sample.setpoint - 20.0) / 0.2;
        writeln!(
            csv,
            "{RUN_ID},{:.1},{},{:.1},{:.6},{:.6},8.0,4.0,{:.6},{:.6},{:.6},{:.6},{:.6},{:.1},{},{:.1},{:.1},0.1,20260828,first_order_dead_time,20.0,0.2,5.0,1.0,0.0,100.0,none,simulation",
            sample.time_s,
            sample.phase,
            sample.setpoint,
            sample.pv,
            sample.setpoint - sample.pv,
            sample.proportional_increment,
            sample.integral_increment,
            sample.requested,
            sample.applied,
            sample.saturation_error,
            if sample.auto { 75.0 } else { sample.requested },
            sample.auto,
            required,
            100.0 - required,
        )
        .expect("append Lesson 8 evidence row");
    }
    std::fs::write(directory.join("saturation-trace.csv"), csv).expect("write Lesson 8 trace");
    let exposure = &trace[550..650];
    let peak_requested = exposure
        .iter()
        .map(|sample| sample.requested)
        .fold(f64::NEG_INFINITY, f64::max);
    let peak_applied = exposure
        .iter()
        .map(|sample| sample.applied)
        .fold(f64::NEG_INFINITY, f64::max);
    let peak_error = exposure
        .iter()
        .map(|sample| sample.saturation_error)
        .fold(f64::NEG_INFINITY, f64::max);
    let upper_limit_s = exposure
        .iter()
        .filter(|sample| sample.applied >= 100.0 - 1.0e-9)
        .count() as f64
        * 0.1;
    let maximum_pv = exposure
        .iter()
        .map(|sample| sample.pv)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut summary = String::from(
        "run_id,plant_model,ambient_temperature_degC,steady_heater_gain_degC_per_percent,time_constant_s,dead_time_s,target_degC,required_steady_mv_percent,reachability_margin_pp,reachable,peak_requested_mv_percent,peak_applied_mv_percent,peak_saturation_error_pp,upper_limit_time_s,post_restore_upper_limit_time_s,recovery_time_s,maximum_pv_degC,validity,abort_state,safe_restoration,conclusion,evidence_class\n",
    );
    writeln!(
        summary,
        "{RUN_ID},first_order_dead_time,20.0,0.2,5.0,1.0,45.0,125.0,-25.0,false,{peak_requested:.6},{peak_applied:.6},{peak_error:.6},{upper_limit_s:.1},{post_restore_hold_s:.1},{},{maximum_pv:.6},valid,none,true,external_clamp_protects_actuator_but_unobserved_integral_state_delays_recovery,simulation",
        recovered_s
            .map(|value| format!("{value:.1}"))
            .unwrap_or_else(|| "Not recovered".to_owned()),
    )
    .expect("append Lesson 8 evidence summary");
    std::fs::write(directory.join("saturation-summary.csv"), summary)
        .expect("write Lesson 8 evidence summary");
}

#[test]
fn control_curriculum_water_heater_saturation_tracks_the_manual_holding_command() {
    let mut runtime = lesson_runtime();

    write_lreal(&mut runtime, "%ML0", 35.0);
    write_lreal(&mut runtime, "%IL0", 35.0);
    write_bool(&mut runtime, "%MX64.0", false);
    write_lreal(&mut runtime, "%ML16", 0.0);
    runtime.execute_cycle().expect("execute safe Manual scan");
    assert_eq!(
        read_lreal(&runtime, "%QL0"),
        0.0,
        "Lesson 8 must start in Manual with zero applied heater power"
    );

    write_lreal(&mut runtime, "%ML16", 75.0);
    runtime
        .execute_cycle()
        .expect("execute Manual holding-output scan");
    assert_eq!(
        read_lreal(&runtime, "%QL0"),
        75.0,
        "Lesson 8 Manual mode must apply the commanded 75 percent holding power before Auto"
    );
}

#[test]
fn control_curriculum_water_heater_saturation_executes_the_shipped_project_sequence() {
    let mut runtime = lesson_runtime();
    write_lreal(&mut runtime, "%IL0", 35.0);
    write_bool(&mut runtime, "%MX72.0", true);

    runtime.execute_cycle().expect("execute project safe-start scan");
    assert_close(read_lreal(&runtime, "%QL0"), 0.0, "cycle 1 must retain safe Manual zero");
    assert!(!read_bool(&runtime, "%MX64.0"), "cycle 1 must remain in Manual");

    for _ in 2..=500 {
        runtime.execute_cycle().expect("execute project Manual preparation");
    }
    assert_close(read_lreal(&runtime, "%QL0"), 75.0, "Manual preparation must apply 75 percent");
    assert!(!read_bool(&runtime, "%MX64.0"), "preparation must remain in Manual");

    for _ in 501..=550 {
        runtime.execute_cycle().expect("execute reachable Auto baseline");
    }
    assert!(read_bool(&runtime, "%MX64.0"), "reachable baseline must use Auto");
    assert_close(read_lreal(&runtime, "%ML0"), 35.0, "baseline setpoint");

    runtime.execute_cycle().expect("execute first unreachable exposure scan");
    assert_close(read_lreal(&runtime, "%ML0"), 45.0, "exposure setpoint");
    assert_close(read_lreal(&runtime, "%ML24"), 159.0, "first unrestricted request");
    assert_close(read_lreal(&runtime, "%QL0"), 100.0, "first applied request is clamped");

    for _ in 552..=650 {
        runtime.execute_cycle().expect("complete unreachable exposure");
    }
    runtime.execute_cycle().expect("begin reachable restoration");
    assert_close(read_lreal(&runtime, "%ML0"), 35.0, "restored setpoint");
    assert!(read_bool(&runtime, "%MX64.0"), "recovery must retain Auto");

    for _ in 652..=950 {
        runtime.execute_cycle().expect("complete reachable recovery window");
    }
    runtime.execute_cycle().expect("execute project safe-restoration scan");
    assert!(!read_bool(&runtime, "%MX64.0"), "project must restore Manual");
    assert_close(read_lreal(&runtime, "%ML24"), 0.0, "project safe requested output");
    assert_close(read_lreal(&runtime, "%QL0"), 0.0, "project safe applied output");
}

#[test]
fn control_curriculum_water_heater_saturation_executes_the_predicted_first_limited_scan() {
    let mut runtime = lesson_runtime();

    write_lreal(&mut runtime, "%IL0", 35.0);
    runtime.execute_cycle().expect("execute safe initialization scan");
    write_lreal(&mut runtime, "%ML0", 35.0);
    write_lreal(&mut runtime, "%ML16", 75.0);
    write_bool(&mut runtime, "%MX64.0", false);
    runtime
        .execute_cycle()
        .expect("establish the qualified Manual operating point");

    write_lreal(&mut runtime, "%ML0", 45.0);
    write_bool(&mut runtime, "%MX64.0", true);
    runtime
        .execute_cycle()
        .expect("execute the first unreachable-setpoint scan");

    assert_close(
        read_lreal(&runtime, "%ML24"),
        159.0,
        "the inherited Kp 8 and Ki 4 controller must calculate the predicted request",
    );
    assert_close(
        read_lreal(&runtime, "%ML40"),
        80.0,
        "the exposed proportional increment must match the actual PI update",
    );
    assert_close(
        read_lreal(&runtime, "%ML48"),
        4.0,
        "the exposed integral increment must match the actual PI update",
    );
    assert_close(
        read_lreal(&runtime, "%QL0"),
        100.0,
        "the separate physical clamp must limit the heater command",
    );
    assert_close(
        read_lreal(&runtime, "%ML32"),
        59.0,
        "saturation error must be requested minus applied heater power",
    );
}

#[test]
fn control_curriculum_water_heater_saturation_isolates_the_setpoint_experiment() {
    let simulation = lesson_simulation();
    let status = simulation.status();
    assert_eq!(
        status.plants.len(),
        1,
        "Lesson 8 must use one water-heater plant"
    );
    assert_eq!(
        status.plants[0].pending_events, 0,
        "Lesson 8 must not confound setpoint saturation with the Lesson 7 cold-water load event"
    );
}

#[test]
fn control_curriculum_water_heater_saturation_retains_windup_and_safe_recovery_evidence() {
    let trace = run_saturation_experiment();
    let baseline = &trace[520..550];
    let exposure = &trace[550..650];
    let restored = &trace[650..950];
    let maximum_baseline_mismatch = baseline
        .iter()
        .map(|sample| (sample.requested - sample.applied).abs())
        .fold(0.0_f64, f64::max);
    let peak_requested = exposure
        .iter()
        .map(|sample| sample.requested)
        .fold(f64::NEG_INFINITY, f64::max);
    let peak_saturation_error = exposure
        .iter()
        .map(|sample| sample.saturation_error)
        .fold(f64::NEG_INFINITY, f64::max);
    let upper_limit_s = exposure
        .iter()
        .filter(|sample| sample.applied >= 100.0 - 1.0e-9)
        .count() as f64
        * 0.1;
    let maximum_pv = exposure
        .iter()
        .map(|sample| sample.pv)
        .fold(f64::NEG_INFINITY, f64::max);
    let post_restore_hold_s = restored
        .iter()
        .take_while(|sample| sample.applied >= 100.0 - 1.0e-9)
        .count() as f64
        * 0.1;
    let recovered_s = restored.iter().enumerate().find_map(|(index, sample)| {
        let dwell_end = index.saturating_add(20);
        (dwell_end <= restored.len()
            && restored[index..dwell_end]
                .iter()
                .all(|future| (future.setpoint - future.pv).abs() <= 0.2))
        .then_some(sample.time_s - 65.0)
    });
    let safe = trace.last().expect("safe-restoration sample");

    eprintln!(
        "Lesson 8: peak request={peak_requested:.3}%, peak saturation error={peak_saturation_error:.3} pp, exposure at upper limit={upper_limit_s:.1}s, post-restore hold={post_restore_hold_s:.1}s, recovery={}",
        recovered_s
            .map(|value| format!("{value:.1}s"))
            .unwrap_or_else(|| "Not recovered".to_owned())
    );
    assert_close(
        (35.0 - 20.0) / 0.2,
        75.0,
        "the achievable target must require 75 percent steady heater power",
    );
    assert_close(
        (45.0 - 20.0) / 0.2,
        125.0,
        "the unreachable target must require 125 percent steady heater power",
    );
    assert!(
        maximum_baseline_mismatch <= 1.0e-6,
        "requested and applied heater power must agree in the qualified baseline; mismatch={maximum_baseline_mismatch}"
    );
    assert!(
        peak_requested > 150.0,
        "unreachable demand must exceed 150%; peak={peak_requested}"
    );
    assert!(
        peak_saturation_error > 50.0,
        "unavailable demand must exceed 50 percentage points; peak={peak_saturation_error}"
    );
    assert!(upper_limit_s >= 9.5,
        "the heater must remain at its upper limit through the bounded exposure; duration={upper_limit_s}");
    assert!(
        maximum_pv < 45.0,
        "the simulated heater must remain below the unreachable setpoint; maximum PV={maximum_pv}"
    );
    assert!(post_restore_hold_s >= 1.0,
        "stored integral demand must keep the heater saturated after SP returns to 35 degC; duration={post_restore_hold_s}");
    assert!(post_restore_hold_s < 30.0,
        "the bounded run must observe release from saturation before safe restoration; duration={post_restore_hold_s}");
    assert!(trace.iter().all(|sample| {
        sample.pv.is_finite()
            && sample.requested.is_finite()
            && sample.applied.is_finite()
            && sample.saturation_error.is_finite()
            && (0.0..=100.0).contains(&sample.applied)
            && (sample.saturation_error - (sample.requested - sample.applied)).abs() <= 1.0e-9
    }), "every retained sample must be finite, physically bounded, and use requested-minus-applied saturation error");
    assert!(!safe.auto, "safe restoration must end in Manual mode");
    assert_close(
        safe.requested,
        0.0,
        "safe restoration must request zero heater power",
    );
    assert_close(
        safe.applied,
        0.0,
        "safe restoration must apply zero heater power",
    );
    write_evidence(&trace, post_restore_hold_s, recovered_s);
}

#[test]
fn control_curriculum_water_heater_saturation_artifacts_identify_and_dispose_the_run() {
    let trace_header = RETAINED_TRACE
        .lines()
        .next()
        .expect("retained trace header");
    for field in [
        "run_id",
        "plant_model",
        "ambient_temperature_degC",
        "steady_heater_gain_degC_per_percent",
        "time_constant_s",
        "dead_time_s",
        "abort_state",
    ] {
        assert!(
            trace_header.split(',').any(|candidate| candidate == field),
            "Lesson 8 retained trace must identify the run and model with field '{field}'"
        );
    }
    let trace_columns = trace_header.split(',').collect::<Vec<_>>();
    let trace_index = |name: &str| {
        trace_columns
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("retained trace field '{name}'"))
    };
    let trace_rows = RETAINED_TRACE.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(
        trace_rows.len(),
        951,
        "retained trace must include safe restoration"
    );
    for row in trace_rows {
        let values = row.split(',').collect::<Vec<_>>();
        assert_eq!(
            values.len(),
            trace_columns.len(),
            "retained trace row width"
        );
        assert_eq!(values[trace_index("run_id")], RUN_ID);
        assert_eq!(values[trace_index("plant_model")], "first_order_dead_time");
        assert_eq!(values[trace_index("ambient_temperature_degC")], "20.0");
        assert_eq!(
            values[trace_index("steady_heater_gain_degC_per_percent")],
            "0.2"
        );
        assert_eq!(values[trace_index("time_constant_s")], "5.0");
        assert_eq!(values[trace_index("dead_time_s")], "1.0");
        assert_eq!(values[trace_index("abort_state")], "none");
    }

    let summary_header = RETAINED_SUMMARY
        .lines()
        .next()
        .expect("retained summary header");
    for field in [
        "run_id",
        "validity",
        "abort_state",
        "safe_restoration",
        "conclusion",
    ] {
        assert!(
            summary_header
                .split(',')
                .any(|candidate| candidate == field),
            "Lesson 8 retained summary must dispose the run with field '{field}'"
        );
    }
    let summary_columns = summary_header.split(',').collect::<Vec<_>>();
    let summary_values = RETAINED_SUMMARY
        .lines()
        .nth(1)
        .expect("retained summary row")
        .split(',')
        .collect::<Vec<_>>();
    let summary_value = |name: &str| {
        let index = summary_columns
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("retained summary field '{name}'"));
        summary_values[index]
    };
    assert_eq!(summary_value("run_id"), RUN_ID);
    assert_eq!(summary_value("validity"), "valid");
    assert_eq!(summary_value("abort_state"), "none");
    assert_eq!(summary_value("safe_restoration"), "true");
    assert_eq!(
        summary_value("conclusion"),
        "external_clamp_protects_actuator_but_unobserved_integral_state_delays_recovery"
    );
}
