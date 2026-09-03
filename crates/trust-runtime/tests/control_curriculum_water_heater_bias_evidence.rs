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
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-bias/src/Main.st");
const SIMULATION_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-bias/simulation.toml");

#[derive(Clone, Copy)]
struct Sample {
    time_s: f64,
    pv: f64,
    requested_mv: f64,
    applied_mv: f64,
    auto: bool,
}

fn addressed_source() -> String {
    LESSON_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX24.0 : BOOL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL32 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "OperatingPointBias : LREAL :=",
            "OperatingPointBias AT %IL16 : LREAL :=",
        )
        .replace(
            "ProportionalGain : LREAL :=",
            "ProportionalGain AT %IL8 : LREAL :=",
        )
        .replace(
            "RequestedHeaterPower : LREAL :=",
            "RequestedHeaterPower AT %ML24 : LREAL :=",
        )
        .replace(
            "ManualHeaterPower : LREAL :=",
            "ManualHeaterPower AT %ML32 : LREAL :=",
        )
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace(
            "AppliedHeaterPower : LREAL :=",
            "AppliedHeaterPower AT %QL0 : LREAL :=",
        )
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

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn temporary_config_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-water-heater-bias-{name}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn simulation_controller(name: &str, with_load_event: bool) -> (SimulationController, PathBuf) {
    let source = if with_load_event {
        SIMULATION_SOURCE.to_owned()
    } else {
        SIMULATION_SOURCE.replace("at_ms = 60000", "at_ms = 999999")
    };
    let path = temporary_config_path(name);
    std::fs::write(&path, source).expect("write bias simulation config");
    let simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load bias simulation config"),
    );
    (simulation, path)
}

fn run_manual_trial(manual_output: f64) -> Vec<Sample> {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-bias/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile manual holding-output trial");
    let (mut simulation, path) = simulation_controller("manual", false);
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let manual = IoAddress::parse("%ML32").expect("manual request address");
    let requested = IoAddress::parse("%ML24").expect("requested MV address");
    let applied = IoAddress::parse("%QL0").expect("applied MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial manual-trial PV");
    let mut trace = Vec::with_capacity(601);
    for cycle in 1..=600 {
        write_lreal(&mut runtime, &sp, 35.0);
        write_lreal(&mut runtime, &manual, manual_output);
        write_bool(&mut runtime, &auto, false);
        runtime
            .execute_cycle()
            .expect("execute manual holding trial");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance manual-trial plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish manual-trial PV");
        trace.push(Sample {
            time_s: cycle as f64 * 0.1,
            pv: read_lreal(&runtime, &pv),
            requested_mv: read_lreal(&runtime, &requested),
            applied_mv: read_lreal(&runtime, &applied),
            auto: false,
        });
    }
    write_lreal(&mut runtime, &manual, 0.0);
    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("execute manual-trial safe restoration");
    trace.push(Sample {
        time_s: 60.1,
        pv: read_lreal(&runtime, &pv),
        requested_mv: read_lreal(&runtime, &requested),
        applied_mv: read_lreal(&runtime, &applied),
        auto: false,
    });
    let _ = std::fs::remove_file(path);
    trace
}

fn run_controller(bias_value: f64) -> Vec<Sample> {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-bias/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile biased P comparison");
    let (mut simulation, path) = simulation_controller("controller", true);
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let kp = IoAddress::parse("%IL8").expect("Kp address");
    let bias = IoAddress::parse("%IL16").expect("bias address");
    let manual = IoAddress::parse("%ML32").expect("manual request address");
    let requested = IoAddress::parse("%ML24").expect("requested MV address");
    let applied = IoAddress::parse("%QL0").expect("applied MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial comparison PV");
    write_lreal(&mut runtime, &sp, 35.0);
    write_lreal(&mut runtime, &kp, 8.0);
    write_lreal(&mut runtime, &bias, bias_value);
    write_lreal(&mut runtime, &manual, 0.0);
    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("establish safe disabled scan");
    assert_eq!(read_lreal(&runtime, &applied), 0.0);

    let mut trace = Vec::with_capacity(1_201);
    for cycle in 1..=1_200 {
        write_lreal(&mut runtime, &sp, 35.0);
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute biased P comparison");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance comparison plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish comparison PV");
        trace.push(Sample {
            time_s: cycle as f64 * 0.1,
            pv: read_lreal(&runtime, &pv),
            requested_mv: read_lreal(&runtime, &requested),
            applied_mv: read_lreal(&runtime, &applied),
            auto: true,
        });
    }
    write_lreal(&mut runtime, &manual, 0.0);
    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("restore zero-output manual state");
    trace.push(Sample {
        time_s: 120.1,
        pv: read_lreal(&runtime, &pv),
        requested_mv: read_lreal(&runtime, &requested),
        applied_mv: read_lreal(&runtime, &applied),
        auto: false,
    });
    let _ = std::fs::remove_file(path);
    trace
}

