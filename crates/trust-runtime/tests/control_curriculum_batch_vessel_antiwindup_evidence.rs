use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/10-saturation-antiwindup/src/Main.st");
const RETAINED_TRACE: &str = include_str!(
    "../../../examples/control/01-feedback-foundations-and-temperature-control/10-saturation-antiwindup/figures/anti-windup-trace.csv"
);
const RETAINED_SUMMARY: &str = include_str!(
    "../../../examples/control/01-feedback-foundations-and-temperature-control/10-saturation-antiwindup/figures/anti-windup-summary.csv"
);
const SEED: u64 = 20260824;

#[derive(Clone, Copy)]
struct Sample {
    strategy: i16,
    time_s: f64,
    phase: &'static str,
    setpoint: f64,
    pv: f64,
    requested: f64,
    applied: f64,
    tracking_time: f64,
    configured_cycle_time: f64,
    kp: f64,
    ki: f64,
    kd: f64,
    auto: bool,
}

#[derive(Clone, Copy)]
struct Metrics {
    recovery_time_s: f64,
    upper_limit_time_s: f64,
    undershoot: f64,
    travel: f64,
    peak_unavailable_request: f64,
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

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 10 program before configuration")
        .replace(
            "    AutoMode : BOOL :=",
            "    AutoMode AT %MX64.0 : BOOL :=",
        )
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %IX80.0 : BOOL :=",
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
            "    AntiWindupStrategy : INT :=",
            "    AntiWindupStrategy AT %IW12 : INT :=",
        )
        .replace(
            "    RequestedOutput : LREAL :=",
            "    RequestedOutput AT %ML8 : LREAL :=",
        )
        .replace(
            "    TrackingTimeUsed : LREAL :=",
            "    TrackingTimeUsed AT %ML16 : LREAL :=",
        )
        .replace(
            "    ProportionalGainUsed : LREAL :=",
            "    ProportionalGainUsed AT %ML24 : LREAL :=",
        )
        .replace(
            "    IntegralGainUsed : LREAL :=",
            "    IntegralGainUsed AT %ML32 : LREAL :=",
        )
        .replace(
            "    DerivativeGainUsed : LREAL :=",
            "    DerivativeGainUsed AT %ML48 : LREAL :=",
        )
        .replace(
            "    ConfiguredCycleTime : LREAL :=",
            "    ConfiguredCycleTime AT %ML56 : LREAL :=",
        )
}

fn lesson_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("saturation-antiwindup/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 10 Structured Text")
}

fn lesson_simulation() -> SimulationController {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/10-saturation-antiwindup/simulation.toml");
    SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load shipped Lesson 10 simulation: {error}")),
    )
}

fn run_strategy(strategy: i16) -> Vec<Sample> {
    let mut runtime = lesson_runtime();
    let mut simulation = lesson_simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 10 plant state");
    let mut trace = Vec::with_capacity(502);

    write_int(&mut runtime, "%IW12", strategy);
    write_bool(&mut runtime, "%IX80.0", false);
    runtime
        .execute_cycle()
        .expect("execute initial Lesson 10 Manual scan");
    trace.push(Sample {
        strategy,
        time_s: 0.0,
        phase: "safe_start",
        setpoint: 0.0,
        pv: read_lreal(&runtime, "%IL0"),
        requested: read_lreal(&runtime, "%ML8"),
        applied: read_lreal(&runtime, "%QL0"),
        tracking_time: read_lreal(&runtime, "%ML16"),
        configured_cycle_time: read_lreal(&runtime, "%ML56"),
        kp: read_lreal(&runtime, "%ML24"),
        ki: read_lreal(&runtime, "%ML32"),
        kd: read_lreal(&runtime, "%ML48"),
        auto: false,
    });

    write_bool(&mut runtime, "%IX80.0", true);
    for cycle in 1..=500 {
        let phase = match cycle {
            1..=9 => "safe_start",
            10..=199 => "unreachable_request",
            _ => "recovery",
        };
        runtime.execute_cycle().expect("execute Lesson 10 scan");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 10 vessel");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 10 temperature");
        trace.push(Sample {
            strategy,
            time_s: cycle as f64 * 0.1,
            phase,
            setpoint: read_lreal(&runtime, "%ML0"),
            pv: read_lreal(&runtime, "%IL0"),
            requested: read_lreal(&runtime, "%ML8"),
            applied: read_lreal(&runtime, "%QL0"),
            tracking_time: read_lreal(&runtime, "%ML16"),
            configured_cycle_time: read_lreal(&runtime, "%ML56"),
            kp: read_lreal(&runtime, "%ML24"),
            ki: read_lreal(&runtime, "%ML32"),
            kd: read_lreal(&runtime, "%ML48"),
            auto: true,
        });
    }

    runtime
        .execute_cycle()
        .expect("execute project-owned Lesson 10 Manual-zero restoration");
    trace.push(Sample {
        strategy,
        time_s: 50.1,
        phase: "safe_restoration",
        setpoint: 0.0,
        pv: read_lreal(&runtime, "%IL0"),
        requested: read_lreal(&runtime, "%ML8"),
        applied: read_lreal(&runtime, "%QL0"),
        tracking_time: read_lreal(&runtime, "%ML16"),
        configured_cycle_time: read_lreal(&runtime, "%ML56"),
        kp: read_lreal(&runtime, "%ML24"),
        ki: read_lreal(&runtime, "%ML32"),
        kd: read_lreal(&runtime, "%ML48"),
        auto: false,
    });
    trace
}

