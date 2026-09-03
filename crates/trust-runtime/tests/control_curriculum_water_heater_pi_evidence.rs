use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-pi/src/Main.st");
const SIMULATION_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-pi/simulation.toml");
const LESSON6_TRACE: &str = include_str!(
    "../../../examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-integral/figures/integral-comparison.csv"
);

#[derive(Clone, Copy)]
struct Sample {
    time_s: f64,
    setpoint: f64,
    pv: f64,
    requested: f64,
    applied: f64,
    proportional_increment: Option<f64>,
    integral_increment: Option<f64>,
    auto: bool,
}

struct Summary {
    settled: bool,
    pv_range: f64,
    requested_range: f64,
    applied_range: f64,
    early_error: f64,
    final_error: Option<f64>,
    recovery_s: Option<f64>,
    overshoot: f64,
    movement: f64,
    saturation_s: f64,
    eligible: bool,
}

fn addressed_source() -> String {
    let mut source = LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 7 program before configuration")
        .to_owned();
    if !source.contains("ManualHeaterPower : LREAL :=") {
        source = source.replace(
            "ManipulatedValue : LREAL := 0.0;",
            "ManipulatedValue : LREAL := 0.0;\n    ManualHeaterPower : LREAL := 0.0;",
        );
    }
    if !source.contains("RequestedHeaterPower : LREAL :=") {
        source = source.replace(
            "ManualHeaterPower : LREAL := 0.0;",
            "ManualHeaterPower : LREAL := 0.0;\n    RequestedHeaterPower : LREAL := 0.0;\n    ProportionalIncrement : LREAL := 0.0;\n    IntegralIncrement : LREAL := 0.0;",
        );
    }
    source
        .replace(
            "    ExperimentEnabled : BOOL :=",
            "    ExperimentEnabled AT %IX24.0 : BOOL :=",
        )
        .replace(
            "    ProportionalGain : LREAL :=",
            "    ProportionalGain AT %IL8 : LREAL :=",
        )
        .replace(
            "    IntegralGain : LREAL :=",
            "    IntegralGain AT %IL16 : LREAL :=",
        )
        .replace(
            "    Setpoint : LREAL :=",
            "    Setpoint AT %IL32 : LREAL :=",
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
            "    MeasuredHoldingOutput : LREAL :=",
            "    MeasuredHoldingOutput AT %ML48 : LREAL :=",
        )
        .replace(
            "    RequestedHeaterPower : LREAL :=",
            "    RequestedHeaterPower AT %ML24 : LREAL :=",
        )
        .replace(
            "    ProportionalIncrement : LREAL :=",
            "    ProportionalIncrement AT %ML32 : LREAL :=",
        )
        .replace(
            "    IntegralIncrement : LREAL :=",
            "    IntegralIncrement AT %ML40 : LREAL :=",
        )
        .replace(
            "    ManipulatedValue : LREAL :=",
            "    ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace(
            "    AppliedHeaterPower : LREAL :=",
            "    AppliedHeaterPower AT %QL0 : LREAL :=",
        )
}

fn source_with_default_observers() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 7 program before configuration")
        .replace(
            "PreviousProcessValue : LREAL := 20.0;",
            "PreviousProcessValue : LREAL := 20.0;\n    ObservedProportionalGain AT %ML56 : LREAL := 0.0;\n    ObservedIntegralGain AT %ML64 : LREAL := 0.0;",
        )
        .replace(
            "LastAutoMode := AutoMode;",
            "LastAutoMode := AutoMode;\nObservedProportionalGain := ProportionalGain;\nObservedIntegralGain := IntegralGain;",
        )
}

fn read_bool(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL value") {
        Value::Bool(value) => value,
        other => panic!("expected BOOL storage, got {other:?}"),
    }
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL process-image value");
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL process-image value");
}

