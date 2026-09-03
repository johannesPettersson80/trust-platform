use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const SINGLE_THRESHOLD_SOURCE: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/src/Main.st"
);
const HYSTERESIS_SOURCE: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/src/Main.st"
);
const HYSTERESIS_SIMULATION: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/simulation.toml"
);
const REACTOR_SOURCE: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/11-reactor-servo-regulatory/src/Main.st"
);
const REACTOR_SIMULATION: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/11-reactor-servo-regulatory/simulation.toml"
);
const WATER_HEATER_SOURCE: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/src/Main.st"
);
const WATER_HEATER_SIMULATION: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/simulation.toml"
);
const WATER_HEATER_PI_SOURCE: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-pi/src/Main.st"
);
const WATER_HEATER_PI_SIMULATION: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-pi/simulation.toml"
);
const SAMPLING_INTERVAL_SOURCE: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/14-sampling-interval/src/Main.st"
);
const SAMPLING_INTERVAL_SIMULATION: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/14-sampling-interval/simulation.toml"
);
const MEASUREMENT_NOISE_SOURCE: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/13-measurement-noise/src/Main.st"
);
const MEASUREMENT_NOISE_SIMULATION: &str = include_str!(
    "../../../editors/vscode/media/examples/control/01-feedback-foundations-and-temperature-control/13-measurement-noise/simulation.toml"
);

fn temporary_config_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-curriculum-{name}-{}-{stamp}.toml",
        std::process::id()
    ))
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

fn write_int(runtime: &mut Runtime, address: &IoAddress, value: i16) {
    runtime
        .io_mut()
        .write(address, Value::Word(value as u16))
        .expect("write INT process-image value");
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn curriculum_project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative);
    assert!(
        path.is_file(),
        "required shipped curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read shipped curriculum file {}: {error}", path.display()))
}

fn addressed_source(source: &str) -> String {
    // Every pattern below is anchored on a leading "\n    " (the standard
    // 4-space VAR-block indent) so it cannot also match a longer declaration
    // that happens to end with the same suffix (e.g. "AutoMode : BOOL :="
    // must not also rewrite "PreviousAutoMode : BOOL :=" or
    // "LastAutoMode : BOOL :=" into an alias of the same %IX24.0 cell, and
    // "Setpoint : LREAL :=" / "ProcessValue : LREAL :=" must not also rewrite
    // "PreviousSetpoint : LREAL :=" / "PreviousProcessValue : LREAL :=").
    source
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace("\n    Setpoint : LREAL :=", "\n    Setpoint AT %IL32 : LREAL :=")
        .replace(
            "\n    HysteresisWidth : LREAL :=",
            "\n    HysteresisWidth AT %IL8 : LREAL :=",
        )
        .replace(
            "\n    AutoMode : BOOL :=",
            "\n    AutoMode AT %IX24.0 : BOOL :=",
        )
        .replace(
            "\n    ProcessValue : LREAL :=",
            "\n    ProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "\n    DoorOpenIndication : LREAL :=",
            "\n    DoorOpenIndication AT %IL8 : LREAL :=",
        )
        .replace(
            "\n    EffectiveSurroundings : LREAL :=",
            "\n    EffectiveSurroundings AT %IL16 : LREAL :=",
        )
        .replace(
            "\n    SimulatorTruePressure : LREAL :=",
            "\n    SimulatorTruePressure AT %IL8 : LREAL :=",
        )
        .replace(
            "\n    BeltSpeed : LREAL :=",
            "\n    BeltSpeed AT %IL16 : LREAL :=",
        )
        .replace(
            "\n    ManipulatedValue : LREAL :=",
            "\n    ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace(
            "\n    AppliedHeaterPower : LREAL :=",
            "\n    AppliedHeaterPower AT %QL0 : LREAL :=",
        )
        .replace(
            "\n    AppliedHeaterTrim : LREAL :=",
            "\n    AppliedHeaterTrim AT %QL0 : LREAL :=",
        )
        .replace(
            "\n    ExperimentEnabled : BOOL :=",
            "\n    ExperimentEnabled AT %IX56.0 : BOOL :=",
        )
}

fn instrumented_thermostat_source(source: &str) -> String {
    addressed_source(source)
        .replace("MAX(MinimumOnTime, MinimumOffTime)", "LREAL#5.0")
        .replace(
            "DwellElapsed := DwellElapsed + CycleTime",
            "DwellElapsed := DwellElapsed + LREAL#0.1",
        )
        .replace(
            "DwellElapsed >= MinimumOffTime - LREAL#1.0e-9",
            "DwellElapsed >= LREAL#5.0 - LREAL#1.0e-9",
        )
        .replace(
            "DwellElapsed >= MinimumOnTime - LREAL#1.0e-9",
            "DwellElapsed >= LREAL#5.0 - LREAL#1.0e-9",
        )
}