#[test]
fn control_curriculum_batch_vessel_project_starts_in_safe_manual() {
    let mut runtime = lesson_runtime();
    let mut simulation = lesson_simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 10 plant state");
    write_int(&mut runtime, "%IW12", 2);
    runtime
        .execute_cycle()
        .expect("execute shipped Lesson 10 safe initial scan");

    assert!(!read_bool(&runtime, "%MX64.0"));
    assert_eq!(read_lreal(&runtime, "%ML0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%ML8"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);

    write_bool(&mut runtime, "%IX80.0", true);
    for cycle in 1..=10 {
        runtime
            .execute_cycle()
            .expect("execute shipped Lesson 10 project sequence");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 10 vessel during project sequence");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 10 temperature during project sequence");
    }
    assert!(read_bool(&runtime, "%MX64.0"));
    assert_eq!(read_lreal(&runtime, "%ML0"), 4.5);
    assert_eq!(read_lreal(&runtime, "%ML8"), 3.5);
    assert_eq!(read_lreal(&runtime, "%QL0"), 3.5);
}

fn metrics(trace: &[Sample]) -> Metrics {
    let recovery = trace
        .iter()
        .filter(|sample| sample.phase == "recovery" && sample.auto)
        .collect::<Vec<_>>();
    let recovery_time_s = recovery
        .iter()
        .rposition(|sample| sample.pv.abs() > 0.05)
        .map(|index| (index + 1) as f64 * 0.1)
        .unwrap_or(0.0);
    let upper_limit_time_s = recovery
        .iter()
        .take_while(|sample| sample.applied >= 3.5 - 1.0e-9)
        .count() as f64
        * 0.1;
    let undershoot = -recovery
        .iter()
        .map(|sample| sample.pv)
        .fold(0.0_f64, f64::min);
    let travel = trace
        .windows(2)
        .map(|samples| (samples[1].applied - samples[0].applied).abs())
        .sum();
    let peak_unavailable_request = trace
        .iter()
        .filter(|sample| sample.auto)
        .map(|sample| sample.requested - sample.applied)
        .fold(0.0_f64, f64::max);
    Metrics {
        recovery_time_s,
        upper_limit_time_s,
        undershoot,
        travel,
        peak_unavailable_request,
    }
}

fn strategy_name(strategy: i16) -> &'static str {
    match strategy {
        0 => "none",
        1 => "signal_clamping",
        2 => "back_calculation",
        3 => "combined",
        _ => panic!("unsupported Lesson 10 strategy {strategy}"),
    }
}

fn run_id(strategy: i16) -> String {
    format!("lesson-10-{}-seed-{SEED}", strategy_name(strategy))
}

fn strategy_complexity(strategy: usize) -> usize {
    match strategy {
        1 => 0,
        2 => 1,
        3 => 2,
        _ => usize::MAX,
    }
}

fn distinctly_lower(candidate: f64, current: f64, threshold: f64) -> bool {
    const COMPARISON_EPSILON: f64 = 1.0e-12;
    candidate + threshold <= current + COMPARISON_EPSILON
}

fn indistinguishable(left: f64, right: f64, threshold: f64) -> bool {
    const COMPARISON_EPSILON: f64 = 1.0e-12;
    (left - right).abs() + COMPARISON_EPSILON < threshold
}

