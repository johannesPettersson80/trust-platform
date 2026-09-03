use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/14-sampling-interval/src/Main.st");
const SEED: u64 = 20260824;
const EVENT_TIME_S: f64 = 1.0;
const END_TIME_S: f64 = 30.0;
const SETTLING_BAND_DEGC: f64 = 0.02;

#[derive(Clone, Copy, Debug)]
struct Case {
    id: &'static str,
    pattern_s: &'static [f64],
    use_actual_time: bool,
    eligible: bool,
}

const FIXED_50: [f64; 1] = [0.05];
const FIXED_100: [f64; 1] = [0.1];
const FIXED_200: [f64; 1] = [0.2];
const JITTER: [f64; 5] = [0.02, 0.18, 0.05, 0.15, 0.1];
const CASES: [Case; 5] = [
    Case {
        id: "measured_50_ms",
        pattern_s: &FIXED_50,
        use_actual_time: true,
        eligible: true,
    },
    Case {
        id: "measured_100_ms",
        pattern_s: &FIXED_100,
        use_actual_time: true,
        eligible: true,
    },
    Case {
        id: "measured_200_ms",
        pattern_s: &FIXED_200,
        use_actual_time: true,
        eligible: true,
    },
    Case {
        id: "measured_jitter",
        pattern_s: &JITTER,
        use_actual_time: true,
        eligible: true,
    },
    Case {
        id: "bad_nominal_200_ms",
        pattern_s: &FIXED_200,
        use_actual_time: false,
        eligible: false,
    },
];

#[derive(Clone, Copy, Debug)]
struct Sample {
    case_id: &'static str,
    index: usize,
    time_s: f64,
    actual_dt_s: f64,
    controller_dt_s: f64,
    setpoint_degc: f64,
    pv_degc: f64,
    requested_trim_pp: f64,
    applied_trim_pp: f64,
    belt_speed_m_s: f64,
    maximum_cycle_time_s: f64,
    timing_valid: bool,
    timing_fault_latched: bool,
    auto: bool,
}

#[derive(Clone, Copy, Debug)]
struct Metrics {
    maximum_timing_error_s: f64,
    overshoot_percent: f64,
    settling_time_s: Option<f64>,
    final_window_error_degc: Option<f64>,
    requested_travel_pp: f64,
    applied_travel_pp: f64,
    qualified: bool,
}

#[derive(Clone, Copy, Debug)]
struct FaultSample {
    journey_id: &'static str,
    phase: &'static str,
    time_s: f64,
    actual_dt_s: f64,
    maximum_cycle_time_s: f64,
    timing_valid: bool,
    timing_fault_latched: bool,
    auto: bool,
    reset: bool,
    requested_trim_pp: f64,
    applied_trim_pp: f64,
}

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 14 program before configuration")
        .replace(
            "    AutoMode : BOOL :=",
            "    AutoMode AT %MX64.0 : BOOL :=",
        )
        .replace(
            "    UseActualCycleTime : BOOL :=",
            "    UseActualCycleTime AT %MX64.1 : BOOL :=",
        )
        .replace(
            "    ResetTimingFault : BOOL :=",
            "    ResetTimingFault AT %MX64.4 : BOOL :=",
        )
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %MX80.0 : BOOL :=",
        )
        .replace(
            "    ActualCycleTime : LREAL :=",
            "    ActualCycleTime AT %ML8 : LREAL :=",
        )
        .replace(
            "    ControllerCycleTime : LREAL :=",
            "    ControllerCycleTime AT %ML24 : LREAL :=",
        )
        .replace(
            "    MaximumCycleTime : LREAL :=",
            "    MaximumCycleTime AT %ML16 : LREAL :=",
        )
        .replace(
            "    TimingValid : BOOL :=",
            "    TimingValid AT %MX64.2 : BOOL :=",
        )
        .replace(
            "    TimingFaultLatched : BOOL :=",
            "    TimingFaultLatched AT %MX64.3 : BOOL :=",
        )
        .replace(
            "    BeltSpeed : LREAL :=",
            "    BeltSpeed AT %IL16 : LREAL :=",
        )
        .replace("    Setpoint : LREAL :=", "    Setpoint AT %ML0 : LREAL :=")
        .replace(
            "    ProcessValue : LREAL :=",
            "    ProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "    RequestedHeaterTrim : LREAL :=",
            "    RequestedHeaterTrim AT %ML32 : LREAL :=",
        )
        .replace(
            "    AppliedHeaterTrim : LREAL :=",
            "    AppliedHeaterTrim AT %QL0 : LREAL :=",
        )
}

