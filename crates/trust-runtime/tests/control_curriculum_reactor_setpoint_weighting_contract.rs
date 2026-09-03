use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/12-reactor-setpoint-weighting/src/Main.st");
const SEED: u64 = 20260827;
const WEIGHTS: [f64; 3] = [1.0, 0.5, 0.0];
const SERVO_OVERSHOOT_LIMIT_DEG_C: f64 = 1.0;
const SERVO_OVERSHOOT_TOLERANCE_DEG_C: f64 = 0.01;
const SETTLING_BAND_DEG_C: f64 = 0.5;
const SETTLING_BAND_TOLERANCE_DEG_C: f64 = 0.01;
const REGULATORY_EXCITATION_MIN_DEG_C: f64 = 1.0;
const REGULATORY_EXCITATION_TOLERANCE_DEG_C: f64 = 0.01;
const REGULATORY_RECOVERY_LIMIT_S: f64 = 40.0;
const REGULATORY_RECOVERY_TOLERANCE_S: f64 = 0.1;
const REGULATORY_FINAL_ERROR_LIMIT_DEG_C: f64 = 0.5;
const REGULATORY_FINAL_ERROR_TOLERANCE_DEG_C: f64 = 0.001;
const TRACE_AGREEMENT_TOLERANCE: f64 = 1.0e-10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Objective {
    Servo,
    Regulatory,
}

impl Objective {
    fn name(self) -> &'static str {
        match self {
            Self::Servo => "servo",
            Self::Regulatory => "regulatory",
        }
    }

    fn event_time_s(self) -> f64 {
        match self {
            Self::Servo => 5.0,
            Self::Regulatory => 60.0,
        }
    }

    fn final_auto_time_s(self) -> f64 {
        match self {
            Self::Servo => 55.0,
            Self::Regulatory => 120.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    objective: Objective,
    weight: f64,
    time_s: f64,
    phase: &'static str,
    setpoint: f64,
    pv: f64,
    applied: f64,
    disturbance: f64,
    manual_output: f64,
    auto: bool,
    kp: f64,
    ki: f64,
    kd: f64,
    initialized_weight: f64,
    bias: f64,
    cycle_time_s: f64,
}

#[derive(Clone, Copy, Debug)]
struct Metrics {
    overshoot_deg_c: f64,
    settling_or_recovery_s: f64,
    applied_valve_travel_pp: f64,
    regulatory_peak_deviation_deg_c: f64,
    final_window_mae_deg_c: f64,
}

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 12 program before configuration")
        .replace(
            "    AutoMode : BOOL :=",
            "    AutoMode AT %MX104.0 : BOOL :=",
        )
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %IX96.0 : BOOL :=",
        )
        .replace(
            "    SelectedObjective : INT :=",
            "    SelectedObjective AT %IW88 : INT :=",
        )
        .replace(
            "    SetpointWeight : LREAL :=",
            "    SetpointWeight AT %ML8 : LREAL :=",
        )
        .replace("    Setpoint : LREAL :=", "    Setpoint AT %ML0 : LREAL :=")
        .replace(
            "    ProcessValue : LREAL :=",
            "    ProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "    ManipulatedValue : LREAL :=",
            "    ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace(
            "    ManualOutput : LREAL :=",
            "    ManualOutput AT %ML16 : LREAL :=",
        )
        .replace(
            "    ProportionalGainUsed : LREAL :=",
            "    ProportionalGainUsed AT %ML32 : LREAL :=",
        )
        .replace(
            "    IntegralGainUsed : LREAL :=",
            "    IntegralGainUsed AT %ML40 : LREAL :=",
        )
        .replace(
            "    DerivativeGainUsed : LREAL :=",
            "    DerivativeGainUsed AT %ML48 : LREAL :=",
        )
        .replace(
            "    ProportionalSetpointWeightUsed : LREAL :=",
            "    ProportionalSetpointWeightUsed AT %ML56 : LREAL :=",
        )
        .replace(
            "    BiasUsed : LREAL :=",
            "    BiasUsed AT %ML64 : LREAL :=",
        )
        .replace(
            "    ConfiguredCycleTime : LREAL :=",
            "    ConfiguredCycleTime AT %ML72 : LREAL :=",
        )
}

