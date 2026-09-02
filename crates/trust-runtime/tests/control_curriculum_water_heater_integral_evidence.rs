use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const LESSON_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-integral/src/Main.st");
const BASELINE_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-bias/src/Main.st");
const SIMULATION_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-integral/simulation.toml");

#[derive(Clone, Copy)]
struct Sample {
    time_s: f64,
    pv: f64,
    requested_mv: f64,
    applied_mv: f64,
    integral_contribution: f64,
    auto: bool,
}

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 6 program before configuration")
        .replacen(
            "    AutoMode : BOOL :=",
            "    AutoMode AT %IX24.0 : BOOL :=",
            1,
        )
        .replace("IntegralGain : LREAL :=", "IntegralGain AT %IL8 : LREAL :=")
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL32 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "ManualHeaterPower : LREAL :=",
            "ManualHeaterPower AT %ML16 : LREAL :=",
        )
        .replace(
            "RequestedHeaterPower : LREAL :=",
            "RequestedHeaterPower AT %ML24 : LREAL :=",
        )
        .replace(
            "IntegralContribution : LREAL :=",
            "IntegralContribution AT %ML32 : LREAL :=",
        )
        .replace(
            "MeasuredHoldingOutput : LREAL :=",
            "MeasuredHoldingOutput AT %ML40 : LREAL :=",
        )
        .replace(
            "AppliedHeaterPower : LREAL :=",
            "AppliedHeaterPower AT %QL0 : LREAL :=",
        )
}

fn addressed_baseline_source() -> String {
    BASELINE_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 5 program before configuration")
        .replacen(
            "    AutoMode : BOOL :=",
            "    AutoMode AT %IX24.0 : BOOL :=",
            1,
        )
        .replace(
            "ProportionalGain : LREAL :=",
            "ProportionalGain AT %IL8 : LREAL :=",
        )
        .replace(
            "OperatingPointBias : LREAL :=",
            "OperatingPointBias AT %IL16 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL32 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "ManualHeaterPower : LREAL :=",
            "ManualHeaterPower AT %ML16 : LREAL :=",
        )
        .replace(
            "RequestedHeaterPower : LREAL :=",
            "RequestedHeaterPower AT %ML24 : LREAL :=",
        )
        .replace(
            "AppliedHeaterPower : LREAL :=",
            "AppliedHeaterPower AT %QL0 : LREAL :=",
        )
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL value") {
        Value::Bool(value) => value,
        other => panic!("expected BOOL storage, got {other:?}"),
    }
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL process-image value");
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL process-image value");
}

fn temporary_config_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-water-heater-integral-{name}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn run_case(integral_only: bool) -> Vec<Sample> {
    let source = if integral_only {
        addressed_source()
    } else {
        addressed_baseline_source()
    };
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-integral/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile Lesson 6 comparison controller");
    let path = temporary_config_path(if integral_only {
        "integral"
    } else {
        "biased-p"
    });
    std::fs::write(&path, SIMULATION_SOURCE).expect("write Lesson 6 simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load Lesson 6 simulation config"),
    );
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let pv = IoAddress::parse("%IL0").expect("process-value address");
    let gain = IoAddress::parse("%IL8").expect("controller-gain address");
    let bias = IoAddress::parse("%IL16").expect("baseline-bias address");
    let setpoint = IoAddress::parse("%IL32").expect("setpoint address");
    let manual = IoAddress::parse("%ML16").expect("manual-output address");
    let requested = IoAddress::parse("%ML24").expect("requested-output address");
    let integral = IoAddress::parse("%ML32").expect("integral-contribution address");
    let applied = IoAddress::parse("%QL0").expect("applied-output address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial water-heater state");
    let mut trace = Vec::with_capacity(1_201);
    for cycle in 1..=1_200 {
        write_lreal(&mut runtime, &setpoint, 35.0);
        write_lreal(&mut runtime, &gain, if integral_only { 0.5 } else { 8.0 });
        if !integral_only {
            write_lreal(&mut runtime, &bias, 75.0);
        }
        write_lreal(&mut runtime, &manual, if cycle == 1 { 0.0 } else { 75.0 });
        write_bool(&mut runtime, &auto, cycle > 500);
        runtime
            .execute_cycle()
            .expect("execute Lesson 6 controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Lesson 6 water heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Lesson 6 process value");
        trace.push(Sample {
            time_s: cycle as f64 * 0.1,
            pv: read_lreal(&runtime, &pv),
            requested_mv: read_lreal(&runtime, &requested),
            applied_mv: read_lreal(&runtime, &applied),
            integral_contribution: if integral_only {
                read_lreal(&runtime, &integral)
            } else {
                0.0
            },
            auto: read_bool(&runtime, &auto),
        });
    }
    write_bool(&mut runtime, &auto, false);
    write_lreal(&mut runtime, &manual, 0.0);
    runtime
        .execute_cycle()
        .expect("restore Lesson 6 safe state");
    trace.push(Sample {
        time_s: 120.1,
        pv: read_lreal(&runtime, &pv),
        requested_mv: read_lreal(&runtime, &requested),
        applied_mv: read_lreal(&runtime, &applied),
        integral_contribution: if integral_only {
            read_lreal(&runtime, &integral)
        } else {
            0.0
        },
        auto: read_bool(&runtime, &auto),
    });
    let _ = std::fs::remove_file(path);
    trace
}