fn runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("sampling-interval/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 14 Structured Text")
}

fn simulation() -> SimulationController {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/14-sampling-interval/simulation.toml");
    SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load shipped Lesson 14 simulation: {error}")),
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

fn set_inputs(
    runtime: &mut Runtime,
    actual_dt_s: f64,
    use_actual_time: bool,
    setpoint_degc: f64,
    auto: bool,
    reset: bool,
) {
    write_lreal(runtime, "%ML8", actual_dt_s);
    write_lreal(runtime, "%ML16", 0.25);
    write_lreal(runtime, "%ML0", setpoint_degc);
    write_bool(runtime, "%MX64.0", auto);
    write_bool(runtime, "%MX64.1", use_actual_time);
    write_bool(runtime, "%MX64.4", reset);
}

fn capture(
    case_id: &'static str,
    index: usize,
    time_s: f64,
    actual_dt_s: f64,
    runtime: &Runtime,
) -> Sample {
    Sample {
        case_id,
        index,
        time_s,
        actual_dt_s,
        controller_dt_s: read_lreal(runtime, "%ML24"),
        setpoint_degc: read_lreal(runtime, "%ML0"),
        pv_degc: read_lreal(runtime, "%IL0"),
        requested_trim_pp: read_lreal(runtime, "%ML32"),
        applied_trim_pp: read_lreal(runtime, "%QL0"),
        belt_speed_m_s: read_lreal(runtime, "%IL16"),
        maximum_cycle_time_s: read_lreal(runtime, "%ML16"),
        timing_valid: read_bool(runtime, "%MX64.2"),
        timing_fault_latched: read_bool(runtime, "%MX64.3"),
        auto: read_bool(runtime, "%MX64.0"),
    }
}

fn run(case: Case) -> Vec<Sample> {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 14 oven state");
    set_inputs(
        &mut runtime,
        case.pattern_s[0],
        case.use_actual_time,
        0.0,
        false,
        false,
    );
    runtime
        .execute_cycle()
        .expect("initialize Lesson 14 at Manual zero");

    let mut trace = vec![capture(case.id, 0, 0.0, 0.0, &runtime)];
    let mut time_s = 0.0;
    let mut index = 0usize;
    while time_s < END_TIME_S - 1.0e-12 {
        let dt_s = case.pattern_s[index % case.pattern_s.len()];
        let next_time_s = time_s + dt_s;
        assert!(
            next_time_s <= END_TIME_S + 1.0e-9,
            "timing pattern must end at 30 s"
        );
        let now_ms = (next_time_s * 1_000.0).round() as i64;
        simulation
            .apply_post_cycle(Duration::from_millis(now_ms), &runtime)
            .expect("advance Lesson 14 oven under held output");
        simulation
            .apply_pre_cycle(Duration::from_millis(now_ms), &mut runtime)
            .expect("publish sampled Lesson 14 temperature");
        let setpoint = if next_time_s + 1.0e-12 >= EVENT_TIME_S {
            1.0
        } else {
            0.0
        };
        set_inputs(
            &mut runtime,
            dt_s,
            case.use_actual_time,
            setpoint,
            true,
            false,
        );
        runtime
            .execute_cycle()
            .expect("execute Lesson 14 controller update");
        index += 1;
        trace.push(capture(case.id, index, next_time_s, dt_s, &runtime));
        time_s = next_time_s;
    }
    trace
}

fn isolated_first_update(case_id: &'static str, actual_dt_s: f64, use_actual_time: bool) -> Sample {
    let mut runtime = runtime();
    set_inputs(&mut runtime, actual_dt_s, use_actual_time, 1.0, true, false);
    write_lreal(&mut runtime, "%IL0", 0.0);
    write_lreal(&mut runtime, "%IL16", 0.5);
    runtime
        .execute_cycle()
        .expect("execute isolated Lesson 14 first update");
    capture(case_id, 1, actual_dt_s, actual_dt_s, &runtime)
}

fn metrics(trace: &[Sample], eligible: bool) -> Metrics {
    let maximum_timing_error_s = trace
        .iter()
        .skip(1)
        .map(|sample| (sample.controller_dt_s - sample.actual_dt_s).abs())
        .fold(0.0, f64::max);
    let maximum_pv = trace
        .iter()
        .filter(|sample| sample.time_s >= EVENT_TIME_S)
        .map(|sample| sample.pv_degc)
        .fold(f64::NEG_INFINITY, f64::max);
    let overshoot_percent = (maximum_pv - 1.0).max(0.0) * 100.0;
    let settling_time_s = trace
        .iter()
        .enumerate()
        .find(|(index, sample)| {
            sample.time_s >= EVENT_TIME_S
                && trace[*index..]
                    .iter()
                    .all(|later| (later.setpoint_degc - later.pv_degc).abs() <= SETTLING_BAND_DEGC)
        })
        .map(|(_, sample)| sample.time_s - EVENT_TIME_S);
    let final_window = trace
        .windows(2)
        .filter(|pair| pair[1].time_s > 25.0 && pair[1].time_s <= 30.0 + 1.0e-9)
        .collect::<Vec<_>>();
    let final_duration = final_window
        .iter()
        .map(|pair| pair[1].time_s - pair[0].time_s)
        .sum::<f64>();
    let final_window_error_degc = settling_time_s.map(|_| {
        final_window
            .iter()
            .map(|pair| {
                (pair[1].setpoint_degc - pair[1].pv_degc).abs() * (pair[1].time_s - pair[0].time_s)
            })
            .sum::<f64>()
            / final_duration
    });
    let requested_travel_pp = trace
        .windows(2)
        .map(|pair| (pair[1].requested_trim_pp - pair[0].requested_trim_pp).abs())
        .sum();
    let applied_travel_pp = trace
        .windows(2)
        .map(|pair| (pair[1].applied_trim_pp - pair[0].applied_trim_pp).abs())
        .sum();
    let valid = trace.iter().all(|sample| {
        sample.time_s.is_finite()
            && sample.actual_dt_s.is_finite()
            && sample.controller_dt_s.is_finite()
            && sample.setpoint_degc.is_finite()
            && sample.pv_degc.is_finite()
            && sample.requested_trim_pp.is_finite()
            && sample.applied_trim_pp.is_finite()
            && sample.belt_speed_m_s.is_finite()
            && (-10.0..=10.0).contains(&sample.pv_degc)
            && (-10.0..=10.0).contains(&sample.requested_trim_pp)
            && (-10.0..=10.0).contains(&sample.applied_trim_pp)
            && (sample.belt_speed_m_s - 0.5).abs() <= 1.0e-12
            && sample.timing_valid
            && !sample.timing_fault_latched
    }) && trace.windows(2).all(|pair| pair[1].time_s > pair[0].time_s);
    let qualified = eligible
        && valid
        && maximum_timing_error_s <= 1.0e-12
        && overshoot_percent <= 20.0 + 0.1
        && settling_time_s.is_some_and(|value| value <= 8.0 + 0.1)
        && final_window_error_degc.is_some_and(|value| value <= 0.02 + 0.001);
    Metrics {
        maximum_timing_error_s,
        overshoot_percent,
        settling_time_s,
        final_window_error_degc,
        requested_travel_pp,
        applied_travel_pp,
        qualified,
    }
}

fn fault_recovery_trace(journey_id: &'static str, invalid_dt_s: f64) -> Vec<FaultSample> {
    let mut runtime = runtime();
    write_lreal(&mut runtime, "%IL0", 0.0);
    write_lreal(&mut runtime, "%IL16", 0.5);
    let steps = [
        ("normal", 0.1, true, false),
        ("invalid_interval", invalid_dt_s, true, false),
        ("valid_but_latched", 0.1, true, false),
        ("explicit_reset", 0.1, false, true),
        ("manual_after_reset", 0.1, false, false),
        ("explicit_reenable", 0.1, true, false),
    ];
    let mut time_s = 0.0;
    steps
        .into_iter()
        .map(|(phase, dt_s, auto, reset)| {
            time_s += dt_s;
            set_inputs(&mut runtime, dt_s, true, 1.0, auto, reset);
            runtime
                .execute_cycle()
                .expect("execute Lesson 14 fault/recovery step");
            FaultSample {
                journey_id,
                phase,
                time_s,
                actual_dt_s: dt_s,
                maximum_cycle_time_s: read_lreal(&runtime, "%ML16"),
                timing_valid: read_bool(&runtime, "%MX64.2"),
                timing_fault_latched: read_bool(&runtime, "%MX64.3"),
                auto,
                reset,
                requested_trim_pp: read_lreal(&runtime, "%ML32"),
                applied_trim_pp: read_lreal(&runtime, "%QL0"),
            }
        })
        .collect()
}

fn evidence_strings(
    traces: &[Vec<Sample>; 5],
    isolated: &[Sample; 5],
    results: &[Metrics; 5],
    fault_journeys: &[Vec<FaultSample>; 2],
) -> (String, String) {
    let mut trace_csv = String::from(
        "run_id,case_id,sample_index,time_s,actual_dt_s,nominal_dt_s,controller_dt_s,jitter_s,setpoint_degc,pv_degc,error_degc,requested_trim_pp,applied_trim_pp,belt_speed_m_s,use_actual_time,timing_valid,timing_fault_latched,auto_mode,reset_requested,event,reviewed_maximum_cycle_time_s,pv_min_degc,pv_max_degc,trim_min_pp,trim_max_pp,belt_speed_min_m_s,belt_speed_max_m_s,plant_revision,controller_revision,seed,evidence_class\n",
    );
    for (case_index, trace) in traces.iter().enumerate() {
        let case = CASES[case_index];
        for sample in trace {
            let event = if (sample.time_s - EVENT_TIME_S).abs() <= 1.0e-9 {
                "setpoint_step"
            } else {
                "none"
            };
            writeln!(
                trace_csv,
                "lesson-14-{},{},{},{:.12},{:.12},0.100000000000,{:.12},{:.12},{:.6},{:.12},{:.12},{:.12},{:.12},{:.12},{},{},{},{},false,{},{:.12},-10.000000000000,10.000000000000,-10.000000000000,10.000000000000,0.000000000000,2.000000000000,sampling-fopdt-v1,Control.PID.PI-timing-v1,{SEED},simulation",
                case.id,
                sample.case_id,
                sample.index,
                sample.time_s,
                sample.actual_dt_s,
                sample.controller_dt_s,
                sample.actual_dt_s - 0.1,
                sample.setpoint_degc,
                sample.pv_degc,
                sample.setpoint_degc - sample.pv_degc,
                sample.requested_trim_pp,
                sample.applied_trim_pp,
                sample.belt_speed_m_s,
                case.use_actual_time,
                sample.timing_valid,
                sample.timing_fault_latched,
                sample.auto,
                event,
                sample.maximum_cycle_time_s,
            )
            .expect("append Lesson 14 response trace");
        }
    }
    for (index, sample) in isolated.iter().enumerate() {
        let case = CASES[index];
        writeln!(
            trace_csv,
            "lesson-14-isolated-{},isolated_{},{},{:.12},{:.12},0.100000000000,{:.12},{:.12},{:.6},{:.12},{:.12},{:.12},{:.12},{:.12},{},{},{},{},false,isolated_first_update,{:.12},-10.000000000000,10.000000000000,-10.000000000000,10.000000000000,0.000000000000,2.000000000000,sampling-fopdt-v1,Control.PID.PI-timing-v1,{SEED},simulation",
            case.id,
            case.id,
            sample.index,
            sample.time_s,
            sample.actual_dt_s,
            sample.controller_dt_s,
            sample.actual_dt_s - 0.1,
            sample.setpoint_degc,
            sample.pv_degc,
            sample.setpoint_degc - sample.pv_degc,
            sample.requested_trim_pp,
            sample.applied_trim_pp,
            sample.belt_speed_m_s,
            case.use_actual_time,
            sample.timing_valid,
            sample.timing_fault_latched,
            sample.auto,
            sample.maximum_cycle_time_s,
        )
        .expect("append Lesson 14 isolated first-update trace");
    }
    for journey in fault_journeys {
        for (index, sample) in journey.iter().enumerate() {
            writeln!(
                trace_csv,
                "lesson-14-{},{},{index},{:.12},{:.12},0.100000000000,{:.12},{:.12},1.000000,0.000000000000,1.000000000000,{:.12},{:.12},0.500000000000,true,{},{},{},{},{},{:.12},-10.000000000000,10.000000000000,-10.000000000000,10.000000000000,0.000000000000,2.000000000000,sampling-fopdt-v1,Control.PID.PI-timing-v1,{SEED},simulation",
                sample.journey_id,
                sample.journey_id,
                sample.time_s,
                sample.actual_dt_s,
                if sample.timing_valid { sample.actual_dt_s } else { 0.0 },
                sample.actual_dt_s - 0.1,
                sample.requested_trim_pp,
                sample.applied_trim_pp,
                sample.timing_valid,
                sample.timing_fault_latched,
                sample.auto,
                sample.reset,
                sample.phase,
                sample.maximum_cycle_time_s,
            )
            .expect("append Lesson 14 fault trace");
        }
    }

    let mut summary = String::from(
        "run_id,case_id,interval_pattern_s,use_actual_time,eligible,first_integral_contribution_pp,maximum_timing_error_s,overshoot_percent,settled,settling_time_s,final_window_error_degc,requested_travel_pp,applied_travel_pp,belt_speed_m_s,first_increment_tolerance_pp,timing_error_limit_s,overshoot_limit_percent,overshoot_tolerance_percent,settling_band_degc,settling_limit_s,settling_tolerance_s,final_window_start_s,final_window_end_s,final_error_limit_degc,final_error_tolerance_degc,travel_physical_threshold,qualified,decision,evidence_class,fault_recovery_result\n",
    );
    for index in 0..CASES.len() {
        let case = CASES[index];
        let result = results[index];
        let pattern = case
            .pattern_s
            .iter()
            .map(|value| format!("{value:.2}"))
            .collect::<Vec<_>>()
            .join("/");
        let settling = result
            .settling_time_s
            .map(|value| format!("{value:.12}"))
            .unwrap_or_else(|| "Not settled".to_string());
        let final_error = result
            .final_window_error_degc
            .map(|value| format!("{value:.12}"))
            .unwrap_or_else(|| "Not settled".to_string());
        let decision = if !case.eligible {
            "Instructional baseline; ineligible"
        } else if result.qualified {
            "Qualified"
        } else if result.settling_time_s.is_none() {
            "Not settled"
        } else {
            "Failed response criterion"
        };
        writeln!(
            summary,
            "lesson-14-{},{},\"{}\",{},{},{:.12},{:.12},{:.12},{},{},{},{:.12},{:.12},0.500000000000,1e-9,1e-12,20.0,0.1,0.02,8.0,0.1,25.0,30.0,0.02,0.001,none,{},{},simulation,not_applicable",
            case.id,
            case.id,
            pattern,
            case.use_actual_time,
            case.eligible,
            isolated[index].applied_trim_pp,
            result.maximum_timing_error_s,
            result.overshoot_percent,
            result.settling_time_s.is_some(),
            settling,
            final_error,
            result.requested_travel_pp,
            result.applied_travel_pp,
            result.qualified,
            decision,
        )
        .expect("append Lesson 14 summary");
    }
    for journey in fault_journeys {
        let invalid = &journey[1];
        let restored = &journey[2];
        let reset = &journey[3];
        let manual = &journey[4];
        let reenabled = &journey[5];
        let result = if !invalid.timing_valid
            && invalid.timing_fault_latched
            && invalid.applied_trim_pp == 0.0
            && restored.timing_valid
            && restored.timing_fault_latched
            && restored.applied_trim_pp == 0.0
            && !reset.timing_fault_latched
            && !reset.auto
            && manual.applied_trim_pp == 0.0
            && reenabled.auto
            && reenabled.applied_trim_pp > 0.0
        {
            "latched_zero_then_manual_reset_then_separate_auto"
        } else {
            "failed"
        };
        writeln!(
            summary,
            "lesson-14-{},{},\"{}\",true,false,,,,,,,,,0.500000000000,,,,,,,,,,,,,false,Timing fault,simulation,{}",
            journey[0].journey_id,
            journey[0].journey_id,
            journey
                .iter()
                .map(|sample| format!("{:.2}", sample.actual_dt_s))
                .collect::<Vec<_>>()
                .join("/"),
            result,
        )
        .expect("append Lesson 14 fault summary");
    }
    (trace_csv, summary)
}

fn artifact_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/14-sampling-interval/figures")
        .join(name)
}