fn runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("reactor-setpoint-weighting/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 12 Structured Text")
}

fn simulation() -> SimulationController {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/12-reactor-setpoint-weighting/simulation.toml");
    SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load shipped Lesson 12 simulation: {error}")),
    )
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

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("BOOL address"),
            Value::Bool(value),
        )
        .expect("write BOOL process-image value");
}

fn write_int(runtime: &mut Runtime, address: &str, value: i16) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("INT address"),
            Value::Word(value as u16),
        )
        .expect("write INT process-image value");
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

fn sample(
    objective: Objective,
    weight: f64,
    time_s: f64,
    phase: &'static str,
    runtime: &Runtime,
) -> Sample {
    Sample {
        objective,
        weight,
        time_s,
        phase,
        setpoint: read_lreal(runtime, "%ML0"),
        pv: read_lreal(runtime, "%IL0"),
        applied: read_lreal(runtime, "%QL0"),
        disturbance: if objective == Objective::Regulatory && time_s >= 60.0 {
            -5.0
        } else {
            0.0
        },
        manual_output: read_lreal(runtime, "%ML16"),
        auto: read_bool(runtime, "%MX104.0"),
        kp: read_lreal(runtime, "%ML32"),
        ki: read_lreal(runtime, "%ML40"),
        kd: read_lreal(runtime, "%ML48"),
        initialized_weight: read_lreal(runtime, "%ML56"),
        bias: read_lreal(runtime, "%ML64"),
        cycle_time_s: read_lreal(runtime, "%ML72"),
    }
}

fn run(objective: Objective, weight: f64) -> Vec<Sample> {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial reactor state");
    write_lreal(&mut runtime, "%ML8", weight);
    write_int(
        &mut runtime,
        "%IW88",
        if objective == Objective::Servo { 0 } else { 1 },
    );
    write_bool(&mut runtime, "%IX96.0", false);
    runtime
        .execute_cycle()
        .expect("retain initial Manual sample");

    let automatic_cycles = match objective {
        Objective::Servo => 550,
        Objective::Regulatory => 1_200,
    };
    let mut trace = Vec::with_capacity(automatic_cycles + 20);
    trace.push(sample(objective, weight, 0.0, "safe_start", &runtime));
    write_bool(&mut runtime, "%IX96.0", true);

    for cycle in 1..=automatic_cycles {
        let time_s = cycle as f64 * 0.1;
        let phase = match objective {
            Objective::Servo if cycle <= 50 => "servo_pre_event",
            Objective::Servo => "servo_response",
            Objective::Regulatory if cycle <= 600 => "regulatory_pre_event",
            Objective::Regulatory => "regulatory_response",
        };
        runtime.execute_cycle().expect("execute Lesson 12 PI scan");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 12 reactor");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 12 reactor temperature");
        trace.push(sample(objective, weight, time_s, phase, &runtime));
    }

    for restoration_cycle in 1..=30 {
        runtime
            .execute_cycle()
            .expect("restore Lesson 12 Manual zero");
        trace.push(sample(
            objective,
            weight,
            objective.final_auto_time_s() + restoration_cycle as f64 * 0.1,
            "safe_restoration",
            &runtime,
        ));
        if read_lreal(&runtime, "%QL0") == 0.0 {
            break;
        }
    }
    write_bool(&mut runtime, "%IX96.0", false);
    runtime
        .execute_cycle()
        .expect("restore experiment authority false");
    trace
}

