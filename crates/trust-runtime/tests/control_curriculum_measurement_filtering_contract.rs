use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/13-measurement-noise/src/Main.st");
const SEED: u64 = 20260824;
const SELECTIONS: [i16; 3] = [0, 1, 2];
const ATTENUATION_LIMIT: f64 = 0.80;
const ATTENUATION_TOLERANCE: f64 = 0.01;
const LAG_LIMIT_BAR_S: f64 = 0.08;
const LAG_TOLERANCE_BAR_S: f64 = 0.005;
const BIAS_LIMIT_BAR: f64 = 0.05;
const BIAS_TOLERANCE_BAR: f64 = 0.001;
const NOISE_AGREEMENT_TOLERANCE_BAR: f64 = 1.0e-12;

#[derive(Clone, Copy, Debug)]
struct Sample {
    selection: i16,
    time_s: f64,
    setpoint: f64,
    true_pressure: f64,
    raw_pressure: f64,
    controller_pressure: f64,
    applied_valve: f64,
    auto: bool,
}

#[derive(Clone, Copy, Debug)]
struct Metrics {
    controller_increment_rms_bar: f64,
    valve_increment_rms_pp: f64,
    valve_travel_pp: f64,
    lag_area_bar_s: f64,
    final_window_absolute_mean_error_bar: f64,
}

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 13 program before configuration")
        .replace(
            "    AutoMode : BOOL :=",
            "    AutoMode AT %MX16.0 : BOOL :=",
        )
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %IX88.0 : BOOL :=",
        )
        .replace(
            "    FilterSelection : INT :=",
            "    FilterSelection AT %MW40 : INT :=",
        )
        .replace("    Setpoint : LREAL :=", "    Setpoint AT %ML0 : LREAL :=")
        .replace(
            "    ProcessValue : LREAL :=",
            "    ProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "    SimulatorTruePressure : LREAL :=",
            "    SimulatorTruePressure AT %IL8 : LREAL :=",
        )
        .replace(
            "    FilteredValue : LREAL;",
            "    FilteredValue AT %ML8 : LREAL;",
        )
        .replace(
            "    ManipulatedValue : LREAL :=",
            "    ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace(
            "    ManualOutput : LREAL :=",
            "    ManualOutput AT %ML72 : LREAL :=",
        )
}

fn runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("measurement-noise/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 13 Structured Text")
}

fn simulation() -> SimulationController {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/13-measurement-noise/simulation.toml");
    SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load shipped Lesson 13 simulation: {error}")),
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

fn write_int(runtime: &mut Runtime, address: &str, value: i16) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("INT address"),
            Value::Word(value as u16),
        )
        .expect("write INT process-image value");
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

fn sample(
    selection: i16,
    time_s: f64,
    setpoint: f64,
    true_pressure: f64,
    raw_pressure: f64,
    runtime: &Runtime,
) -> Sample {
    Sample {
        selection,
        time_s,
        setpoint,
        true_pressure,
        raw_pressure,
        controller_pressure: read_lreal(runtime, "%ML8"),
        applied_valve: read_lreal(runtime, "%QL0"),
        auto: read_bool(runtime, "%MX16.0"),
    }
}

fn run(selection: i16) -> Vec<Sample> {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 13 pressure");
    write_int(&mut runtime, "%MW40", selection);
    write_bool(&mut runtime, "%IX88.0", false);
    let initial_raw = read_lreal(&runtime, "%IL0");
    let initial_true = read_lreal(&runtime, "%IL8");
    runtime
        .execute_cycle()
        .expect("retain initial Manual-zero sample");

    let mut trace = Vec::with_capacity(402);
    trace.push(sample(
        selection,
        0.0,
        0.0,
        initial_true,
        initial_raw,
        &runtime,
    ));
    write_bool(&mut runtime, "%IX88.0", true);

    for cycle in 1..=400 {
        let time_s = cycle as f64 * 0.1;
        let setpoint = if cycle <= 50 { 0.0 } else { 1.0 };
        let raw = read_lreal(&runtime, "%IL0");
        let true_pressure = read_lreal(&runtime, "%IL8");
        runtime.execute_cycle().expect("execute Lesson 13 PI scan");
        trace.push(sample(
            selection,
            time_s,
            setpoint,
            true_pressure,
            raw,
            &runtime,
        ));
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 13 pressure plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 13 pressure measurements");
    }

    let raw = read_lreal(&runtime, "%IL0");
    let true_pressure = read_lreal(&runtime, "%IL8");
    runtime
        .execute_cycle()
        .expect("restore Lesson 13 Manual zero");
    trace.push(sample(selection, 40.1, 0.0, true_pressure, raw, &runtime));
    write_bool(&mut runtime, "%IX88.0", false);
    runtime
        .execute_cycle()
        .expect("restore Lesson 13 experiment authority false");
    trace
}

