use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::Duration;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/11-reactor-servo-regulatory/src/Main.st");
const RETAINED_TRACE: &str = include_str!(
    "../../../examples/control/01-feedback-foundations-and-temperature-control/11-reactor-servo-regulatory/figures/servo-regulatory-trace.csv"
);
const RETAINED_SUMMARY: &str = include_str!(
    "../../../examples/control/01-feedback-foundations-and-temperature-control/11-reactor-servo-regulatory/figures/servo-regulatory-summary.csv"
);
const SEED: u64 = 20260827;
const SERVO_OVERSHOOT_LIMIT_DEG_C: f64 = 1.0;
const SERVO_OVERSHOOT_TOLERANCE_DEG_C: f64 = 0.01;
const REGULATORY_EXCITATION_MIN_DEG_C: f64 = 1.0;
const REGULATORY_EXCITATION_TOLERANCE_DEG_C: f64 = 0.01;
const REGULATORY_RECOVERY_LIMIT_S: f64 = 40.0;
const REGULATORY_RECOVERY_TOLERANCE_S: f64 = 0.1;
const REGULATORY_FINAL_ERROR_LIMIT_DEG_C: f64 = 0.5;
const REGULATORY_FINAL_ERROR_TOLERANCE_DEG_C: f64 = 0.001;
const SETTLING_BAND_DEG_C: f64 = 0.5;
const SETTLING_BAND_TOLERANCE_DEG_C: f64 = 0.01;

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

    fn final_automatic_time_s(self) -> f64 {
        match self {
            Self::Servo => 55.0,
            Self::Regulatory => 120.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    objective: Objective,
    time_s: f64,
    phase: &'static str,
    setpoint: f64,
    pv: f64,
    applied: f64,
    cold_feed_equivalent: f64,
    kp: f64,
    ki: f64,
    kd: f64,
    proportional_setpoint_weight: f64,
    bias: f64,
    cycle_time_s: f64,
    manual_output: f64,
    auto: bool,
}

#[derive(Clone, Copy, Debug)]
struct Metrics {
    overshoot_deg_c: f64,
    peak_deviation_deg_c: f64,
    settling_or_recovery_s: f64,
    final_window_mean_absolute_error_deg_c: f64,
    iae_deg_c_s: f64,
    ise_deg_c2_s: f64,
    itae_deg_c_s2: f64,
    applied_valve_travel_pp: f64,
}

fn upper_bound_pass(value: f64, limit: f64, tolerance: f64) -> bool {
    value.is_finite() && value <= limit + tolerance
}

fn lower_bound_pass(value: f64, limit: f64, tolerance: f64) -> bool {
    value.is_finite() && value >= limit - tolerance
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

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 11 program before configuration")
        .replace(
            "    AutoMode : BOOL :=",
            "    AutoMode AT %MX72.0 : BOOL :=",
        )
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %IX80.0 : BOOL :=",
        )
        .replace(
            "    SelectedObjective : INT :=",
            "    SelectedObjective AT %IW64 : INT :=",
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
            "    ManualOutput AT %ML56 : LREAL :=",
        )
        .replace(
            "    ProportionalGainUsed : LREAL :=",
            "    ProportionalGainUsed AT %ML8 : LREAL :=",
        )
        .replace(
            "    IntegralGainUsed : LREAL :=",
            "    IntegralGainUsed AT %ML16 : LREAL :=",
        )
        .replace(
            "    DerivativeGainUsed : LREAL :=",
            "    DerivativeGainUsed AT %ML24 : LREAL :=",
        )
        .replace(
            "    ProportionalSetpointWeightUsed : LREAL :=",
            "    ProportionalSetpointWeightUsed AT %ML32 : LREAL :=",
        )
        .replace(
            "    BiasUsed : LREAL :=",
            "    BiasUsed AT %ML40 : LREAL :=",
        )
        .replace(
            "    ConfiguredCycleTime : LREAL :=",
            "    ConfiguredCycleTime AT %ML48 : LREAL :=",
        )
}

fn lesson_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("reactor-servo-regulatory/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 11 Structured Text")
}

