use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/09-rate-limit/src/Main.st");
const RUNTIME_CONFIG: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/09-rate-limit/runtime.toml");
const RETAINED_TRACE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/09-rate-limit/figures/rate-limit-trace.csv");
const RETAINED_SUMMARY: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/09-rate-limit/figures/rate-limit-summary.csv");
const RUN_ID: &str = "lesson-09-rate-limit-seed-20260824-dt0.1";

#[derive(Clone, Copy)]
struct Sample {
    time_s: f64,
    phase: &'static str,
    setpoint: f64,
    pv: f64,
    requested: f64,
    applied: f64,
    previous: f64,
    travel_rate: f64,
    lower: f64,
    upper: f64,
    auto: bool,
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

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 9 program before configuration")
        .replace(
            "    AutoMode : BOOL :=",
            "    AutoMode AT %MX72.0 : BOOL :=",
        )
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %MX80.0 : BOOL :=",
        )
        .replace("    Setpoint : LREAL :=", "    Setpoint AT %ML0 : LREAL :=")
        .replace(
            "    ProcessValue : LREAL :=",
            "    ProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "    RequestedManipulatedValue : LREAL :=",
            "    RequestedManipulatedValue AT %ML8 : LREAL :=",
        )
        .replace(
            "    RequestedValvePosition : LREAL :=",
            "    RequestedValvePosition AT %ML8 : LREAL :=",
        )
        .replace(
            "    ManipulatedValue : LREAL :=",
            "    ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace(
            "    AppliedValvePosition : LREAL :=",
            "    AppliedValvePosition AT %QL0 : LREAL :=",
        )
        .replace(
            "    TravelRate : LREAL :=",
            "    TravelRate AT %ML16 : LREAL :=",
        )
        .replace(
            "    MeasuredTravelRate : LREAL :=",
            "    MeasuredTravelRate AT %ML16 : LREAL :=",
        )
        .replace(
            "    OpeningRate : LREAL :=",
            "    OpeningRate AT %ML24 : LREAL :=",
        )
        .replace(
            "    MaximumOpeningRate : LREAL :=",
            "    MaximumOpeningRate AT %ML24 : LREAL :=",
        )
        .replace(
            "    ClosingRate : LREAL :=",
            "    ClosingRate AT %ML32 : LREAL :=",
        )
        .replace(
            "    MaximumClosingRate : LREAL :=",
            "    MaximumClosingRate AT %ML32 : LREAL :=",
        )
        .replace(
            "    ConfiguredCycleTime : LREAL :=",
            "    ConfiguredCycleTime AT %ML40 : LREAL :=",
        )
        .replace(
            "    PreviousPositionUsedForUpdate : LREAL :=",
            "    PreviousPositionUsedForUpdate AT %ML48 : LREAL :=",
        )
        .replace(
            "    LowerAdmissiblePosition : LREAL :=",
            "    LowerAdmissiblePosition AT %ML56 : LREAL :=",
        )
        .replace(
            "    UpperAdmissiblePosition : LREAL :=",
            "    UpperAdmissiblePosition AT %ML64 : LREAL :=",
        )
}

#[test]
fn control_curriculum_heating_valve_executes_the_shipped_project_sequence() {
    let trace = run_rate_experiment();
    assert!(!trace[0].auto && trace[0].setpoint == 0.0 && trace[0].applied == 0.0);
    assert!(trace[1].auto && trace[1].setpoint == 10.0);
    assert!(trace[150].auto && trace[150].setpoint == 10.0);
    assert!(trace[151].auto && trace[151].setpoint == 0.0);
    assert!(trace[1..151]
        .iter()
        .any(|sample| (sample.travel_rate - 1.5).abs() <= 1.0e-9));
    assert!(trace[151..900]
        .iter()
        .any(|sample| (sample.travel_rate + 0.5).abs() <= 1.0e-9));
    let safe = trace.last().unwrap();
    assert!(!safe.auto && safe.setpoint == 0.0 && safe.requested == 0.0 && safe.applied == 0.0);
}

fn lesson_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("rate-limit/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 9 Structured Text")
}

fn lesson_simulation() -> SimulationController {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/09-rate-limit/simulation.toml");
    SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load shipped Lesson 9 simulation: {error}")),
    )
}