fn temporary_config_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-water-heater-pi-{name}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn run_pi(integral_gain: f64, unreachable_setpoint: bool) -> Vec<Sample> {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-pi/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 7 Structured Text");
    let simulation_source = if unreachable_setpoint {
        SIMULATION_SOURCE
            .split("[[plant_events]]")
            .next()
            .expect("Lesson 7 plant before load event")
    } else {
        SIMULATION_SOURCE
    };
    let path = temporary_config_path(if unreachable_setpoint {
        "unreachable"
    } else {
        "normal"
    });
    std::fs::write(&path, simulation_source).expect("write Lesson 7 simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load Lesson 7 simulation config"),
    );
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let kp = IoAddress::parse("%IL8").expect("proportional-gain address");
    let ki = IoAddress::parse("%IL16").expect("integral-gain address");
    let setpoint = IoAddress::parse("%IL32").expect("setpoint address");
    let pv = IoAddress::parse("%IL0").expect("process-value address");
    let manual = IoAddress::parse("%ML16").expect("manual-output address");
    let requested = IoAddress::parse("%ML24").expect("requested-output address");
    let proportional = IoAddress::parse("%ML32").expect("proportional-increment address");
    let integral = IoAddress::parse("%ML40").expect("integral-increment address");
    let applied = IoAddress::parse("%QL0").expect("applied-output address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial Lesson 7 plant state");
    let mut trace = Vec::with_capacity(1_201);
    for cycle in 1..=1_200 {
        let sp = if unreachable_setpoint && cycle >= 600 {
            45.0
        } else {
            35.0
        };
        write_lreal(&mut runtime, &kp, 8.0);
        write_lreal(&mut runtime, &ki, integral_gain);
        write_lreal(&mut runtime, &setpoint, sp);
        write_lreal(&mut runtime, &manual, if cycle == 1 { 0.0 } else { 75.0 });
        write_bool(&mut runtime, &auto, cycle > 500);
        runtime.execute_cycle().expect("execute Lesson 7 PI scan");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 7 water heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 7 process value");
        trace.push(Sample {
            time_s: cycle as f64 * 0.1,
            setpoint: sp,
            pv: read_lreal(&runtime, &pv),
            requested: read_lreal(&runtime, &requested),
            applied: read_lreal(&runtime, &applied),
            proportional_increment: Some(read_lreal(&runtime, &proportional)),
            integral_increment: Some(read_lreal(&runtime, &integral)),
            auto: read_bool(&runtime, &auto),
        });
    }
    write_bool(&mut runtime, &auto, false);
    write_lreal(&mut runtime, &manual, 0.0);
    runtime
        .execute_cycle()
        .expect("restore Lesson 7 safe state");
    trace.push(Sample {
        time_s: 120.1,
        setpoint: if unreachable_setpoint { 45.0 } else { 35.0 },
        pv: read_lreal(&runtime, &pv),
        requested: read_lreal(&runtime, &requested),
        applied: read_lreal(&runtime, &applied),
        proportional_increment: Some(read_lreal(&runtime, &proportional)),
        integral_increment: Some(read_lreal(&runtime, &integral)),
        auto: read_bool(&runtime, &auto),
    });
    let _ = std::fs::remove_file(path);
    trace
}

fn inherited_baselines() -> Vec<(&'static str, Vec<Sample>)> {
    let mut biased = Vec::new();
    let mut integral = Vec::new();
    for row in LESSON6_TRACE.lines().skip(1) {
        let fields = row.split(',').collect::<Vec<_>>();
        let sample = Sample {
            time_s: fields[1].parse().expect("Lesson 6 time"),
            setpoint: fields[2].parse().expect("Lesson 6 setpoint"),
            pv: fields[3].parse().expect("Lesson 6 PV"),
            requested: fields[6].parse().expect("Lesson 6 requested output"),
            applied: fields[7].parse().expect("Lesson 6 applied output"),
            proportional_increment: None,
            integral_increment: None,
            auto: fields[8].parse().expect("Lesson 6 mode"),
        };
        match fields[0] {
            "biased_p" => biased.push(sample),
            "integral_only" => integral.push(sample),
            other => panic!("unexpected Lesson 6 case {other}"),
        }
    }
    vec![("biased_p", biased), ("integral_only", integral)]
}

fn range(samples: &[Sample], select: impl Fn(&Sample) -> f64) -> f64 {
    let minimum = samples.iter().map(&select).fold(f64::INFINITY, f64::min);
    let maximum = samples.iter().map(select).fold(f64::NEG_INFINITY, f64::max);
    maximum - minimum
}

fn mean_absolute_error(samples: &[Sample]) -> f64 {
    samples
        .iter()
        .map(|sample| (sample.setpoint - sample.pv).abs())
        .sum::<f64>()
        / samples.len() as f64
}