fn lesson_simulation() -> SimulationController {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/11-reactor-servo-regulatory/simulation.toml");
    SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load shipped Lesson 11 simulation: {error}")),
    )
}

fn sample(objective: Objective, time_s: f64, phase: &'static str, runtime: &Runtime) -> Sample {
    Sample {
        objective,
        time_s,
        phase,
        setpoint: read_lreal(runtime, "%ML0"),
        pv: read_lreal(runtime, "%IL0"),
        applied: read_lreal(runtime, "%QL0"),
        cold_feed_equivalent: if objective == Objective::Regulatory && time_s >= 60.0 {
            -5.0
        } else {
            0.0
        },
        kp: read_lreal(runtime, "%ML8"),
        ki: read_lreal(runtime, "%ML16"),
        kd: read_lreal(runtime, "%ML24"),
        proportional_setpoint_weight: read_lreal(runtime, "%ML32"),
        bias: read_lreal(runtime, "%ML40"),
        cycle_time_s: read_lreal(runtime, "%ML48"),
        manual_output: read_lreal(runtime, "%ML56"),
        auto: read_bool(runtime, "%MX72.0"),
    }
}

fn run_objective(objective: Objective) -> Vec<Sample> {
    let mut runtime = lesson_runtime();
    let mut simulation = lesson_simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 11 reactor state");

    write_int(
        &mut runtime,
        "%IW64",
        match objective {
            Objective::Servo => 0,
            Objective::Regulatory => 1,
        },
    );
    write_bool(&mut runtime, "%IX80.0", false);
    runtime
        .execute_cycle()
        .expect("execute initial Lesson 11 Manual scan");

    let automatic_cycles = match objective {
        Objective::Servo => 550,
        Objective::Regulatory => 1_200,
    };
    let mut trace = Vec::with_capacity(automatic_cycles + 2);
    trace.push(sample(objective, 0.0, "safe_start", &runtime));

    write_bool(&mut runtime, "%IX80.0", true);
    for cycle in 1..=automatic_cycles {
        let time_s = cycle as f64 * 0.1;
        let phase = match objective {
            Objective::Servo if cycle <= 50 => "servo_pre_event",
            Objective::Servo => "servo_response",
            Objective::Regulatory if cycle <= 600 => "regulatory_pre_event",
            Objective::Regulatory => "regulatory_response",
        };
        runtime.execute_cycle().expect("execute Lesson 11 PI scan");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 11 reactor");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 11 reactor temperature");
        trace.push(sample(objective, time_s, phase, &runtime));
    }

    for restoration_cycle in 1..=30 {
        runtime
            .execute_cycle()
            .expect("restore Lesson 11 Manual zero");
        trace.push(sample(
            objective,
            objective.final_automatic_time_s() + restoration_cycle as f64 * 0.1,
            "safe_restoration",
            &runtime,
        ));
        if read_lreal(&runtime, "%QL0") == 0.0 {
            break;
        }
    }
    trace
}