fn decision(results: &[Metrics]) -> (Vec<bool>, usize) {
    let baseline_recovery = results[0].recovery_time_s;
    let eligible = results
        .iter()
        .enumerate()
        .map(|(strategy, result)| {
            strategy > 0
                && result.recovery_time_s <= baseline_recovery - 1.0
                && result.upper_limit_time_s <= 0.1
                && result.undershoot <= 0.05
        })
        .collect::<Vec<_>>();
    let mut selected = eligible
        .iter()
        .position(|candidate| *candidate)
        .expect("Lesson 10 requires at least one eligible protected strategy");
    for candidate in selected + 1..results.len() {
        if !eligible[candidate] {
            continue;
        }
        let recovery_better = distinctly_lower(
            results[candidate].recovery_time_s,
            results[selected].recovery_time_s,
            0.1,
        );
        let recovery_tied = indistinguishable(
            results[candidate].recovery_time_s,
            results[selected].recovery_time_s,
            0.1,
        );
        let travel_better =
            distinctly_lower(results[candidate].travel, results[selected].travel, 0.1);
        let travel_tied =
            indistinguishable(results[candidate].travel, results[selected].travel, 0.1);
        if recovery_better
            || (recovery_tied && travel_better)
            || (recovery_tied
                && travel_tied
                && strategy_complexity(candidate) < strategy_complexity(selected))
        {
            selected = candidate;
        }
    }
    (eligible, selected)
}

fn write_evidence(traces: &[Vec<Sample>], results: &[Metrics]) {
    let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("create Lesson 10 evidence directory");
    let mut csv = String::from(
        "run_id,strategy,time_s,phase,setpoint_degC,pv_degC,requested_mv_percent,applied_mv_percent,tracking_time_s,configured_cycle_time_s,kp_percent_per_degC,ki_percent_per_degC_s,kd_percent_s_per_degC,internal_mv_min_percent,internal_mv_max_percent,physical_mv_min_percent,physical_mv_max_percent,seed,auto_mode,abort_state,evidence_class\n",
    );
    for trace in traces {
        for sample in trace {
            let (internal_min, internal_max) = if sample.strategy == 0 {
                (-1_000_000.0, 1_000_000.0)
            } else {
                (-3.5, 3.5)
            };
            writeln!(
                csv,
                "{},{},{:.1},{},{:.1},{:.12},{:.12},{:.12},{:.2},{:.1},{:.12},{:.12},{:.1},{internal_min:.1},{internal_max:.1},-3.5,3.5,{SEED},{},none,simulation",
                run_id(sample.strategy),
                strategy_name(sample.strategy),
                sample.time_s,
                sample.phase,
                sample.setpoint,
                sample.pv,
                sample.requested,
                sample.applied,
                sample.tracking_time,
                sample.configured_cycle_time,
                sample.kp,
                sample.ki,
                sample.kd,
                sample.auto,
            )
            .expect("append Lesson 10 evidence row");
        }
    }
    std::fs::write(directory.join("anti-windup-trace.csv"), csv).expect("write Lesson 10 trace");

    let mut summary = String::from(
        "run_id,strategy,recovery_time_s,post_restoration_upper_limit_s,recovery_undershoot_degC,total_applied_valve_travel_pp,peak_unavailable_request_pp,finite,physical_limit_valid,upper_limit_exercised,safe_restoration,eligible,selected,validity,abort_state,conclusion,evidence_class\n",
    );
    let (eligible, selected) = decision(results);
    for (strategy, result) in results.iter().enumerate() {
        let strategy = strategy as i16;
        let is_eligible = eligible[strategy as usize];
        let is_selected = strategy as usize == selected;
        let conclusion = match (strategy, is_eligible, is_selected) {
            (0, _, _) => "windup_reproduced_not_eligible",
            (_, true, true) => "selected_shortest_recovery_eligible_strategy",
            (_, true, false) => "eligible_not_selected",
            _ => "protected_strategy_not_eligible",
        };
        writeln!(
            summary,
            "{},{},{:.1},{:.1},{:.12},{:.12},{:.12},true,true,true,true,{is_eligible},{is_selected},valid,none,{conclusion},simulation",
            run_id(strategy),
            strategy_name(strategy),
            result.recovery_time_s,
            result.upper_limit_time_s,
            result.undershoot,
            result.travel,
            result.peak_unavailable_request,
        )
        .expect("append Lesson 10 summary row");
    }
    std::fs::write(directory.join("anti-windup-summary.csv"), summary)
        .expect("write Lesson 10 summary");
}