fn summarize(samples: &[Sample], pi_candidate: bool) -> Summary {
    let early = &samples[600..750];
    let final_window = &samples[1_100..1_200];
    let post_load = &samples[600..1_200];
    let pv_range = range(final_window, |sample| sample.pv);
    let requested_range = range(final_window, |sample| sample.requested);
    let applied_range = range(final_window, |sample| sample.applied);
    let settled = pv_range <= 0.1
        && applied_range <= 1.0
        && final_window
            .iter()
            .all(|sample| sample.applied > 0.0 && sample.applied < 100.0);
    let final_error = settled.then(|| mean_absolute_error(final_window));
    let recovery_s = post_load.iter().enumerate().find_map(|(index, sample)| {
        post_load[index..]
            .iter()
            .all(|future| (future.setpoint - future.pv).abs() <= 0.2)
            .then_some(sample.time_s - 60.0)
    });
    let overshoot = post_load
        .iter()
        .map(|sample| (sample.pv - sample.setpoint).max(0.0))
        .fold(0.0_f64, f64::max);
    let movement = samples[599..1_200]
        .windows(2)
        .map(|pair| (pair[1].applied - pair[0].applied).abs())
        .sum();
    let saturation_s = post_load
        .iter()
        .filter(|sample| sample.applied <= 1.0e-9 || sample.applied >= 100.0 - 1.0e-9)
        .count() as f64
        * 0.1;
    let eligible = pi_candidate
        && settled
        && final_error.is_some_and(|value| value <= 0.2)
        && recovery_s.is_some_and(|value| value <= 15.0)
        && overshoot <= 0.25
        && saturation_s == 0.0;
    Summary {
        settled,
        pv_range,
        requested_range,
        applied_range,
        early_error: mean_absolute_error(early),
        final_error,
        recovery_s,
        overshoot,
        movement,
        saturation_s,
        eligible,
    }
}