fn metrics(objective: Objective, trace: &[Sample]) -> Metrics {
    let event_time = objective.event_time_s();
    let final_time = objective.final_auto_time_s();
    let target = match objective {
        Objective::Servo => 85.0,
        Objective::Regulatory => 80.0,
    };
    let response = trace
        .iter()
        .filter(|sample| {
            sample.auto && sample.time_s >= event_time + 0.1 && sample.time_s <= final_time
        })
        .collect::<Vec<_>>();
    let with_pre_event = trace
        .iter()
        .filter(|sample| sample.auto && sample.time_s >= event_time && sample.time_s <= final_time)
        .collect::<Vec<_>>();
    let last_outside = response.iter().rposition(|sample| {
        (target - sample.pv).abs() > SETTLING_BAND_DEG_C + SETTLING_BAND_TOLERANCE_DEG_C
    });
    let settling_or_recovery_s = match last_outside {
        Some(index) if index + 1 < response.len() => response[index + 1].time_s - event_time,
        None => 0.1,
        _ => f64::INFINITY,
    };
    let final_window = response
        .iter()
        .filter(|sample| sample.time_s >= 110.1 && sample.time_s <= 120.0)
        .collect::<Vec<_>>();
    Metrics {
        overshoot_deg_c: response
            .iter()
            .map(|sample| sample.pv - target)
            .fold(0.0_f64, f64::max),
        settling_or_recovery_s,
        applied_valve_travel_pp: with_pre_event
            .windows(2)
            .map(|pair| (pair[1].applied - pair[0].applied).abs())
            .sum(),
        regulatory_peak_deviation_deg_c: response
            .iter()
            .map(|sample| (target - sample.pv).abs())
            .fold(0.0_f64, f64::max),
        final_window_mae_deg_c: if objective == Objective::Regulatory {
            final_window
                .iter()
                .map(|sample| (target - sample.pv).abs())
                .sum::<f64>()
                / final_window.len() as f64
        } else {
            f64::NAN
        },
    }
}

fn regulatory_disagreement(reference: &[Sample], candidate: &[Sample]) -> (f64, f64) {
    let reference = reference
        .iter()
        .filter(|sample| sample.auto && (60.0..=120.0).contains(&sample.time_s));
    let candidate = candidate
        .iter()
        .filter(|sample| sample.auto && (60.0..=120.0).contains(&sample.time_s));
    reference
        .zip(candidate)
        .fold((0.0_f64, 0.0_f64), |maximum, pair| {
            (
                maximum.0.max((pair.0.pv - pair.1.pv).abs()),
                maximum.1.max((pair.0.applied - pair.1.applied).abs()),
            )
        })
}

fn upper_pass(value: f64, limit: f64, tolerance: f64) -> bool {
    value.is_finite() && value <= limit + tolerance
}

fn lower_pass(value: f64, limit: f64, tolerance: f64) -> bool {
    value.is_finite() && value >= limit - tolerance
}

fn run_id(objective: Objective, weight: f64) -> String {
    format!(
        "lesson-12-{}-b{}-seed-{SEED}",
        objective.name(),
        if weight == 1.0 {
            "1"
        } else if weight == 0.5 {
            "0_5"
        } else {
            "0"
        }
    )
}