fn metrics(objective: Objective, trace: &[Sample]) -> Metrics {
    let event_time_s = objective.event_time_s();
    let final_time_s = objective.final_automatic_time_s();
    let response = trace
        .iter()
        .filter(|sample| {
            sample.auto && sample.time_s >= event_time_s + 0.1 && sample.time_s <= final_time_s
        })
        .collect::<Vec<_>>();
    let with_pre_event = trace
        .iter()
        .filter(|sample| {
            sample.auto && sample.time_s >= event_time_s && sample.time_s <= final_time_s
        })
        .collect::<Vec<_>>();
    let target = match objective {
        Objective::Servo => 85.0,
        Objective::Regulatory => 80.0,
    };
    let last_outside = response
        .iter()
        .rposition(|sample| {
            (target - sample.pv).abs() > SETTLING_BAND_DEG_C + SETTLING_BAND_TOLERANCE_DEG_C
        })
        .unwrap_or_else(|| {
            panic!(
                "Lesson 11 {objective:?} response must initially leave its tolerance band; setpoints={:?}; pv_range={:?}",
                response.iter().map(|sample| sample.setpoint).fold((f64::INFINITY, f64::NEG_INFINITY), |range, value| (range.0.min(value), range.1.max(value))),
                response.iter().map(|sample| sample.pv).fold((f64::INFINITY, f64::NEG_INFINITY), |range, value| (range.0.min(value), range.1.max(value)))
            )
        });
    let settling_or_recovery_s = if last_outside + 1 < response.len() {
        response[last_outside + 1].time_s - event_time_s
    } else {
        f64::INFINITY
    };
    let final_window = response
        .iter()
        .filter(|sample| sample.time_s >= 110.1 && sample.time_s <= 120.0)
        .collect::<Vec<_>>();
    let final_window_mean_absolute_error_deg_c = if objective == Objective::Regulatory {
        final_window
            .iter()
            .map(|sample| (target - sample.pv).abs())
            .sum::<f64>()
            / final_window.len() as f64
    } else {
        f64::NAN
    };
    Metrics {
        overshoot_deg_c: response
            .iter()
            .map(|sample| sample.pv - target)
            .fold(0.0_f64, f64::max),
        peak_deviation_deg_c: response
            .iter()
            .map(|sample| (target - sample.pv).abs())
            .fold(0.0_f64, f64::max),
        settling_or_recovery_s,
        final_window_mean_absolute_error_deg_c,
        iae_deg_c_s: response
            .iter()
            .map(|sample| (target - sample.pv).abs() * 0.1)
            .sum(),
        ise_deg_c2_s: response
            .iter()
            .map(|sample| (target - sample.pv).powi(2) * 0.1)
            .sum(),
        itae_deg_c_s2: response
            .iter()
            .map(|sample| (sample.time_s - event_time_s) * (target - sample.pv).abs() * 0.1)
            .sum(),
        applied_valve_travel_pp: with_pre_event
            .windows(2)
            .map(|samples| (samples[1].applied - samples[0].applied).abs())
            .sum(),
    }
}