fn write_evidence(cases: &[(&str, Option<f64>, Vec<Sample>)], unreachable: &[Sample]) {
    let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("create Lesson 7 figure directory");
    let mut trace = String::from(
        "case,time_s,setpoint_degC,pv_degC,error_degC,kp_percent_per_degC,ki_percent_per_degC_s,integral_time_s,proportional_increment_pp,integral_increment_pp,requested_mv_percent,applied_mv_percent,auto_mode,load_event,sample_time_s,seed,pv_min_degC,pv_max_degC,mv_min_percent,mv_max_percent,phase,evidence_class\n",
    );
    for (name, ki, samples) in cases {
        for sample in samples {
            let phase = if sample.time_s == 120.1 {
                "safe_restoration"
            } else if sample.time_s <= 50.0 {
                "manual_preparation"
            } else if sample.time_s <= 60.0 {
                "nominal_auto"
            } else {
                "changed_load"
            };
            let (kp, integral_gain, integral_time) = match (*name, ki) {
                ("biased_p", _) => ("8.0".to_owned(), "0.0".to_owned(), String::new()),
                ("integral_only", _) => ("0.0".to_owned(), "0.5".to_owned(), String::new()),
                (_, Some(value)) => (
                    "8.0".to_owned(),
                    format!("{value:.1}"),
                    format!("{:.1}", 8.0 / value),
                ),
                _ => unreachable!("PI case has Ki"),
            };
            writeln!(trace,
                "{name},{:.1},{:.1},{:.6},{:.6},{kp},{integral_gain},{integral_time},{},{},{:.6},{:.6},{},{},{:.1},20260828,0.0,80.0,0.0,100.0,{phase},simulation",
                sample.time_s, sample.setpoint, sample.pv, sample.setpoint - sample.pv,
                sample.proportional_increment.map(|value| format!("{value:.6}")).unwrap_or_default(),
                sample.integral_increment.map(|value| format!("{value:.6}")).unwrap_or_default(),
                sample.requested, sample.applied, sample.auto,
                if sample.time_s >= 60.0 && sample.time_s < 120.1 { "increased" } else { "nominal" }, 0.1
            ).expect("append Lesson 7 comparison row");
        }
    }
    std::fs::write(directory.join("pi-comparison.csv"), trace)
        .expect("write Lesson 7 comparison trace");

    let summaries = cases
        .iter()
        .map(|(name, ki, samples)| (*name, *ki, summarize(samples, ki.is_some())))
        .collect::<Vec<_>>();
    let selected = summaries
        .iter()
        .filter(|(_, _, summary)| summary.eligible)
        .min_by(|left, right| {
            left.2
                .recovery_s
                .partial_cmp(&right.2.recovery_s)
                .expect("finite recovery")
                .then_with(|| {
                    left.2
                        .movement
                        .partial_cmp(&right.2.movement)
                        .expect("finite movement")
                })
        })
        .map(|(name, _, _)| *name);
    let mut summary_csv = String::from(
        "case,kp_percent_per_degC,ki_percent_per_degC_s,integral_time_s,settled,pv_range_degC,requested_range_pp,applied_range_pp,early_mean_absolute_error_degC,final_mean_absolute_error_degC,recovery_time_s,post_load_overshoot_degC,cumulative_applied_movement_pp,saturation_duration_s,simulation_criteria,eligible,selected,evidence_class\n",
    );
    for (name, ki, summary) in &summaries {
        let (kp, integral_gain, integral_time) = match (*name, ki) {
            ("biased_p", _) => ("8.0".to_owned(), "0.0".to_owned(), String::new()),
            ("integral_only", _) => ("0.0".to_owned(), "0.5".to_owned(), String::new()),
            (_, Some(value)) => (
                "8.0".to_owned(),
                format!("{value:.1}"),
                format!("{:.1}", 8.0 / value),
            ),
            _ => unreachable!("PI case has Ki"),
        };
        let criteria = if ki.is_none() {
            "reference"
        } else if summary.eligible {
            "pass"
        } else {
            "fail"
        };
        writeln!(summary_csv,
            "{name},{kp},{integral_gain},{integral_time},{},{:.6},{:.6},{:.6},{:.6},{},{},{:.6},{:.6},{:.1},{criteria},{},{},simulation",
            summary.settled, summary.pv_range, summary.requested_range, summary.applied_range,
            summary.early_error,
            summary.final_error.map(|value| format!("{value:.6}")).unwrap_or_else(|| "Not settled".to_owned()),
            summary.recovery_s.map(|value| format!("{value:.1}")).unwrap_or_else(|| "Not recovered".to_owned()),
            summary.overshoot, summary.movement, summary.saturation_s, summary.eligible,
            selected == Some(*name)
        ).expect("append Lesson 7 qualification row");
    }
    std::fs::write(directory.join("qualification-summary.csv"), summary_csv)
        .expect("write Lesson 7 qualification summary");

    let mut exposure = String::from(
        "time_s,setpoint_degC,pv_degC,error_degC,proportional_increment_pp,integral_increment_pp,requested_mv_percent,applied_mv_percent,saturated,auto_mode,sample_time_s,seed,pv_min_degC,pv_max_degC,mv_min_percent,mv_max_percent,phase,evidence_class\n",
    );
    for sample in unreachable {
        let phase = if sample.time_s == 120.1 {
            "safe_restoration"
        } else if sample.time_s <= 50.0 {
            "manual_preparation"
        } else if sample.time_s < 60.0 {
            "nominal_auto"
        } else {
            "unreachable_setpoint"
        };
        writeln!(exposure,
            "{:.1},{:.1},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{:.1},20260828,0.0,80.0,0.0,100.0,{phase},simulation",
            sample.time_s, sample.setpoint, sample.pv, sample.setpoint - sample.pv,
            sample.proportional_increment.unwrap_or(0.0), sample.integral_increment.unwrap_or(0.0),
            sample.requested, sample.applied, sample.requested > sample.applied + 1.0e-9,
            sample.auto, 0.1
        ).expect("append Lesson 7 unreachable row");
    }
    std::fs::write(directory.join("unreachable-exposure.csv"), exposure)
        .expect("write Lesson 7 unreachable trace");
}

#[test]
fn control_curriculum_water_heater_pi_starts_disabled_with_zero_applied_output() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-pi/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 7 Structured Text");
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let applied = IoAddress::parse("%QL0").expect("applied-output address");

    runtime
        .execute_cycle()
        .expect("execute Lesson 7 startup scan");

    assert!(
        !read_bool(&runtime, &auto),
        "Lesson 7 must start in Manual rather than applying PI automatically"
    );
    assert_eq!(
        read_lreal(&runtime, &applied),
        0.0,
        "Lesson 7 must start with zero applied heater output"
    );
}