fn increment_rms(values: &[f64]) -> f64 {
    let squared = values
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).powi(2))
        .sum::<f64>();
    (squared / (values.len() - 1) as f64).sqrt()
}

fn metrics(trace: &[Sample]) -> Metrics {
    let steady = trace
        .iter()
        .filter(|sample| sample.auto && (20.1..=40.0).contains(&sample.time_s))
        .collect::<Vec<_>>();
    let controller = steady
        .iter()
        .map(|sample| sample.controller_pressure)
        .collect::<Vec<_>>();
    let valve = steady
        .iter()
        .map(|sample| sample.applied_valve)
        .collect::<Vec<_>>();
    let response = trace
        .iter()
        .filter(|sample| sample.auto && (5.1..=15.0).contains(&sample.time_s));
    let final_window = trace
        .iter()
        .filter(|sample| sample.auto && (35.1..=40.0).contains(&sample.time_s))
        .collect::<Vec<_>>();
    Metrics {
        controller_increment_rms_bar: increment_rms(&controller),
        valve_increment_rms_pp: increment_rms(&valve),
        valve_travel_pp: valve.windows(2).map(|pair| (pair[1] - pair[0]).abs()).sum(),
        lag_area_bar_s: response
            .map(|sample| (sample.raw_pressure - sample.controller_pressure) * 0.1)
            .sum(),
        final_window_absolute_mean_error_bar: (final_window
            .iter()
            .map(|sample| sample.setpoint - sample.controller_pressure)
            .sum::<f64>()
            / final_window.len() as f64)
            .abs(),
    }
}

fn upper_pass(value: f64, limit: f64, tolerance: f64) -> bool {
    value.is_finite() && value <= limit + tolerance
}

fn filter_name(selection: i16) -> &'static str {
    match selection {
        0 => "bypass",
        1 => "first_order",
        2 => "second_order",
        _ => "invalid",
    }
}

fn run_id(selection: i16) -> String {
    format!("lesson-13-{}-seed-{SEED}", filter_name(selection))
}