fn run_rate_experiment() -> Vec<Sample> {
    let mut runtime = lesson_runtime();
    let mut simulation = lesson_simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 9 plant state");
    let mut trace = Vec::with_capacity(901);
    write_bool(&mut runtime, "%MX80.0", true);

    for cycle in 1..=900 {
        let phase = match cycle {
            1 => "safe_start",
            2..=151 => "opening_request",
            _ => "closing_restoration",
        };
        runtime.execute_cycle().expect("execute Lesson 9 scan");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 9 flow plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 9 flow measurement");
        trace.push(Sample {
            time_s: cycle as f64 * 0.1,
            phase,
            setpoint: read_lreal(&runtime, "%ML0"),
            pv: read_lreal(&runtime, "%IL0"),
            requested: read_lreal(&runtime, "%ML8"),
            applied: read_lreal(&runtime, "%QL0"),
            previous: read_lreal(&runtime, "%ML48"),
            travel_rate: read_lreal(&runtime, "%ML16"),
            lower: read_lreal(&runtime, "%ML56"),
            upper: read_lreal(&runtime, "%ML64"),
            auto: matches!(
                runtime
                    .io()
                    .read(&IoAddress::parse("%MX72.0").unwrap())
                    .unwrap(),
                Value::Bool(true)
            ),
        });
    }

    runtime
        .execute_cycle()
        .expect("execute Lesson 9 project-owned Manual safe restoration");
    trace.push(Sample {
        time_s: 90.1,
        phase: "safe_restoration",
        setpoint: 0.0,
        pv: read_lreal(&runtime, "%IL0"),
        requested: read_lreal(&runtime, "%ML8"),
        applied: read_lreal(&runtime, "%QL0"),
        previous: read_lreal(&runtime, "%ML48"),
        travel_rate: read_lreal(&runtime, "%ML16"),
        lower: read_lreal(&runtime, "%ML56"),
        upper: read_lreal(&runtime, "%ML64"),
        auto: matches!(
            runtime
                .io()
                .read(&IoAddress::parse("%MX72.0").unwrap())
                .unwrap(),
            Value::Bool(true)
        ),
    });
    trace
}

fn write_evidence(
    trace: &[Sample],
    maximum_opening_rate: f64,
    maximum_closing_rate: f64,
    maximum_gap: f64,
    flow_at_five_seconds: f64,
    opening_limit_time_s: f64,
    closing_limit_time_s: f64,
    return_to_closed_s: f64,
) {
    let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("create Lesson 9 evidence directory");
    let mut csv = String::from(
        "run_id,time_s,phase,setpoint_L_min,pv_L_min,requested_mv_percent,applied_mv_percent,previous_applied_mv_used_percent,measured_travel_rate_pp_s,maximum_opening_rate_pp_s,maximum_closing_rate_pp_s,lower_admissible_mv_percent,upper_admissible_mv_percent,configured_cycle_time_s,kp_percent_per_L_min,ki_percent_per_L_min_s,seed,plant_model,plant_gain_L_min_per_percent,plant_time_constant_s,plant_dead_time_s,mv_min_percent,mv_max_percent,auto_mode,abort_state,evidence_class\n",
    );
    for sample in trace {
        writeln!(
            csv,
            "{RUN_ID},{:.1},{},{:.1},{:.6},{:.6},{:.12},{:.12},{:.12},1.5,0.5,{:.12},{:.12},0.1,0.667,0.667,20260824,first_order_dead_time,1.0,1.0,0.5,0.0,100.0,{},none,simulation",
            sample.time_s,
            sample.phase,
            sample.setpoint,
            sample.pv,
            sample.requested,
            sample.applied,
            sample.previous,
            sample.travel_rate,
            sample.lower,
            sample.upper,
            sample.auto,
        )
        .expect("append Lesson 9 evidence row");
    }
    std::fs::write(directory.join("rate-limit-trace.csv"), csv).expect("write Lesson 9 trace");
    let mut summary = String::from(
        "run_id,maximum_opening_rate_pp_s,maximum_closing_rate_pp_s,maximum_requested_applied_gap_pp,flow_at_5_s_L_min,opening_limit_time_s,closing_limit_time_s,return_to_closed_time_s,position_limit_valid,opening_rate_valid,closing_rate_valid,both_directions_exercised,safe_restoration,validity,abort_state,conclusion,evidence_class\n",
    );
    writeln!(
        summary,
        "{RUN_ID},{maximum_opening_rate:.6},{maximum_closing_rate:.6},{maximum_gap:.6},{flow_at_five_seconds:.6},{opening_limit_time_s:.1},{closing_limit_time_s:.1},{return_to_closed_s:.1},true,true,true,true,true,valid,none,actuator_rate_limits_create_direction_dependent_requested_applied_motion_gap,simulation"
    )
    .expect("append Lesson 9 summary row");
    std::fs::write(directory.join("rate-limit-summary.csv"), summary)
        .expect("write Lesson 9 summary");
}