#[test]
fn control_curriculum_water_heater_pi_preloads_measured_holding_output_for_shipped_auto_run() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-pi/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 7 Structured Text");
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let kp = IoAddress::parse("%IL8").expect("proportional-gain address");
    let ki = IoAddress::parse("%IL16").expect("integral-gain address");
    let setpoint = IoAddress::parse("%IL32").expect("setpoint address");
    let pv = IoAddress::parse("%IL0").expect("process-value address");
    let measured_holding = IoAddress::parse("%ML48").expect("measured holding-output address");
    let requested = IoAddress::parse("%ML24").expect("requested-output address");
    let proportional = IoAddress::parse("%ML32").expect("proportional-increment address");
    let integral = IoAddress::parse("%ML40").expect("integral-increment address");
    let applied = IoAddress::parse("%QL0").expect("applied-output address");

    write_lreal(&mut runtime, &kp, 8.0);
    write_lreal(&mut runtime, &ki, 4.0);
    write_lreal(&mut runtime, &setpoint, 35.0);
    write_lreal(&mut runtime, &pv, 20.0);
    write_bool(&mut runtime, &auto, true);
    runtime
        .execute_cycle()
        .expect("execute shipped first PI Auto scan");

    assert_eq!(
        read_lreal(&runtime, &measured_holding),
        75.0,
        "the shipped lesson must expose Lesson 5's measured holding output"
    );
    assert_eq!(
        read_lreal(&runtime, &proportional),
        0.0,
        "the transfer resets error history, so the first proportional increment is zero"
    );
    assert_eq!(
        read_lreal(&runtime, &integral),
        6.0,
        "Ki 4, 15 degC error, and dt 0.1 s must add 6 pp"
    );
    assert_eq!(
        read_lreal(&runtime, &requested),
        81.0,
        "the first request must add the 6 pp integral increment to the measured 75% preload"
    );
    assert_eq!(
        read_lreal(&runtime, &applied),
        81.0,
        "the unsaturated 81% request must be applied unchanged"
    );
}

#[test]
fn control_curriculum_water_heater_pi_tracks_holding_output_before_auto() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-pi/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 7 Structured Text");
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let kp = IoAddress::parse("%IL8").expect("proportional-gain address");
    let ki = IoAddress::parse("%IL16").expect("integral-gain address");
    let setpoint = IoAddress::parse("%IL32").expect("setpoint address");
    let pv = IoAddress::parse("%IL0").expect("process-value address");
    let manual = IoAddress::parse("%ML16").expect("manual-output address");
    let applied = IoAddress::parse("%QL0").expect("applied-output address");

    write_lreal(&mut runtime, &setpoint, 35.0);
    write_lreal(&mut runtime, &pv, 35.0);
    write_lreal(&mut runtime, &kp, 8.0);
    write_lreal(&mut runtime, &ki, 4.0);
    write_lreal(&mut runtime, &manual, 75.0);
    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("track measured holding output in Manual");
    assert_eq!(
        read_lreal(&runtime, &applied),
        75.0,
        "Manual must apply the measured 75% holding output"
    );

    write_bool(&mut runtime, &auto, true);
    runtime
        .execute_cycle()
        .expect("transfer the measured holding output to PI Auto");
    assert_eq!(
        read_lreal(&runtime, &applied),
        75.0,
        "zero-error Manual-to-Auto transfer must retain 75% without a bump"
    );
}