fn run_water_heater(proportional_gain: f64) -> Vec<(f64, f64)> {
    let source = addressed_source(WATER_HEATER_SOURCE).replace(
        "ProportionalGain : LREAL :=",
        "ProportionalGain AT %IL8 : LREAL :=",
    );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete water-heater lesson");
    let path = temporary_config_path("water-heater-flow-load");
    std::fs::write(&path, WATER_HEATER_SIMULATION).expect("write water-heater simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load water-heater simulation"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let kp = IoAddress::parse("%IL8").expect("Kp address");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial heater state");
    let mut trace = Vec::with_capacity(1_200);
    for cycle in 1..=1_200 {
        write_lreal(&mut runtime, &sp, 35.0);
        write_lreal(&mut runtime, &kp, proportional_gain);
        write_bool(&mut runtime, &experiment_enabled, true);
        runtime.execute_cycle().expect("execute P controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance water-heater plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish water-heater PV");
        trace.push((read_lreal(&runtime, &pv), read_lreal(&runtime, &mv)));
    }
    let _ = std::fs::remove_file(path);
    trace
}

#[test]
fn control_curriculum_water_heater_p_shipped_defaults_run_selected_candidate_safely() {
    let source = addressed_source(WATER_HEATER_SOURCE);
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped water-heater P lesson");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");
    let mv = IoAddress::parse("%QL0").expect("MV address");

    write_bool(&mut runtime, &experiment_enabled, true);
    runtime.execute_cycle().expect("run selected P candidate");
    assert!(
        (read_lreal(&runtime, &mv) - 100.0).abs() <= 1.0e-12,
        "the shipped 35 degC, Kp 8 candidate must clamp its initial 120% request to 100%"
    );

    write_bool(&mut runtime, &experiment_enabled, false);
    runtime.execute_cycle().expect("restore manual safe state");
    assert!(
        read_lreal(&runtime, &mv).abs() <= 1.0e-12,
        "disabling lesson authority must restore zero applied heater power"
    );
}

