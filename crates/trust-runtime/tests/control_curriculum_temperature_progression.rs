use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");

fn project_file(project: &str, relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control")
        .join(match project {
            "water-heater-bias" => "05-water-heater-bias",
            "water-heater-integral" => "06-water-heater-integral",
            "water-heater-pi" => "07-water-heater-pi",
            "water-heater-saturation" => "08-water-heater-saturation",
            "rate-limit" => "09-rate-limit",
            "saturation-antiwindup" => "10-saturation-antiwindup",
            "reactor-servo-regulatory" => "11-reactor-servo-regulatory",
            "reactor-setpoint-weighting" => "12-reactor-setpoint-weighting",
            "measurement-noise" => "13-measurement-noise",
            "sampling-interval" => "14-sampling-interval",
            _ => panic!("unknown temperature-control lesson: {project}"),
        })
        .join(relative);
    assert!(
        path.is_file(),
        "required shipped curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read shipped curriculum file {}: {error}", path.display()))
}

fn temporary_config_path(project: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-temperature-{project}-{}-{stamp}.toml",
        std::process::id()
    ))
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
            "\n    AutoMode : BOOL :=",
            "\n    AutoMode AT %IX24.0 : BOOL :=",
        )
        .replace(
            "\n    ProcessValue : LREAL :=",
            "\n    ProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "\n    SimulatorTruePressure : LREAL :=",
            "\n    SimulatorTruePressure AT %IL8 : LREAL :=",
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
            "\n    AppliedValvePosition : LREAL :=",
            "\n    AppliedValvePosition AT %QL0 : LREAL :=",
        )
        .replace(
            "\n    ExperimentEnabled : BOOL :=",
            "\n    ExperimentEnabled AT %IX56.0 : BOOL :=",
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

fn read_bool(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL value") {
        Value::Bool(value) => value,
        other => panic!("expected BOOL storage, got {other:?}"),
    }
}

fn run_heater(project: &str) -> Vec<(f64, f64)> {
    run_heater_at_setpoint(project, 35.0)
}