#[test]
fn control_curriculum_batch_vessel_retains_equivalent_pi_anti_windup_runs() {
    let traces = (0..=3).map(run_strategy).collect::<Vec<_>>();
    let results = traces
        .iter()
        .map(|trace| metrics(trace))
        .collect::<Vec<_>>();
    let expected_recovery = [20.0, 13.1, 2.7, 9.2];
    let expected_travel = [
        7.169437650682,
        14.699941789530,
        14.462547403916,
        14.507017477152,
    ];

    for (strategy, (trace, result)) in traces.iter().zip(&results).enumerate() {
        assert_eq!(trace.len(), 502, "retain initial and final Manual samples");
        let initial = trace.first().expect("Lesson 10 initial Manual sample");
        assert_eq!(initial.phase, "safe_start");
        assert_eq!(initial.time_s, 0.0);
        assert!(!initial.auto);
        assert_eq!(initial.setpoint, 0.0);
        assert_eq!(initial.requested, 0.0);
        assert_eq!(initial.applied, 0.0);
        assert!(trace[1..501].iter().all(|sample| {
            sample.pv.is_finite()
                && sample.requested.is_finite()
                && sample.applied.is_finite()
                && (-3.5..=3.5).contains(&sample.applied)
                && sample.configured_cycle_time == 0.1
                && sample.kd == 0.0
        }));
        assert!(trace[1..501]
            .iter()
            .any(|sample| sample.applied >= 3.5 - 1.0e-9));
        let safe = trace.last().expect("Lesson 10 safe sample");
        assert!(!safe.auto && safe.setpoint == 0.0 && safe.requested == 0.0 && safe.applied == 0.0);
        assert!((result.recovery_time_s - expected_recovery[strategy]).abs() <= 0.1);
        assert!((result.travel - expected_travel[strategy]).abs() <= 1.0e-9);
    }
    assert!(results[0].upper_limit_time_s >= 4.5);
    assert!(results[0].peak_unavailable_request >= 26.0);
    assert!(results[1..]
        .iter()
        .all(|result| result.upper_limit_time_s <= 0.1));
    assert!(results[2].undershoot <= 0.05);
    assert!(results[2].recovery_time_s < results[1].recovery_time_s);
    assert!(results[2].recovery_time_s < results[3].recovery_time_s);
    eprintln!(
        "Lesson 10 PI metrics [recovery, hold, undershoot, travel, gap]: {:?}",
        results
            .iter()
            .map(|value| (
                value.recovery_time_s,
                value.upper_limit_time_s,
                value.undershoot,
                value.travel,
                value.peak_unavailable_request
            ))
            .collect::<Vec<_>>()
    );
    write_evidence(&traces, &results);
}

#[test]
fn control_curriculum_batch_vessel_artifacts_identify_and_dispose_every_run() {
    let trace_header = RETAINED_TRACE
        .lines()
        .next()
        .expect("Lesson 10 trace header");
    let trace_columns = trace_header.split(',').collect::<Vec<_>>();
    let trace_index = |name: &str| {
        trace_columns
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("Lesson 10 trace field '{name}'"))
    };
    let rows = RETAINED_TRACE.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(rows.len(), 2008, "four runs must each retain 502 samples");
    let mut counts = [0_usize; 4];
    for row in rows {
        let values = row.split(',').collect::<Vec<_>>();
        assert_eq!(values.len(), trace_columns.len());
        let strategy = match values[trace_index("strategy")] {
            "none" => 0,
            "signal_clamping" => 1,
            "back_calculation" => 2,
            "combined" => 3,
            other => panic!("unknown retained Lesson 10 strategy {other}"),
        };
        counts[strategy] += 1;
        assert_eq!(values[trace_index("seed")], SEED.to_string());
        assert_eq!(values[trace_index("configured_cycle_time_s")], "0.1");
        assert_eq!(values[trace_index("kd_percent_s_per_degC")], "0.0");
        assert_eq!(values[trace_index("abort_state")], "none");
        assert_eq!(values[trace_index("evidence_class")], "simulation");
    }
    assert_eq!(counts, [502, 502, 502, 502]);

    let summary_header = RETAINED_SUMMARY
        .lines()
        .next()
        .expect("Lesson 10 summary header");
    let summary_columns = summary_header.split(',').collect::<Vec<_>>();
    let summary_index = |name: &str| {
        summary_columns
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("Lesson 10 summary field '{name}'"))
    };
    let summary_rows = RETAINED_SUMMARY.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(summary_rows.len(), 4);
    let mut selected = Vec::new();
    for row in summary_rows {
        let values = row.split(',').collect::<Vec<_>>();
        assert_eq!(values[summary_index("validity")], "valid");
        assert_eq!(values[summary_index("safe_restoration")], "true");
        assert_eq!(values[summary_index("abort_state")], "none");
        assert_eq!(values[summary_index("evidence_class")], "simulation");
        if values[summary_index("selected")] == "true" {
            selected.push(values[summary_index("strategy")]);
        }
    }
    assert_eq!(selected, ["back_calculation"]);
}