fn run_water_heater_with_bias() -> Vec<(f64, f64)> {
    let source = addressed_source(&curriculum_project_file(
        "01-feedback-foundations-and-temperature-control/05-water-heater-bias/src/Main.st",
    ));
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-bias/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete water-heater bias lesson");
    let simulation_source = curriculum_project_file(
        "01-feedback-foundations-and-temperature-control/05-water-heater-bias/simulation.toml",
    );
    let path = temporary_config_path("water-heater-bias");
    std::fs::write(&path, simulation_source).expect("write bias simulation");
    let mut simulation =
        SimulationController::new(SimulationConfig::load(&path).expect("load bias simulation"));
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial biased-heater state");
    let mut trace = Vec::with_capacity(1_200);
    for cycle in 1..=1_200 {
        write_lreal(&mut runtime, &sp, 35.0);
        write_bool(&mut runtime, &experiment_enabled, true);
        runtime
            .execute_cycle()
            .expect("execute biased P controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance biased water heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish biased water-heater PV");
        trace.push((read_lreal(&runtime, &pv), read_lreal(&runtime, &mv)));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_water_heater_integral_only() -> Vec<(f64, f64)> {
    let source = addressed_source(&curriculum_project_file(
        "01-feedback-foundations-and-temperature-control/06-water-heater-integral/src/Main.st",
    ));
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-integral/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete integral-only heater lesson");
    let simulation_source = curriculum_project_file(
        "01-feedback-foundations-and-temperature-control/06-water-heater-integral/simulation.toml",
    );
    let path = temporary_config_path("water-heater-integral");
    std::fs::write(&path, simulation_source).expect("write integral-only simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load integral-only simulation"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial integral-only heater state");
    let mut trace = Vec::with_capacity(1_200);
    for cycle in 1..=1_200 {
        write_lreal(&mut runtime, &sp, 35.0);
        write_bool(&mut runtime, &experiment_enabled, true);
        runtime
            .execute_cycle()
            .expect("execute integral-only controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance integral-only heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish integral-only heater PV");
        trace.push((read_lreal(&runtime, &pv), read_lreal(&runtime, &mv)));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_water_heater_pi() -> Vec<(f64, f64)> {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path(
            "water-heater-pi/src/Main.st",
            addressed_source(WATER_HEATER_PI_SOURCE),
        ),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete water-heater PI lesson");
    let path = temporary_config_path("water-heater-pi-flow-load");
    std::fs::write(&path, WATER_HEATER_PI_SIMULATION).expect("write PI simulation");
    let mut simulation =
        SimulationController::new(SimulationConfig::load(&path).expect("load PI simulation"));
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial PI heater state");
    let mut trace = Vec::with_capacity(1_200);
    for cycle in 1..=1_200 {
        write_lreal(&mut runtime, &sp, 35.0);
        write_bool(&mut runtime, &experiment_enabled, true);
        runtime.execute_cycle().expect("execute PI controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance PI heater plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish PI heater PV");
        trace.push((read_lreal(&runtime, &pv), read_lreal(&runtime, &mv)));
    }
    let _ = std::fs::remove_file(path);
    trace
}

#[test]
fn control_curriculum_water_heater_p_exposes_offset_gain_tradeoff_under_same_load() {
    let low_gain = run_water_heater(2.0);
    let high_gain = run_water_heater(8.0);
    let summarize = |trace: &[(f64, f64)]| {
        let before = trace[590];
        let final_sample = *trace.last().expect("heater samples");
        (
            (35.0 - final_sample.0).abs(),
            (final_sample.1 - before.1).abs(),
        )
    };
    let (low_error, low_mv_change) = summarize(&low_gain);
    let (high_error, high_mv_change) = summarize(&high_gain);

    assert!(
        low_error >= 5.0 && high_error >= 5.0,
        "P-only control must retain visible offset; low={low_error}, high={high_error}"
    );
    assert!(
        high_error < low_error,
        "larger Kp must reduce offset on the identical load; low={low_error}, high={high_error}"
    );
    assert!(high_mv_change > low_mv_change,
        "larger Kp must visibly spend more actuator movement; low={low_mv_change}, high={high_mv_change}");
    assert!(
        low_gain
            .iter()
            .chain(high_gain.iter())
            .all(|(pv, mv)| pv.is_finite() && mv.is_finite() && (0.0..=100.0).contains(mv)),
        "both runs must respect the heater limits"
    );
}

#[test]
fn control_curriculum_water_heater_bias_removes_nominal_offset_but_not_load_offset() {
    let zero_bias = run_water_heater(8.0);
    let biased = run_water_heater_with_bias();
    let mean_error = |samples: &[(f64, f64)]| {
        samples.iter().map(|(pv, _)| (35.0 - pv).abs()).sum::<f64>() / samples.len() as f64
    };
    let mean_mv = |samples: &[(f64, f64)]| {
        samples.iter().map(|(_, mv)| mv).sum::<f64>() / samples.len() as f64
    };
    let zero_bias_nominal_error = mean_error(&zero_bias[500..600]);
    let biased_nominal_error = mean_error(&biased[500..600]);
    let biased_load_error = mean_error(&biased[1_100..1_200]);
    let biased_mv_change = mean_mv(&biased[1_100..1_200]) - mean_mv(&biased[500..600]);

    assert!(
        (5.5..=6.0).contains(&zero_bias_nominal_error),
        "the selected Lesson 4 gain-8 zero-bias baseline must retain nominal offset; error={zero_bias_nominal_error}"
    );
    assert!(
        biased_nominal_error <= 0.1,
        "the measured 75% operating-point bias must remove nominal offset; error={biased_nominal_error}"
    );
    assert!(
        (1.0..=1.3).contains(&biased_load_error),
        "P plus fixed bias must still retain a visible changed-load offset; error={biased_load_error}"
    );
    assert!(
        biased_mv_change >= 8.0,
        "the controller must increase heater power after the cold-water load; change={biased_mv_change}"
    );
    assert!(
        biased
            .iter()
            .all(|(pv, mv)| pv.is_finite() && mv.is_finite() && (0.0..=100.0).contains(mv)),
        "the biased run must remain finite and inside heater limits"
    );
}

#[test]
fn control_curriculum_integral_only_removes_offset_but_responds_slower_than_biased_p() {
    let biased_p = run_water_heater_with_bias();
    let integral_only = run_water_heater_integral_only();
    let first_reach = |trace: &[(f64, f64)]| {
        trace[..600]
            .iter()
            .position(|(pv, _)| *pv >= 34.5)
            .expect("controller must reach 34.5 degC before the load event")
    };
    let biased_p_reach = first_reach(&biased_p);
    let integral_reach = first_reach(&integral_only);
    let final_error = (35.0 - integral_only[1_199].0).abs();

    assert!(
        integral_only[..20]
            .windows(2)
            .all(|pair| pair[1].1 >= pair[0].1 - 1.0e-12),
        "positive persistent error must initially accumulate monotonically into integral MV"
    );
    assert!(
        integral_reach >= biased_p_reach + 20,
        "integral-only control must visibly respond slower than biased P; I={integral_reach}, P={biased_p_reach}"
    );
    assert!(
        final_error <= 0.2,
        "integral-only control must remove the constant-load offset; error={final_error}"
    );
    assert!(
        integral_only
            .iter()
            .all(|(pv, mv)| pv.is_finite() && mv.is_finite() && (0.0..=100.0).contains(mv)),
        "the integral-only run must remain finite and inside heater limits"
    );
}

#[test]
fn control_curriculum_water_heater_pi_removes_identical_load_offset() {
    let proportional = run_water_heater(4.0);
    let proportional_integral = run_water_heater_pi();
    let p_final = *proportional.last().expect("P samples");
    let pi_final = *proportional_integral.last().expect("PI samples");
    let p_error = (35.0 - p_final.0).abs();
    let pi_error = (35.0 - pi_final.0).abs();
    let pi_mv_before_load = proportional_integral[590].1;
    let pi_mv_after_load = proportional_integral[1_199].1;

    assert!(
        p_error >= 5.0,
        "P baseline must retain visible offset; error={p_error}"
    );
    assert!(
        pi_error <= 0.2,
        "PI control must remove the same load offset; error={pi_error}"
    );
    assert!(pi_mv_after_load >= pi_mv_before_load + 10.0,
        "the identical 60 s cold-flow load must require visibly more PI heater demand; before={pi_mv_before_load}, after={pi_mv_after_load}");
    assert!(
        proportional_integral
            .iter()
            .all(|(pv, mv)| pv.is_finite() && mv.is_finite() && (0.0..=100.0).contains(mv)),
        "PI samples must remain finite and within the heater limits"
    );
}

#[test]
fn control_curriculum_on_off_hysteresis_has_two_thresholds() {
    let source = curriculum_project_file(
        "01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/src/Main.st",
    );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "cold-storage/src/Main.st",
        instrumented_thermostat_source(&source),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile cold-storage lesson");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let hysteresis = IoAddress::parse("%IL8").expect("hysteresis address");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");

    let run = |runtime: &mut Runtime, process_value: f64, auto_mode: bool| {
        write_lreal(runtime, &sp, 2.0);
        write_lreal(runtime, &hysteresis, 1.0);
        write_bool(runtime, &experiment_enabled, auto_mode);
        write_lreal(runtime, &pv, process_value);
        runtime.execute_cycle().expect("execute thermostat cycle");
        read_lreal(&runtime, &mv)
    };

    assert_eq!(run(&mut runtime, 2.60, true), 100.0);
    assert_eq!(run(&mut runtime, 2.00, true), 100.0);
    for cycle in 1..49 {
        assert_eq!(
            run(&mut runtime, 1.40, true),
            100.0,
            "minimum-on dwell released at cycle {cycle}"
        );
    }
    assert_eq!(run(&mut runtime, 1.40, true), 0.0);
    assert_eq!(run(&mut runtime, 2.00, true), 0.0);
    for cycle in 1..49 {
        assert_eq!(
            run(&mut runtime, 2.60, true),
            0.0,
            "minimum-off dwell released at cycle {cycle}"
        );
    }
    assert_eq!(run(&mut runtime, 2.60, true), 100.0);

    assert_eq!(
        run(&mut runtime, 2.60, false),
        0.0,
        "leaving Auto must stop cooling immediately"
    );
    for cycle in 1..50 {
        assert_eq!(
            run(&mut runtime, 2.60, true),
            0.0,
            "Auto return bypassed minimum-off dwell at cycle {cycle}"
        );
    }
    assert_eq!(run(&mut runtime, 2.60, true), 100.0);
}

#[test]
fn control_curriculum_cold_storage_single_threshold_reproduces_noise_chatter() {
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "cold-storage/src/Main.st",
        addressed_source(SINGLE_THRESHOLD_SOURCE),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile the shipped single-threshold cold-storage lesson");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");

    let mut outputs = Vec::new();
    for process_value in [2.05, 1.95, 2.04, 1.96, 2.03, 1.97, 2.02, 1.98] {
        write_lreal(&mut runtime, &sp, 2.0);
        write_bool(&mut runtime, &experiment_enabled, true);
        write_lreal(&mut runtime, &pv, process_value);
        runtime.execute_cycle().expect("execute ON/OFF controller");
        outputs.push(read_lreal(&runtime, &mv));
    }

    let transitions = outputs.windows(2).filter(|pair| pair[0] != pair[1]).count();
    assert_eq!(
        transitions, 7,
        "the Lesson 2 single threshold must visibly chatter on every alternating noisy crossing; outputs={outputs:?}"
    );
}

#[test]
fn control_curriculum_cold_storage_loop_anatomy_bounds_manual_actuation_and_exposes_thermal_response(
) {
    let source = curriculum_project_file(
        "01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/src/Main.st",
    )
    .split("CONFIGURATION Config")
    .next()
    .expect("lesson program before configuration")
    .replace(
        "ManualDemand : LREAL :=",
        "ManualDemand AT %IL40 : LREAL :=",
    )
    .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
    .replace(
        "ManipulatedValue : LREAL :=",
        "ManipulatedValue AT %QL0 : LREAL :=",
    )
    .replace(
        "DoorOpenIndication : LREAL :=",
        "DoorOpenIndication AT %IL8 : LREAL :=",
    )
    .replace(
        "EffectiveSurroundings : LREAL :=",
        "EffectiveSurroundings AT %IL16 : LREAL :=",
    )
    .replace(
        "ExperimentEnabled : BOOL :=",
        "ExperimentEnabled AT %IX56.0 : BOOL :=",
    );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "cold-storage-loop-anatomy/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile the shipped loop-anatomy lesson");
    let simulation_source =
        curriculum_project_file(
            "01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/simulation.toml",
        );
    let path = temporary_config_path("cold-storage-loop-anatomy");
    std::fs::write(&path, simulation_source).expect("write loop-anatomy simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load loop-anatomy simulation"),
    );
    let manual_demand = IoAddress::parse("%IL40").expect("manual demand address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial room temperature");
    let initial_temperature = read_lreal(&runtime, &pv);
    let mut cooled_temperature = initial_temperature;

    // Control Studio grants the project's one Boolean authority; the project
    // itself then owns the bounded 150% manual-demand interval and the
    // restoration that follows, matching the loop below.
    write_bool(&mut runtime, &experiment_enabled, true);
    for cycle in 1..=200 {
        write_lreal(&mut runtime, &manual_demand, 150.0);
        runtime
            .execute_cycle()
            .expect("execute bounded manual command");
        assert_eq!(
            read_lreal(&runtime, &mv),
            100.0,
            "unsafe manual demand must be clamped"
        );
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance cold room");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish cold-room PV");
        cooled_temperature = read_lreal(&runtime, &pv);
    }
    assert!(
        cooled_temperature <= initial_temperature - 2.5,
        "compressor actuation must produce a visible delayed cooling response; initial={initial_temperature}, cooled={cooled_temperature}"
    );

    for cycle in 201..=400 {
        write_lreal(&mut runtime, &manual_demand, 0.0);
        runtime
            .execute_cycle()
            .expect("remove manual compressor demand");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance recovering cold room");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish recovering PV");
    }
    let recovered_temperature = read_lreal(&runtime, &pv);
    assert!(
        recovered_temperature >= cooled_temperature + 1.5,
        "removing compressor demand must let the room warm toward ambient; cooled={cooled_temperature}, recovered={recovered_temperature}"
    );
    let _ = std::fs::remove_file(path);
}