fn write_evidence(traces: &[Vec<Sample>], results: &[Metrics]) {
    let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("create Lesson 11 evidence directory");
    let mut csv = String::from(
        "run_id,objective,time_s,phase,setpoint_degC,pv_degC,error_degC,manual_mv_percent,applied_mv_percent,cold_feed_equivalent_degC,kp_percent_per_degC,ki_percent_per_degC_s,kd_percent_s_per_degC,proportional_setpoint_weight,bias_percent,anti_windup,configured_cycle_time_s,controller_rate_min_pp_s,controller_rate_max_pp_s,pv_min_degC,pv_max_degC,mv_min_percent,mv_max_percent,plant_id,plant_model,plant_gain_degC_per_percent,plant_time_constant_s,plant_dead_time_s,plant_initial_delayed_input_percent,plant_output_bias_degC,seed,auto_mode,safe_restoration,abort_state,evidence_class\n",
    );
    for trace in traces {
        for sample in trace {
            let safe_restoration = if sample.phase == "safe_restoration" {
                if sample.applied == 0.0 {
                    "achieved"
                } else {
                    "in_progress"
                }
            } else {
                "not_applicable"
            };
            let plant_output_bias =
                if sample.objective == Objective::Regulatory && sample.time_s >= 60.0 {
                    62.5
                } else {
                    67.5
                };
            writeln!(
                csv,
                "lesson-11-{}-seed-{SEED},{},{:.1},{},{:.1},{:.12},{:.12},{:.12},{:.12},{:.12},{:.1},{:.1},{:.1},{:.1},{:.1},combined,{:.1},-50.0,50.0,60.0,100.0,0.0,100.0,jacketed-reactor-temperature,first_order_dead_time,0.25,12.0,1.5,50.0,{plant_output_bias:.1},{SEED},{},{safe_restoration},none,simulation",
                sample.objective.name(),
                sample.objective.name(),
                sample.time_s,
                sample.phase,
                sample.setpoint,
                sample.pv,
                sample.setpoint - sample.pv,
                sample.manual_output,
                sample.applied,
                sample.cold_feed_equivalent,
                sample.kp,
                sample.ki,
                sample.kd,
                sample.proportional_setpoint_weight,
                sample.bias,
                sample.cycle_time_s,
                sample.auto,
            )
            .expect("append Lesson 11 trace row");
        }
    }
    std::fs::write(directory.join("servo-regulatory-trace.csv"), csv)
        .expect("write Lesson 11 trace");

    let mut summary = String::from(
        "run_id,objective,controller_id,kp_percent_per_degC,ki_percent_per_degC_s,kd_percent_s_per_degC,proportional_setpoint_weight,bias_percent,anti_windup,controller_output_min_percent,controller_output_max_percent,controller_rate_min_pp_s,controller_rate_max_pp_s,configured_cycle_time_s,plant_id,plant_model,plant_gain_degC_per_percent,plant_time_constant_s,plant_dead_time_s,plant_initial_delayed_input_percent,plant_initial_output_bias_degC,plant_event_output_bias_degC,event_time_s,analysis_start_s,analysis_end_s,overshoot_degC,peak_deviation_degC,settling_or_recovery_s,final_window_start_s,final_window_end_s,final_window_mean_absolute_error_degC,IAE_degC_s,ISE_degC2_s,ITAE_degC_s2,total_applied_valve_travel_pp,safe_restoration,numerically_valid,servo_overshoot_limit_degC,servo_overshoot_tolerance_degC,servo_overshoot_pass,regulatory_excitation_min_degC,regulatory_excitation_tolerance_degC,regulatory_excitation_pass,regulatory_recovery_limit_s,regulatory_recovery_tolerance_s,regulatory_recovery_pass,regulatory_final_error_limit_degC,regulatory_final_error_tolerance_degC,regulatory_final_error_pass,physical_criterion_passed,selected,abort_state,conclusion,evidence_class,settling_band_degC,settling_band_tolerance_degC,servo_settling_reproduction_tolerance_s\n",
    );
    for (objective, result) in [Objective::Servo, Objective::Regulatory]
        .into_iter()
        .zip(results)
    {
        let servo_pass = upper_bound_pass(
            result.overshoot_deg_c,
            SERVO_OVERSHOOT_LIMIT_DEG_C,
            SERVO_OVERSHOOT_TOLERANCE_DEG_C,
        );
        let excitation_pass = lower_bound_pass(
            result.peak_deviation_deg_c,
            REGULATORY_EXCITATION_MIN_DEG_C,
            REGULATORY_EXCITATION_TOLERANCE_DEG_C,
        );
        let recovery_pass = upper_bound_pass(
            result.settling_or_recovery_s,
            REGULATORY_RECOVERY_LIMIT_S,
            REGULATORY_RECOVERY_TOLERANCE_S,
        );
        let final_error_pass = upper_bound_pass(
            result.final_window_mean_absolute_error_deg_c,
            REGULATORY_FINAL_ERROR_LIMIT_DEG_C,
            REGULATORY_FINAL_ERROR_TOLERANCE_DEG_C,
        );
        let (criterion, conclusion) = match objective {
            Objective::Servo => (
                servo_pass,
                if servo_pass {
                    "servo_overshoot_requirement_passed"
                } else {
                    "servo_overshoot_requirement_failed"
                },
            ),
            Objective::Regulatory => (
                excitation_pass && recovery_pass && final_error_pass,
                if excitation_pass && recovery_pass && final_error_pass {
                    "regulatory_requirements_passed"
                } else {
                    "regulatory_requirements_failed"
                },
            ),
        };
        let final_window = if objective == Objective::Regulatory {
            "110.1,120.0"
        } else {
            ","
        };
        writeln!(
            summary,
            "lesson-11-{}-seed-{SEED},{},Control.PID.PI,8.0,2.0,0.0,1.0,50.0,combined,0.0,100.0,-50.0,50.0,0.1,jacketed-reactor-temperature,first_order_dead_time,0.25,12.0,1.5,50.0,67.5,{},{:.1},{:.1},{:.1},{:.12},{:.12},{:.1},{final_window},{},{:.12},{:.12},{:.12},{:.12},achieved,true,{},{},{},{},{},{},{},{},{},{},{},{},{criterion},false,none,{conclusion},simulation,0.5,0.01,0.1",
                objective.name(),
                objective.name(),
                if objective == Objective::Regulatory { "62.5" } else { "" },
                objective.event_time_s(),
                objective.event_time_s() + 0.1,
                objective.final_automatic_time_s(),
            result.overshoot_deg_c,
            result.peak_deviation_deg_c,
            result.settling_or_recovery_s,
            if result.final_window_mean_absolute_error_deg_c.is_nan() { String::new() } else { format!("{:.12}", result.final_window_mean_absolute_error_deg_c) },
            result.iae_deg_c_s,
                result.ise_deg_c2_s,
                result.itae_deg_c_s2,
                result.applied_valve_travel_pp,
                if objective == Objective::Servo { "1.0" } else { "" },
                if objective == Objective::Servo { "0.01" } else { "" },
                if objective == Objective::Servo { servo_pass.to_string() } else { String::new() },
                if objective == Objective::Regulatory { "1.0" } else { "" },
                if objective == Objective::Regulatory { "0.01" } else { "" },
                if objective == Objective::Regulatory { excitation_pass.to_string() } else { String::new() },
                if objective == Objective::Regulatory { "40.0" } else { "" },
                if objective == Objective::Regulatory { "0.1" } else { "" },
                if objective == Objective::Regulatory { recovery_pass.to_string() } else { String::new() },
                if objective == Objective::Regulatory { "0.5" } else { "" },
                if objective == Objective::Regulatory { "0.001" } else { "" },
                if objective == Objective::Regulatory { final_error_pass.to_string() } else { String::new() },
            )
        .expect("append Lesson 11 summary row");
    }
    std::fs::write(directory.join("servo-regulatory-summary.csv"), summary)
        .expect("write Lesson 11 summary");
}