fn range(samples: &[Sample], select: impl Fn(&Sample) -> f64) -> f64 {
    let minimum = samples.iter().map(&select).fold(f64::INFINITY, f64::min);
    let maximum = samples.iter().map(select).fold(f64::NEG_INFINITY, f64::max);
    maximum - minimum
}

fn mean_absolute_error(samples: &[Sample]) -> f64 {
    samples
        .iter()
        .map(|sample| (35.0 - sample.pv).abs())
        .sum::<f64>()
        / samples.len() as f64
}

fn applied_movement(samples: &[Sample]) -> f64 {
    samples
        .windows(2)
        .map(|pair| (pair[1].applied_mv - pair[0].applied_mv).abs())
        .sum()
}

fn evidence_directory() -> Option<PathBuf> {
    std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR").map(PathBuf::from)
}

fn write_evidence(baseline: &[Sample], integral: &[Sample]) {
    let Some(directory) = evidence_directory() else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("create Lesson 6 figure directory");
    let mut trace = String::from(
        "case,time_s,setpoint_degC,pv_degC,error_degC,integral_contribution_percent,requested_mv_percent,applied_mv_percent,auto_mode,load_event,sample_time_s,seed,pv_min_degC,pv_max_degC,mv_min_percent,mv_max_percent,phase,evidence_class\n",
    );
    for (case, samples) in [("biased_p", baseline), ("integral_only", integral)] {
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
            writeln!(trace,
                "{case},{:.1},35.0,{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{:.1},20260828,0.0,80.0,0.0,100.0,{phase},simulation",
                sample.time_s, sample.pv, 35.0 - sample.pv,
                sample.integral_contribution, sample.requested_mv, sample.applied_mv,
                sample.auto, if sample.time_s >= 60.0 && sample.time_s < 120.1 { "increased" } else { "nominal" }, 0.1
            ).expect("append Lesson 6 trace row");
        }
    }
    std::fs::write(directory.join("integral-comparison.csv"), trace)
        .expect("write Lesson 6 comparison trace");

    let mut summary = String::from(
        "case,window,start_s,end_s,settled,pv_range_degC,requested_range_pp,applied_range_pp,mean_absolute_error_degC,cumulative_applied_movement_pp,simulation_criterion,eligible,selected,evidence_class\n",
    );
    for (case, samples, selected) in [
        ("biased_p", baseline, false),
        ("integral_only", integral, false),
    ] {
        let early = &samples[600..750];
        let final_window = &samples[1_100..1_200];
        let settled = range(final_window, |sample| sample.pv) <= 0.1
            && range(final_window, |sample| sample.applied_mv) <= 1.0
            && final_window
                .iter()
                .all(|sample| sample.applied_mv > 0.0 && sample.applied_mv < 100.0);
        writeln!(summary,
            "{case},early_load,60.1,75.0,false,{:.6},{:.6},{:.6},{:.6},{:.6},comparison,true,{selected},simulation",
            range(early, |sample| sample.pv), range(early, |sample| sample.requested_mv),
            range(early, |sample| sample.applied_mv), mean_absolute_error(early),
            applied_movement(&samples[599..1_200])
        ).expect("append Lesson 6 early summary");
        writeln!(summary,
            "{case},final,110.1,120.0,{settled},{:.6},{:.6},{:.6},{:.6},{:.6},{},{settled},{selected},simulation",
            range(final_window, |sample| sample.pv),
            range(final_window, |sample| sample.requested_mv),
            range(final_window, |sample| sample.applied_mv),
            mean_absolute_error(final_window), applied_movement(&samples[599..1_200]),
            if case == "integral_only" && mean_absolute_error(final_window) <= 0.2 { "pass" } else if case == "integral_only" { "fail" } else { "baseline" }
        ).expect("append Lesson 6 final summary");
    }
    std::fs::write(directory.join("qualification-summary.csv"), summary)
        .expect("write Lesson 6 qualification summary");
}