fn write_or_compare(path: &Path, expected: &str) {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("artifact parent"))
            .expect("create Lesson 14 artifact directory");
        std::fs::write(path, expected).expect("write Lesson 14 artifact");
    }
    let actual = std::fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "read required Lesson 14 artifact {}: {error}",
            path.display()
        )
    });
    assert_eq!(
        actual,
        expected,
        "Lesson 14 artifact drift: {}",
        path.display()
    );
}

#[test]
fn lesson_14_executes_equivalent_fixed_and_jittered_timing_runs() {
    let traces: [Vec<Sample>; 5] = CASES.map(run);
    let results: [Metrics; 5] =
        std::array::from_fn(|index| metrics(&traces[index], CASES[index].eligible));

    for (case_id, dt_s, expected) in [
        ("measured_50_ms", 0.05, 0.03335),
        ("measured_100_ms", 0.1, 0.0667),
        ("measured_200_ms", 0.2, 0.1334),
    ] {
        let measured = isolated_first_update(case_id, dt_s, true).applied_trim_pp;
        assert!(
            (measured - expected).abs() <= 1.0e-9,
            "measured elapsed time must produce Ki*e*dt at {dt_s}s; got {measured}"
        );
    }
    let bad_nominal = isolated_first_update("bad_nominal_200_ms", 0.2, false).applied_trim_pp;
    assert!(
        (bad_nominal - 0.0667).abs() <= 1.0e-9,
        "the explicit bad baseline must expose a nominal 100 ms contribution"
    );

    for index in 0..4 {
        assert!(
            results[index].qualified,
            "qualified measured-time case {} failed: {:?}",
            CASES[index].id, results[index]
        );
    }
    assert!(
        !results[4].qualified && results[4].maximum_timing_error_s >= 0.1 - 1.0e-12,
        "the bad nominal-time baseline must remain visibly ineligible: {:?}",
        results[4]
    );
    assert!(
        traces[3]
            .iter()
            .skip(1)
            .zip(JITTER.iter().cycle())
            .all(|(sample, expected)| (sample.actual_dt_s - expected).abs() <= 1.0e-12),
        "the jitter run must execute the declared deterministic pattern"
    );

    eprintln!("Lesson 14 timing metrics: {results:?}");
}