#[test]
fn control_curriculum_reactor_exposes_the_unchanged_pi_objective_settings() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("reactor-servo-regulatory/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 11 Structured Text");

    write_lreal(&mut runtime, "%ML0", 80.0);
    write_lreal(&mut runtime, "%IL0", 80.0);
    write_lreal(&mut runtime, "%QL0", 50.0);
    write_bool(&mut runtime, "%IX80.0", false);
    runtime.execute_cycle().expect("initialize Lesson 11 PI");

    assert_eq!(read_lreal(&runtime, "%ML8"), 8.0);
    assert_eq!(read_lreal(&runtime, "%ML16"), 2.0);
    assert_eq!(read_lreal(&runtime, "%ML24"), 0.0);
    assert_eq!(read_lreal(&runtime, "%ML32"), 1.0);
    assert_eq!(read_lreal(&runtime, "%ML40"), 50.0);
    assert_eq!(read_lreal(&runtime, "%ML48"), 0.1);
}

#[test]
fn control_curriculum_reactor_project_executes_the_servo_sequence() {
    let mut runtime = lesson_runtime();
    write_lreal(&mut runtime, "%IL0", 80.0);
    write_bool(&mut runtime, "%IX80.0", false);
    runtime.execute_cycle().expect("retain Lesson 11 safe start");
    assert_eq!(read_lreal(&runtime, "%ML0"), 80.0);

    write_bool(&mut runtime, "%IX80.0", true);
    for _ in 1..=51 {
        runtime
            .execute_cycle()
            .expect("execute shipped Lesson 11 servo sequence");
    }
    assert_eq!(read_lreal(&runtime, "%ML0"), 85.0);
}