#[test]
fn control_curriculum_water_heater_integral_starts_disabled_with_zero_applied_output() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-integral/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 6 Structured Text");
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let applied = IoAddress::parse("%QL0").expect("applied heater address");

    runtime
        .execute_cycle()
        .expect("execute disabled startup scan");

    assert!(
        !read_bool(&runtime, &auto),
        "Lesson 6 must start with automatic mode disabled"
    );
    assert_eq!(
        read_lreal(&runtime, &applied),
        0.0,
        "Lesson 6 must start in Manual with zero applied heater output"
    );
}

#[test]
fn control_curriculum_water_heater_integral_exposes_requested_and_applied_increment() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-integral/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 6 Structured Text");
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let setpoint = IoAddress::parse("%IL32").expect("setpoint address");
    let gain = IoAddress::parse("%IL8").expect("integral-gain address");
    let pv = IoAddress::parse("%IL0").expect("process-value address");
    let measured_holding = IoAddress::parse("%ML40").expect("measured holding-output address");
    let requested = IoAddress::parse("%ML24").expect("requested-output address");
    let applied = IoAddress::parse("%QL0").expect("applied-output address");

    write_lreal(&mut runtime, &setpoint, 35.0);
    write_lreal(&mut runtime, &gain, 0.5);
    write_lreal(&mut runtime, &pv, 20.0);
    write_bool(&mut runtime, &auto, true);
    runtime
        .execute_cycle()
        .expect("execute first integral scan");

    let requested_value = read_lreal(&runtime, &requested);
    let applied_value = read_lreal(&runtime, &applied);
    let measured_holding_value = read_lreal(&runtime, &measured_holding);
    eprintln!("Lesson 6 first integral scan measured holding={measured_holding_value}, requested={requested_value}, applied={applied_value}");
    assert!((measured_holding_value - 75.0).abs() <= 1.0e-12,
        "the shipped lesson must expose the measured 75% holding output; measured={measured_holding_value}");
    assert!((requested_value - 75.75).abs() <= 1.0e-12,
        "the shipped Auto transfer must preload the measured 75% holding output, then add the 0.75 pp increment from 15 degC error at Ki 0.5 and dt 0.1; requested={requested_value}");
    assert!((applied_value - requested_value).abs() <= 1.0e-12,
        "the unsaturated first increment must be applied unchanged; requested={requested_value}, applied={applied_value}");
}