#[test]
fn lesson_14_project_executes_selected_timing_fault_and_recovery_sequence() {
    let mut runtime = runtime();
    write_lreal(&mut runtime, "%IL0", 0.0);
    write_lreal(&mut runtime, "%IL16", 0.5);
    set_inputs(&mut runtime, 0.1, true, 0.0, false, false);
    write_bool(&mut runtime, "%MX80.0", false);
    runtime
        .execute_cycle()
        .expect("initialize Lesson 14 safely");

    assert!(!read_bool(&runtime, "%MX64.0"));
    assert_eq!(read_lreal(&runtime, "%ML0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);

    write_bool(&mut runtime, "%MX80.0", true);
    for cycle in 1..=306 {
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("execute Lesson 14 project cycle {cycle}: {error}"));
        match cycle {
            10 => {
                assert!(read_bool(&runtime, "%MX64.0"));
                assert_eq!(read_lreal(&runtime, "%ML0"), 0.0);
                assert_eq!(read_lreal(&runtime, "%ML8"), 0.1);
            }
            11 => {
                assert!(read_bool(&runtime, "%MX64.0"));
                assert_eq!(read_lreal(&runtime, "%ML0"), 1.0);
                let applied = read_lreal(&runtime, "%QL0");
                assert!(
                    (applied - 0.7337).abs() <= 1.0e-9,
                    "selected response first request must be 0.7337 pp, got {applied}"
                );
            }
            301 => {
                assert!(!read_bool(&runtime, "%MX64.2"));
                assert!(read_bool(&runtime, "%MX64.3"));
                assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
            }
            302 => {
                assert!(read_bool(&runtime, "%MX64.2"));
                assert!(read_bool(&runtime, "%MX64.3"));
                assert!(read_bool(&runtime, "%MX64.0"));
                assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
            }
            303 => {
                assert!(read_bool(&runtime, "%MX64.2"));
                assert!(!read_bool(&runtime, "%MX64.3"));
                assert!(!read_bool(&runtime, "%MX64.0"));
                assert!(read_bool(&runtime, "%MX64.4"));
                assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
            }
            304 => {
                assert!(!read_bool(&runtime, "%MX64.0"));
                assert!(!read_bool(&runtime, "%MX64.4"));
                assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
            }
            305 => {
                assert!(read_bool(&runtime, "%MX64.0"));
                assert!(read_lreal(&runtime, "%QL0") > 0.0);
            }
            306 => {
                assert!(!read_bool(&runtime, "%MX64.0"));
                assert_eq!(read_lreal(&runtime, "%ML0"), 0.0);
                assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
            }
            _ => {}
        }
    }

    write_bool(&mut runtime, "%MX80.0", false);
    runtime
        .execute_cycle()
        .expect("restore Lesson 14 experiment authority false");
    assert!(!read_bool(&runtime, "%MX64.0"));
    assert_eq!(read_lreal(&runtime, "%ML0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
}

#[test]
fn lesson_14_latches_invalid_timing_until_reviewed_reset_and_reenable() {
    let trace = fault_recovery_trace("missed_scan_recovery", 0.3);
    assert!(
        trace[0].timing_valid && !trace[0].timing_fault_latched && trace[0].applied_trim_pp > 0.0
    );
    assert!(
        !trace[1].timing_valid && trace[1].timing_fault_latched && trace[1].applied_trim_pp == 0.0
    );
    assert!(
        trace[2].timing_valid && trace[2].timing_fault_latched && trace[2].applied_trim_pp == 0.0,
        "valid timing alone must not restore automatic authority"
    );
    assert!(
        trace[3].timing_valid
            && !trace[3].timing_fault_latched
            && !trace[3].auto
            && trace[3].reset
            && trace[3].applied_trim_pp == 0.0,
        "reset must require valid timing and Manual authority"
    );
    assert!(
        !trace[4].auto && trace[4].applied_trim_pp == 0.0,
        "reset must not itself re-enable automatic heat"
    );
    assert!(
        trace[5].auto
            && trace[5].timing_valid
            && !trace[5].timing_fault_latched
            && trace[5].applied_trim_pp > 0.0,
        "a later explicit automatic enable may resume from reinitialized state"
    );
}

#[test]
fn lesson_14_enforces_the_reviewed_250_ms_ceiling_even_if_configuration_is_higher() {
    let mut runtime = runtime();
    write_lreal(&mut runtime, "%IL0", 0.0);
    write_lreal(&mut runtime, "%IL16", 0.5);
    set_inputs(&mut runtime, 0.3, true, 1.0, true, false);
    write_lreal(&mut runtime, "%ML16", 0.5);

    runtime
        .execute_cycle()
        .expect("execute an interval above the reviewed Lesson 14 ceiling");

    assert!(
        !read_bool(&runtime, "%MX64.2"),
        "a configured value must not widen the reviewed 0.25 s safety ceiling"
    );
    assert!(read_bool(&runtime, "%MX64.3"));
    assert_eq!(read_lreal(&runtime, "%ML32"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
}

#[test]
fn lesson_14_retained_artifacts_reproduce_timing_decisions() {
    let traces: [Vec<Sample>; 5] = CASES.map(run);
    let isolated = [
        isolated_first_update("measured_50_ms", 0.05, true),
        isolated_first_update("measured_100_ms", 0.1, true),
        isolated_first_update("measured_200_ms", 0.2, true),
        isolated_first_update("measured_jitter", 0.02, true),
        isolated_first_update("bad_nominal_200_ms", 0.2, false),
    ];
    let results: [Metrics; 5] =
        std::array::from_fn(|index| metrics(&traces[index], CASES[index].eligible));
    let fault_journeys = [
        fault_recovery_trace("missed_scan_recovery", 0.3),
        fault_recovery_trace("zero_interval_recovery", 0.0),
    ];
    for journey in &fault_journeys {
        assert!((journey[0].time_s - journey[0].actual_dt_s).abs() <= 1.0e-12);
        assert!(
            journey.windows(2).all(|pair| {
                ((pair[1].time_s - pair[0].time_s) - pair[1].actual_dt_s).abs() <= 1.0e-12
            }),
            "retained fault timestamps must reproduce every declared interval"
        );
        assert!(!journey[1].timing_valid && journey[1].timing_fault_latched);
        assert_eq!(journey[1].requested_trim_pp, 0.0);
        assert_eq!(journey[1].applied_trim_pp, 0.0);
        assert!(journey
            .iter()
            .all(|sample| { (sample.maximum_cycle_time_s - 0.25).abs() <= 1.0e-12 }));
    }
    assert_eq!(fault_journeys[0][1].actual_dt_s, 0.3);
    assert_eq!(fault_journeys[1][1].actual_dt_s, 0.0);
    assert!(isolated.iter().all(|sample| {
        sample.timing_valid
            && (sample.maximum_cycle_time_s - 0.25).abs() <= 1.0e-12
            && sample.requested_trim_pp == sample.applied_trim_pp
    }));
    let (trace_csv, summary_csv) = evidence_strings(&traces, &isolated, &results, &fault_journeys);
    assert!(trace_csv.contains("isolated_first_update"));
    assert!(trace_csv.contains("reviewed_maximum_cycle_time_s"));
    assert!(summary_csv.contains("missed_scan_recovery"));
    assert!(summary_csv.contains("zero_interval_recovery"));
    assert!(summary_csv.contains("latched_zero_then_manual_reset_then_separate_auto"));
    for case_id in ["missed_scan_recovery", "zero_interval_recovery"] {
        let row = summary_csv
            .lines()
            .find(|row| row.starts_with(&format!("lesson-14-{case_id},")))
            .expect("fault summary row");
        assert!(
            row.contains(",false,Timing fault,simulation,latched_zero_then_manual_reset_then_separate_auto"),
            "an invalid timing case must remain unqualified while separately recording successful recovery"
        );
    }
    for (name, csv) in [("trace", &trace_csv), ("summary", &summary_csv)] {
        let mut rows = csv.lines();
        let field_count = rows.next().expect("CSV header").split(',').count();
        for (index, row) in rows.enumerate() {
            assert_eq!(
                row.split(',').count(),
                field_count,
                "Lesson 14 {name} CSV row {} has the wrong field count",
                index + 2
            );
        }
    }
    write_or_compare(&artifact_path("sampling-timing-trace.csv"), &trace_csv);
    write_or_compare(&artifact_path("sampling-timing-summary.csv"), &summary_csv);
}