#[test]
fn control_curriculum_water_heater_pi_exposes_parallel_increments_and_request() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-pi/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 7 Structured Text");
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let kp = IoAddress::parse("%IL8").expect("proportional-gain address");
    let ki = IoAddress::parse("%IL16").expect("integral-gain address");
    let setpoint = IoAddress::parse("%IL32").expect("setpoint address");
    let pv = IoAddress::parse("%IL0").expect("process-value address");
    let manual = IoAddress::parse("%ML16").expect("manual-output address");
    let requested = IoAddress::parse("%ML24").expect("requested-output address");
    let proportional = IoAddress::parse("%ML32").expect("proportional-increment address");
    let integral = IoAddress::parse("%ML40").expect("integral-increment address");
    let applied = IoAddress::parse("%QL0").expect("applied-output address");

    write_lreal(&mut runtime, &kp, 8.0);
    write_lreal(&mut runtime, &ki, 4.0);
    write_lreal(&mut runtime, &setpoint, 35.0);
    write_lreal(&mut runtime, &pv, 33.0);
    write_lreal(&mut runtime, &manual, 75.0);
    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("establish Manual holding output");

    write_bool(&mut runtime, &auto, true);
    runtime.execute_cycle().expect("execute first PI Auto scan");

    assert_eq!(
        read_lreal(&runtime, &proportional),
        0.0,
        "unchanged error at transfer must produce zero proportional increment"
    );
    assert!(
        (read_lreal(&runtime, &integral) - 0.8).abs() <= 1.0e-12,
        "Ki 4, error 2 degC and dt 0.1 s must add 0.8 pp"
    );
    assert!(
        (read_lreal(&runtime, &requested) - 75.8).abs() <= 1.0e-12,
        "the preliminary request must add both PI increments to the prior 75% output"
    );
    assert!(
        (read_lreal(&runtime, &applied) - 75.8).abs() <= 1.0e-12,
        "the unsaturated preliminary request must be applied unchanged"
    );
}

#[test]
fn control_curriculum_water_heater_pi_adopts_live_gain_on_the_next_scan() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-pi/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 7 Structured Text");
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let kp = IoAddress::parse("%IL8").expect("proportional-gain address");
    let ki = IoAddress::parse("%IL16").expect("integral-gain address");
    let setpoint = IoAddress::parse("%IL32").expect("setpoint address");
    let pv = IoAddress::parse("%IL0").expect("process-value address");
    let manual = IoAddress::parse("%ML16").expect("manual-output address");
    let requested = IoAddress::parse("%ML24").expect("requested-output address");
    let integral = IoAddress::parse("%ML40").expect("integral-increment address");
    let applied = IoAddress::parse("%QL0").expect("applied-output address");

    write_lreal(&mut runtime, &kp, 8.0);
    write_lreal(&mut runtime, &ki, 0.5);
    write_lreal(&mut runtime, &setpoint, 35.0);
    write_lreal(&mut runtime, &pv, 33.0);
    write_lreal(&mut runtime, &manual, 75.0);
    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("establish Manual holding output");

    write_bool(&mut runtime, &auto, true);
    runtime
        .execute_cycle()
        .expect("execute PI scan with Ki 0.5");
    assert!(
        (read_lreal(&runtime, &applied) - 75.1).abs() <= 1.0e-12,
        "Ki 0.5 must add 0.1 pp for a 2 degC error in one 0.1 s scan"
    );

    write_lreal(&mut runtime, &ki, 4.0);
    runtime
        .execute_cycle()
        .expect("execute first scan after live Ki change");

    assert!(
        (read_lreal(&runtime, &integral) - 0.8).abs() <= 1.0e-12,
        "the exposed integral increment must adopt live Ki 4 on the next scan"
    );
    assert!(
        (read_lreal(&runtime, &requested) - 75.9).abs() <= 1.0e-12,
        "the displayed request must use the live Ki 4 increment"
    );
    assert!(
        (read_lreal(&runtime, &applied) - 75.9).abs() <= 1.0e-12,
        "Controller.Update must adopt live Ki 4 in the same scan as its diagnostics"
    );
}