#[test]
fn control_curriculum_water_heater_integral_tracks_measured_holding_output_before_auto() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-integral/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 6 Structured Text");
    let auto = IoAddress::parse("%IX24.0").expect("automatic-mode address");
    let setpoint = IoAddress::parse("%IL32").expect("setpoint address");
    let gain = IoAddress::parse("%IL8").expect("integral-gain address");
    let manual = IoAddress::parse("%ML16").expect("manual-output address");
    let requested = IoAddress::parse("%ML24").expect("requested-output address");
    let applied = IoAddress::parse("%QL0").expect("applied-output address");
    let pv = IoAddress::parse("%IL0").expect("process-value address");

    write_lreal(&mut runtime, &setpoint, 35.0);
    write_lreal(&mut runtime, &gain, 0.5);
    write_lreal(&mut runtime, &pv, 35.0);
    write_lreal(&mut runtime, &manual, 75.0);
    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("track measured holding output in Manual");
    assert!(
        (read_lreal(&runtime, &requested) - 75.0).abs() <= 1.0e-12,
        "Manual must request the measured 75% holding output"
    );
    assert!(
        (read_lreal(&runtime, &applied) - 75.0).abs() <= 1.0e-12,
        "Manual must apply the measured 75% holding output"
    );

    write_bool(&mut runtime, &auto, true);
    runtime
        .execute_cycle()
        .expect("transfer to integral-only Auto");
    let requested_value = read_lreal(&runtime, &requested);
    let applied_value = read_lreal(&runtime, &applied);
    assert!(
        (requested_value - 75.0).abs() <= 1.0e-12,
        "zero-error transfer must retain the measured holding output; requested={requested_value}"
    );
    assert!(
        (applied_value - 75.0).abs() <= 1.0e-12,
        "zero-error transfer must be bumpless; applied={applied_value}"
    );
}

#[test]
fn control_curriculum_water_heater_integral_removes_load_offset_with_slower_early_response() {
    let baseline = run_case(false);
    let integral = run_case(true);
    let baseline_early_error = mean_absolute_error(&baseline[600..750]);
    let integral_early_error = mean_absolute_error(&integral[600..750]);
    let baseline_final = &baseline[1_100..1_200];
    let integral_final = &integral[1_100..1_200];
    let baseline_final_error = mean_absolute_error(baseline_final);
    let integral_final_error = mean_absolute_error(integral_final);
    let integral_settled = range(integral_final, |sample| sample.pv) <= 0.1
        && range(integral_final, |sample| sample.applied_mv) <= 1.0;
    let baseline_movement = applied_movement(&baseline[599..1_200]);
    let integral_movement = applied_movement(&integral[599..1_200]);

    assert!(
        baseline[500..600]
            .iter()
            .chain(&integral[500..600])
            .all(|sample| (sample.pv - 35.0).abs() <= 0.1),
        "both cases must enter the load event from the qualified measured operating point"
    );
    assert!((1.0..=1.3).contains(&baseline_final_error),
        "the inherited gain-8, bias-75 baseline must retain Lesson 5 load offset; error={baseline_final_error}");
    assert!(
        integral_settled,
        "the integral-only final window must satisfy the declared settling rule"
    );
    assert!(
        integral_final_error <= 0.2,
        "integral memory must remove the constant-load offset; error={integral_final_error}"
    );
    assert!(integral_early_error > baseline_early_error,
        "integral-only action must provide less prompt early-load correction; I={integral_early_error}, P={baseline_early_error}");
    assert!(
        integral[..1_200]
            .iter()
            .chain(&baseline[..1_200])
            .all(|sample| {
                sample.pv.is_finite()
                    && sample.requested_mv.is_finite()
                    && sample.applied_mv.is_finite()
                    && (0.0..=100.0).contains(&sample.applied_mv)
            }),
        "both controller traces must remain finite and inside heater limits"
    );
    for restored in [
        baseline.last().expect("baseline restoration"),
        integral.last().expect("integral restoration"),
    ] {
        assert!(
            !restored.auto && restored.requested_mv == 0.0 && restored.applied_mv == 0.0,
            "every retained run must end in Manual with zero requested and applied heater output"
        );
    }
    write_evidence(&baseline, &integral);
    eprintln!(
        "Lesson 6 early MAE P/I={baseline_early_error:.6}/{integral_early_error:.6}degC; final MAE P/I={baseline_final_error:.6}/{integral_final_error:.6}degC; applied movement P/I={baseline_movement:.6}/{integral_movement:.6}pp"
    );
}