fn range(samples: &[Sample], field: fn(Sample) -> f64) -> f64 {
    let minimum = samples
        .iter()
        .map(|sample| field(*sample))
        .fold(f64::INFINITY, f64::min);
    let maximum = samples
        .iter()
        .map(|sample| field(*sample))
        .fold(f64::NEG_INFINITY, f64::max);
    maximum - minimum
}

fn mean_absolute_error(samples: &[Sample]) -> f64 {
    samples
        .iter()
        .map(|sample| (35.0 - sample.pv).abs())
        .sum::<f64>()
        / samples.len() as f64
}

fn mean_applied(samples: &[Sample]) -> f64 {
    samples.iter().map(|sample| sample.applied_mv).sum::<f64>() / samples.len() as f64
}

fn export_artifacts(
    directory: &std::path::Path,
    manual_trials: &[(f64, Vec<Sample>)],
    zero_bias: &[Sample],
    measured_bias: &[Sample],
) {
    std::fs::create_dir_all(directory).expect("create bias figure directory");
    let mut trials = String::from(
        "trial,time_s,setpoint_degC,pv_degC,requested_mv_percent,applied_mv_percent,bias_percent,kp_percent_per_degC,auto_mode,load_event,sample_time_s,seed,pv_min_degC,pv_max_degC,mv_min_percent,mv_max_percent,phase,evidence_class\n",
    );
    for (output, trace) in manual_trials {
        for (index, sample) in trace.iter().enumerate() {
            let phase = if index + 1 == trace.len() {
                "safe_restoration"
            } else {
                "manual_trial"
            };
            writeln!(
                trials,
                "manual_{output:.0}_percent,{:.1},35.0,{:.6},{:.6},{:.6},0.0,8.0,false,disabled,0.1,20260828,0.0,80.0,0.0,100.0,{phase},simulation",
                sample.time_s, sample.pv, sample.requested_mv, sample.applied_mv,
            )
            .expect("write holding-output row");
        }
    }
    std::fs::write(directory.join("holding-output-trials.csv"), trials)
        .expect("write holding-output evidence");

    let mut comparison = String::from(
        "case,time_s,setpoint_degC,pv_degC,requested_mv_percent,applied_mv_percent,bias_percent,kp_percent_per_degC,auto_mode,load_event,sample_time_s,seed,pv_min_degC,pv_max_degC,mv_min_percent,mv_max_percent,evidence_class\n",
    );
    for (case, bias, trace) in [
        ("zero_bias", 0.0, zero_bias),
        ("measured_bias", 75.0, measured_bias),
    ] {
        for sample in trace {
            let event = if (sample.time_s - 60.0).abs() < 0.05 {
                "cold_water_flow_increase"
            } else {
                ""
            };
            writeln!(
                comparison,
                "{case},{:.1},35.0,{:.6},{:.6},{:.6},{bias:.1},8.0,{},{},0.1,20260828,0.0,80.0,0.0,100.0,simulation",
                sample.time_s,
                sample.pv,
                sample.requested_mv,
                sample.applied_mv,
                sample.auto,
                event,
            )
            .expect("write biased P comparison row");
        }
    }
    std::fs::write(directory.join("bias-comparison.csv"), comparison)
        .expect("write biased P comparison evidence");

    let mut qualification = String::from(
        "case,window,start_s,end_s,settled,pv_range_degC,requested_range_pp,applied_range_pp,mean_absolute_error_degC,simulation_criterion,eligible,selected,evidence_class\n",
    );
    for (output, trace) in manual_trials {
        let window = &trace[500..600];
        let pv_range = range(window, |sample| sample.pv);
        let requested_range = range(window, |sample| sample.requested_mv);
        let applied_range = range(window, |sample| sample.applied_mv);
        let error = mean_absolute_error(window);
        let settled = pv_range <= 0.1 && applied_range <= 1.0;
        let criterion = if settled && error <= 0.1 {
            "pass"
        } else {
            "fail"
        };
        writeln!(
            qualification,
            "manual_{output:.0}_percent,holding_output,50.1,60.0,{settled},{pv_range:.6},{requested_range:.6},{applied_range:.6},{error:.6},{criterion},{settled},{},simulation",
            (*output - 75.0).abs() < f64::EPSILON,
        )
        .expect("write manual qualification row");
    }
    for (case, trace, selected) in [
        ("zero_bias", zero_bias, false),
        ("measured_bias", measured_bias, true),
    ] {
        for (window_name, start_s, end_s, window) in [
            ("nominal", 50.1, 60.0, &trace[500..600]),
            ("changed_load", 110.1, 120.0, &trace[1_100..1_200]),
        ] {
            let pv_range = range(window, |sample| sample.pv);
            let requested_range = range(window, |sample| sample.requested_mv);
            let applied_range = range(window, |sample| sample.applied_mv);
            let error = mean_absolute_error(window);
            let settled = pv_range <= 0.1 && applied_range <= 1.0;
            let criterion = match (case, window_name) {
                ("measured_bias", "nominal") if settled && error <= 0.1 => "pass",
                ("measured_bias", "changed_load") => "retained_limitation",
                _ => "baseline",
            };
            writeln!(
                qualification,
                "{case},{window_name},{start_s:.1},{end_s:.1},{settled},{pv_range:.6},{requested_range:.6},{applied_range:.6},{error:.6},{criterion},{settled},{selected},simulation",
            )
            .expect("write controller qualification row");
        }
    }
    std::fs::write(directory.join("qualification-summary.csv"), qualification)
        .expect("write qualification summary evidence");
}