fn write_evidence(
    traces: &[Vec<Sample>; 3],
    results: &[Metrics; 3],
    pressure_ratios: &[f64; 3],
    valve_ratios: &[f64; 3],
    qualified: &[bool],
    selected: i16,
    maximum_noise_disagreement: f64,
) {
    let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("create Lesson 13 evidence directory");

    let mut trace_csv = String::from(
        "run_id,filter,filter_selection,time_s,phase,event,setpoint_bar,simulator_true_pressure_bar,measurement_noise_bar,raw_pv_bar,controller_pv_bar,error_bar,applied_mv_pp,kp_percent_per_bar,ki_percent_per_bar_s,kd_percent_s_per_bar,filter_tau_s,cycle_time_s,filter_coefficient,output_min_pp,output_max_pp,plant_gain_bar_per_percent,plant_time_constant_s,plant_dead_time_s,noise_standard_deviation_bar,seed,auto_mode,safe_restoration,abort_state,evidence_class\n",
    );
    for trace in traces {
        for sample in trace {
            let phase = if sample.time_s == 0.0 {
                "safe_start"
            } else if sample.time_s <= 5.0 {
                "noisy_baseline"
            } else if sample.time_s <= 20.0 {
                "setpoint_response"
            } else if sample.time_s <= 40.0 {
                "steady_noise_window"
            } else {
                "safe_restoration"
            };
            let event = if (sample.time_s - 5.1).abs() <= 1.0e-9 {
                "setpoint_step"
            } else if (sample.time_s - 40.1).abs() <= 1.0e-9 {
                "manual_zero"
            } else {
                "none"
            };
            let safe_restoration = if sample.time_s > 40.0 {
                if sample.applied_valve.abs() <= 1.0e-9 {
                    "achieved"
                } else {
                    "in_progress"
                }
            } else {
                "not_applicable"
            };
            writeln!(
                trace_csv,
                "{},{},{},{:.1},{},{},{:.1},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},0.667,0.667,0.0,0.1,0.1,{:.15},-10.0,10.0,1.0,1.0,0.5,0.02,{SEED},{},{safe_restoration},none,simulation",
                run_id(sample.selection),
                filter_name(sample.selection),
                sample.selection,
                sample.time_s,
                phase,
                event,
                sample.setpoint,
                sample.true_pressure,
                sample.raw_pressure - sample.true_pressure,
                sample.raw_pressure,
                sample.controller_pressure,
                sample.setpoint - sample.controller_pressure,
                sample.applied_valve,
                2.0 / 3.0,
                sample.auto,
            )
            .expect("append Lesson 13 trace row");
        }
    }
    std::fs::write(directory.join("measurement-filter-trace.csv"), trace_csv)
        .expect("write Lesson 13 trace");

    let mut summary = String::from(
        "run_id,filter,filter_selection,filter_poles,controller_id,kp_percent_per_bar,ki_percent_per_bar_s,kd_percent_s_per_bar,filter_tau_s,cycle_time_s,filter_coefficient,sampling_frequency_hz,nyquist_frequency_hz,continuous_reference_cutoff_hz,discrete_minus_3db_hz,noise_seed,steady_window_start_s,steady_window_end_s,response_window_start_s,response_window_end_s,final_window_start_s,final_window_end_s,controller_pv_increment_rms_bar,pressure_attenuation_ratio,pressure_attenuation_limit,pressure_attenuation_tolerance,pressure_attenuation_pass,mv_increment_rms_pp,valve_attenuation_ratio,valve_attenuation_limit,valve_attenuation_tolerance,valve_attenuation_pass,total_applied_valve_travel_pp,travel_physical_threshold,lag_area_bar_s,lag_limit_bar_s,lag_tolerance_bar_s,lag_pass,final_window_absolute_mean_error_bar,bias_limit_bar,bias_tolerance_bar,bias_pass,maximum_noise_disagreement_bar,noise_agreement_tolerance_bar,noise_equivalent,numerically_valid,physical_criterion_passed,eligible,selected,safe_restoration,abort_state,evidence_class\n",
    );
    for index in 0..3 {
        let selection = SELECTIONS[index];
        let pressure_pass = selection != 0
            && upper_pass(
                pressure_ratios[index],
                ATTENUATION_LIMIT,
                ATTENUATION_TOLERANCE,
            );
        let valve_pass = selection != 0
            && upper_pass(
                valve_ratios[index],
                ATTENUATION_LIMIT,
                ATTENUATION_TOLERANCE,
            );
        let lag_pass = upper_pass(
            results[index].lag_area_bar_s,
            LAG_LIMIT_BAR_S,
            LAG_TOLERANCE_BAR_S,
        );
        let bias_pass = upper_pass(
            results[index].final_window_absolute_mean_error_bar,
            BIAS_LIMIT_BAR,
            BIAS_TOLERANCE_BAR,
        );
        let noise_equivalent = maximum_noise_disagreement <= NOISE_AGREEMENT_TOLERANCE_BAR;
        let discrete_cutoff = match selection {
            1 => "1.959132760153",
            2 => "1.211836153077",
            _ => "",
        };
        writeln!(
            summary,
            "{},{},{},{},Control.PID.PI,0.667,0.667,0.0,0.1,0.1,{:.15},10.0,5.0,1.591549430919,{discrete_cutoff},{SEED},20.1,40.0,5.1,15.0,35.1,40.0,{:.12},{:.12},0.80,0.01,{pressure_pass},{:.12},{:.12},0.80,0.01,{valve_pass},{:.12},none,{:.12},0.08,0.005,{lag_pass},{:.12},0.05,0.001,{bias_pass},{maximum_noise_disagreement:.15},1e-12,{noise_equivalent},true,{},{},{},achieved,none,simulation",
            run_id(selection),
            filter_name(selection),
            selection,
            selection,
            2.0 / 3.0,
            results[index].controller_increment_rms_bar,
            pressure_ratios[index],
            results[index].valve_increment_rms_pp,
            valve_ratios[index],
            results[index].valve_travel_pp,
            results[index].lag_area_bar_s,
            results[index].final_window_absolute_mean_error_bar,
            qualified[index],
            selection != 0,
            selection == selected,
        )
        .expect("append Lesson 13 summary row");
    }
    std::fs::write(directory.join("measurement-filter-summary.csv"), summary)
        .expect("write Lesson 13 summary");
}