fn run_cold_storage(simulation_source: &str) -> (Vec<(f64, f64)>, usize) {
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "cold-storage/src/Main.st",
        instrumented_thermostat_source(HYSTERESIS_SOURCE),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete cold-storage lesson");
    let path = temporary_config_path("cold-storage-door-load");
    std::fs::write(&path, simulation_source).expect("write cold-storage simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load cold-storage simulation"),
    );
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let hysteresis = IoAddress::parse("%IL8").expect("hysteresis address");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial cold-storage state");
    let mut trace = Vec::with_capacity(1_200);
    for cycle in 1..=1_200 {
        write_lreal(&mut runtime, &sp, 2.0);
        write_lreal(&mut runtime, &hysteresis, 1.0);
        write_bool(&mut runtime, &experiment_enabled, true);
        runtime.execute_cycle().expect("execute thermostat");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance cold-storage room");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish cold-storage temperature");
        trace.push((read_lreal(&runtime, &pv), read_lreal(&runtime, &mv)));
    }
    let pending_events = simulation
        .status()
        .plants
        .first()
        .expect("cold-storage plant status")
        .pending_events;
    let _ = std::fs::remove_file(path);
    (trace, pending_events)
}

#[test]
fn control_curriculum_cold_storage_door_load_respects_temperature_and_dwell_limits() {
    let baseline_source = HYSTERESIS_SIMULATION
        .split("[[plant_events]]")
        .next()
        .expect("configuration before door event");
    let (baseline, baseline_pending_events) = run_cold_storage(baseline_source);
    let (door_load, door_pending_events) = run_cold_storage(HYSTERESIS_SIMULATION);
    assert_eq!(baseline_pending_events, 0);
    assert_eq!(
        door_pending_events, 0,
        "both declared door events must execute"
    );
    let on_fraction = |trace: &[(f64, f64)]| {
        trace[600..900].iter().filter(|(_, mv)| *mv > 50.0).count() as f64 / 300.0
    };
    let baseline_duty = on_fraction(&baseline);
    let door_duty = on_fraction(&door_load);
    assert!(
        door_duty > baseline_duty + 0.10,
        "the declared door load must measurably increase compressor duty; baseline={baseline_duty:.3}, door={door_duty:.3}, baseline PV=[{:.3}, {:.3}, {:.3}], door PV=[{:.3}, {:.3}, {:.3}]",
        baseline[600].0,
        baseline[750].0,
        baseline[899].0,
        door_load[600].0,
        door_load[750].0,
        door_load[899].0,
    );
    assert!(
        door_load[600..900].iter().all(|(pv, _)| *pv <= 3.0),
        "food-temperature limit exceeded during the bounded door event"
    );

    let transitions = door_load
        .windows(2)
        .enumerate()
        .filter_map(|(index, pair)| (pair[0].1 != pair[1].1).then_some(index + 1))
        .collect::<Vec<_>>();
    assert!(
        transitions.len() >= 3,
        "lesson must show repeated bounded switching"
    );
    assert!(
        transitions.windows(2).all(|pair| pair[1] - pair[0] >= 50),
        "every completed on/off interval must satisfy the 5 s dwell"
    );
}