#[test]
fn control_curriculum_reactor_retains_separate_servo_and_regulatory_evidence() {
    let traces = [
        run_objective(Objective::Servo),
        run_objective(Objective::Regulatory),
    ];
    let results = [
        metrics(Objective::Servo, &traces[0]),
        metrics(Objective::Regulatory, &traces[1]),
    ];

    assert_eq!(traces[0].len(), 565);
    assert_eq!(traces[1].len(), 1_215);
    for trace in &traces {
        let initial = trace.first().expect("initial Manual sample");
        assert!(!initial.auto && initial.time_s == 0.0 && initial.applied == 50.0);
        assert!(trace
            .iter()
            .filter(|sample| sample.phase != "safe_start" && sample.phase != "safe_restoration")
            .all(|sample| {
                sample.auto
                    && sample.pv.is_finite()
                    && sample.applied.is_finite()
                    && (60.0..=100.0).contains(&sample.pv)
                    && (0.0..=100.0).contains(&sample.applied)
                    && sample.kp == 8.0
                    && sample.ki == 2.0
                    && sample.kd == 0.0
                    && sample.proportional_setpoint_weight == 1.0
                    && sample.bias == 50.0
                    && sample.cycle_time_s == 0.1
            }));
        assert!(trace
            .iter()
            .filter(|sample| sample.phase == "safe_restoration")
            .all(|sample| !sample.auto && sample.manual_output == 0.0));
        let safe = trace.last().expect("safe restoration sample");
        assert!(
            !safe.auto && safe.setpoint == 80.0 && safe.applied == 0.0,
            "safe restoration must be Manual at SP 80 degC and zero valve; sample={safe:?}"
        );
    }
    assert!((results[0].overshoot_deg_c - 1.049273342257).abs() <= 1.0e-12);
    assert!((results[0].settling_or_recovery_s - 19.7).abs() <= 1.0e-12);
    assert!((results[0].iae_deg_c_s - 36.769637447556).abs() <= 1.0e-12);
    assert!((results[0].ise_deg_c2_s - 107.293092341897).abs() <= 1.0e-12);
    assert!((results[0].itae_deg_c_s2 - 278.527695530736).abs() <= 1.0e-12);
    assert!((results[0].applied_valve_travel_pp - 87.838026103884).abs() <= 1.0e-12);
    assert!((results[1].peak_deviation_deg_c - 1.276193205671).abs() <= 1.0e-12);
    assert!((results[1].settling_or_recovery_s - 11.6).abs() <= 1.0e-12);
    assert!((results[1].final_window_mean_absolute_error_deg_c - 0.003731619944).abs() <= 1.0e-12);
    assert!((results[1].iae_deg_c_s - 13.842573483039).abs() <= 1.0e-12);
    assert!((results[1].ise_deg_c2_s - 11.618714071685).abs() <= 1.0e-12);
    assert!((results[1].itae_deg_c_s2 - 131.199454872507).abs() <= 1.0e-12);
    assert!((results[1].applied_valve_travel_pp - 33.485594923714).abs() <= 1.0e-12);
    assert!(results.iter().all(|result| {
        result.iae_deg_c_s.is_finite()
            && result.iae_deg_c_s > 0.0
            && result.ise_deg_c2_s.is_finite()
            && result.ise_deg_c2_s > 0.0
            && result.itae_deg_c_s2.is_finite()
            && result.itae_deg_c_s2 > 0.0
            && result.applied_valve_travel_pp.is_finite()
            && result.applied_valve_travel_pp > 0.0
    }));
    eprintln!("Lesson 11 servo metrics: {:?}", results[0]);
    eprintln!("Lesson 11 regulatory metrics: {:?}", results[1]);
    write_evidence(&traces, &results);
}