fn write_evidence(
    traces: &[Vec<Sample>],
    results: &[Metrics],
    disagreements: &[(f64, f64)],
    selected_weight: f64,
) {
    let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("create Lesson 12 evidence directory");
    let mut trace_csv = String::from(
        "run_id,objective,proportional_setpoint_weight,time_s,phase,event,setpoint_degC,pv_degC,error_degC,manual_mv_percent,applied_mv_percent,cold_feed_equivalent_degC,kp_percent_per_degC,ki_percent_per_degC_s,kd_percent_s_per_degC,initialized_weight,bias_percent,anti_windup,configured_cycle_time_s,tracking_time_s,controller_output_min_percent,controller_output_max_percent,controller_rate_min_pp_s,controller_rate_max_pp_s,pv_min_degC,pv_max_degC,plant_id,plant_model,plant_gain_degC_per_percent,plant_time_constant_s,plant_dead_time_s,plant_initial_delayed_input_percent,plant_output_bias_degC,seed,auto_mode,safe_restoration,abort_state,evidence_class\n",
    );
    for trace in traces {
        for sample in trace {
            let event = match (sample.objective, sample.time_s) {
                (Objective::Servo, time) if (time - 5.1).abs() < 1.0e-9 => "setpoint_step",
                (Objective::Regulatory, time) if (time - 60.0).abs() < 1.0e-9 => {
                    "cold_feed_equivalent"
                }
                _ => "none",
            };
            let safe = if sample.phase == "safe_restoration" {
                if sample.applied == 0.0 {
                    "achieved"
                } else {
                    "in_progress"
                }
            } else {
                "not_applicable"
            };
            let plant_bias = if sample.objective == Objective::Regulatory && sample.time_s >= 60.0 {
                62.5
            } else {
                67.5
            };
            writeln!(
                trace_csv,
                "{},{},{:.1},{:.1},{},{},{:.1},{:.12},{:.12},{:.12},{:.12},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},combined,{:.1},1.0,0.0,100.0,-50.0,50.0,60.0,100.0,jacketed-reactor-temperature,first_order_dead_time,0.25,12.0,1.5,50.0,{plant_bias:.1},{SEED},{},{safe},none,simulation",
                run_id(sample.objective, sample.weight),
                sample.objective.name(),
                sample.weight,
                sample.time_s,
                sample.phase,
                event,
                sample.setpoint,
                sample.pv,
                sample.setpoint - sample.pv,
                sample.manual_output,
                sample.applied,
                sample.disturbance,
                sample.kp,
                sample.ki,
                sample.kd,
                sample.initialized_weight,
                sample.bias,
                sample.cycle_time_s,
                sample.auto,
            )
            .expect("append Lesson 12 trace row");
        }
    }
    std::fs::write(directory.join("setpoint-weighting-trace.csv"), trace_csv)
        .expect("write Lesson 12 trace");

    let mut summary = String::from(
        "run_id,objective,proportional_setpoint_weight,controller_id,kp_percent_per_degC,ki_percent_per_degC_s,kd_percent_s_per_degC,bias_percent,anti_windup,configured_cycle_time_s,tracking_time_s,output_min_percent,output_max_percent,rate_min_pp_s,rate_max_pp_s,plant_id,plant_model,plant_gain_degC_per_percent,plant_time_constant_s,plant_dead_time_s,plant_initial_delayed_input_percent,plant_initial_output_bias_degC,plant_event_output_bias_degC,seed,event_time_s,analysis_start_s,analysis_end_s,settling_band_degC,settling_band_tolerance_degC,servo_overshoot_degC,servo_overshoot_limit_degC,servo_overshoot_tolerance_degC,servo_overshoot_pass,settling_or_recovery_s,settling_reproduction_tolerance_s,total_applied_valve_travel_pp,travel_reproduction_tolerance_pp,regulatory_peak_deviation_degC,regulatory_excitation_min_degC,regulatory_excitation_tolerance_degC,regulatory_excitation_pass,regulatory_recovery_limit_s,regulatory_recovery_tolerance_s,regulatory_recovery_pass,final_window_start_s,final_window_end_s,final_window_mae_degC,final_window_limit_degC,final_window_tolerance_degC,final_window_pass,max_aligned_pv_delta_degC,max_aligned_mv_delta_pp,trace_agreement_tolerance,controlled_settings_equal,numerically_valid,physical_criterion_passed,regulatory_equivalent,selected,safe_restoration,abort_state,evidence_class\n",
    );
    for index in 0..traces.len() {
        let first = traces[index].first().expect("Lesson 12 run");
        let result = results[index];
        let servo_pass = upper_pass(
            result.overshoot_deg_c,
            SERVO_OVERSHOOT_LIMIT_DEG_C,
            SERVO_OVERSHOOT_TOLERANCE_DEG_C,
        );
        let excitation_pass = lower_pass(
            result.regulatory_peak_deviation_deg_c,
            REGULATORY_EXCITATION_MIN_DEG_C,
            REGULATORY_EXCITATION_TOLERANCE_DEG_C,
        );
        let recovery_pass = upper_pass(
            result.settling_or_recovery_s,
            REGULATORY_RECOVERY_LIMIT_S,
            REGULATORY_RECOVERY_TOLERANCE_S,
        );
        let final_pass = upper_pass(
            result.final_window_mae_deg_c,
            REGULATORY_FINAL_ERROR_LIMIT_DEG_C,
            REGULATORY_FINAL_ERROR_TOLERANCE_DEG_C,
        );
        let disagreement = disagreements[index];
        let regulatory_equivalent = first.objective == Objective::Servo
            || (disagreement.0 <= TRACE_AGREEMENT_TOLERANCE
                && disagreement.1 <= TRACE_AGREEMENT_TOLERANCE);
        let physical_pass = if first.objective == Objective::Servo {
            servo_pass
        } else {
            excitation_pass && recovery_pass && final_pass
        };
        writeln!(
            summary,
            "{},{},{:.1},Control.PID.PI,8.0,2.0,0.0,50.0,combined,0.1,1.0,0.0,100.0,-50.0,50.0,jacketed-reactor-temperature,first_order_dead_time,0.25,12.0,1.5,50.0,67.5,{},{SEED},{:.1},{:.1},{:.1},0.5,0.01,{},{},{},{},{:.1},0.1,{:.12},0.01,{},{},{},{},{},{},{},{},{},{},{},{},{},{:.12},{:.12},1e-10,true,true,{physical_pass},{regulatory_equivalent},{},achieved,none,simulation",
            run_id(first.objective, first.weight),
            first.objective.name(),
            first.weight,
            if first.objective == Objective::Regulatory { "62.5" } else { "" },
            first.objective.event_time_s(),
            first.objective.event_time_s() + 0.1,
            first.objective.final_auto_time_s(),
            if first.objective == Objective::Servo { format!("{:.12}", result.overshoot_deg_c) } else { String::new() },
            if first.objective == Objective::Servo { "1.0" } else { "" },
            if first.objective == Objective::Servo { "0.01" } else { "" },
            if first.objective == Objective::Servo { servo_pass.to_string() } else { String::new() },
            result.settling_or_recovery_s,
            result.applied_valve_travel_pp,
            if first.objective == Objective::Regulatory { format!("{:.12}", result.regulatory_peak_deviation_deg_c) } else { String::new() },
            if first.objective == Objective::Regulatory { "1.0" } else { "" },
            if first.objective == Objective::Regulatory { "0.01" } else { "" },
            if first.objective == Objective::Regulatory { excitation_pass.to_string() } else { String::new() },
            if first.objective == Objective::Regulatory { "40.0" } else { "" },
            if first.objective == Objective::Regulatory { "0.1" } else { "" },
            if first.objective == Objective::Regulatory { recovery_pass.to_string() } else { String::new() },
            if first.objective == Objective::Regulatory { "110.1" } else { "" },
            if first.objective == Objective::Regulatory { "120.0" } else { "" },
            if first.objective == Objective::Regulatory { format!("{:.12}", result.final_window_mae_deg_c) } else { String::new() },
            if first.objective == Objective::Regulatory { "0.5" } else { "" },
            if first.objective == Objective::Regulatory { "0.001" } else { "" },
            if first.objective == Objective::Regulatory { final_pass.to_string() } else { String::new() },
            disagreement.0,
            disagreement.1,
            first.objective == Objective::Servo && first.weight == selected_weight,
        )
        .expect("append Lesson 12 summary row");
    }
    std::fs::write(directory.join("setpoint-weighting-summary.csv"), summary)
        .expect("write Lesson 12 summary");
}