#[test]
fn control_curriculum_water_heater_pi_selects_balanced_ki_then_exposes_saturation() {
    let mut cases = inherited_baselines()
        .into_iter()
        .map(|(name, samples)| (name, None, samples))
        .collect::<Vec<_>>();
    for (name, ki) in [
        ("pi_ki_0_5", 0.5),
        ("pi_ki_2", 2.0),
        ("pi_ki_4", 4.0),
        ("pi_ki_8", 8.0),
    ] {
        cases.push((name, Some(ki), run_pi(ki, false)));
    }
    let summaries = cases
        .iter()
        .map(|(name, ki, samples)| (*name, *ki, summarize(samples, ki.is_some())))
        .collect::<Vec<_>>();
    let selected = summaries
        .iter()
        .filter(|(_, _, summary)| summary.eligible)
        .min_by(|left, right| {
            left.2
                .recovery_s
                .partial_cmp(&right.2.recovery_s)
                .expect("finite recovery")
                .then_with(|| {
                    left.2
                        .movement
                        .partial_cmp(&right.2.movement)
                        .expect("finite movement")
                })
        })
        .expect("at least one eligible PI tuning");
    for (name, ki, summary) in &summaries {
        eprintln!(
            "Lesson 7 {name} Ki={ki:?}: early={:.6} final={:?} recovery={:?} overshoot={:.6} movement={:.6} saturation={:.1} eligible={}",
            summary.early_error, summary.final_error, summary.recovery_s,
            summary.overshoot, summary.movement, summary.saturation_s, summary.eligible
        );
    }
    assert_eq!(
        selected.0, "pi_ki_4",
        "the predeclared eligibility and ranking contract must select Ki 4"
    );
    assert_eq!(selected.1, Some(4.0));
    assert!(
        summaries
            .iter()
            .find(|(name, _, _)| *name == "biased_p")
            .and_then(|(_, _, summary)| summary.final_error)
            .is_some_and(|value| (1.0..=1.3).contains(&value)),
        "the inherited biased-P reference must retain the Lesson 5 load offset"
    );
    assert!(
        summaries
            .iter()
            .find(|(name, _, _)| *name == "integral_only")
            .and_then(|(_, _, summary)| summary.final_error)
            .is_some_and(|value| value <= 0.2),
        "the inherited integral-only reference must retain Lesson 6 offset removal"
    );
    assert!(
        !summaries
            .iter()
            .find(|(name, _, _)| *name == "pi_ki_0_5")
            .expect("Ki 0.5 summary")
            .2
            .eligible,
        "the slow Ki 0.5 candidate must miss the declared recovery criterion"
    );
    assert!(
        !summaries
            .iter()
            .find(|(name, _, _)| *name == "pi_ki_8")
            .expect("Ki 8 summary")
            .2
            .eligible,
        "the aggressive Ki 8 candidate must fail overshoot or saturation eligibility"
    );
    for (_, _, samples) in &cases {
        assert!(
            samples[..1_200].iter().all(|sample| {
                sample.pv.is_finite()
                    && sample.requested.is_finite()
                    && sample.applied.is_finite()
                    && (0.0..=100.0).contains(&sample.applied)
            }),
            "every normal comparison must remain finite and inside physical limits"
        );
        let restored = samples.last().expect("safe restoration sample");
        assert!(
            !restored.auto && restored.requested == 0.0 && restored.applied == 0.0,
            "every normal comparison must retain Manual zero-output restoration"
        );
    }

    let unreachable = run_pi(4.0, true);
    let exposure = &unreachable[599..1_200];
    let peak_requested = exposure
        .iter()
        .map(|sample| sample.requested)
        .fold(f64::NEG_INFINITY, f64::max);
    let peak_applied = exposure
        .iter()
        .map(|sample| sample.applied)
        .fold(f64::NEG_INFINITY, f64::max);
    let maximum_pv = exposure
        .iter()
        .map(|sample| sample.pv)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(peak_requested > 100.0,
        "the unreachable setpoint must expose a preliminary request above 100%; peak={peak_requested}");
    assert!(
        (peak_applied - 100.0).abs() <= 1.0e-12,
        "the one-sided heater must clamp the applied command at 100%; peak={peak_applied}"
    );
    assert!(maximum_pv < 45.0,
        "the configured heater must remain below the unreachable 45 degC setpoint; maximum={maximum_pv}");
    let restored = unreachable.last().expect("unreachable restoration");
    assert!(
        !restored.auto && restored.requested == 0.0 && restored.applied == 0.0,
        "the unreachable exposure must retain Manual zero-output restoration"
    );

    write_evidence(&cases, &unreachable);
}

#[test]
fn control_curriculum_water_heater_pi_defaults_to_selected_tuning() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path(
            "water-heater-pi/src/Main.st",
            source_with_default_observers(),
        ),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 7 defaults");
    let kp = IoAddress::parse("%ML56").expect("observed proportional-gain address");
    let ki = IoAddress::parse("%ML64").expect("observed integral-gain address");

    runtime
        .execute_cycle()
        .expect("observe Lesson 7 default tuning");

    assert_eq!(
        read_lreal(&runtime, &kp),
        8.0,
        "Lesson 7 must retain the previously selected proportional gain"
    );
    assert_eq!(
        read_lreal(&runtime, &ki),
        4.0,
        "Lesson 7 must default to the PI candidate selected by its tuning contract"
    );
}