#[test]
fn control_curriculum_reactor_artifacts_preserve_objective_identity_and_decisions() {
    let mut trace_lines = RETAINED_TRACE.lines();
    let header = trace_lines
        .next()
        .expect("Lesson 11 retained trace header")
        .split(',')
        .collect::<Vec<_>>();
    let column = |name: &str| {
        header
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("Lesson 11 trace field '{name}'"))
    };
    for required in [
        "error_degC",
        "anti_windup",
        "controller_rate_min_pp_s",
        "controller_rate_max_pp_s",
        "plant_id",
        "plant_model",
        "plant_gain_degC_per_percent",
        "plant_time_constant_s",
        "plant_dead_time_s",
        "plant_initial_delayed_input_percent",
        "plant_output_bias_degC",
        "safe_restoration",
    ] {
        column(required);
    }
    let rows = trace_lines.collect::<Vec<_>>();
    assert_eq!(rows.len(), 1_780);
    let mut servo_count = 0;
    let mut regulatory_count = 0;
    for row in &rows {
        let fields = row.split(',').collect::<Vec<_>>();
        assert_eq!(fields.len(), header.len());
        match fields[column("objective")] {
            "servo" => servo_count += 1,
            "regulatory" => regulatory_count += 1,
            other => panic!("unknown Lesson 11 objective '{other}'"),
        }
        assert_eq!(fields[column("configured_cycle_time_s")], "0.1");
        assert_eq!(fields[column("seed")], SEED.to_string());
        assert_eq!(fields[column("abort_state")], "none");
        assert_eq!(fields[column("evidence_class")], "simulation");
    }
    assert_eq!((servo_count, regulatory_count), (565, 1_215));
    for objective in ["servo", "regulatory"] {
        let final_row = rows
            .iter()
            .rev()
            .map(|row| row.split(',').collect::<Vec<_>>())
            .find(|fields| fields[column("objective")] == objective)
            .expect("Lesson 11 objective restoration row");
        assert_eq!(final_row[column("phase")], "safe_restoration");
        assert_eq!(final_row[column("manual_mv_percent")], "0.000000000000");
        assert_eq!(final_row[column("applied_mv_percent")], "0.000000000000");
        assert_eq!(final_row[column("auto_mode")], "false");
    }

    let mut summary_lines = RETAINED_SUMMARY.lines();
    let summary_header = summary_lines
        .next()
        .expect("Lesson 11 retained summary header")
        .split(',')
        .collect::<Vec<_>>();
    let summary_column = |name: &str| {
        summary_header
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("Lesson 11 summary field '{name}'"))
    };
    for required in [
        "controller_id",
        "anti_windup",
        "controller_rate_min_pp_s",
        "controller_rate_max_pp_s",
        "plant_id",
        "plant_model",
        "plant_gain_degC_per_percent",
        "plant_time_constant_s",
        "plant_dead_time_s",
        "plant_initial_delayed_input_percent",
        "plant_initial_output_bias_degC",
        "plant_event_output_bias_degC",
        "safe_restoration",
        "settling_band_degC",
        "settling_band_tolerance_degC",
        "servo_settling_reproduction_tolerance_s",
        "servo_overshoot_limit_degC",
        "servo_overshoot_tolerance_degC",
        "servo_overshoot_pass",
        "regulatory_excitation_min_degC",
        "regulatory_excitation_tolerance_degC",
        "regulatory_excitation_pass",
        "regulatory_recovery_limit_s",
        "regulatory_recovery_tolerance_s",
        "regulatory_recovery_pass",
        "regulatory_final_error_limit_degC",
        "regulatory_final_error_tolerance_degC",
        "regulatory_final_error_pass",
    ] {
        summary_column(required);
    }
    let summary = summary_lines
        .map(|row| row.split(',').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(summary.len(), 2);
    assert_eq!(summary[0][summary_column("objective")], "servo");
    assert_eq!(summary[0][summary_column("safe_restoration")], "achieved");
    assert_eq!(summary[0][summary_column("settling_band_degC")], "0.5");
    assert_eq!(
        summary[0][summary_column("settling_band_tolerance_degC")],
        "0.01"
    );
    assert_eq!(
        summary[0][summary_column("servo_settling_reproduction_tolerance_s")],
        "0.1"
    );
    assert_eq!(summary[0][summary_column("servo_overshoot_pass")], "false");
    assert_eq!(
        summary[0][summary_column("physical_criterion_passed")],
        "false"
    );
    assert_eq!(
        summary[0][summary_column("conclusion")],
        "servo_overshoot_requirement_failed"
    );
    assert_eq!(summary[1][summary_column("objective")], "regulatory");
    assert_eq!(summary[1][summary_column("safe_restoration")], "achieved");
    assert_eq!(
        summary[1][summary_column("regulatory_excitation_pass")],
        "true"
    );
    assert_eq!(
        summary[1][summary_column("regulatory_recovery_pass")],
        "true"
    );
    assert_eq!(
        summary[1][summary_column("regulatory_final_error_pass")],
        "true"
    );
    assert_eq!(summary[1][summary_column("final_window_start_s")], "110.1");
    assert_eq!(summary[1][summary_column("final_window_end_s")], "120.0");
    assert_eq!(
        summary[1][summary_column("physical_criterion_passed")],
        "true"
    );
    assert!(summary
        .iter()
        .all(|row| row[summary_column("selected")] == "false"));
}