#[test]
fn lesson_12_uses_a_distinct_manual_request_and_rate_constrained_safe_restoration() {
    let mut runtime = runtime();
    write_lreal(&mut runtime, "%ML0", 80.0);
    write_lreal(&mut runtime, "%IL0", 80.0);
    write_lreal(&mut runtime, "%ML8", 0.5);
    write_lreal(&mut runtime, "%ML16", 50.0);
    write_bool(&mut runtime, "%MX104.0", false);
    runtime
        .execute_cycle()
        .expect("establish Manual operating point");
    assert_eq!(read_lreal(&runtime, "%QL0"), 50.0);
    assert_eq!(read_lreal(&runtime, "%ML32"), 8.0);
    assert_eq!(read_lreal(&runtime, "%ML40"), 2.0);
    assert_eq!(read_lreal(&runtime, "%ML48"), 0.0);
    assert_eq!(read_lreal(&runtime, "%ML56"), 0.5);
    assert_eq!(read_lreal(&runtime, "%ML64"), 50.0);
    assert_eq!(read_lreal(&runtime, "%ML72"), 0.1);

    write_lreal(&mut runtime, "%ML16", 0.0);
    runtime
        .execute_cycle()
        .expect("begin reviewed Manual restoration");
    assert_eq!(read_lreal(&runtime, "%QL0"), 45.0);
    for _ in 0..9 {
        runtime
            .execute_cycle()
            .expect("continue Manual restoration");
    }
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
}