#[test]
fn lesson_13_rejects_an_invalid_filter_selection_to_manual_zero() {
    let mut runtime = runtime();
    write_lreal(&mut runtime, "%ML0", 0.0);
    write_lreal(&mut runtime, "%IL0", 0.1);
    write_lreal(&mut runtime, "%ML72", 6.5);
    write_int(&mut runtime, "%MW40", 7);
    write_bool(&mut runtime, "%MX16.0", true);

    runtime
        .execute_cycle()
        .expect("execute invalid Lesson 13 configuration safely");

    assert_eq!(
        read_lreal(&runtime, "%QL0"),
        0.0,
        "an invalid filter selection must remove automatic authority and apply Manual zero"
    );
}

#[test]
fn lesson_13_project_executes_the_selected_filter_sequence() {
    let mut runtime = runtime();
    let mut simulation = simulation();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 13 pressure");
    write_int(&mut runtime, "%MW40", 1);
    write_bool(&mut runtime, "%IX88.0", false);
    runtime.execute_cycle().expect("retain safe initial sample");
    assert!(!read_bool(&runtime, "%MX16.0"));
    assert_eq!(read_lreal(&runtime, "%ML0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);

    write_bool(&mut runtime, "%IX88.0", true);
    for cycle in 1..=401 {
        let raw = read_lreal(&runtime, "%IL0");
        let true_pressure = read_lreal(&runtime, "%IL8");
        runtime.execute_cycle().expect("execute project filter scan");
        if cycle == 1 || cycle == 50 {
            assert!(read_bool(&runtime, "%MX16.0"));
            assert_eq!(read_lreal(&runtime, "%ML0"), 0.0);
        }
        if cycle == 51 || cycle == 400 {
            assert!(read_bool(&runtime, "%MX16.0"));
            assert_eq!(read_lreal(&runtime, "%ML0"), 1.0);
        }
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance project pressure plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish project pressure measurements");
        assert!(raw.is_finite() && true_pressure.is_finite());
    }
    assert!(!read_bool(&runtime, "%MX16.0"));
    assert_eq!(read_lreal(&runtime, "%ML0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);

    write_bool(&mut runtime, "%IX88.0", false);
    runtime.execute_cycle().expect("restore experiment authority false");
}

#[test]
fn lesson_13_executes_equivalent_runs_and_selects_the_weakest_qualified_filter() {
    let program = LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 13 program before configuration");
    assert!(
        program.matches("SimulatorTruePressure").count() == 1,
        "the controller program may declare but must not read the simulator-only true-pressure channel"
    );
    let traces = SELECTIONS.map(run);
    let results: [Metrics; 3] = std::array::from_fn(|index| metrics(&traces[index]));

    for trace in &traces {
        assert_eq!(trace.first().expect("initial sample").applied_valve, 0.0);
        let restored = trace.last().expect("Manual-zero restoration sample");
        assert!(!restored.auto && restored.applied_valve.abs() <= 1.0e-9);
        assert!(trace.iter().all(|sample| {
            sample.true_pressure.is_finite()
                && sample.raw_pressure.is_finite()
                && sample.controller_pressure.is_finite()
                && sample.applied_valve.is_finite()
                && (-5.0..=5.0).contains(&sample.raw_pressure)
                && (-5.0..=5.0).contains(&sample.controller_pressure)
                && (-10.0..=10.0).contains(&sample.applied_valve)
        }));
    }

    let maximum_noise_disagreement = traces[0]
        .iter()
        .zip(&traces[1])
        .zip(&traces[2])
        .map(|((bypass, first), second)| {
            let noises = [
                bypass.raw_pressure - bypass.true_pressure,
                first.raw_pressure - first.true_pressure,
                second.raw_pressure - second.true_pressure,
            ];
            (noises[0] - noises[1])
                .abs()
                .max((noises[0] - noises[2]).abs())
        })
        .fold(0.0_f64, f64::max);
    assert!(
        maximum_noise_disagreement <= NOISE_AGREEMENT_TOLERANCE_BAR,
        "fresh runs must reproduce one aligned noise realization; maximum disagreement={maximum_noise_disagreement} bar"
    );

    let pressure_ratios = [
        1.0,
        results[1].controller_increment_rms_bar / results[0].controller_increment_rms_bar,
        results[2].controller_increment_rms_bar / results[0].controller_increment_rms_bar,
    ];
    let valve_ratios = [
        1.0,
        results[1].valve_increment_rms_pp / results[0].valve_increment_rms_pp,
        results[2].valve_increment_rms_pp / results[0].valve_increment_rms_pp,
    ];
    let qualified = [false, true, true].map(|candidate| candidate);
    let qualified = qualified
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| {
            candidate
                && upper_pass(
                    pressure_ratios[index],
                    ATTENUATION_LIMIT,
                    ATTENUATION_TOLERANCE,
                )
                && upper_pass(
                    valve_ratios[index],
                    ATTENUATION_LIMIT,
                    ATTENUATION_TOLERANCE,
                )
                && upper_pass(
                    results[index].lag_area_bar_s,
                    LAG_LIMIT_BAR_S,
                    LAG_TOLERANCE_BAR_S,
                )
                && upper_pass(
                    results[index].final_window_absolute_mean_error_bar,
                    BIAS_LIMIT_BAR,
                    BIAS_TOLERANCE_BAR,
                )
        })
        .collect::<Vec<_>>();
    let selected = SELECTIONS
        .into_iter()
        .zip(&qualified)
        .find(|(_, passes)| **passes)
        .map(|(selection, _)| selection)
        .expect("at least one measurement filter must satisfy the declared contract");

    eprintln!("Lesson 13 metrics: {results:#?}");
    eprintln!("Lesson 13 pressure attenuation ratios: {pressure_ratios:?}");
    eprintln!("Lesson 13 valve attenuation ratios: {valve_ratios:?}");
    eprintln!("Lesson 13 qualified: {qualified:?}; selected={selected}");
    assert_eq!(selected, 1, "the fewest qualifying poles must be selected");
    write_evidence(
        &traces,
        &results,
        &pressure_ratios,
        &valve_ratios,
        &qualified,
        selected,
        maximum_noise_disagreement,
    );
}

#[test]
fn lesson_13_retained_artifacts_preserve_filter_qualification_decisions() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/13-measurement-noise/figures");
    let trace_path = directory.join("measurement-filter-trace.csv");
    let summary_path = directory.join("measurement-filter-summary.csv");
    assert!(
        trace_path.is_file() && summary_path.is_file(),
        "Lesson 13 must retain the native three-run trace and qualification summary"
    );
    let trace = std::fs::read_to_string(trace_path).expect("read Lesson 13 trace");
    let trace_header = trace.lines().next().expect("Lesson 13 trace header");
    for field in [
        "simulator_true_pressure_bar",
        "measurement_noise_bar",
        "raw_pv_bar",
        "controller_pv_bar",
        "applied_mv_pp",
        "filter_coefficient",
        "safe_restoration",
        "evidence_class",
    ] {
        assert!(trace_header.split(',').any(|candidate| candidate == field));
    }
    assert_eq!(trace.lines().count() - 1, 1_206);

    let summary = std::fs::read_to_string(summary_path).expect("read Lesson 13 summary");
    let mut lines = summary.lines();
    let header = lines
        .next()
        .expect("Lesson 13 summary header")
        .split(',')
        .collect::<Vec<_>>();
    let column = |name: &str| {
        header
            .iter()
            .position(|candidate| *candidate == name)
            .unwrap_or_else(|| panic!("Lesson 13 summary field '{name}'"))
    };
    for field in [
        "continuous_reference_cutoff_hz",
        "discrete_minus_3db_hz",
        "pressure_attenuation_ratio",
        "valve_attenuation_ratio",
        "total_applied_valve_travel_pp",
        "travel_physical_threshold",
        "lag_area_bar_s",
        "final_window_absolute_mean_error_bar",
        "maximum_noise_disagreement_bar",
        "physical_criterion_passed",
        "eligible",
        "selected",
        "safe_restoration",
    ] {
        column(field);
    }
    let rows = lines
        .map(|line| line.split(',').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter()
            .filter(|row| row[column("selected")] == "true")
            .count(),
        1
    );
    let selected = rows
        .iter()
        .find(|row| row[column("selected")] == "true")
        .expect("selected Lesson 13 row");
    assert_eq!(selected[column("filter")], "first_order");
    assert!(rows
        .iter()
        .all(|row| row[column("safe_restoration")] == "achieved"));
}