#[test]
fn control_curriculum_water_heater_bias_starts_disabled_and_restores_zero_output() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-bias/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete operating-point-bias lesson");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let bias = IoAddress::parse("%IL16").expect("bias address");
    let applied = IoAddress::parse("%QL0").expect("applied MV address");

    write_lreal(&mut runtime, &sp, 35.0);
    write_lreal(&mut runtime, &pv, 35.0);
    write_lreal(&mut runtime, &bias, 75.0);
    write_bool(&mut runtime, &auto, false);
    runtime.execute_cycle().expect("execute disabled scan");

    assert_eq!(
        read_lreal(&runtime, &applied),
        0.0,
        "the lesson must start and restore in Manual with zero applied heater power"
    );
}

#[test]
fn control_curriculum_water_heater_bias_exposes_requested_and_applied_holding_output() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-bias/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete operating-point-bias lesson");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let kp = IoAddress::parse("%IL8").expect("Kp address");
    let bias = IoAddress::parse("%IL16").expect("bias address");
    let requested = IoAddress::parse("%ML24").expect("requested MV address");
    let applied = IoAddress::parse("%QL0").expect("applied MV address");

    write_lreal(&mut runtime, &sp, 35.0);
    write_lreal(&mut runtime, &pv, 35.0);
    write_lreal(&mut runtime, &kp, 8.0);
    write_lreal(&mut runtime, &bias, 75.0);
    write_bool(&mut runtime, &auto, false);
    runtime.execute_cycle().expect("establish disabled scan");
    write_bool(&mut runtime, &auto, true);
    runtime.execute_cycle().expect("enable biased P controller");

    assert_eq!(
        read_lreal(&runtime, &requested),
        75.0,
        "at zero error the requested output must equal the measured 75 percent bias"
    );
    assert_eq!(
        read_lreal(&runtime, &applied),
        75.0,
        "the non-saturated holding request must be applied without an unrelated rate limit"
    );
}

#[test]
fn control_curriculum_water_heater_bias_allows_bounded_manual_holding_trials() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-bias/src/Main.st", addressed_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete operating-point-bias lesson");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let manual = IoAddress::parse("%ML32").expect("manual request address");
    let requested = IoAddress::parse("%ML24").expect("requested MV address");
    let applied = IoAddress::parse("%QL0").expect("applied MV address");

    write_lreal(&mut runtime, &sp, 35.0);
    write_lreal(&mut runtime, &pv, 35.0);
    write_lreal(&mut runtime, &manual, 75.0);
    write_bool(&mut runtime, &auto, false);
    runtime
        .execute_cycle()
        .expect("execute manual holding trial");

    assert_eq!(read_lreal(&runtime, &requested), 75.0);
    assert_eq!(read_lreal(&runtime, &applied), 75.0);
}