#[test]
fn control_curriculum_heating_valve_uses_the_configured_cycle_time() {
    assert!(
        LESSON_SOURCE.contains("TASK MainTask(INTERVAL := T#100ms"),
        "the shipped ST task interval must remain 100 ms"
    );
    assert!(
        RUNTIME_CONFIG.contains("cycle_interval_ms = 100"),
        "the shipped runtime cycle must remain synchronized to the ST task"
    );
    let mut runtime = lesson_runtime();
    let mut simulation = lesson_simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 9 plant state");

    write_bool(&mut runtime, "%MX72.0", false);
    write_lreal(&mut runtime, "%ML0", 0.0);
    write_lreal(&mut runtime, "%ML24", 1.5);
    write_lreal(&mut runtime, "%ML32", 0.5);
    write_lreal(&mut runtime, "%ML40", 0.1);
    runtime
        .execute_cycle()
        .expect("initialize Lesson 9 at the closed operating point");
    let mut applied_positions = vec![0.0];
    for cycle in 1..=5 {
        write_bool(&mut runtime, "%MX72.0", true);
        write_lreal(&mut runtime, "%ML0", 10.0);
        write_lreal(&mut runtime, "%ML24", 1.5);
        write_lreal(&mut runtime, "%ML32", 0.5);
        write_lreal(&mut runtime, "%ML40", 0.2);
        runtime.execute_cycle().expect("execute Lesson 9 scan");
        applied_positions.push(read_lreal(&runtime, "%QL0"));
        let now = Duration::from_millis((cycle * 200) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 9 flow plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 9 flow measurement");
    }

    let maximum_increment = applied_positions
        .windows(2)
        .map(|values| values[1] - values[0])
        .fold(0.0_f64, f64::max);
    let measured_rate = read_lreal(&runtime, "%ML16");
    let requested = read_lreal(&runtime, "%ML8");
    let setpoint = read_lreal(&runtime, "%ML0");
    assert!(
        (maximum_increment - 0.3).abs() <= 1.0e-9,
        "1.5 percentage points per second over a synchronized configured 0.2-second interval must apply 0.3 percentage point per update, got {maximum_increment}; requested={requested}, setpoint={setpoint}, positions={applied_positions:?}"
    );
    assert!(
        (measured_rate - 1.5).abs() <= 1.0e-9,
        "travel rate must divide by the same configured interval, got {measured_rate}"
    );
}

#[test]
fn control_curriculum_heating_valve_retains_a_bounded_asymmetric_rate_experiment() {
    let trace = run_rate_experiment();
    let opening = &trace[1..151];
    let closing = &trace[151..900];
    let maximum_opening_rate = opening
        .iter()
        .map(|sample| sample.travel_rate)
        .fold(0.0_f64, f64::max);
    let maximum_closing_rate = -closing
        .iter()
        .map(|sample| sample.travel_rate)
        .fold(0.0_f64, f64::min);
    let maximum_gap = trace
        .iter()
        .map(|sample| (sample.requested - sample.applied).abs())
        .fold(0.0_f64, f64::max);
    let flow_at_five_seconds = trace
        .iter()
        .find(|sample| (sample.time_s - 5.0).abs() <= 1.0e-9)
        .expect("5-second Lesson 9 sample")
        .pv;
    let opening_limit_time_s = opening
        .iter()
        .filter(|sample| (sample.travel_rate - 1.5).abs() <= 1.0e-9)
        .count() as f64
        * 0.1;
    let closing_limit_time_s = closing
        .iter()
        .filter(|sample| (sample.travel_rate + 0.5).abs() <= 1.0e-9)
        .count() as f64
        * 0.1;
    let return_to_closed_s = closing
        .iter()
        .find(|sample| sample.applied.abs() <= 1.0e-9)
        .map(|sample| sample.time_s - 15.1)
        .expect("rate-limited valve returns closed");
    let safe = trace.last().expect("safe restoration sample");

    eprintln!(
        "Lesson 9: opening={maximum_opening_rate:.6} pp/s, closing={maximum_closing_rate:.6} pp/s, gap={maximum_gap:.6} pp, PV(5s)={flow_at_five_seconds:.6} L/min, opening limit={opening_limit_time_s:.1}s, closing limit={closing_limit_time_s:.1}s, closed={return_to_closed_s:.1}s"
    );
    assert!((1.4..=1.5 + 1.0e-9).contains(&maximum_opening_rate));
    assert!((0.4..=0.5 + 1.0e-9).contains(&maximum_closing_rate));
    assert!(maximum_gap >= 1.0);
    assert!(flow_at_five_seconds < 6.0);
    assert!(opening_limit_time_s > 0.0 && closing_limit_time_s > 0.0);
    assert!(trace.iter().all(|sample| {
        sample.pv.is_finite()
            && sample.requested.is_finite()
            && sample.applied.is_finite()
            && sample.previous.is_finite()
            && sample.travel_rate.is_finite()
            && sample.lower.is_finite()
            && sample.upper.is_finite()
            && (0.0..=100.0).contains(&sample.applied)
            && sample.travel_rate <= 1.5 + 1.0e-9
            && sample.travel_rate >= -0.5 - 1.0e-9
    }));
    assert!(!safe.auto);
    assert!(safe.requested.abs() <= 1.0e-9);
    assert!(safe.applied.abs() <= 1.0e-9);
    write_evidence(
        &trace,
        maximum_opening_rate,
        maximum_closing_rate,
        maximum_gap,
        flow_at_five_seconds,
        opening_limit_time_s,
        closing_limit_time_s,
        return_to_closed_s,
    );
}

#[test]
fn control_curriculum_heating_valve_artifacts_retain_identity_and_disposition() {
    let trace_header = RETAINED_TRACE
        .lines()
        .next()
        .expect("Lesson 9 trace header");
    let trace_columns = trace_header.split(',').collect::<Vec<_>>();
    let trace_index = |name: &str| {
        trace_columns
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("Lesson 9 trace field '{name}'"))
    };
    let trace_rows = RETAINED_TRACE.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(
        trace_rows.len(),
        901,
        "trace includes final Manual restoration"
    );
    for (row_number, row) in trace_rows.into_iter().enumerate() {
        let values = row.split(',').collect::<Vec<_>>();
        assert_eq!(
            values.len(),
            trace_columns.len(),
            "Lesson 9 trace row width"
        );
        assert_eq!(values[trace_index("run_id")], RUN_ID);
        assert_eq!(values[trace_index("configured_cycle_time_s")], "0.1");
        assert_eq!(values[trace_index("seed")], "20260824");
        assert_eq!(values[trace_index("abort_state")], "none");
        assert_eq!(values[trace_index("evidence_class")], "simulation");

        let number = |name: &str| {
            values[trace_index(name)]
                .parse::<f64>()
                .unwrap_or_else(|error| {
                    panic!(
                        "Lesson 9 trace row {} field '{name}': {error}",
                        row_number + 2
                    )
                })
        };
        let previous = number("previous_applied_mv_used_percent");
        let applied = number("applied_mv_percent");
        let cycle_time = number("configured_cycle_time_s");
        let retained_rate = number("measured_travel_rate_pp_s");
        let reconstructed_rate = (applied - previous) / cycle_time;
        assert!(
            (reconstructed_rate - retained_rate).abs() <= 1.0e-9,
            "Lesson 9 trace row {} must reproduce measured travel rate from the retained previous position: reconstructed={reconstructed_rate}, retained={retained_rate}",
            row_number + 2
        );
    }

    let summary_header = RETAINED_SUMMARY
        .lines()
        .next()
        .expect("Lesson 9 summary header");
    let summary_columns = summary_header.split(',').collect::<Vec<_>>();
    let summary_values = RETAINED_SUMMARY
        .lines()
        .nth(1)
        .expect("Lesson 9 summary row")
        .split(',')
        .collect::<Vec<_>>();
    let summary_value = |name: &str| {
        let index = summary_columns
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("Lesson 9 summary field '{name}'"));
        summary_values[index]
    };
    assert_eq!(summary_value("run_id"), RUN_ID);
    assert_eq!(summary_value("validity"), "valid");
    assert_eq!(summary_value("abort_state"), "none");
    assert_eq!(summary_value("safe_restoration"), "true");
    assert_eq!(summary_value("opening_rate_valid"), "true");
    assert_eq!(summary_value("closing_rate_valid"), "true");
    assert_eq!(summary_value("both_directions_exercised"), "true");
    assert_eq!(
        summary_value("conclusion"),
        "actuator_rate_limits_create_direction_dependent_requested_applied_motion_gap"
    );
}
use std::fmt::Write as _;
use std::path::PathBuf;