#[test]
fn control_curriculum_batch_vessel_selection_matches_predeclared_rules() {
    let mut rows = RETAINED_SUMMARY.lines();
    let header = rows
        .next()
        .expect("Lesson 10 retained summary header")
        .split(',')
        .collect::<Vec<_>>();
    let column = |name: &str| {
        header
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("Lesson 10 summary field '{name}'"))
    };
    let parsed = rows
        .map(|row| {
            let values = row.split(',').collect::<Vec<_>>();
            (
                values[column("strategy")],
                values[column("recovery_time_s")].parse::<f64>().unwrap(),
                values[column("post_restoration_upper_limit_s")]
                    .parse::<f64>()
                    .unwrap(),
                values[column("recovery_undershoot_degC")]
                    .parse::<f64>()
                    .unwrap(),
                values[column("total_applied_valve_travel_pp")]
                    .parse::<f64>()
                    .unwrap(),
                values[column("eligible")] == "true",
                values[column("selected")] == "true",
            )
        })
        .collect::<Vec<_>>();
    let baseline_recovery = parsed[0].1;
    let expected_eligible = parsed
        .iter()
        .enumerate()
        .map(|(index, row)| {
            index > 0 && row.1 <= baseline_recovery - 1.0 && row.2 <= 0.1 && row.3 <= 0.05
        })
        .collect::<Vec<_>>();
    assert_eq!(
        parsed.iter().map(|row| row.5).collect::<Vec<_>>(),
        expected_eligible,
        "eligibility must follow the declared recovery, hold, and undershoot rules"
    );

    let mut selected_index = expected_eligible
        .iter()
        .position(|eligible| *eligible)
        .expect("at least one eligible protected strategy");
    for candidate in selected_index + 1..parsed.len() {
        if !expected_eligible[candidate] {
            continue;
        }
        let recovery_better = parsed[candidate].1 + 0.1 <= parsed[selected_index].1 + 1.0e-12;
        let recovery_tied = (parsed[candidate].1 - parsed[selected_index].1).abs() + 1.0e-12 < 0.1;
        let travel_better = parsed[candidate].4 + 0.1 <= parsed[selected_index].4 + 1.0e-12;
        let travel_tied = (parsed[candidate].4 - parsed[selected_index].4).abs() + 1.0e-12 < 0.1;
        if recovery_better
            || (recovery_tied && travel_better)
            || (recovery_tied
                && travel_tied
                && strategy_complexity(candidate) < strategy_complexity(selected_index))
        {
            selected_index = candidate;
        }
    }
    assert_eq!(
        parsed
            .iter()
            .enumerate()
            .filter_map(|(index, row)| row.6.then_some(index))
            .collect::<Vec<_>>(),
        vec![selected_index],
        "the retained selection must be derivable from the predeclared rules"
    );
}

#[test]
fn control_curriculum_batch_vessel_selection_honours_declared_tie_boundaries() {
    let metric = |recovery_time_s: f64, undershoot: f64, travel: f64| Metrics {
        recovery_time_s,
        upper_limit_time_s: 0.0,
        undershoot,
        travel,
        peak_unavailable_request: 0.0,
    };

    let exact_recovery_boundary = [
        metric(20.0, 0.0, 0.0),
        metric(5.0, 0.0, 10.0),
        metric(4.9, 0.0, 20.0),
        metric(10.0, 1.0, 0.0),
    ];
    assert_eq!(
        decision(&exact_recovery_boundary).1,
        2,
        "exactly 0.1 second faster is distinguishable even with higher travel"
    );

    let exact_travel_boundary = [
        metric(20.0, 0.0, 0.0),
        metric(5.0, 0.0, 10.0),
        metric(5.0, 0.0, 9.9),
        metric(10.0, 1.0, 0.0),
    ];
    assert_eq!(
        decision(&exact_travel_boundary).1,
        2,
        "exactly 0.1 percentage point less travel is distinguishable"
    );

    let indistinguishable_metrics = [
        metric(20.0, 0.0, 0.0),
        metric(5.0, 0.0, 10.0),
        metric(4.95, 0.0, 9.95),
        metric(10.0, 1.0, 0.0),
    ];
    assert_eq!(
        decision(&indistinguishable_metrics).1,
        1,
        "differences below both thresholds must resolve to the simpler state equation"
    );
}