#[test]
fn lesson_12_project_executes_the_selected_weight_servo_sequence() {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial reactor state");
    write_lreal(&mut runtime, "%ML8", 0.5);
    write_bool(&mut runtime, "%IX96.0", false);
    runtime.execute_cycle().expect("retain safe initial sample");
    assert!(!read_bool(&runtime, "%MX104.0"));
    assert_eq!(read_lreal(&runtime, "%ML0"), 80.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 50.0);

    write_bool(&mut runtime, "%IX96.0", true);
    assert!(read_bool(&runtime, "%IX96.0"));
    for cycle in 1..=551 {
        runtime.execute_cycle().expect("execute project servo scan");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance project reactor");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish project reactor temperature");
        if cycle == 1 || cycle == 50 {
            assert!(
                read_bool(&runtime, "%MX104.0"),
                "cycle={cycle} experiment={} setpoint={} manual={}",
                read_bool(&runtime, "%IX96.0"),
                read_lreal(&runtime, "%ML0"),
                read_lreal(&runtime, "%ML16")
            );
            assert_eq!(read_lreal(&runtime, "%ML0"), 80.0);
        }
        if cycle == 51 || cycle == 550 {
            assert!(read_bool(&runtime, "%MX104.0"));
            assert_eq!(read_lreal(&runtime, "%ML0"), 85.0);
        }
    }
    assert!(!read_bool(&runtime, "%MX104.0"));
    assert_eq!(read_lreal(&runtime, "%ML0"), 80.0);
    assert_eq!(read_lreal(&runtime, "%ML16"), 0.0);

    write_bool(&mut runtime, "%IX96.0", false);
    for _ in 0..14 {
        runtime
            .execute_cycle()
            .expect("complete project restoration");
    }
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
}