fn run_heater_at_setpoint(project: &str, setpoint: f64) -> Vec<(f64, f64)> {
    let source = addressed_source(&project_file(project, "src/Main.st"));
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path(format!("{project}/src/Main.st"), source),
    ])
    .label_errors(true)
    .build_runtime()
    .unwrap_or_else(|error| panic!("compile shipped {project} lesson: {error:?}"));
    let path = temporary_config_path(project);
    std::fs::write(&path, project_file(project, "simulation.toml"))
        .expect("write heater simulation");
    let mut simulation =
        SimulationController::new(SimulationConfig::load(&path).expect("load heater simulation"));
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let experiment_enabled = IoAddress::parse("%IX56.0").expect("experiment-enabled address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial heater state");
    let mut trace = Vec::with_capacity(1_200);
    for cycle in 1..=1_200 {
        write_lreal(&mut runtime, &sp, setpoint);
        write_bool(&mut runtime, &auto, true);
        write_bool(&mut runtime, &experiment_enabled, true);
        runtime.execute_cycle().expect("execute heater controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish heater PV");
        trace.push((read_lreal(&runtime, &pv), read_lreal(&runtime, &mv)));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_heater_saturation_experiment() -> Vec<(f64, f64, f64, f64, f64)> {
    let source = project_file("water-heater-saturation", "src/Main.st")
        .replace(
            "RequestedHeaterPower : LREAL :=",
            "RequestedHeaterPower AT %ML8 : LREAL :=",
        )
        .replace(
            "RequestedManipulatedValue : LREAL :=",
            "RequestedManipulatedValue AT %ML8 : LREAL :=",
        )
        .replace(
            "SaturationError : LREAL :=",
            "SaturationError AT %ML16 : LREAL :=",
        );
    let source = addressed_source(&source);
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("water-heater-saturation/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped water-heater-saturation lesson");
    let path = temporary_config_path("water-heater-saturation");
    std::fs::write(
        &path,
        project_file("water-heater-saturation", "simulation.toml"),
    )
    .expect("write heater saturation simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load heater saturation simulation"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let applied_mv = IoAddress::parse("%QL0").expect("applied MV address");
    let requested_mv = IoAddress::parse("%ML8").expect("requested MV address");
    let saturation_error = IoAddress::parse("%ML16").expect("saturation-error address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial heater state");
    let mut trace = Vec::with_capacity(1_200);
    for cycle in 1..=1_200 {
        let setpoint = if cycle <= 600 { 45.0 } else { 35.0 };
        write_lreal(&mut runtime, &sp, setpoint);
        write_bool(&mut runtime, &auto, true);
        runtime.execute_cycle().expect("execute heater controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish heater PV");
        trace.push((
            cycle as f64 * 0.1,
            read_lreal(&runtime, &pv),
            read_lreal(&runtime, &requested_mv),
            read_lreal(&runtime, &applied_mv),
            read_lreal(&runtime, &saturation_error),
        ));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_heating_valve_rate_experiment() -> Vec<(f64, f64, f64, f64)> {
    let mut source = project_file("rate-limit", "src/Main.st");
    if source.contains("RequestedValvePosition : LREAL :=") {
        source = source.replace(
            "RequestedValvePosition : LREAL :=",
            "RequestedValvePosition AT %ML8 : LREAL :=",
        );
    } else if source.contains("RequestedManipulatedValue : LREAL :=") {
        source = source.replace(
            "RequestedManipulatedValue : LREAL :=",
            "RequestedManipulatedValue AT %ML8 : LREAL :=",
        );
    } else {
        source = source.replace(
            "ManipulatedValue : LREAL := 0.0;",
            "ManipulatedValue : LREAL := 0.0;\n    RequestedManipulatedValue AT %ML8 : LREAL := 0.0;",
        );
        source = source.replace(
            "ManipulatedValue := Controller.Update(Input := Input);",
            "ManipulatedValue := Controller.Update(Input := Input);\nRequestedManipulatedValue := ManipulatedValue;",
        );
    }
    let source = addressed_source(&source);
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("rate-limit/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped heating-valve rate-limit lesson");
    let path = temporary_config_path("rate-limit");
    std::fs::write(&path, project_file("rate-limit", "simulation.toml"))
        .expect("write heating-valve simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load heating-valve simulation"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let applied = IoAddress::parse("%QL0").expect("applied valve address");
    let requested = IoAddress::parse("%ML8").expect("requested valve address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial heating-valve state");
    let mut trace = Vec::with_capacity(400);
    for cycle in 1..=400 {
        let setpoint = if cycle <= 150 { 10.0 } else { 0.0 };
        write_lreal(&mut runtime, &sp, setpoint);
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute heating-valve controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance flow plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish flow PV");
        trace.push((
            cycle as f64 * 0.1,
            read_lreal(&runtime, &pv),
            read_lreal(&runtime, &requested),
            read_lreal(&runtime, &applied),
        ));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_batch_vessel_anti_windup_strategy(strategy: i16) -> Vec<(f64, f64, f64, f64)> {
    let source = project_file("saturation-antiwindup", "src/Main.st");
    run_batch_vessel_anti_windup_strategy_from_source(strategy, &source)
}

fn run_batch_vessel_anti_windup_strategy_from_source(
    strategy: i16,
    lesson_source: &str,
) -> Vec<(f64, f64, f64, f64)> {
    let mut source = lesson_source.replace(
        "RequestedOutput : LREAL :=",
        "RequestedOutput AT %ML8 : LREAL :=",
    );
    let uses_strategy_selector = source.contains("AntiWindupStrategy : INT :=");
    if uses_strategy_selector {
        source = source.replace(
            "AntiWindupStrategy : INT :=",
            "AntiWindupStrategy AT %IW12 : INT :=",
        );
    } else {
        source = source.replace(
            "AntiWindupEnabled : BOOL :=",
            "AntiWindupEnabled AT %IX24.1 : BOOL :=",
        );
    }
    let source = addressed_source(&source);
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("saturation-antiwindup/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped four-strategy anti-windup lesson");
    let path = temporary_config_path("saturation-antiwindup-strategies");
    std::fs::write(
        &path,
        project_file("saturation-antiwindup", "simulation.toml"),
    )
    .expect("write batch-vessel simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load batch-vessel simulation"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let selector = if uses_strategy_selector {
        IoAddress::parse("%IW12").expect("strategy address")
    } else {
        IoAddress::parse("%IX24.1").expect("legacy anti-windup address")
    };
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("applied MV address");
    let requested = IoAddress::parse("%ML8").expect("requested-output address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial batch-vessel state");
    let mut trace = Vec::with_capacity(500);
    for cycle in 1..=500 {
        let setpoint = if (10..200).contains(&cycle) { 4.5 } else { 0.0 };
        write_lreal(&mut runtime, &sp, setpoint);
        write_bool(&mut runtime, &auto, true);
        if uses_strategy_selector {
            write_int(&mut runtime, &selector, strategy);
        } else {
            write_bool(&mut runtime, &selector, strategy != 0);
        }
        runtime
            .execute_cycle()
            .expect("execute batch-vessel controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance batch vessel");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish vessel PV");
        trace.push((
            cycle as f64 * 0.1,
            read_lreal(&runtime, &pv),
            read_lreal(&runtime, &mv),
            read_lreal(&runtime, &requested),
        ));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn batch_vessel_selected_settings(strategy: i16) -> (f64, f64, f64, f64, f64) {
    let source = project_file("saturation-antiwindup", "src/Main.st")
        .replace(
            "AntiWindupStrategy : INT :=",
            "AntiWindupStrategy AT %IW12 : INT :=",
        )
        .replace(
            "TrackingTimeUsed : LREAL :=",
            "TrackingTimeUsed AT %ML16 : LREAL :=",
        )
        .replace(
            "ProportionalGainUsed : LREAL :=",
            "ProportionalGainUsed AT %ML24 : LREAL :=",
        )
        .replace(
            "IntegralGainUsed : LREAL :=",
            "IntegralGainUsed AT %ML32 : LREAL :=",
        )
        .replace(
            "DerivativeGainUsed : LREAL :=",
            "DerivativeGainUsed AT %ML48 : LREAL :=",
        )
        .replace(
            "ConfiguredCycleTime : LREAL :=",
            "ConfiguredCycleTime AT %ML56 : LREAL :=",
        );
    let source = addressed_source(&source);
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("saturation-antiwindup/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile Lesson 10 setting diagnostics");
    write_int(
        &mut runtime,
        &IoAddress::parse("%IW12").expect("strategy address"),
        strategy,
    );
    write_bool(
        &mut runtime,
        &IoAddress::parse("%IX24.0").expect("mode address"),
        true,
    );
    runtime
        .execute_cycle()
        .expect("initialize Lesson 10 strategy");
    let read = |address| {
        read_lreal(
            &runtime,
            &IoAddress::parse(address).expect("Lesson 10 diagnostic address"),
        )
    };
    (
        read("%ML16"),
        read("%ML24"),
        read("%ML32"),
        read("%ML48"),
        read("%ML56"),
    )
}

fn run_reactor_objective_experiment(
    setpoint: f64,
    duration_seconds: usize,
) -> Vec<(f64, f64, f64)> {
    let source = addressed_source(&project_file("reactor-servo-regulatory", "src/Main.st"));
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("reactor-servo-regulatory/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped reactor servo/regulatory lesson");
    let path = temporary_config_path("reactor-servo-regulatory");
    std::fs::write(
        &path,
        project_file("reactor-servo-regulatory", "simulation.toml"),
    )
    .expect("write reactor simulation");
    let mut simulation =
        SimulationController::new(SimulationConfig::load(&path).expect("load reactor simulation"));
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial reactor state");
    let mut trace = Vec::with_capacity(duration_seconds * 10);
    for cycle in 1..=duration_seconds * 10 {
        let commanded_setpoint = if setpoint != 80.0 && cycle <= 50 {
            80.0
        } else {
            setpoint
        };
        write_lreal(&mut runtime, &sp, commanded_setpoint);
        write_bool(&mut runtime, &auto, true);
        runtime.execute_cycle().expect("execute reactor controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance reactor");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish reactor PV");
        trace.push((
            cycle as f64 * 0.1,
            read_lreal(&runtime, &pv),
            read_lreal(&runtime, &mv),
        ));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_reactor_setpoint_weight(
    setpoint_weight: f64,
    setpoint: f64,
    duration_seconds: usize,
) -> Vec<(f64, f64, f64)> {
    let source = project_file("reactor-setpoint-weighting", "src/Main.st").replace(
        "SetpointWeight : LREAL :=",
        "SetpointWeight AT %IL40 : LREAL :=",
    );
    let source = addressed_source(&source);
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("reactor-setpoint-weighting/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped reactor setpoint-weighting lesson");
    let path = temporary_config_path("reactor-setpoint-weighting");
    std::fs::write(
        &path,
        project_file("reactor-setpoint-weighting", "simulation.toml"),
    )
    .expect("write setpoint-weighting reactor simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load setpoint-weighting reactor simulation"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let weight = IoAddress::parse("%IL40").expect("setpoint-weight address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial reactor state");
    let mut trace = Vec::with_capacity(duration_seconds * 10);
    for cycle in 1..=duration_seconds * 10 {
        let commanded_setpoint = if setpoint != 80.0 && cycle <= 50 {
            80.0
        } else {
            setpoint
        };
        write_lreal(&mut runtime, &sp, commanded_setpoint);
        write_lreal(&mut runtime, &weight, setpoint_weight);
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute weighted reactor controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance reactor");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish reactor PV");
        trace.push((
            cycle as f64 * 0.1,
            read_lreal(&runtime, &pv),
            read_lreal(&runtime, &mv),
        ));
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_pump_pressure_filter_strategy(strategy: i16) -> Vec<(f64, f64, f64, f64)> {
    let mut source = project_file("measurement-noise", "src/Main.st")
        .replace("FilteredValue : LREAL;", "FilteredValue AT %ML8 : LREAL;");
    if source.contains("FilterSelection : INT :=") {
        source = source.replace(
            "FilterSelection : INT :=",
            "FilterSelection AT %IW12 : INT :=",
        );
    } else {
        source = source.replace(
            "AutoMode : BOOL := TRUE;",
            "AutoMode : BOOL := TRUE;\n    FilterSelection AT %IW12 : INT := 2;",
        );
    }
    let source = addressed_source(&source);
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("measurement-noise/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped pump-pressure filtering lesson");
    let path = temporary_config_path("measurement-noise-strategies");
    std::fs::write(&path, project_file("measurement-noise", "simulation.toml"))
        .expect("write noisy pump simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load noisy pump simulation"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let selector = IoAddress::parse("%IW12").expect("filter selector address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let raw_pv = IoAddress::parse("%IL0").expect("raw PV address");
    let controller_pv = IoAddress::parse("%ML8").expect("controller PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial pump pressure");
    let mut trace = Vec::with_capacity(400);
    for cycle in 1..=400 {
        let setpoint = if cycle <= 50 { 0.0 } else { 1.0 };
        let aligned_raw_pv = read_lreal(&runtime, &raw_pv);
        write_lreal(&mut runtime, &sp, setpoint);
        write_int(&mut runtime, &selector, strategy);
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute pressure controller");
        trace.push((
            cycle as f64 * 0.1,
            aligned_raw_pv,
            read_lreal(&runtime, &controller_pv),
            read_lreal(&runtime, &mv),
        ));
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance pressure plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish noisy pressure");
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn run_oven_timing_sample(cycle_time: f64, use_actual_cycle_time: bool) -> (bool, f64) {
    run_oven_timing_sequence(&[(cycle_time, use_actual_cycle_time, true)])
        .pop()
        .expect("one oven timing sample")
}

fn run_oven_timing_sequence(samples: &[(f64, bool, bool)]) -> Vec<(bool, f64)> {
    let mut source = project_file("sampling-interval", "src/Main.st")
        .replace(
            "ActualCycleTime : LREAL :=",
            "ActualCycleTime AT %IL32 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL40 : LREAL :=");
    if source.contains("TimingValid : BOOL :=") {
        source = source.replace("TimingValid : BOOL :=", "TimingValid AT %MX20.1 : BOOL :=");
    } else {
        source = source.replace(
            "AutoMode : BOOL := TRUE;",
            "AutoMode : BOOL := TRUE;\n    TimingValid AT %MX20.1 : BOOL := TRUE;",
        );
        source = source.replace(
            "Input.Setpoint := Setpoint;",
            "TimingValid := TRUE;\nInput.Setpoint := Setpoint;",
        );
    }
    if source.contains("UseActualCycleTime : BOOL :=") {
        source = source.replace(
            "UseActualCycleTime : BOOL :=",
            "UseActualCycleTime AT %IX24.2 : BOOL :=",
        );
    } else {
        source = source.replace(
            "AutoMode : BOOL := TRUE;",
            "AutoMode : BOOL := TRUE;\n    UseActualCycleTime AT %IX24.2 : BOOL := TRUE;",
        );
    }
    let source = addressed_source(&source);
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("sampling-interval/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped conveyor-oven timing lesson");
    let dt = IoAddress::parse("%IL32").expect("actual-dt address");
    let valid = IoAddress::parse("%MX20.1").expect("timing-valid address");
    let sp = IoAddress::parse("%IL40").expect("SP address");
    let auto = IoAddress::parse("%IX24.0").expect("mode address");
    let actual_mode = IoAddress::parse("%IX24.2").expect("cycle-time-mode address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    write_lreal(&mut runtime, &sp, 1.0);
    samples
        .iter()
        .map(|(cycle_time, use_actual_cycle_time, auto_mode)| {
            write_lreal(&mut runtime, &dt, *cycle_time);
            write_bool(&mut runtime, &auto, *auto_mode);
            write_bool(&mut runtime, &actual_mode, *use_actual_cycle_time);
            runtime.execute_cycle().expect("execute oven timing sample");
            (read_bool(&runtime, &valid), read_lreal(&runtime, &mv))
        })
        .collect()
}

#[test]
fn control_curriculum_water_heater_pi_combines_prompt_response_and_zero_offset_then_exposes_saturation(
) {
    let biased_p = run_heater("water-heater-bias");
    let integral_only = run_heater("water-heater-integral");
    let pi = run_heater("water-heater-pi");
    let unreachable_pi = run_heater_at_setpoint("water-heater-pi", 45.0);
    let settle_index = |trace: &[(f64, f64)]| {
        trace[..600]
            .iter()
            .rposition(|(pv, _)| (35.0 - *pv).abs() > 0.5)
            .map(|index| index + 1)
            .unwrap_or(0)
    };
    let p_load_error = (35.0 - biased_p[1_199].0).abs();
    let pi_load_error = (35.0 - pi[1_199].0).abs();
    let saturation_samples = unreachable_pi
        .iter()
        .filter(|(_, mv)| *mv >= 100.0 - 1.0e-9)
        .count();
    let integral_settle = settle_index(&integral_only);
    let pi_settle = settle_index(&pi);
    let integral_overshoot = integral_only[..600]
        .iter()
        .map(|(pv, _)| *pv - 35.0)
        .fold(0.0_f64, f64::max);
    let pi_overshoot = pi[..600]
        .iter()
        .map(|(pv, _)| *pv - 35.0)
        .fold(0.0_f64, f64::max);

    assert!(pi_settle + 20 <= integral_settle,
        "PI must settle visibly sooner than integral-only control; PI={pi_settle}, I={integral_settle}");
    assert!(pi_overshoot < integral_overshoot,
        "PI must be better damped than integral-only control; PI overshoot={pi_overshoot}, I overshoot={integral_overshoot}");
    assert!(
        p_load_error >= 1.0,
        "the biased-P baseline must retain changed-load offset; error={p_load_error}"
    );
    assert!(
        pi_load_error <= 0.2,
        "PI must remove the same changed-load offset; error={pi_load_error}"
    );
    assert!(
        saturation_samples > 0,
        "the PI setpoint experiment must expose the output saturation that motivates Lesson 8"
    );
    assert!(
        pi.iter()
            .all(|(pv, mv)| pv.is_finite() && mv.is_finite() && (0.0..=100.0).contains(mv)),
        "PI samples must remain finite and respect the physical heater range"
    );
}

#[test]
fn control_curriculum_heater_saturation_separates_requested_and_applied_mv_and_exposes_windup() {
    let trace = run_heater_saturation_experiment();
    let unreachable = &trace[..600];
    let restored = &trace[600..];
    let time_at_upper_limit = unreachable
        .iter()
        .filter(|(_, _, _, applied, _)| *applied >= 100.0 - 1.0e-9)
        .count() as f64
        * 0.1;
    let peak_requested = unreachable
        .iter()
        .map(|(_, _, requested, _, _)| *requested)
        .fold(f64::NEG_INFINITY, f64::max);
    let peak_saturation_error = unreachable
        .iter()
        .map(|(_, _, _, _, error)| *error)
        .fold(f64::NEG_INFINITY, f64::max);
    let post_restore_saturation = restored
        .iter()
        .take_while(|(_, _, _, applied, _)| *applied >= 100.0 - 1.0e-9)
        .count() as f64
        * 0.1;

    eprintln!(
        "heater saturation: peak requested={peak_requested:.3}%, peak saturation error={peak_saturation_error:.3}%, time at limit={time_at_upper_limit:.3}s, post-restore saturation={post_restore_saturation:.3}s"
    );
    assert!(time_at_upper_limit >= 20.0,
        "the unreachable request must hold the real heater at its upper limit long enough to observe; time={time_at_upper_limit}");
    assert!(
        peak_requested >= 120.0,
        "the hidden PI state must accumulate unavailable demand above 100%; peak={peak_requested}"
    );
    assert!(peak_saturation_error >= 20.0,
        "saturation error must equal a visibly positive requested-minus-applied gap; peak={peak_saturation_error}");
    assert!(post_restore_saturation >= 1.0,
        "windup must keep applied MV at 100% after SP returns to an achievable value; duration={post_restore_saturation}");
    assert!(trace.iter().all(|(_, pv, requested, applied, error)|
        pv.is_finite()
            && requested.is_finite()
            && applied.is_finite()
            && error.is_finite()
            && (0.0..=100.0).contains(applied)
            && (*error - (*requested - *applied)).abs() <= 1.0e-9),
        "every sample must remain finite, respect the physical heater range, and report requested-minus-applied saturation error");
}

#[test]
fn control_curriculum_heating_valve_rate_limit_uses_configured_dt_and_asymmetric_travel() {
    let trace = run_heating_valve_rate_experiment();
    let increments: Vec<f64> = trace
        .windows(2)
        .map(|samples| samples[1].3 - samples[0].3)
        .collect();
    let maximum_opening_rate = increments.iter().copied().fold(0.0_f64, f64::max) / 0.1;
    let maximum_closing_rate = -increments.iter().copied().fold(0.0_f64, f64::min) / 0.1;
    let maximum_request_gap = trace
        .iter()
        .map(|(_, _, requested, applied)| (*requested - *applied).abs())
        .fold(0.0_f64, f64::max);
    let pv_at_five_seconds = trace[49].1;

    eprintln!(
        "heating-valve rate limit: opening={maximum_opening_rate:.6}%/s, closing={maximum_closing_rate:.6}%/s, request gap={maximum_request_gap:.6}%, PV at 5 s={pv_at_five_seconds:.6} L/min"
    );
    assert!((1.4..=1.5 + 1.0e-9).contains(&maximum_opening_rate),
        "the valve must demonstrate its 1.5%/s opening limit using dt=0.1 s; rate={maximum_opening_rate}");
    assert!((0.4..=0.5 + 1.0e-9).contains(&maximum_closing_rate),
        "the valve must demonstrate its slower 0.5%/s closing limit using dt=0.1 s; rate={maximum_closing_rate}");
    assert!(maximum_request_gap >= 1.0,
        "the lesson must retain a requested position distinct from rate-limited applied position; gap={maximum_request_gap}");
    assert!(pv_at_five_seconds < 6.0,
        "the rate-limited actuator must visibly constrain achievable early flow response; PV={pv_at_five_seconds}");
    assert!(trace.iter().all(|(_, pv, requested, applied)|
        pv.is_finite()
            && requested.is_finite()
            && applied.is_finite()
            && (0.0..=100.0).contains(applied)),
        "all samples must remain finite and applied valve position must respect its amplitude range");
}

#[test]
fn control_curriculum_batch_vessel_exposes_the_selected_pi_settings() {
    let signal_clamping = batch_vessel_selected_settings(1);
    let back_calculation = batch_vessel_selected_settings(2);
    let combined = batch_vessel_selected_settings(3);

    assert_eq!(signal_clamping, (0.1, 2.83, 0.8707692307692307, 0.0, 0.1));
    assert_eq!(back_calculation, (3.25, 2.83, 0.8707692307692307, 0.0, 0.1));
    assert_eq!(combined, (3.25, 2.83, 0.8707692307692307, 0.0, 0.1));
}

#[test]
fn control_curriculum_batch_vessel_uses_pi_before_derivative_is_taught() {
    let shipped = run_batch_vessel_anti_windup_strategy(2);
    let pi_source = project_file("saturation-antiwindup", "src/Main.st").replace(
        "Parameters.DerivativeGain := LREAL#0.6509;",
        "Parameters.DerivativeGain := LREAL#0;",
    );
    let zero_derivative_reference =
        run_batch_vessel_anti_windup_strategy_from_source(2, &pi_source);
    let maximum_difference = shipped
        .iter()
        .zip(&zero_derivative_reference)
        .flat_map(|(actual, expected)| {
            [
                (actual.1 - expected.1).abs(),
                (actual.2 - expected.2).abs(),
                (actual.3 - expected.3).abs(),
            ]
        })
        .fold(0.0_f64, f64::max);

    assert!(
        maximum_difference <= 1.0e-12,
        "Lesson 10 must execute as PI because derivative action is not taught until Lesson 20; maximum trajectory difference from zero-derivative reference={maximum_difference}"
    );
}

#[test]
fn control_curriculum_batch_vessel_compares_four_anti_windup_strategies() {
    let traces: Vec<Vec<(f64, f64, f64, f64)>> =
        (0..=3).map(run_batch_vessel_anti_windup_strategy).collect();
    let recovery_time = |trace: &[(f64, f64, f64, f64)]| {
        trace[199..]
            .iter()
            .rposition(|(_, pv, _, _)| pv.abs() > 0.05)
            .map(|index| (index + 1) as f64 * 0.1)
            .unwrap_or(0.0)
    };
    let post_return_saturation = |trace: &[(f64, f64, f64, f64)]| {
        trace[199..]
            .iter()
            .take_while(|(_, _, mv, _)| *mv >= 3.5 - 1.0e-9)
            .count() as f64
            * 0.1
    };
    let total_valve_travel = |trace: &[(f64, f64, f64, f64)]| {
        trace
            .windows(2)
            .map(|samples| (samples[1].2 - samples[0].2).abs())
            .sum::<f64>()
    };
    let recovery_undershoot = |trace: &[(f64, f64, f64, f64)]| {
        -trace[199..]
            .iter()
            .map(|(_, pv, _, _)| *pv)
            .fold(0.0_f64, f64::min)
    };
    let peak_unavailable_request = |trace: &[(f64, f64, f64, f64)]| {
        trace
            .iter()
            .map(|(_, _, applied, requested)| requested - applied)
            .fold(0.0_f64, f64::max)
    };
    let metrics: Vec<(f64, f64, f64, f64, f64)> = traces
        .iter()
        .map(|trace| {
            (
                recovery_time(trace),
                post_return_saturation(trace),
                recovery_undershoot(trace),
                total_valve_travel(trace),
                peak_unavailable_request(trace),
            )
        })
        .collect();

    eprintln!(
        "anti-windup strategy metrics [recovery, post-return saturation, undershoot, travel, unavailable request]: {metrics:?}"
    );
    assert!(metrics[0].0 >= metrics[1].0 + 1.0
            && metrics[0].0 >= metrics[2].0 + 1.0
            && metrics[0].0 >= metrics[3].0 + 1.0,
        "each limit-aware strategy must recover at least 1 s faster than no anti-windup; metrics={metrics:?}");
    assert!(metrics[1].1 <= 0.1,
        "control-signal clamping with Tt=dt must remove retained excess at the limit; metrics={metrics:?}");
    assert!((metrics[1].0 - metrics[2].0).abs() >= 0.1
            || (metrics[1].3 - metrics[2].3).abs() >= 0.1,
        "clamping and ordinary back calculation must execute as distinct strategies; metrics={metrics:?}");
    assert!((metrics[2].0 - metrics[3].0).abs() >= 0.1
            || (metrics[2].3 - metrics[3].3).abs() >= 0.1,
        "back calculation and the combined strategy must execute as distinct strategies; metrics={metrics:?}");
    assert!(
        traces
            .iter()
            .flatten()
            .all(|(_, pv, mv, requested)| pv.is_finite()
                && mv.is_finite()
                && requested.is_finite()
                && (-3.5..=3.5).contains(mv)),
        "all four strategies must remain finite and respect the physical steam-valve range"
    );
}

#[test]
fn control_curriculum_reactor_separates_servo_and_regulatory_objectives_and_metrics() {
    let servo = run_reactor_objective_experiment(85.0, 55);
    let regulatory = run_reactor_objective_experiment(80.0, 120);
    let servo_response = &servo[50..];
    let servo_event_window = &servo[49..];
    let servo_overshoot = servo_response
        .iter()
        .map(|(_, pv, _)| *pv - 85.0)
        .fold(0.0_f64, f64::max);
    let servo_settling = servo_response
        .iter()
        .rfind(|(_, pv, _)| (85.0 - *pv).abs() > 0.5 + 0.01)
        .map(|(time, _, _)| *time - 5.0 + 0.1)
        .unwrap_or(0.0);
    let servo_iae = servo_response
        .iter()
        .map(|(_, pv, _)| (85.0 - *pv).abs() * 0.1)
        .sum::<f64>();
    let servo_ise = servo_response
        .iter()
        .map(|(_, pv, _)| (85.0 - *pv).powi(2) * 0.1)
        .sum::<f64>();
    let servo_itae = servo_response
        .iter()
        .map(|(time, pv, _)| (*time - 5.0) * (85.0 - *pv).abs() * 0.1)
        .sum::<f64>();
    let servo_travel = servo_event_window
        .windows(2)
        .map(|samples| (samples[1].2 - samples[0].2).abs())
        .sum::<f64>();

    let after_load = &regulatory[600..];
    let regulatory_event_window = &regulatory[599..];
    let regulatory_peak_deviation = after_load
        .iter()
        .map(|(_, pv, _)| (80.0 - *pv).abs())
        .fold(0.0_f64, f64::max);
    let regulatory_recovery = after_load
        .iter()
        .rfind(|(_, pv, _)| (80.0 - *pv).abs() > 0.5 + 0.01)
        .map(|(time, _, _)| *time - 60.0 + 0.1)
        .unwrap_or(0.0);
    let regulatory_iae = after_load
        .iter()
        .map(|(_, pv, _)| (80.0 - *pv).abs() * 0.1)
        .sum::<f64>();
    let regulatory_ise = after_load
        .iter()
        .map(|(_, pv, _)| (80.0 - *pv).powi(2) * 0.1)
        .sum::<f64>();
    let regulatory_itae = after_load
        .iter()
        .map(|(time, pv, _)| (*time - 60.0) * (80.0 - *pv).abs() * 0.1)
        .sum::<f64>();
    let regulatory_travel = regulatory_event_window
        .windows(2)
        .map(|samples| (samples[1].2 - samples[0].2).abs())
        .sum::<f64>();
    let regulatory_final_window_error = regulatory
        .iter()
        .filter(|(time, _, _)| *time >= 110.1 && *time <= 120.0)
        .map(|(_, pv, _)| (80.0 - *pv).abs())
        .sum::<f64>()
        / 100.0;

    eprintln!(
        "reactor objectives: servo overshoot={servo_overshoot:.6} degC, settling={servo_settling:.3}s, IAE={servo_iae:.6}, ISE={servo_ise:.6}, ITAE={servo_itae:.6}, travel={servo_travel:.6} pp; regulatory peak={regulatory_peak_deviation:.6} degC, recovery={regulatory_recovery:.3}s, final-window mean absolute error={regulatory_final_window_error:.6} degC, IAE={regulatory_iae:.6}, ISE={regulatory_ise:.6}, ITAE={regulatory_itae:.6}, travel={regulatory_travel:.6} pp"
    );
    assert!(servo_overshoot >= 1.0,
        "the b=1 tuning must reproduce the excessive servo overshoot that motivates Lesson 12; overshoot={servo_overshoot}");
    assert!(
        regulatory_peak_deviation >= 1.0,
        "the fixed-SP cold-feed event must remain visible; peak={regulatory_peak_deviation}"
    );
    assert!(regulatory_recovery <= 40.0 && regulatory_final_window_error <= 0.5,
        "the same tuning must satisfy the declared regulatory objective; recovery={regulatory_recovery}, final-window error={regulatory_final_window_error}");
    assert!(
        [
            servo_iae,
            servo_ise,
            servo_itae,
            servo_travel,
            regulatory_iae,
            regulatory_ise,
            regulatory_itae,
            regulatory_travel
        ]
        .iter()
        .all(|metric| metric.is_finite() && *metric > 0.0),
        "IAE, ISE, ITAE and actuator travel must be finite positive measures for both objectives"
    );
    assert!(
        servo
            .iter()
            .chain(regulatory.iter())
            .all(|(_, pv, mv)| pv.is_finite()
                && mv.is_finite()
                && (60.0..=100.0).contains(pv)
                && (0.0..=100.0).contains(mv)),
        "both objective tests must remain finite and inside declared reactor and valve limits"
    );
}

#[test]
fn control_curriculum_reactor_setpoint_weight_reduces_servo_overshoot_without_changing_regulatory_path(
) {
    let weights = [1.0, 0.5, 0.0];
    let servo: Vec<Vec<(f64, f64, f64)>> = weights
        .iter()
        .map(|weight| run_reactor_setpoint_weight(*weight, 85.0, 55))
        .collect();
    let regulatory: Vec<Vec<(f64, f64, f64)>> = weights
        .iter()
        .map(|weight| run_reactor_setpoint_weight(*weight, 80.0, 120))
        .collect();
    let overshoot = |trace: &[(f64, f64, f64)]| {
        trace
            .iter()
            .map(|(_, pv, _)| *pv - 85.0)
            .fold(0.0_f64, f64::max)
    };
    let overshoots: Vec<f64> = servo.iter().map(|trace| overshoot(trace)).collect();
    let maximum_regulatory_delta = regulatory[0]
        .iter()
        .zip(&regulatory[1])
        .chain(regulatory[0].iter().zip(&regulatory[2]))
        .map(|(left, right)| (left.1 - right.1).abs().max((left.2 - right.2).abs()))
        .fold(0.0_f64, f64::max);
    let regulatory_final_window_errors: Vec<f64> = regulatory
        .iter()
        .map(|trace| {
            let window = trace
                .iter()
                .filter(|(time_s, _, _)| (110.1..=120.0).contains(time_s))
                .collect::<Vec<_>>();
            window
                .iter()
                .map(|(_, pv, _)| (80.0 - pv).abs())
                .sum::<f64>()
                / window.len() as f64
        })
        .collect();

    eprintln!(
        "reactor setpoint weights: overshoot b=1/0.5/0={overshoots:?} degC, maximum regulatory trace delta={maximum_regulatory_delta:.12}, final-window mean absolute errors={regulatory_final_window_errors:?} degC"
    );
    assert!(
        overshoots[0] >= 1.0,
        "b=1 must reproduce the Lesson 11 servo failure; overshoots={overshoots:?}"
    );
    assert!(
        overshoots[1] + 0.15 <= overshoots[0],
        "intermediate b must materially reduce servo overshoot; overshoots={overshoots:?}"
    );
    assert!(overshoots[2] <= 0.9,
        "b=0 must bring servo overshoot below the declared 0.9 degC acceptance limit; overshoots={overshoots:?}");
    assert!(maximum_regulatory_delta <= 1.0e-10,
        "setpoint weighting must not weaken the unchanged fixed-SP disturbance path; delta={maximum_regulatory_delta}");
    assert!(regulatory_final_window_errors.iter().all(|error| *error <= 0.5 + 0.001),
        "qualified final-window performance must retain acceptable regulatory recovery; errors={regulatory_final_window_errors:?}");
    assert!(
        servo
            .iter()
            .chain(regulatory.iter())
            .flatten()
            .all(|(_, pv, mv)| pv.is_finite()
                && mv.is_finite()
                && (60.0..=100.0).contains(pv)
                && (0.0..=100.0).contains(mv)),
        "all weighting comparisons must remain finite and inside declared reactor and valve limits"
    );
}

#[test]
fn control_curriculum_pump_pressure_compares_unfiltered_first_and_second_order_filters() {
    let traces: Vec<Vec<(f64, f64, f64, f64)>> =
        (0..=2).map(run_pump_pressure_filter_strategy).collect();
    let increment_rms = |values: &[f64]| {
        let squared_sum = values
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).powi(2))
            .sum::<f64>();
        (squared_sum / (values.len() - 1) as f64).sqrt()
    };
    let noise_window = 200..400;
    let controller_pv_rms: Vec<f64> = traces
        .iter()
        .map(|trace| {
            increment_rms(
                &trace[noise_window.clone()]
                    .iter()
                    .map(|sample| sample.2)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let mv_rms: Vec<f64> = traces
        .iter()
        .map(|trace| {
            increment_rms(
                &trace[noise_window.clone()]
                    .iter()
                    .map(|sample| sample.3)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let lag_areas: Vec<f64> = traces
        .iter()
        .map(|trace| {
            trace[50..150]
                .iter()
                .map(|(_, raw_pv, controller_pv, _)| (*raw_pv - *controller_pv) * 0.1)
                .sum()
        })
        .collect();
    let final_errors: Vec<f64> = traces
        .iter()
        .map(|trace| {
            let mean = trace[350..].iter().map(|sample| sample.2).sum::<f64>() / 50.0;
            (1.0 - mean).abs()
        })
        .collect();

    eprintln!(
        "pump-pressure filters [none, first, second]: controller-PV increment RMS={controller_pv_rms:?}, MV increment RMS={mv_rms:?}, aligned measurement-path lag area={lag_areas:?}bar*s, final-window absolute mean error={final_errors:?}bar"
    );
    assert!(controller_pv_rms[1] <= controller_pv_rms[0] * 0.8
            && controller_pv_rms[2] <= controller_pv_rms[1] * 0.9,
        "first and second order filters must progressively attenuate seeded measurement noise; RMS={controller_pv_rms:?}");
    assert!(
        mv_rms[1] <= mv_rms[0] * 0.8 && mv_rms[2] <= mv_rms[1] * 0.9,
        "progressive filtering must reduce noise-driven valve movement; RMS={mv_rms:?}"
    );
    assert!(lag_areas[0].abs() <= 1.0e-12
            && lag_areas[1] <= 0.08 + 0.005
            && lag_areas[2] > 0.08 + 0.005,
        "aligned lag must qualify first order and reject second order against the declared limit; area={lag_areas:?}");
    assert!(
        final_errors.iter().all(|error| *error <= 0.05),
        "all filter choices must retain acceptable mean pressure tracking; errors={final_errors:?}"
    );
    assert!(
        traces
            .iter()
            .flatten()
            .all(|(_, raw, controller_pv, mv)| raw.is_finite()
                && controller_pv.is_finite()
                && mv.is_finite()
                && (-10.0..=10.0).contains(mv)),
        "all seeded comparisons must remain finite and inside recycle-valve limits"
    );
}

#[test]
fn control_curriculum_oven_rejects_nonpositive_and_missed_scan_intervals_safely() {
    for cycle_time in [0.05, 0.1, 0.2] {
        let (valid, mv) = run_oven_timing_sample(cycle_time, true);
        assert!(valid && mv.is_finite(),
            "an interval within 0 to 250 ms must execute normally; dt={cycle_time}, valid={valid}, MV={mv}");
    }
    for cycle_time in [0.3, 0.0] {
        let (valid, mv) = run_oven_timing_sample(cycle_time, true);
        assert!(!valid,
            "a nonpositive or missed-scan interval must raise the visible timing fault; dt={cycle_time}, valid={valid}");
        assert!(mv.abs() <= 1.0e-12,
            "a timing fault must hold the heater at its declared zero-trim safe value; dt={cycle_time}, MV={mv}");
    }
}

#[test]
fn control_curriculum_oven_does_not_silently_resume_after_a_timing_fault() {
    let samples =
        run_oven_timing_sequence(&[(0.1, true, true), (0.3, true, true), (0.1, true, true)]);
    assert!(
        samples[0].0 && samples[0].1 > 0.0,
        "the initial valid automatic update must establish active heater trim"
    );
    assert!(
        !samples[1].0 && samples[1].1.abs() <= 1.0e-12,
        "the missed scan must command the declared zero-trim safe state"
    );
    assert!(
        samples[2].0,
        "the restored interval may be numerically valid while the timing fault remains latched"
    );
    assert!(
        samples[2].1.abs() <= 1.0e-12,
        "restoring a valid interval must not silently resume automatic heater trim"
    );
}

#[test]
fn control_curriculum_oven_contrasts_nominal_and_measured_elapsed_time() {
    let (actual_valid, actual_mv) = run_oven_timing_sample(0.2, true);
    let (nominal_valid, nominal_mv) = run_oven_timing_sample(0.2, false);
    assert!(
        actual_valid && nominal_valid,
        "200 ms is inside the declared timing envelope"
    );
    assert!(
        (actual_mv - 0.1334).abs() <= 1.0e-9,
        "measured-dt mode must integrate Ki*e*0.2; MV={actual_mv}"
    );
    assert!((nominal_mv - 0.0667).abs() <= 1.0e-9,
        "the executable bad baseline must expose the wrong nominal-100 ms increment; MV={nominal_mv}");
    assert!(
        actual_mv >= nominal_mv * 1.99,
        "using nominal dt during a 200 ms scan must visibly halve the intended integral update"
    );
}