#[test]
fn control_curriculum_water_heater_bias_exports_measured_operating_point_evidence() {
    let manual_trials = [70.0, 75.0, 80.0]
        .into_iter()
        .map(|output| (output, run_manual_trial(output)))
        .collect::<Vec<_>>();
    for (output, trace) in &manual_trials {
        assert_eq!(
            trace.len(),
            601,
            "manual {output}% evidence must include 600 trial samples and its own restoration"
        );
        let restoration = trace.last().expect("manual restoration sample");
        assert!(!restoration.auto);
        assert_eq!(restoration.requested_mv, 0.0);
        assert_eq!(restoration.applied_mv, 0.0);
        let window = &trace[500..600];
        assert!(
            range(window, |sample| sample.pv) <= 0.02,
            "manual {output}% trial must settle before its holding error is compared"
        );
        assert!(
            range(window, |sample| sample.applied_mv) <= 1.0e-9,
            "manual {output}% trial must retain a constant applied command"
        );
        assert!(trace.iter().all(|sample| {
            sample.pv.is_finite()
                && sample.requested_mv.is_finite()
                && sample.applied_mv.is_finite()
                && (0.0..=100.0).contains(&sample.applied_mv)
        }));
    }
    let manual_errors = manual_trials
        .iter()
        .map(|(output, trace)| (*output, mean_absolute_error(&trace[500..600])))
        .collect::<Vec<_>>();
    let selected = manual_errors
        .iter()
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .expect("manual candidates");
    assert_eq!(
        selected.0, 75.0,
        "the executable trials must measure 75% holding output"
    );
    assert!(selected.1 <= 0.01, "selected holding error={}", selected.1);
    assert!(manual_errors[0].1 >= 0.9 && manual_errors[2].1 >= 0.9);

    let zero_bias = run_controller(0.0);
    let measured_bias = run_controller(75.0);
    assert_eq!(zero_bias.len(), 1_201);
    assert_eq!(measured_bias.len(), 1_201);
    assert!(!zero_bias.last().expect("restoration sample").auto);
    assert!(!measured_bias.last().expect("restoration sample").auto);
    assert_eq!(
        zero_bias.last().expect("restoration sample").applied_mv,
        0.0
    );
    assert_eq!(
        measured_bias.last().expect("restoration sample").applied_mv,
        0.0
    );

    let zero_nominal = &zero_bias[500..600];
    let zero_load = &zero_bias[1_100..1_200];
    let bias_nominal = &measured_bias[500..600];
    let bias_load = &measured_bias[1_100..1_200];
    for (name, window) in [
        ("zero nominal", zero_nominal),
        ("zero changed-load", zero_load),
        ("bias nominal", bias_nominal),
        ("bias changed-load", bias_load),
    ] {
        let pv_range = range(window, |sample| sample.pv);
        let applied_range = range(window, |sample| sample.applied_mv);
        assert!(
            pv_range <= 0.1,
            "{name} PV must settle before mean error is reported; range={pv_range}"
        );
        assert!(
            applied_range <= 1.0,
            "{name} applied output must settle before mean error is reported; range={applied_range}"
        );
    }
    let zero_nominal_error = mean_absolute_error(zero_nominal);
    let zero_load_error = mean_absolute_error(zero_load);
    let bias_nominal_error = mean_absolute_error(bias_nominal);
    let bias_load_error = mean_absolute_error(bias_load);
    let load_output_increase = mean_applied(bias_load) - mean_applied(bias_nominal);
    assert!((5.5..=6.0).contains(&zero_nominal_error));
    assert!((6.5..=7.2).contains(&zero_load_error));
    assert!(bias_nominal_error <= 0.05);
    assert!((1.0..=1.3).contains(&bias_load_error));
    assert!(load_output_increase >= 8.0);
    assert!(measured_bias.iter().all(|sample| {
        sample.pv.is_finite()
            && sample.requested_mv.is_finite()
            && sample.applied_mv.is_finite()
            && (0.0..=100.0).contains(&sample.applied_mv)
    }));

    if let Ok(directory) = std::env::var("TRUST_CURRICULUM_FIGURE_DIR") {
        export_artifacts(
            std::path::Path::new(&directory),
            &manual_trials,
            &zero_bias,
            &measured_bias,
        );
    }
    eprintln!(
        "manual final-window MAE 70/75/80={:.6}/{:.6}/{:.6}degC; zero-bias nominal/load MAE={zero_nominal_error:.6}/{zero_load_error:.6}degC; measured-bias nominal/load MAE={bias_nominal_error:.6}/{bias_load_error:.6}degC; measured-bias applied-output increase={load_output_increase:.6}pp",
        manual_errors[0].1,
        manual_errors[1].1,
        manual_errors[2].1,
    );
}