fn run_reactor(setpoint: f64, duration_s: usize) -> Vec<(f64, f64)> {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("reactor/src/Main.st", addressed_source(REACTOR_SOURCE)),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile servo/regulatory lesson");
    let path = temporary_config_path("servo-regulatory");
    std::fs::write(&path, REACTOR_SIMULATION).expect("write simulation configuration");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load reactor simulation configuration"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial reactor state");
    write_lreal(&mut runtime, &sp, setpoint);

    let mut trace = Vec::with_capacity(duration_s * 10);
    for cycle in 1..=duration_s * 10 {
        write_lreal(&mut runtime, &sp, setpoint);
        write_bool(&mut runtime, &auto, true);
        runtime.execute_cycle().expect("execute reactor controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance reactor plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish reactor PV");
        trace.push((read_lreal(&runtime, &pv), read_lreal(&runtime, &mv)));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_sampling_oven(intervals_ms: &[i64]) -> Vec<(f64, f64, f64, f64)> {
    let source = addressed_source(SAMPLING_INTERVAL_SOURCE).replace(
        "ActualCycleTime : LREAL :=",
        "ActualCycleTime AT %ML8 : LREAL :=",
    );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("sampling-interval/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete sampling-interval lesson");
    let path = temporary_config_path("sampling-interval");
    std::fs::write(&path, SAMPLING_INTERVAL_SIMULATION)
        .expect("write sampling-interval simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load sampling-interval simulation"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let actual_cycle_time = IoAddress::parse("%ML8").expect("actual cycle-time address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let belt_speed = IoAddress::parse("%IL16").expect("belt-speed address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial oven state");
    let mut elapsed_ms = 0_i64;
    let mut trace = Vec::with_capacity(intervals_ms.len());
    for interval_ms in intervals_ms.iter().copied() {
        write_lreal(&mut runtime, &sp, 1.0);
        write_lreal(
            &mut runtime,
            &actual_cycle_time,
            interval_ms as f64 / 1_000.0,
        );
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute sampling controller");
        elapsed_ms += interval_ms;
        let now = Duration::from_millis(elapsed_ms);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance sampling plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish sampling plant PV");
        trace.push((
            elapsed_ms as f64 / 1_000.0,
            read_lreal(&runtime, &pv),
            read_lreal(&runtime, &mv),
            read_lreal(&runtime, &belt_speed),
        ));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_pump_pressure_noise(filter_enabled: bool) -> Vec<(f64, f64, f64, f64)> {
    let source = addressed_source(MEASUREMENT_NOISE_SOURCE)
        .replace("FilteredValue : LREAL;", "FilteredValue AT %ML8 : LREAL;")
        .replace(
            "FilterSelection : INT :=",
            "FilterSelection AT %MW40 : INT :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("measurement-noise/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile complete measurement-noise lesson");
    let path = temporary_config_path("pump-pressure-noise");
    std::fs::write(&path, MEASUREMENT_NOISE_SIMULATION)
        .expect("write measurement-noise simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load measurement-noise simulation"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let filter = IoAddress::parse("%MW40").expect("filter selector address");
    let raw_pv = IoAddress::parse("%IL0").expect("raw PV address");
    let filtered_pv = IoAddress::parse("%ML8").expect("filtered PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial noisy pressure");
    let mut trace = Vec::with_capacity(300);
    for cycle in 1..=300 {
        write_lreal(&mut runtime, &sp, 1.0);
        write_bool(&mut runtime, &auto, true);
        write_int(&mut runtime, &filter, if filter_enabled { 1 } else { 0 });
        runtime
            .execute_cycle()
            .expect("execute pressure controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance pressure plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish noisy pressure");
        trace.push((
            cycle as f64 / 10.0,
            read_lreal(&runtime, &raw_pv),
            read_lreal(&runtime, &filtered_pv),
            read_lreal(&runtime, &mv),
        ));
    }
    let _ = std::fs::remove_file(path);
    trace
}

#[test]
fn control_curriculum_low_pass_trades_seeded_noise_activity_for_lag() {
    let raw_control = run_pump_pressure_noise(false);
    let filtered_control = run_pump_pressure_noise(true);
    let increment_rms = |values: &[f64]| {
        let sum = values
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).powi(2))
            .sum::<f64>();
        (sum / (values.len() - 1) as f64).sqrt()
    };
    let raw_pressure = filtered_control[100..]
        .iter()
        .map(|(_, raw, _, _)| *raw)
        .collect::<Vec<_>>();
    let filtered_pressure = filtered_control[100..]
        .iter()
        .map(|(_, _, filtered, _)| *filtered)
        .collect::<Vec<_>>();
    let raw_mv = raw_control[100..]
        .iter()
        .map(|(_, _, _, mv)| *mv)
        .collect::<Vec<_>>();
    let filtered_mv = filtered_control[100..]
        .iter()
        .map(|(_, _, _, mv)| *mv)
        .collect::<Vec<_>>();
    let raw_pressure_rms = increment_rms(&raw_pressure);
    let filtered_pressure_rms = increment_rms(&filtered_pressure);
    let raw_mv_rms = increment_rms(&raw_mv);
    let filtered_mv_rms = increment_rms(&filtered_mv);
    let mean_final_error = |trace: &[(f64, f64, f64, f64)], use_filtered: bool| {
        let mean = trace[250..]
            .iter()
            .map(|(_, raw, filtered, _)| if use_filtered { *filtered } else { *raw })
            .sum::<f64>()
            / 50.0;
        (1.0 - mean).abs()
    };
    let rise_time = |trace: &[(f64, f64, f64, f64)], use_filtered: bool| {
        trace
            .iter()
            .find(|(_, raw, filtered, _)| {
                if use_filtered {
                    *filtered >= 0.9
                } else {
                    *raw >= 0.9
                }
            })
            .map(|(time, _, _, _)| *time)
            .unwrap_or(f64::INFINITY)
    };
    let raw_error = mean_final_error(&raw_control, false);
    let filtered_error = mean_final_error(&filtered_control, true);
    let raw_rise = rise_time(&raw_control, false);
    let filtered_rise = rise_time(&filtered_control, true);
    eprintln!(
        "pressure filter: PV increment RMS raw={raw_pressure_rms:.8}, filtered={filtered_pressure_rms:.8}; MV increment RMS raw={raw_mv_rms:.8}, filtered={filtered_mv_rms:.8}; rise raw={raw_rise:.3}s, filtered={filtered_rise:.3}s; mean final error raw={raw_error:.8}, filtered={filtered_error:.8}"
    );

    assert!(filtered_pressure_rms <= raw_pressure_rms * 0.75,
        "the visible filter must attenuate seeded pressure noise; raw={raw_pressure_rms}, filtered={filtered_pressure_rms}");
    assert!(filtered_mv_rms <= raw_mv_rms * 0.75,
        "filtering must reduce noise-driven valve movement; raw={raw_mv_rms}, filtered={filtered_mv_rms}");
    assert!(filtered_rise >= raw_rise,
        "the lesson must expose rather than hide filter lag; raw={raw_rise}, filtered={filtered_rise}");
    assert!(raw_error <= 0.05 && filtered_error <= 0.05,
        "both runs must retain acceptable mean tracking; raw={raw_error}, filtered={filtered_error}");
    assert!(
        raw_control
            .iter()
            .chain(filtered_control.iter())
            .all(|(_, raw, filtered, mv)| raw.is_finite()
                && filtered.is_finite()
                && mv.is_finite()
                && (-10.0..=10.0).contains(mv)),
        "both seeded runs must remain finite and within valve limits"
    );
}

#[test]
fn control_curriculum_sampling_project_uses_each_actual_cycle_interval() {
    let first_output = |step_ms| run_sampling_oven(&[step_ms])[0].2;
    let fast_output = first_output(50);
    let nominal_output = first_output(100);
    let slow_output = first_output(200);

    assert!(
        (fast_output - 0.03335).abs() <= 1.0e-9,
        "50 ms update must use its actual integral increment; output={fast_output}"
    );
    assert!(
        (nominal_output - 0.0667).abs() <= 1.0e-9,
        "100 ms update must use its actual integral increment; output={nominal_output}"
    );
    assert!(
        (slow_output - 0.1334).abs() <= 1.0e-9,
        "200 ms update must use its actual integral increment; output={slow_output}"
    );
    assert!(
        fast_output < nominal_output && nominal_output < slow_output,
        "a hard-coded nominal dt would make all three executed increments identical"
    );

    let response_metrics = |trace: &[(f64, f64, f64, f64)]| {
        let peak = trace
            .iter()
            .map(|(_, pv, _, _)| *pv)
            .fold(f64::NEG_INFINITY, f64::max)
            .max(1.0);
        let overshoot_percent = (peak - 1.0) * 100.0;
        let settling_time = trace
            .iter()
            .filter_map(|(time, pv, _, _)| ((1.0 - pv).abs() > 0.02).then_some(*time))
            .last()
            .unwrap_or(0.0);
        let final_error = (1.0 - trace.last().expect("response samples").1).abs();
        (overshoot_percent, settling_time, final_error)
    };
    for step_ms in [50_i64, 100, 200] {
        let intervals = vec![step_ms; (30_000 / step_ms) as usize];
        let trace = run_sampling_oven(&intervals);
        let (overshoot, settling, final_error) = response_metrics(&trace);
        assert!(
            overshoot <= 20.0,
            "{step_ms} ms response exceeded 20% overshoot: {overshoot}"
        );
        assert!(
            settling <= 8.0,
            "{step_ms} ms response exceeded 8 s settling: {settling}"
        );
        assert!(
            final_error <= 0.02,
            "{step_ms} ms response exceeded 2% final error: {final_error}"
        );
    }

    let jitter_pattern = [20_i64, 180, 50, 150, 100];
    let intervals = jitter_pattern
        .into_iter()
        .cycle()
        .take(300)
        .collect::<Vec<_>>();
    let jitter = run_sampling_oven(&intervals);
    let (jitter_overshoot, jitter_settling, jitter_final_error) = response_metrics(&jitter);
    assert!(
        jitter_overshoot <= 20.0,
        "actual-dt jitter run exceeded 20% overshoot: {jitter_overshoot}"
    );
    assert!(
        jitter_settling <= 8.0,
        "actual-dt jitter run exceeded 8 s settling: {jitter_settling}"
    );
    assert!(
        jitter_final_error <= 0.02,
        "actual-dt jitter run must finish within 2% of SP; final error={jitter_final_error}"
    );
    assert!(
        jitter
            .iter()
            .all(|(_, pv, mv, _)| pv.is_finite() && mv.is_finite() && mv.abs() <= 10.0),
        "actual-dt jitter run must remain finite and inside heater limits"
    );
    assert!(
        jitter
            .iter()
            .all(|(_, _, _, belt_speed)| (*belt_speed - 0.5).abs() <= 1.0e-12),
        "the declared 0.5 m/s belt-speed operating condition must cross ordinary input I/O"
    );
}

#[test]
fn control_curriculum_separates_servo_and_regulatory_metrics() {
    let servo = run_reactor(85.0, 50);
    let servo_peak = servo
        .iter()
        .map(|(pv, _)| *pv)
        .fold(f64::NEG_INFINITY, f64::max);
    let servo_final = servo.last().expect("servo samples").0;
    assert!(
        servo_peak > 84.0,
        "servo run must visibly follow its 85 °C SP; peak={servo_peak}, final={servo_final}"
    );
    assert!(
        (85.0 - servo_final).abs() < 1.0,
        "servo final error must be reported against its changed SP; final={servo_final}"
    );

    let regulatory = run_reactor(80.0, 120);
    let before_event = regulatory[590].0;
    let minimum_after_event = regulatory[600..]
        .iter()
        .map(|(pv, _)| *pv)
        .fold(f64::INFINITY, f64::min);
    let (regulatory_final, final_mv) = *regulatory.last().expect("regulatory samples");
    assert!((before_event - 80.0).abs() < 0.2);
    assert!(
        minimum_after_event < 79.0,
        "cold feed must visibly disturb PV"
    );
    assert!(
        (regulatory_final - 80.0).abs() < 0.5,
        "PI must recover toward the unchanged regulatory SP"
    );
    assert!(
        (final_mv - 70.0).abs() < 3.0,
        "the load balance requires about 20 percentage points more valve"
    );
}