#[test]
fn lesson_12_executes_six_qualified_runs_and_selects_the_largest_eligible_weight() {
    let mut traces = Vec::new();
    for objective in [Objective::Servo, Objective::Regulatory] {
        for weight in WEIGHTS {
            traces.push(run(objective, weight));
        }
    }
    let results = traces
        .iter()
        .map(|trace| metrics(trace[0].objective, trace))
        .collect::<Vec<_>>();
    for trace in &traces {
        let initial = trace.first().expect("initial Manual sample");
        assert!(!initial.auto && initial.time_s == 0.0 && initial.applied == 50.0);
        assert!(trace.iter().filter(|sample| sample.auto).all(|sample| {
            sample.pv.is_finite()
                && sample.applied.is_finite()
                && (60.0..=100.0).contains(&sample.pv)
                && (0.0..=100.0).contains(&sample.applied)
                && sample.kp == 8.0
                && sample.ki == 2.0
                && sample.kd == 0.0
                && sample.initialized_weight == initial.weight
                && sample.bias == 50.0
                && sample.cycle_time_s == 0.1
        }));
        let safe = trace.last().expect("final Manual restoration sample");
        assert!(!safe.auto && safe.manual_output == 0.0 && safe.applied == 0.0);
    }

    let servo_passes = results[0..3]
        .iter()
        .map(|result| {
            upper_pass(
                result.overshoot_deg_c,
                SERVO_OVERSHOOT_LIMIT_DEG_C,
                SERVO_OVERSHOOT_TOLERANCE_DEG_C,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(servo_passes, vec![false, true, true]);
    let selected_weight = WEIGHTS
        .into_iter()
        .zip(&servo_passes)
        .filter(|(_, passes)| **passes)
        .map(|(weight, _)| weight)
        .fold(f64::NEG_INFINITY, f64::max);
    assert_eq!(selected_weight, 0.5);

    let mut disagreements = vec![(0.0, 0.0); 6];
    disagreements[4] = regulatory_disagreement(&traces[3], &traces[4]);
    disagreements[5] = regulatory_disagreement(&traces[3], &traces[5]);
    for index in 3..6 {
        assert!(lower_pass(
            results[index].regulatory_peak_deviation_deg_c,
            REGULATORY_EXCITATION_MIN_DEG_C,
            REGULATORY_EXCITATION_TOLERANCE_DEG_C,
        ));
        assert!(upper_pass(
            results[index].settling_or_recovery_s,
            REGULATORY_RECOVERY_LIMIT_S,
            REGULATORY_RECOVERY_TOLERANCE_S,
        ));
        assert!(upper_pass(
            results[index].final_window_mae_deg_c,
            REGULATORY_FINAL_ERROR_LIMIT_DEG_C,
            REGULATORY_FINAL_ERROR_TOLERANCE_DEG_C,
        ));
        assert!(disagreements[index].0 <= TRACE_AGREEMENT_TOLERANCE);
        assert!(disagreements[index].1 <= TRACE_AGREEMENT_TOLERANCE);
    }
    eprintln!("Lesson 12 metrics: {results:#?}");
    eprintln!("Lesson 12 regulatory disagreements: {disagreements:?}");
    write_evidence(&traces, &results, &disagreements, selected_weight);
}

#[test]
fn lesson_12_retained_artifacts_preserve_six_run_decisions() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/12-reactor-setpoint-weighting/figures");
    let trace_path = directory.join("setpoint-weighting-trace.csv");
    let summary_path = directory.join("setpoint-weighting-summary.csv");
    assert!(
        trace_path.is_file() && summary_path.is_file(),
        "Lesson 12 must retain the native six-run trace and qualification summary"
    );
    let trace = std::fs::read_to_string(trace_path).expect("read Lesson 12 trace");
    let trace_header = trace.lines().next().expect("Lesson 12 trace header");
    for field in [
        "objective",
        "proportional_setpoint_weight",
        "initialized_weight",
        "error_degC",
        "manual_mv_percent",
        "applied_mv_percent",
        "plant_model",
        "safe_restoration",
        "evidence_class",
    ] {
        assert!(trace_header.split(',').any(|candidate| candidate == field));
    }
    assert_eq!(
        trace
            .lines()
            .skip(1)
            .filter(|line| line.contains(",servo,"))
            .count()
            > 0,
        true
    );
    assert_eq!(
        trace
            .lines()
            .skip(1)
            .filter(|line| line.contains(",regulatory,"))
            .count()
            > 0,
        true
    );
    assert_eq!(
        trace.matches(",simulation\n").count(),
        trace.lines().count() - 1
    );

    let summary = std::fs::read_to_string(summary_path).expect("read Lesson 12 summary");
    let mut lines = summary.lines();
    let header = lines
        .next()
        .expect("Lesson 12 summary header")
        .split(',')
        .collect::<Vec<_>>();
    let column = |name: &str| {
        header
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("Lesson 12 summary field '{name}'"))
    };
    for field in [
        "servo_overshoot_limit_degC",
        "servo_overshoot_tolerance_degC",
        "settling_band_degC",
        "settling_band_tolerance_degC",
        "regulatory_recovery_limit_s",
        "regulatory_recovery_tolerance_s",
        "final_window_mae_degC",
        "final_window_tolerance_degC",
        "max_aligned_pv_delta_degC",
        "max_aligned_mv_delta_pp",
        "trace_agreement_tolerance",
        "controlled_settings_equal",
        "physical_criterion_passed",
        "regulatory_equivalent",
        "selected",
        "safe_restoration",
    ] {
        column(field);
    }
    let rows = lines
        .map(|line| line.split(',').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 6);
    assert_eq!(
        rows.iter()
            .filter(|row| row[column("selected")] == "true")
            .count(),
        1
    );
    let selected = rows
        .iter()
        .find(|row| row[column("selected")] == "true")
        .expect("selected Lesson 12 row");
    assert_eq!(selected[column("objective")], "servo");
    assert_eq!(selected[column("proportional_setpoint_weight")], "0.5");
    assert!(rows
        .iter()
        .all(|row| row[column("safe_restoration")] == "achieved"));
}
