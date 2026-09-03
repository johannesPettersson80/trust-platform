use std::path::{Path, PathBuf};

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};

#[path = "control_curriculum_motion_progression/evidence.rs"]
mod evidence;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");

fn project_file(project: &str, relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(match project {
            "servo-position-p" => "02-derivative-action-and-dynamic-response/19-servo-position-p",
            "servo-position-pd" => "02-derivative-action-and-dynamic-response/20-servo-position-pd",
            "../practitioner/underdamped-pid" => {
                "02-derivative-action-and-dynamic-response/21-underdamped-pid"
            }
            "servo-derivative-kick" => {
                "02-derivative-action-and-dynamic-response/22-servo-derivative-kick"
            }
            "servo-filtered-derivative" => {
                "02-derivative-action-and-dynamic-response/23-servo-filtered-derivative"
            }
            "servo-mechanical-nonidealities" => {
                "02-derivative-action-and-dynamic-response/24-servo-mechanical-nonidealities"
            }
            "servo-digital-realization" => {
                "02-derivative-action-and-dynamic-response/25-servo-digital-realization"
            }
            "pid-forms" => "03-production-quality-digital-pid/26-pid-forms",
            _ => panic!("unknown motion-control lesson: {project}"),
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
        "trust-control-motion-{project}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_or_compare_curriculum_artifact(path: &Path, expected: &str) {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("motion artifact parent"))
            .expect("create motion artifact directory");
        std::fs::write(path, expected).expect("write motion artifact");
        return;
    }
    let actual = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read retained artifact {}: {error}", path.display()));
    assert_eq!(
        actual,
        expected,
        "retained artifact drift: {}",
        path.display()
    );
}

fn write_lreal(runtime: &mut trust_runtime::Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL process-image value");
}

fn write_bool(runtime: &mut trust_runtime::Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL process-image value");
}

fn write_int(runtime: &mut trust_runtime::Runtime, address: &IoAddress, value: i16) {
    runtime
        .io_mut()
        .write(address, Value::Word(value as u16))
        .expect("write INT process-image value");
}

fn read_lreal(runtime: &trust_runtime::Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn read_bool(runtime: &trust_runtime::Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL value") {
        Value::Bool(value) => value,
        other => panic!("expected BOOL storage, got {other:?}"),
    }
}

fn run_position_p(proportional_gain: f64) -> Vec<(f64, f64, f64)> {
    let source = project_file("servo-position-p", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "ProportionalGain : LREAL :=",
            "ProportionalGain AT %IL32 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL40 : LREAL :=")
        .replace("AutoMode : BOOL :=", "AutoMode AT %IX48.0 : BOOL :=")
        .replace("Position : LREAL :=", "Position AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace("MotorTorque : LREAL :=", "MotorTorque AT %QL0 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("servo-position-p/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped P position-control lesson");
    let config_path = temporary_config_path("servo-position-p");
    std::fs::write(
        &config_path,
        project_file("servo-position-p", "simulation.toml"),
    )
    .expect("write positioning-table simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load positioning-table simulation"),
    );
    let kp = IoAddress::parse("%IL32").expect("Kp address");
    let sp = IoAddress::parse("%IL40").expect("SP address");
    let auto = IoAddress::parse("%IX48.0").expect("mode address");
    let position = IoAddress::parse("%IL0").expect("position address");
    let velocity = IoAddress::parse("%IL8").expect("velocity address");
    let torque = IoAddress::parse("%QL0").expect("torque address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial table state");
    let mut trace = Vec::with_capacity(750);
    for cycle in 1..=750 {
        write_lreal(&mut runtime, &kp, proportional_gain);
        write_lreal(&mut runtime, &sp, 1.0);
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute P position controller");
        let now = Duration::from_millis((cycle * 20) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance positioning table");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish table feedback");
        trace.push((
            read_lreal(&runtime, &position),
            read_lreal(&runtime, &velocity),
            read_lreal(&runtime, &torque),
        ));
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

fn run_position_pd(derivative_gain: f64) -> Vec<(f64, f64, f64)> {
    let source = project_file("servo-position-pd", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "DerivativeGain : LREAL :=",
            "DerivativeGain AT %IL32 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL40 : LREAL :=")
        .replace("AutoMode : BOOL :=", "AutoMode AT %IX48.0 : BOOL :=")
        .replace("Position : LREAL :=", "Position AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace("MotorTorque : LREAL :=", "MotorTorque AT %QL0 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("servo-position-pd/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped PD position-control lesson");
    let config_path = temporary_config_path("servo-position-pd");
    std::fs::write(
        &config_path,
        project_file("servo-position-pd", "simulation.toml"),
    )
    .expect("write PD positioning-table simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load PD positioning-table simulation"),
    );
    let kd = IoAddress::parse("%IL32").expect("Kd address");
    let sp = IoAddress::parse("%IL40").expect("SP address");
    let auto = IoAddress::parse("%IX48.0").expect("mode address");
    let position = IoAddress::parse("%IL0").expect("position address");
    let velocity = IoAddress::parse("%IL8").expect("velocity address");
    let torque = IoAddress::parse("%QL0").expect("torque address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial PD table state");
    let mut trace = Vec::with_capacity(750);
    for cycle in 1..=750 {
        write_lreal(&mut runtime, &kd, derivative_gain);
        write_lreal(&mut runtime, &sp, 1.0);
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute PD position controller");
        let now = Duration::from_millis((cycle * 20) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance PD positioning table");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish PD table feedback");
        trace.push((
            read_lreal(&runtime, &position),
            read_lreal(&runtime, &velocity),
            read_lreal(&runtime, &torque),
        ));
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

fn run_loaded_position_pid(integral_gain: f64, saturation_recovery: bool) -> Vec<(f64, f64, f64)> {
    let original = project_file("../practitioner/underdamped-pid", "src/Main.st");
    let has_velocity = original.contains("Velocity : LREAL :=");
    let source = original
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "IntegralGain : LREAL :=",
            "IntegralGain AT %IL32 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL40 : LREAL :=")
        .replace("AutoMode : BOOL :=", "AutoMode AT %IX48.0 : BOOL :=")
        .replace("Position : LREAL :=", "Position AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace("MotorTorque : LREAL :=", "MotorTorque AT %QL0 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL0 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("underdamped-pid/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped loaded-position PID lesson");
    let config_path = temporary_config_path("loaded-position-pid");
    std::fs::write(
        &config_path,
        project_file("../practitioner/underdamped-pid", "simulation.toml"),
    )
    .expect("write loaded positioning-table simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load loaded positioning-table simulation"),
    );
    let ki = IoAddress::parse("%IL32").expect("Ki address");
    let sp = IoAddress::parse("%IL40").expect("SP address");
    let auto = IoAddress::parse("%IX48.0").expect("mode address");
    let position = IoAddress::parse("%IL0").expect("position address");
    let velocity = IoAddress::parse("%IL8").expect("velocity address");
    let torque = IoAddress::parse("%QL0").expect("torque address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial loaded table state");
    let mut trace = Vec::with_capacity(1_500);
    for cycle in 1..=1_500 {
        write_lreal(&mut runtime, &ki, integral_gain);
        write_lreal(
            &mut runtime,
            &sp,
            if saturation_recovery {
                if cycle == 1 {
                    0.0
                } else if cycle <= 250 {
                    2.0
                } else {
                    0.5
                }
            } else {
                0.5
            },
        );
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute loaded-position controller");
        let now = Duration::from_millis((cycle * 20) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance loaded positioning table");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish loaded table feedback");
        trace.push((
            read_lreal(&runtime, &position),
            if has_velocity {
                read_lreal(&runtime, &velocity)
            } else {
                0.0
            },
            read_lreal(&runtime, &torque),
        ));
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

fn run_derivative_weight(weight: f64) -> Vec<(f64, f64, f64)> {
    let source = project_file("servo-derivative-kick", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "DerivativeSetpointWeight : LREAL :=",
            "DerivativeSetpointWeight AT %IL32 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL40 : LREAL :=")
        .replace("AutoMode : BOOL :=", "AutoMode AT %IX48.0 : BOOL :=")
        .replace("Position : LREAL :=", "Position AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace(
            "RequestedTorque : LREAL :=",
            "RequestedTorque AT %ML0 : LREAL :=",
        )
        .replace(
            "AppliedTorque : LREAL :=",
            "AppliedTorque AT %QL0 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("servo-derivative-kick/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped derivative-kick lesson");
    let config_path = temporary_config_path("servo-derivative-kick");
    std::fs::write(
        &config_path,
        project_file("servo-derivative-kick", "simulation.toml"),
    )
    .expect("write derivative-kick simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load derivative-kick simulation"),
    );
    let c = IoAddress::parse("%IL32").expect("derivative weight address");
    let sp = IoAddress::parse("%IL40").expect("SP address");
    let auto = IoAddress::parse("%IX48.0").expect("mode address");
    let position = IoAddress::parse("%IL0").expect("position address");
    let requested = IoAddress::parse("%ML0").expect("requested torque address");
    let applied = IoAddress::parse("%QL0").expect("applied torque address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial derivative-kick state");
    let mut trace = Vec::with_capacity(750);
    for cycle in 1..=750 {
        write_lreal(&mut runtime, &c, weight);
        write_lreal(&mut runtime, &sp, if cycle == 1 { 0.0 } else { 0.5 });
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute derivative-kick controller");
        let now = Duration::from_millis((cycle * 20) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance derivative-kick table");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish derivative-kick feedback");
        trace.push((
            read_lreal(&runtime, &position),
            read_lreal(&runtime, &requested),
            read_lreal(&runtime, &applied),
        ));
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

fn run_filtered_derivative(selection: i16, time_constant_s: f64) -> Vec<(f64, f64, f64, f64)> {
    let source = project_file("servo-filtered-derivative", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "FilterSelection : INT :=",
            "FilterSelection AT %IW32 : INT :=",
        )
        .replace(
            "FilterTimeConstant : LREAL :=",
            "FilterTimeConstant AT %IL48 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL40 : LREAL :=")
        .replace("AutoMode : BOOL :=", "AutoMode AT %IX48.0 : BOOL :=")
        .replace("RawPosition : LREAL :=", "RawPosition AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace(
            "ControllerPosition : LREAL :=",
            "ControllerPosition AT %ML0 : LREAL :=",
        )
        .replace("MotorTorque : LREAL :=", "MotorTorque AT %QL0 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("servo-filtered-derivative/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped filtered-derivative lesson");
    let config_path = temporary_config_path("servo-filtered-derivative");
    std::fs::write(
        &config_path,
        project_file("servo-filtered-derivative", "simulation.toml"),
    )
    .expect("write filtered-derivative simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load filtered-derivative simulation"),
    );
    let filter = IoAddress::parse("%IW32").expect("filter selection address");
    let time_constant = IoAddress::parse("%IL48").expect("filter time-constant address");
    let sp = IoAddress::parse("%IL40").expect("SP address");
    let auto = IoAddress::parse("%IX48.0").expect("mode address");
    let raw = IoAddress::parse("%IL0").expect("raw position address");
    let controller_position = IoAddress::parse("%ML0").expect("controller position address");
    let torque = IoAddress::parse("%QL0").expect("torque address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial encoder state");
    let mut trace = Vec::with_capacity(1_000);
    for cycle in 1..=1_000 {
        write_int(&mut runtime, &filter, selection);
        write_lreal(&mut runtime, &time_constant, time_constant_s);
        write_lreal(&mut runtime, &sp, 0.5);
        write_bool(&mut runtime, &auto, true);
        runtime
            .execute_cycle()
            .expect("execute filtered-derivative controller");
        let now = Duration::from_millis((cycle * 20) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance encoder-noise table");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish noisy encoder feedback");
        trace.push((
            read_lreal(&runtime, &raw),
            read_lreal(&runtime, &controller_position),
            read_lreal(&runtime, &torque),
            cycle as f64 * 0.02,
        ));
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

#[derive(Debug)]
struct MechanicalSample {
    load_position: f64,
    profiled_position: f64,
    profiled_velocity: f64,
    requested_torque: f64,
    applied_torque: f64,
    saturation: bool,
    slew_limit: bool,
    deadband: bool,
    stiction: bool,
    backlash_active: bool,
}

#[derive(Debug)]
struct DigitalSample {
    time_s: f64,
    position: f64,
    ideal_position: f64,
    torque: f64,
    update: bool,
}

fn run_digital_realization(
    method: i16,
    interval_selection: i16,
    delay_enabled: bool,
) -> Vec<DigitalSample> {
    let source = project_file("servo-digital-realization", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "MethodSelection : INT :=",
            "MethodSelection AT %IW32 : INT :=",
        )
        .replace(
            "IntervalSelection : INT :=",
            "IntervalSelection AT %IW34 : INT :=",
        )
        .replace(
            "DelayEnabled : BOOL :=",
            "DelayEnabled AT %IX36.0 : BOOL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL40 : LREAL :=")
        .replace(
            "MeasuredPosition : LREAL :=",
            "MeasuredPosition AT %IL0 : LREAL :=",
        )
        .replace(
            "MotorVelocity : LREAL :=",
            "MotorVelocity AT %IL8 : LREAL :=",
        )
        .replace(
            "RequestedTorque : LREAL :=",
            "RequestedTorque AT %QL0 : LREAL :=",
        )
        .replace("UpdatePulse : BOOL :=", "UpdatePulse AT %MX48.0 : BOOL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "servo-digital-realization/src/DiscretePidController.st",
            project_file("servo-digital-realization", "src/DiscretePidController.st"),
        ),
        SourceFile::with_path("servo-digital-realization/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped digital-realization lesson");
    let config_path = temporary_config_path("servo-digital-realization");
    std::fs::write(
        &config_path,
        project_file("servo-digital-realization", "simulation.toml"),
    )
    .expect("write digital-realization simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load digital-realization simulation"),
    );
    let method_address = IoAddress::parse("%IW32").unwrap();
    let interval = IoAddress::parse("%IW34").unwrap();
    let delay = IoAddress::parse("%IX36.0").unwrap();
    let setpoint = IoAddress::parse("%IL40").unwrap();
    let position = IoAddress::parse("%IL0").unwrap();
    let torque = IoAddress::parse("%QL0").unwrap();
    let update = IoAddress::parse("%MX48.0").unwrap();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let mut trace = Vec::with_capacity(2_000);
    for cycle in 1..=2_000 {
        write_int(&mut runtime, &method_address, method);
        write_int(&mut runtime, &interval, interval_selection);
        write_bool(&mut runtime, &delay, delay_enabled);
        write_lreal(&mut runtime, &setpoint, 0.5);
        runtime
            .execute_cycle()
            .expect("execute digital controller base scan");
        let now = Duration::from_millis((cycle * 10) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance digital positioning table");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish sampled position");
        let status = simulation.status();
        trace.push(DigitalSample {
            time_s: cycle as f64 * 0.01,
            position: read_lreal(&runtime, &position),
            ideal_position: status.plants[0].ideal_output[0],
            torque: read_lreal(&runtime, &torque),
            update: read_bool(&runtime, &update),
        });
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

#[derive(Clone, Copy, Debug)]
enum MechanicalCase {
    Ideal,
    Profile,
    Torque,
    Friction,
    Deadband,
    Backlash,
    Stiction,
    Combined,
}

fn run_mechanical_nonidealities(case: MechanicalCase) -> Vec<MechanicalSample> {
    let source = project_file("servo-mechanical-nonidealities", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace("AutoMode : BOOL :=", "AutoMode AT %IX48.0 : BOOL :=")
        .replace(
            "ProfileEnabled : BOOL :=",
            "ProfileEnabled AT %IX48.1 : BOOL :=",
        )
        .replace(
            "TargetPosition : LREAL :=",
            "TargetPosition AT %IL40 : LREAL :=",
        )
        .replace("LoadPosition : LREAL :=", "LoadPosition AT %IL0 : LREAL :=")
        .replace(
            "MotorVelocity : LREAL :=",
            "MotorVelocity AT %IL8 : LREAL :=",
        )
        .replace(
            "ProfiledPosition : LREAL :=",
            "ProfiledPosition AT %ML48 : LREAL :=",
        )
        .replace(
            "ProfiledVelocity : LREAL :=",
            "ProfiledVelocity AT %ML56 : LREAL :=",
        )
        .replace(
            "RequestedTorque : LREAL :=",
            "RequestedTorque AT %QL0 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("servo-mechanical-nonidealities/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped mechanical-nonidealities lesson");

    let mut simulation_text = project_file("servo-mechanical-nonidealities", "simulation.toml");
    if !matches!(case, MechanicalCase::Torque | MechanicalCase::Combined) {
        simulation_text = simulation_text
            .replace("minimum = -2.5", "minimum = -10.0")
            .replace("maximum = 2.5", "maximum = 10.0")
            .replace("rate_up_per_s = 20.0", "rate_up_per_s = 500.0")
            .replace("rate_down_per_s = 20.0", "rate_down_per_s = 500.0");
    }
    if matches!(case, MechanicalCase::Torque) {
        simulation_text = simulation_text
            .replace("minimum = -2.5", "minimum = -1.0")
            .replace("maximum = 2.5", "maximum = 1.0");
    }
    if !matches!(case, MechanicalCase::Friction | MechanicalCase::Combined) {
        simulation_text =
            simulation_text.replace("coulomb_friction_nm = 0.8", "coulomb_friction_nm = 0.0");
    }
    if !matches!(case, MechanicalCase::Deadband | MechanicalCase::Combined) {
        simulation_text = simulation_text.replace("deadband = 0.2", "deadband = 0.0");
    }
    if !matches!(case, MechanicalCase::Stiction | MechanicalCase::Combined) {
        simulation_text = simulation_text
            .replace("stiction_band = 0.5", "stiction_band = 0.0")
            .replace("slip_jump = 0.1", "slip_jump = 0.0");
    }
    if !matches!(case, MechanicalCase::Backlash | MechanicalCase::Combined) {
        simulation_text = simulation_text.replace("width = 0.05", "width = 0.000001");
    }
    let config_path = temporary_config_path("servo-mechanical-nonidealities");
    std::fs::write(&config_path, simulation_text)
        .expect("write mechanical-nonidealities simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load mechanical-nonidealities simulation"),
    );
    let auto = IoAddress::parse("%IX48.0").unwrap();
    let profile_enabled = IoAddress::parse("%IX48.1").unwrap();
    let target = IoAddress::parse("%IL40").unwrap();
    let load_position = IoAddress::parse("%IL0").unwrap();
    let profiled_position = IoAddress::parse("%ML48").unwrap();
    let profiled_velocity = IoAddress::parse("%ML56").unwrap();
    let requested_torque = IoAddress::parse("%QL0").unwrap();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial mechanical state");
    let mut trace = Vec::with_capacity(1_000);
    for cycle in 1..=1_000 {
        write_bool(&mut runtime, &auto, true);
        write_bool(
            &mut runtime,
            &profile_enabled,
            !matches!(case, MechanicalCase::Ideal),
        );
        write_lreal(&mut runtime, &target, if cycle <= 500 { 0.5 } else { -0.5 });
        runtime
            .execute_cycle()
            .expect("execute mechanical-nonideality controller");
        let now = Duration::from_millis((cycle * 20) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance nonideal positioning table");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish nonideal table feedback");
        let status = simulation.status();
        let plant = &status.plants[0];
        let actuator = &plant.actuators[0];
        trace.push(MechanicalSample {
            load_position: read_lreal(&runtime, &load_position),
            profiled_position: read_lreal(&runtime, &profiled_position),
            profiled_velocity: read_lreal(&runtime, &profiled_velocity),
            requested_torque: read_lreal(&runtime, &requested_torque),
            applied_torque: plant.applied_input[0],
            saturation: actuator.saturation_active,
            slew_limit: actuator.slew_limit_active,
            deadband: actuator.deadband_active,
            stiction: actuator.stiction_active,
            backlash_active: plant
                .active_uncertainties
                .iter()
                .any(|id| id == "gear-clearance"),
        });
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

#[test]
fn control_curriculum_position_p_boolean_authority_owns_high_gain_run_and_safe_restoration() {
    let source = project_file("servo-position-p", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX82.5 : BOOL :=",
        )
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX100.0 : BOOL :=")
        .replace(
            "ProportionalGain : LREAL :=",
            "ProportionalGain AT %ML120 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %ML128 : LREAL :=")
        .replace("Position : LREAL :=", "Position AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace("MotorTorque : LREAL :=", "MotorTorque AT %QL0 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("servo-position-p/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped project-owned P position sequence");
    let authority = IoAddress::parse("%IX82.5").expect("experiment authority address");
    let auto = IoAddress::parse("%MX100.0").expect("automatic-mode address");
    let kp = IoAddress::parse("%ML120").expect("proportional-gain address");
    let sp = IoAddress::parse("%ML128").expect("setpoint address");
    let position = IoAddress::parse("%IL0").expect("position address");
    let velocity = IoAddress::parse("%IL8").expect("velocity address");
    let torque = IoAddress::parse("%QL0").expect("torque address");

    write_lreal(&mut runtime, &position, 0.0);
    write_lreal(&mut runtime, &velocity, 0.0);
    runtime.execute_cycle().expect("initialize safe baseline");
    write_bool(&mut runtime, &authority, true);
    assert!(read_bool(&runtime, &authority));
    runtime
        .execute_cycle()
        .expect("start selected high-gain run");
    let selected_kp = read_lreal(&runtime, &kp);
    assert!(
        (selected_kp - 8.0).abs() <= 1e-12,
        "authority must select Kp=8, got {selected_kp}"
    );
    assert!((read_lreal(&runtime, &sp) - 1.0).abs() <= 1e-12);
    assert!(read_bool(&runtime, &auto));
    assert!((read_lreal(&runtime, &torque) - 8.0).abs() <= 1e-12);

    write_bool(&mut runtime, &authority, false);
    runtime.execute_cycle().expect("restore safe baseline");
    assert!((read_lreal(&runtime, &kp) - 2.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &sp) - 0.0).abs() <= 1e-12);
    assert!(!read_bool(&runtime, &auto));
    assert!((read_lreal(&runtime, &torque) - 0.0).abs() <= 1e-12);
}

#[test]
fn control_curriculum_position_p_exposes_stiffness_damping_tradeoff() {
    let low = run_position_p(2.0);
    let high = run_position_p(8.0);
    let rise_time = |trace: &[(f64, f64, f64)]| {
        trace
            .iter()
            .position(|sample| sample.0 >= 0.9)
            .expect("90 percent crossing") as f64
            * 0.02
    };
    let overshoot = |trace: &[(f64, f64, f64)]| {
        trace
            .iter()
            .map(|sample| sample.0)
            .fold(f64::NEG_INFINITY, f64::max)
            - 1.0
    };
    let peak_torque = |trace: &[(f64, f64, f64)]| {
        trace
            .iter()
            .map(|sample| sample.2.abs())
            .fold(0.0, f64::max)
    };
    let low_rise = rise_time(&low);
    let high_rise = rise_time(&high);
    let low_overshoot = overshoot(&low);
    let high_overshoot = overshoot(&high);
    let low_peak_torque = peak_torque(&low);
    let high_peak_torque = peak_torque(&high);
    println!(
        "low/high rise={low_rise:.3}/{high_rise:.3}s, overshoot={low_overshoot:.6}/{high_overshoot:.6}rad, peak torque={low_peak_torque:.6}/{high_peak_torque:.6}Nm"
    );

    let summary = format!(
        "gain_nm_per_rad,rise_time_s,overshoot_rad,peak_applied_torque_nm,decision\n2.0,{low_rise:.12},{low_overshoot:.12},{low_peak_torque:.12},slower lower-effort baseline\n8.0,{high_rise:.12},{high_overshoot:.12},{high_peak_torque:.12},faster but insufficiently damped\n"
    );
    let low_position = low.iter().map(|sample| sample.0).collect::<Vec<_>>();
    let high_position = high.iter().map(|sample| sample.0).collect::<Vec<_>>();
    let low_torque = low.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let high_torque = high.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/02-derivative-action-and-dynamic-response/19-servo-position-p/figures");
    evidence::retain(
        &artifact_root,
        "position-p",
        "Position stiffness and damping from one gain sweep",
        "Configured Simulation evidence · same table, torque limit, timing and axes",
        0.02,
        &[
            evidence::Panel {
                title: "Table position",
                unit: "rad",
                minimum: -0.7,
                maximum: 1.7,
                series: vec![
                    evidence::Series {
                        name: "Kp 2 position",
                        color: "#d55e00",
                        dashed: false,
                        values: &low_position,
                    },
                    evidence::Series {
                        name: "Kp 8 position",
                        color: "#0072b2",
                        dashed: false,
                        values: &high_position,
                    },
                ],
            },
            evidence::Panel {
                title: "Applied motor torque",
                unit: "N m",
                minimum: -10.0,
                maximum: 10.0,
                series: vec![
                    evidence::Series {
                        name: "Kp 2 torque",
                        color: "#d55e00",
                        dashed: false,
                        values: &low_torque,
                    },
                    evidence::Series {
                        name: "Kp 8 torque",
                        color: "#0072b2",
                        dashed: false,
                        values: &high_torque,
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );

    assert!(high_rise < low_rise, "high-gain rise must be faster");
    assert!(
        high_overshoot >= low_overshoot + 0.1,
        "high gain must expose poorer damping"
    );
    assert!(
        high_peak_torque > low_peak_torque,
        "higher stiffness must cost more peak torque"
    );
    assert!(high
        .iter()
        .all(|(p, v, t)| p.is_finite() && v.is_finite() && t.is_finite()));
    assert!(high.iter().all(|(_, _, torque)| torque.abs() <= 10.0));
}

#[test]
fn control_curriculum_position_pd_boolean_authority_owns_damped_run_and_safe_restoration() {
    let source = project_file("servo-position-pd", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .unwrap()
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX82.5 : BOOL :=",
        )
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX100.0 : BOOL :=")
        .replace(
            "DerivativeGain : LREAL :=",
            "DerivativeGain AT %ML120 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %ML128 : LREAL :=")
        .replace("Position : LREAL :=", "Position AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace("MotorTorque : LREAL :=", "MotorTorque AT %QL0 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("servo-position-pd/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile project-owned PD sequence");
    let authority = IoAddress::parse("%IX82.5").unwrap();
    let auto = IoAddress::parse("%MX100.0").unwrap();
    let kd = IoAddress::parse("%ML120").unwrap();
    let sp = IoAddress::parse("%ML128").unwrap();
    let position = IoAddress::parse("%IL0").unwrap();
    let velocity = IoAddress::parse("%IL8").unwrap();
    let torque = IoAddress::parse("%QL0").unwrap();
    write_lreal(&mut runtime, &position, 0.0);
    write_lreal(&mut runtime, &velocity, 0.0);
    runtime
        .execute_cycle()
        .expect("initialize safe PD baseline");
    write_bool(&mut runtime, &authority, true);
    runtime.execute_cycle().expect("start selected damped run");
    assert!((read_lreal(&runtime, &kd) - 3.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &sp) - 1.0).abs() <= 1e-12);
    assert!(read_bool(&runtime, &auto));
    assert!((read_lreal(&runtime, &torque) - 8.0).abs() <= 1e-12);
    write_bool(&mut runtime, &authority, false);
    runtime
        .execute_cycle()
        .expect("restore undamped Manual baseline");
    assert!((read_lreal(&runtime, &kd) - 0.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &sp) - 0.0).abs() <= 1e-12);
    assert!(!read_bool(&runtime, &auto));
    assert!((read_lreal(&runtime, &torque) - 0.0).abs() <= 1e-12);
}

#[test]
fn control_curriculum_position_pd_adds_damping_without_changing_stiffness() {
    let proportional = run_position_pd(0.0);
    let proportional_derivative = run_position_pd(3.0);
    let overshoot = |trace: &[(f64, f64, f64)]| {
        trace
            .iter()
            .map(|sample| sample.0)
            .fold(f64::NEG_INFINITY, f64::max)
            - 1.0
    };
    let late_position_span = |trace: &[(f64, f64, f64)]| {
        let minimum = trace[250..]
            .iter()
            .map(|sample| sample.0)
            .fold(f64::INFINITY, f64::min);
        let maximum = trace[250..]
            .iter()
            .map(|sample| sample.0)
            .fold(f64::NEG_INFINITY, f64::max);
        maximum - minimum
    };
    let p_overshoot = overshoot(&proportional);
    let pd_overshoot = overshoot(&proportional_derivative);
    let p_late_span = late_position_span(&proportional);
    let pd_late_span = late_position_span(&proportional_derivative);
    println!(
        "P/PD overshoot={p_overshoot:.6}/{pd_overshoot:.6}rad, late position span={p_late_span:.6}/{pd_late_span:.6}rad"
    );

    let summary = format!(
        "controller,derivative_gain_nm_s_per_rad,overshoot_rad,late_position_span_rad,decision\nP,0.0,{p_overshoot:.12},{p_late_span:.12},insufficient damping\nPD,3.0,{pd_overshoot:.12},{pd_late_span:.12},damped response\n"
    );
    let p_position = proportional
        .iter()
        .map(|sample| sample.0)
        .collect::<Vec<_>>();
    let pd_position = proportional_derivative
        .iter()
        .map(|sample| sample.0)
        .collect::<Vec<_>>();
    let p_torque = proportional
        .iter()
        .map(|sample| sample.2)
        .collect::<Vec<_>>();
    let pd_torque = proportional_derivative
        .iter()
        .map(|sample| sample.2)
        .collect::<Vec<_>>();
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/02-derivative-action-and-dynamic-response/20-servo-position-pd/figures");
    evidence::retain(
        &artifact_root,
        "position-pd",
        "Derivative action adds damping to the same positioning table",
        "Configured Simulation evidence · Kp remains 8 N m/rad; only Kd changes",
        0.02,
        &[
            evidence::Panel {
                title: "Table position",
                unit: "rad",
                minimum: -0.7,
                maximum: 1.7,
                series: vec![
                    evidence::Series {
                        name: "P position",
                        color: "#d55e00",
                        dashed: false,
                        values: &p_position,
                    },
                    evidence::Series {
                        name: "PD position",
                        color: "#0072b2",
                        dashed: false,
                        values: &pd_position,
                    },
                ],
            },
            evidence::Panel {
                title: "Applied motor torque",
                unit: "N m",
                minimum: -10.0,
                maximum: 10.0,
                series: vec![
                    evidence::Series {
                        name: "P torque",
                        color: "#d55e00",
                        dashed: false,
                        values: &p_torque,
                    },
                    evidence::Series {
                        name: "PD torque",
                        color: "#0072b2",
                        dashed: false,
                        values: &pd_torque,
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );

    assert!(pd_overshoot <= 0.1, "PD overshoot={pd_overshoot}");
    assert!(
        pd_overshoot <= p_overshoot * 0.25,
        "P/PD overshoot={p_overshoot}/{pd_overshoot}"
    );
    assert!(
        pd_late_span <= p_late_span * 0.2,
        "P/PD late span={p_late_span}/{pd_late_span}"
    );
    assert!(proportional_derivative
        .iter()
        .all(|(_, _, torque)| torque.abs() <= 10.0));
}

#[test]
fn control_curriculum_loaded_pid_boolean_authority_owns_selected_run_and_supported_restoration() {
    let source = project_file("../practitioner/underdamped-pid", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .unwrap()
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX82.6 : BOOL :=",
        )
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX101.0 : BOOL :=")
        .replace(
            "IntegralGain : LREAL :=",
            "IntegralGain AT %ML136 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %ML144 : LREAL :=")
        .replace("Position : LREAL :=", "Position AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace("MotorTorque : LREAL :=", "MotorTorque AT %QL0 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("underdamped-pid/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile project-owned loaded PID sequence");
    let authority = IoAddress::parse("%IX82.6").unwrap();
    let auto = IoAddress::parse("%MX101.0").unwrap();
    let ki = IoAddress::parse("%ML136").unwrap();
    let sp = IoAddress::parse("%ML144").unwrap();
    let position = IoAddress::parse("%IL0").unwrap();
    let velocity = IoAddress::parse("%IL8").unwrap();
    let torque = IoAddress::parse("%QL0").unwrap();
    write_lreal(&mut runtime, &position, 0.0);
    write_lreal(&mut runtime, &velocity, 0.0);
    runtime
        .execute_cycle()
        .expect("initialize supported loaded-table baseline");
    write_bool(&mut runtime, &authority, true);
    runtime
        .execute_cycle()
        .expect("start selected loaded PID run");
    assert!((read_lreal(&runtime, &ki) - 2.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &sp) - 0.5).abs() <= 1e-12);
    assert!(read_bool(&runtime, &auto));
    let selected_torque = read_lreal(&runtime, &torque);
    assert!(
        (selected_torque - 0.02).abs() <= 1e-12,
        "selected bumpless torque={selected_torque}"
    );
    write_bool(&mut runtime, &authority, false);
    runtime
        .execute_cycle()
        .expect("restore supported Manual baseline");
    assert!((read_lreal(&runtime, &ki) - 0.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &sp) - 0.0).abs() <= 1e-12);
    assert!(!read_bool(&runtime, &auto));
    assert!((read_lreal(&runtime, &torque) - 0.0).abs() <= 1e-12);
}

#[test]
fn control_curriculum_loaded_position_pid_removes_pd_load_error_with_bounded_torque() {
    let pd = run_loaded_position_pid(0.0, false);
    let pid = run_loaded_position_pid(2.0, false);
    let recovery = run_loaded_position_pid(2.0, true);
    let mean_final_error = |trace: &[(f64, f64, f64)]| {
        (0.5 - trace[1400..].iter().map(|sample| sample.0).sum::<f64>() / 100.0).abs()
    };
    let final_range = |trace: &[(f64, f64, f64)], field: fn(&(f64, f64, f64)) -> f64| {
        let minimum = trace[1400..]
            .iter()
            .map(field)
            .fold(f64::INFINITY, f64::min);
        let maximum = trace[1400..]
            .iter()
            .map(field)
            .fold(f64::NEG_INFINITY, f64::max);
        maximum - minimum
    };
    let pd_error = mean_final_error(&pd);
    let pid_error = mean_final_error(&pid);
    let pid_peak_excess = pid
        .iter()
        .map(|sample| sample.0)
        .fold(f64::NEG_INFINITY, f64::max)
        - 0.5;
    let pid_overshoot = pid_peak_excess.max(0.0);
    assert!(
        pid_overshoot >= 0.0,
        "conventional overshoot must not be reported as negative: {pid_overshoot}"
    );
    let pd_position_range = final_range(&pd, |sample| sample.0);
    let pid_position_range = final_range(&pid, |sample| sample.0);
    let pd_torque_range = final_range(&pd, |sample| sample.2);
    let pid_torque_range = final_range(&pid, |sample| sample.2);
    println!(
        "PD/PID final error={pd_error:.6}/{pid_error:.6}rad, final PV range={pd_position_range:.6}/{pid_position_range:.6}rad, final applied-torque range={pd_torque_range:.6}/{pid_torque_range:.6}Nm, PID signed peak excess={pid_peak_excess:.6}rad, overshoot={pid_overshoot:.6}rad"
    );
    let pd_position = pd.iter().map(|sample| sample.0).collect::<Vec<_>>();
    let pid_position = pid.iter().map(|sample| sample.0).collect::<Vec<_>>();
    let pd_torque = pd.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let pid_torque = pid.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let recovery_position = recovery.iter().map(|sample| sample.0).collect::<Vec<_>>();
    let recovery_torque = recovery.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let recovery_setpoint = (1..=recovery.len())
        .map(|cycle| {
            if cycle == 1 {
                0.0
            } else if cycle <= 250 {
                2.0
            } else {
                0.5
            }
        })
        .collect::<Vec<_>>();
    let saturation_samples = recovery
        .iter()
        .filter(|sample| sample.2.abs() >= 10.0 - 1.0e-9)
        .count();
    let recovery_peak_torque = recovery
        .iter()
        .map(|sample| sample.2.abs())
        .fold(0.0, f64::max);
    let recovery_index = (250..recovery.len() - 50)
        .find(|start| {
            recovery[*start..*start + 50]
                .iter()
                .all(|sample| (sample.0 - 0.5).abs() <= 0.02 && sample.2.abs() < 9.9)
        })
        .expect("PID must recover after the saturated demand is restored");
    let recovery_time_s = (recovery_index - 250) as f64 * 0.02;
    let summary = format!(
        "controller,final_mean_absolute_error_rad,final_position_range_rad,final_applied_torque_range_nm,overshoot_rad,decision\nPD,{pd_error:.12},{pd_position_range:.12},{pd_torque_range:.12},,gravity-load offset remains\nPID,{pid_error:.12},{pid_position_range:.12},{pid_torque_range:.12},{pid_overshoot:.12},gravity-load error removed with retained damping\n"
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/02-derivative-action-and-dynamic-response/21-underdamped-pid/figures");
    evidence::retain(
        &artifact_root,
        "loaded-position-pid",
        "Integral action removes gravity-load position error",
        "Configured Simulation evidence · same sine-gravity table, Kp, Kd, limits, timing and axes",
        0.02,
        &[
            evidence::Panel {
                title: "Table position",
                unit: "rad",
                minimum: 0.0,
                maximum: 0.6,
                series: vec![
                    evidence::Series {
                        name: "PD position",
                        color: "#d55e00",
                        dashed: false,
                        values: &pd_position,
                    },
                    evidence::Series {
                        name: "PID position",
                        color: "#0072b2",
                        dashed: false,
                        values: &pid_position,
                    },
                ],
            },
            evidence::Panel {
                title: "Applied motor torque",
                unit: "N m",
                minimum: -2.0,
                maximum: 10.0,
                series: vec![
                    evidence::Series {
                        name: "PD torque",
                        color: "#d55e00",
                        dashed: false,
                        values: &pd_torque,
                    },
                    evidence::Series {
                        name: "PID torque",
                        color: "#0072b2",
                        dashed: false,
                        values: &pid_torque,
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );
    let recovery_summary = format!(
        "case,saturation_samples,saturation_time_s,recovery_time_s,final_position_error_rad,decision\nPID saturation and recovery,{saturation_samples},{:.12},{recovery_time_s:.12},{:.12},recovers after bounded torque exposure\n",
        saturation_samples as f64 * 0.02,
        (recovery.last().expect("recovery trace").0 - 0.5).abs(),
    );
    evidence::retain(
        &artifact_root,
        "loaded-position-recovery",
        "Bounded PID recovers after torque saturation",
        "Configured Simulation evidence · SP is restored at 5 s; gains, load and torque limits remain fixed",
        0.02,
        &[
            evidence::Panel {
                title: "Setpoint and table position",
                unit: "rad",
                minimum: 0.0,
                maximum: 2.2,
                series: vec![
                    evidence::Series { name: "SP", color: "#555555", dashed: true, values: &recovery_setpoint },
                    evidence::Series { name: "PID position", color: "#0072b2", dashed: false, values: &recovery_position },
                ],
            },
            evidence::Panel {
                title: "Applied motor torque",
                unit: "N m",
                minimum: -10.0,
                maximum: 10.0,
                series: vec![
                    evidence::Series { name: "applied torque", color: "#d55e00", dashed: false, values: &recovery_torque },
                ],
            },
        ],
        &[],
        &[evidence::Marker { at_seconds: 5.0, label: "restore SP to 0.5 rad" }],
        &recovery_summary,
    );

    assert!(
        pd_position_range <= 0.01 && pid_position_range <= 0.01,
        "final windows must settle in position: PD={pd_position_range}, PID={pid_position_range}"
    );
    assert!(
        pd_torque_range <= 0.05 && pid_torque_range <= 0.05,
        "final windows must settle in applied torque: PD={pd_torque_range}, PID={pid_torque_range}"
    );
    assert!(
        pd_error >= 0.1,
        "PD must expose load-dependent error: {pd_error}"
    );
    assert!(pid_error <= 0.01, "PID final error={pid_error}");
    assert!(
        pid_error <= pd_error * 0.1,
        "PD/PID error={pd_error}/{pid_error}"
    );
    assert!(pid_overshoot <= 0.15, "PID overshoot={pid_overshoot}");
    assert!(pid.iter().all(|(_, _, torque)| torque.abs() <= 10.0));
    assert!(
        saturation_samples >= 5,
        "saturation samples={saturation_samples}, peak torque={recovery_peak_torque}"
    );
    assert!(recovery_time_s <= 15.0, "recovery time={recovery_time_s}s");
}

#[test]
fn control_curriculum_derivative_weight_boolean_authority_owns_selected_run_and_safe_restoration() {
    let source = project_file("servo-derivative-kick", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .unwrap()
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX82.7 : BOOL :=",
        )
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX102.0 : BOOL :=")
        .replace(
            "DerivativeSetpointWeight : LREAL :=",
            "DerivativeSetpointWeight AT %ML152 : LREAL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %ML160 : LREAL :=")
        .replace("Position : LREAL :=", "Position AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace(
            "RequestedTorque : LREAL :=",
            "RequestedTorque AT %ML168 : LREAL :=",
        )
        .replace(
            "AppliedTorque : LREAL :=",
            "AppliedTorque AT %QL0 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("servo-derivative-kick/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile project-owned derivative-weight sequence");
    let authority = IoAddress::parse("%IX82.7").unwrap();
    let auto = IoAddress::parse("%MX102.0").unwrap();
    let weight = IoAddress::parse("%ML152").unwrap();
    let sp = IoAddress::parse("%ML160").unwrap();
    let position = IoAddress::parse("%IL0").unwrap();
    let velocity = IoAddress::parse("%IL8").unwrap();
    let requested = IoAddress::parse("%ML168").unwrap();
    let applied = IoAddress::parse("%QL0").unwrap();
    write_lreal(&mut runtime, &position, 0.0);
    write_lreal(&mut runtime, &velocity, 0.0);
    runtime
        .execute_cycle()
        .expect("initialize derivative-kick baseline");
    write_bool(&mut runtime, &authority, true);
    runtime
        .execute_cycle()
        .expect("start selected derivative-on-measurement run");
    assert!((read_lreal(&runtime, &weight) - 0.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &sp) - 0.5).abs() <= 1e-12);
    assert!(read_bool(&runtime, &auto));
    assert!((read_lreal(&runtime, &requested) - 4.02).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &applied) - 4.02).abs() <= 1e-12);
    write_bool(&mut runtime, &authority, false);
    runtime
        .execute_cycle()
        .expect("restore rejected full-error Manual baseline");
    assert!((read_lreal(&runtime, &weight) - 1.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &sp) - 0.0).abs() <= 1e-12);
    assert!(!read_bool(&runtime, &auto));
    assert!((read_lreal(&runtime, &requested) - 0.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &applied) - 0.0).abs() <= 1e-12);
}

#[test]
fn control_curriculum_derivative_setpoint_weight_exposes_and_removes_torque_kick() {
    let full_error = run_derivative_weight(1.0);
    let intermediate = run_derivative_weight(0.5);
    let measurement = run_derivative_weight(0.0);
    let first_requested = [full_error[1].1, intermediate[1].1, measurement[1].1];
    let first_applied = [full_error[1].2, intermediate[1].2, measurement[1].2];
    println!(
        "first requested torque c=1/0.5/0: {:.6}/{:.6}/{:.6}Nm; applied: {:.6}/{:.6}/{:.6}Nm",
        first_requested[0],
        first_requested[1],
        first_requested[2],
        first_applied[0],
        first_applied[1],
        first_applied[2]
    );
    let full_position = full_error.iter().map(|sample| sample.0).collect::<Vec<_>>();
    let intermediate_position = intermediate
        .iter()
        .map(|sample| sample.0)
        .collect::<Vec<_>>();
    let measurement_position = measurement
        .iter()
        .map(|sample| sample.0)
        .collect::<Vec<_>>();
    let full_requested = full_error.iter().map(|sample| sample.1).collect::<Vec<_>>();
    let intermediate_requested = intermediate
        .iter()
        .map(|sample| sample.1)
        .collect::<Vec<_>>();
    let measurement_requested = measurement
        .iter()
        .map(|sample| sample.1)
        .collect::<Vec<_>>();
    let full_applied = full_error.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let intermediate_applied = intermediate
        .iter()
        .map(|sample| sample.2)
        .collect::<Vec<_>>();
    let measurement_applied = measurement
        .iter()
        .map(|sample| sample.2)
        .collect::<Vec<_>>();
    let summary = format!(
        "derivative_setpoint_weight,first_requested_torque_nm,first_applied_torque_nm,decision\n1.0,{:.12},{:.12},full derivative kick\n0.5,{:.12},{:.12},reduced but saturated request\n0.0,{:.12},{:.12},derivative on measurement selected\n",
        first_requested[0], first_applied[0], first_requested[1], first_applied[1],
        first_requested[2], first_applied[2]
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/02-derivative-action-and-dynamic-response/22-servo-derivative-kick/figures");
    evidence::retain(
        &artifact_root,
        "derivative-kick",
        "Derivative setpoint handling removes the torque-command kick",
        "Configured Simulation evidence · same PID, positioning table, step, limits and axes",
        0.02,
        &[
            evidence::Panel {
                title: "Table position",
                unit: "rad",
                minimum: 0.0,
                maximum: 0.6,
                series: vec![
                    evidence::Series {
                        name: "c=1 position",
                        color: "#d55e00",
                        dashed: false,
                        values: &full_position,
                    },
                    evidence::Series {
                        name: "c=.5 position",
                        color: "#009e73",
                        dashed: false,
                        values: &intermediate_position,
                    },
                    evidence::Series {
                        name: "c=0 position",
                        color: "#0072b2",
                        dashed: false,
                        values: &measurement_position,
                    },
                ],
            },
            evidence::Panel {
                title: "Requested and applied torque",
                unit: "N m",
                minimum: -10.0,
                maximum: 80.0,
                series: vec![
                    evidence::Series {
                        name: "c=1 request",
                        color: "#d55e00",
                        dashed: false,
                        values: &full_requested,
                    },
                    evidence::Series {
                        name: "c=.5 request",
                        color: "#009e73",
                        dashed: false,
                        values: &intermediate_requested,
                    },
                    evidence::Series {
                        name: "c=0 request",
                        color: "#0072b2",
                        dashed: false,
                        values: &measurement_requested,
                    },
                    evidence::Series {
                        name: "c=1 applied",
                        color: "#d55e00",
                        dashed: true,
                        values: &full_applied,
                    },
                    evidence::Series {
                        name: "c=.5 applied",
                        color: "#009e73",
                        dashed: true,
                        values: &intermediate_applied,
                    },
                    evidence::Series {
                        name: "c=0 applied",
                        color: "#0072b2",
                        dashed: true,
                        values: &measurement_applied,
                    },
                ],
            },
        ],
        &[],
        &[evidence::Marker {
            at_seconds: 0.04,
            label: "setpoint step",
        }],
        &summary,
    );

    assert!(
        first_requested[0] >= 70.0,
        "full-error derivative kick={}",
        first_requested[0]
    );
    assert!(first_requested[1] > first_requested[2] + 30.0);
    assert!(
        first_requested[2] <= 5.0,
        "measurement derivative request={}",
        first_requested[2]
    );
    assert!(first_applied.iter().all(|torque| torque.abs() <= 10.0));
    assert!((measurement.last().expect("measurement trace").0 - 0.5).abs() <= 0.02);
}

#[test]
fn control_curriculum_filtered_derivative_trades_encoder_noise_for_measured_lag() {
    let bypass = run_filtered_derivative(0, 0.05);
    let first_fast = run_filtered_derivative(1, 0.02);
    let first = run_filtered_derivative(1, 0.05);
    let first_slow = run_filtered_derivative(1, 0.10);
    let second = run_filtered_derivative(2, 0.05);
    let increment_rms = |trace: &[(f64, f64, f64, f64)]| {
        (trace[750..]
            .windows(2)
            .map(|samples| (samples[1].2 - samples[0].2).powi(2))
            .sum::<f64>()
            / 249.0)
            .sqrt()
    };
    let positive_lag_area = |trace: &[(f64, f64, f64, f64)]| {
        trace[..250]
            .iter()
            .map(|sample| (sample.0 - sample.1).max(0.0) * 0.02)
            .sum::<f64>()
    };
    let rms = [
        increment_rms(&bypass),
        increment_rms(&first_fast),
        increment_rms(&first),
        increment_rms(&first_slow),
        increment_rms(&second),
    ];
    let lag = [
        positive_lag_area(&bypass),
        positive_lag_area(&first_fast),
        positive_lag_area(&first),
        positive_lag_area(&first_slow),
        positive_lag_area(&second),
    ];
    println!(
        "torque increment RMS bypass/first(0.02/0.05/0.10)/second(0.05)={:.6}/{:.6}/{:.6}/{:.6}/{:.6}Nm; positive lag={:.6}/{:.6}/{:.6}/{:.6}/{:.6}rad*s",
        rms[0], rms[1], rms[2], rms[3], rms[4], lag[0], lag[1], lag[2], lag[3], lag[4]
    );
    let raw_position = bypass.iter().map(|sample| sample.0).collect::<Vec<_>>();
    let bypass_position = bypass.iter().map(|sample| sample.1).collect::<Vec<_>>();
    let fast_position = first_fast.iter().map(|sample| sample.1).collect::<Vec<_>>();
    let first_position = first.iter().map(|sample| sample.1).collect::<Vec<_>>();
    let slow_position = first_slow.iter().map(|sample| sample.1).collect::<Vec<_>>();
    let second_position = second.iter().map(|sample| sample.1).collect::<Vec<_>>();
    let bypass_torque = bypass.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let fast_torque = first_fast.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let first_torque = first.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let slow_torque = first_slow.iter().map(|sample| sample.2).collect::<Vec<_>>();
    let second_torque = second.iter().map(|sample| sample.2).collect::<Vec<_>>();

    // Qualify the behavior before comparing retained files so a missing cutoff
    // implementation fails as a controller assertion, not as artifact drift.
    assert!(
        rms[2] <= rms[1] * 0.9 && rms[3] <= rms[2] * 0.9,
        "lower cutoff must reduce torque activity: {rms:?}"
    );
    assert!(lag[1] < lag[2] && lag[2] < lag[3], "cutoff lag={lag:?}");
    assert!(rms[2] <= rms[0] * 0.8, "first-order RMS={rms:?}");
    assert!(rms[4] <= rms[2] * 0.8, "second-order RMS={rms:?}");
    assert!(lag[4] > lag[2], "filter-order lag={lag:?}");
    let qualifies = |index: usize| rms[index] <= 0.250 && lag[index] <= 0.020;
    assert!(!qualifies(0), "bypass must fail the activity limit");
    assert!(
        !qualifies(1),
        "the 0.02 s filter must fail the activity limit"
    );
    assert!(
        qualifies(2),
        "the 0.05 s first-order filter must satisfy both limits"
    );
    assert!(!qualifies(3), "the 0.10 s filter must fail the lag limit");
    assert!(
        !qualifies(4),
        "the second-order filter must fail the lag limit"
    );
    for trace in [&bypass, &first_fast, &first, &first_slow, &second] {
        assert!(
            (trace[900..].iter().map(|sample| sample.1).sum::<f64>() / 100.0 - 0.5).abs() <= 0.02
        );
        assert!(trace.iter().all(|sample| sample.2.abs() <= 10.0));
    }

    let summary = format!(
        "filter,time_constant_s,torque_increment_rms_nm,positive_position_lag_rad_s,decision\nBypass,,{:.12},{:.12},reject: activity above 0.250 N m\nFirst order,0.02,{:.12},{:.12},reject: activity above 0.250 N m\nFirst order,0.05,{:.12},{:.12},select: satisfies activity and lag limits\nFirst order,0.10,{:.12},{:.12},reject: lag above 0.020 rad s\nSecond order,0.05,{:.12},{:.12},reject: lag above 0.020 rad s\n",
        rms[0], lag[0], rms[1], lag[1], rms[2], lag[2], rms[3], lag[3], rms[4], lag[4]
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/02-derivative-action-and-dynamic-response/23-servo-filtered-derivative/figures");
    evidence::retain(
        &artifact_root,
        "filtered-derivative",
        "Derivative filtering trades encoder-noise activity for lag",
        "Configured Simulation evidence · same servo, noise seed, controller and axes",
        0.02,
        &[
            evidence::Panel {
                title: "Raw and controller-facing position",
                unit: "rad",
                minimum: 0.0,
                maximum: 0.6,
                series: vec![
                    evidence::Series {
                        name: "raw position",
                        color: "#555555",
                        dashed: true,
                        values: &raw_position,
                    },
                    evidence::Series {
                        name: "bypass",
                        color: "#d55e00",
                        dashed: false,
                        values: &bypass_position,
                    },
                    evidence::Series {
                        name: "first τ=.02",
                        color: "#cc79a7",
                        dashed: false,
                        values: &fast_position,
                    },
                    evidence::Series {
                        name: "first τ=.05",
                        color: "#0072b2",
                        dashed: false,
                        values: &first_position,
                    },
                    evidence::Series {
                        name: "first τ=.10",
                        color: "#e69f00",
                        dashed: false,
                        values: &slow_position,
                    },
                    evidence::Series {
                        name: "second τ=.05",
                        color: "#009e73",
                        dashed: false,
                        values: &second_position,
                    },
                ],
            },
            evidence::Panel {
                title: "Applied motor torque",
                unit: "N m",
                minimum: -2.0,
                maximum: 6.0,
                series: vec![
                    evidence::Series {
                        name: "bypass torque",
                        color: "#d55e00",
                        dashed: false,
                        values: &bypass_torque,
                    },
                    evidence::Series {
                        name: "τ=.02 torque",
                        color: "#cc79a7",
                        dashed: false,
                        values: &fast_torque,
                    },
                    evidence::Series {
                        name: "τ=.05 torque",
                        color: "#0072b2",
                        dashed: false,
                        values: &first_torque,
                    },
                    evidence::Series {
                        name: "τ=.10 torque",
                        color: "#e69f00",
                        dashed: false,
                        values: &slow_torque,
                    },
                    evidence::Series {
                        name: "second torque",
                        color: "#009e73",
                        dashed: false,
                        values: &second_torque,
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );
}

#[test]
fn control_curriculum_mechanical_nonidealities_are_diagnosed_before_retuning() {
    let ideal = run_mechanical_nonidealities(MechanicalCase::Ideal);
    let profile = run_mechanical_nonidealities(MechanicalCase::Profile);
    let torque = run_mechanical_nonidealities(MechanicalCase::Torque);
    let friction = run_mechanical_nonidealities(MechanicalCase::Friction);
    let deadband_only = run_mechanical_nonidealities(MechanicalCase::Deadband);
    let backlash_only = run_mechanical_nonidealities(MechanicalCase::Backlash);
    let stiction_only = run_mechanical_nonidealities(MechanicalCase::Stiction);
    let nonideal = run_mechanical_nonidealities(MechanicalCase::Combined);
    let reversal_delay = |trace: &[MechanicalSample]| {
        let start = trace[499].load_position;
        trace[500..]
            .iter()
            .position(|sample| sample.load_position <= start - 0.02)
            .expect("load position must move after reversal") as f64
            * 0.02
    };
    let ideal_delay = reversal_delay(&ideal);
    let profile_delay = reversal_delay(&profile);
    let nonideal_delay = reversal_delay(&nonideal);
    assert!(profile_delay >= ideal_delay + 0.02,
        "the trajectory-limited case must differ from the unprofiled ideal reference: ideal={ideal_delay}, profile={profile_delay}");
    let torque_gap = torque
        .iter()
        .map(|sample| (sample.requested_torque - sample.applied_torque).abs())
        .fold(0.0_f64, f64::max);
    assert!(
        torque_gap >= 0.1,
        "isolated torque-limit request/application gap={torque_gap}"
    );
    assert!(torque
        .iter()
        .all(|sample| sample.applied_torque.abs() <= 1.0 + 1e-9));
    assert!(deadband_only.iter().any(|sample| sample.deadband));
    assert!(stiction_only.iter().any(|sample| sample.stiction));
    assert!(backlash_only.iter().all(|sample| sample.backlash_active));
    assert!(friction
        .iter()
        .all(|sample| sample.load_position.is_finite()));
    let requested_applied_gap = nonideal
        .iter()
        .map(|sample| (sample.requested_torque - sample.applied_torque).abs())
        .fold(0.0_f64, f64::max);
    let maximum_profile_velocity = nonideal
        .iter()
        .map(|sample| sample.profiled_velocity.abs())
        .fold(0.0_f64, f64::max);
    let maximum_profile_acceleration = nonideal
        .windows(2)
        .map(|samples| (samples[1].profiled_velocity - samples[0].profiled_velocity).abs() / 0.02)
        .fold(0.0_f64, f64::max);
    let maximum_requested_torque = nonideal
        .iter()
        .map(|sample| sample.requested_torque.abs())
        .fold(0.0_f64, f64::max);
    let maximum_applied_torque = nonideal
        .iter()
        .map(|sample| sample.applied_torque.abs())
        .fold(0.0_f64, f64::max);
    println!(
        "ideal/nonideal reversal delay={ideal_delay:.3}/{nonideal_delay:.3}s, requested/applied maxima={maximum_requested_torque:.3}/{maximum_applied_torque:.3}Nm, maximum gap={requested_applied_gap:.3}Nm, profile velocity={maximum_profile_velocity:.3}rad/s, profile acceleration={maximum_profile_acceleration:.3}rad/s2, state counts saturation/slew/deadband/stiction={}/{}/{}/{}",
        nonideal.iter().filter(|sample| sample.saturation).count(),
        nonideal.iter().filter(|sample| sample.slew_limit).count(),
        nonideal.iter().filter(|sample| sample.deadband).count(),
        nonideal.iter().filter(|sample| sample.stiction).count(),
    );
    let ideal_position = ideal
        .iter()
        .map(|sample| sample.load_position)
        .collect::<Vec<_>>();
    let profile_position = profile
        .iter()
        .map(|sample| sample.load_position)
        .collect::<Vec<_>>();
    let torque_position = torque
        .iter()
        .map(|sample| sample.load_position)
        .collect::<Vec<_>>();
    let friction_position = friction
        .iter()
        .map(|sample| sample.load_position)
        .collect::<Vec<_>>();
    let deadband_position = deadband_only
        .iter()
        .map(|sample| sample.load_position)
        .collect::<Vec<_>>();
    let backlash_position = backlash_only
        .iter()
        .map(|sample| sample.load_position)
        .collect::<Vec<_>>();
    let stiction_position = stiction_only
        .iter()
        .map(|sample| sample.load_position)
        .collect::<Vec<_>>();
    let profile_requested = profile
        .iter()
        .map(|s| s.requested_torque)
        .collect::<Vec<_>>();
    let profile_applied = profile.iter().map(|s| s.applied_torque).collect::<Vec<_>>();
    let torque_requested = torque
        .iter()
        .map(|s| s.requested_torque)
        .collect::<Vec<_>>();
    let torque_applied = torque.iter().map(|s| s.applied_torque).collect::<Vec<_>>();
    let torque_saturation = torque
        .iter()
        .map(|s| f64::from(s.saturation))
        .collect::<Vec<_>>();
    let torque_slew = torque
        .iter()
        .map(|s| f64::from(s.slew_limit))
        .collect::<Vec<_>>();
    let friction_requested = friction
        .iter()
        .map(|s| s.requested_torque)
        .collect::<Vec<_>>();
    let friction_applied = friction
        .iter()
        .map(|s| s.applied_torque)
        .collect::<Vec<_>>();
    let deadband_requested = deadband_only
        .iter()
        .map(|s| s.requested_torque)
        .collect::<Vec<_>>();
    let deadband_applied = deadband_only
        .iter()
        .map(|s| s.applied_torque)
        .collect::<Vec<_>>();
    let deadband_state = deadband_only
        .iter()
        .map(|s| f64::from(s.deadband))
        .collect::<Vec<_>>();
    let backlash_requested = backlash_only
        .iter()
        .map(|s| s.requested_torque)
        .collect::<Vec<_>>();
    let backlash_applied = backlash_only
        .iter()
        .map(|s| s.applied_torque)
        .collect::<Vec<_>>();
    let backlash_state = backlash_only
        .iter()
        .map(|s| f64::from(s.backlash_active))
        .collect::<Vec<_>>();
    let stiction_requested = stiction_only
        .iter()
        .map(|s| s.requested_torque)
        .collect::<Vec<_>>();
    let stiction_applied = stiction_only
        .iter()
        .map(|s| s.applied_torque)
        .collect::<Vec<_>>();
    let stiction_state = stiction_only
        .iter()
        .map(|s| f64::from(s.stiction))
        .collect::<Vec<_>>();
    let nonideal_position = nonideal
        .iter()
        .map(|sample| sample.load_position)
        .collect::<Vec<_>>();
    let profiled_position = nonideal
        .iter()
        .map(|sample| sample.profiled_position)
        .collect::<Vec<_>>();
    let requested_torque = nonideal
        .iter()
        .map(|sample| sample.requested_torque)
        .collect::<Vec<_>>();
    let applied_torque = nonideal
        .iter()
        .map(|sample| sample.applied_torque)
        .collect::<Vec<_>>();
    let saturation = nonideal
        .iter()
        .map(|sample| f64::from(sample.saturation))
        .collect::<Vec<_>>();
    let slew_limit = nonideal
        .iter()
        .map(|sample| f64::from(sample.slew_limit))
        .collect::<Vec<_>>();
    let deadband = nonideal
        .iter()
        .map(|sample| f64::from(sample.deadband))
        .collect::<Vec<_>>();
    let stiction = nonideal
        .iter()
        .map(|sample| f64::from(sample.stiction))
        .collect::<Vec<_>>();
    let backlash = nonideal
        .iter()
        .map(|sample| f64::from(sample.backlash_active))
        .collect::<Vec<_>>();
    let mut summary = String::from("case,reversal_delay_s,peak_following_error_rad,settled,final_absolute_error_rad,requested_torque_total_variation_nm,applied_torque_total_variation_nm,saturation_time_s,slew_limit_time_s,deadband_time_s,stiction_time_s,velocity_limit_violations,acceleration_limit_violations,torque_limit_violations,decision\n");
    for (name, trace, decision) in [
        ("ideal", ideal.as_slice(), "unprofiled reference"),
        (
            "trajectory profile",
            profile.as_slice(),
            "shape the demand before retuning",
        ),
        (
            "torque limit and slew",
            torque.as_slice(),
            "respect drive constraints",
        ),
        (
            "Coulomb friction",
            friction.as_slice(),
            "characterise friction before compensation",
        ),
        (
            "command deadband",
            deadband_only.as_slice(),
            "distinguish ignored command from clearance",
        ),
        (
            "backlash",
            backlash_only.as_slice(),
            "inspect mechanical clearance",
        ),
        (
            "stiction",
            stiction_only.as_slice(),
            "inspect sticking mechanism",
        ),
        (
            "combined",
            nonideal.as_slice(),
            "diagnose mechanics before retuning",
        ),
    ] {
        let delay = reversal_delay(trace);
        let following = trace
            .iter()
            .map(|s| (s.profiled_position - s.load_position).abs())
            .fold(0.0_f64, f64::max);
        let final_slice = &trace[900..];
        let range = |field: fn(&MechanicalSample) -> f64| {
            let lo = final_slice.iter().map(field).fold(f64::INFINITY, f64::min);
            let hi = final_slice
                .iter()
                .map(field)
                .fold(f64::NEG_INFINITY, f64::max);
            hi - lo
        };
        let settled = range(|s| s.load_position) <= 0.01 && range(|s| s.applied_torque) <= 0.05;
        let final_error = if settled {
            format!(
                "{:.12}",
                final_slice
                    .iter()
                    .map(|s| (s.profiled_position - s.load_position).abs())
                    .sum::<f64>()
                    / final_slice.len() as f64
            )
        } else {
            String::new()
        };
        let variation = |field: fn(&MechanicalSample) -> f64| {
            trace
                .windows(2)
                .map(|w| (field(&w[1]) - field(&w[0])).abs())
                .sum::<f64>()
        };
        let velocity_violations = trace
            .iter()
            .filter(|s| s.profiled_velocity.abs() > 0.5 + 1e-9)
            .count();
        let acceleration_violations = trace
            .windows(2)
            .filter(|w| (w[1].profiled_velocity - w[0].profiled_velocity).abs() / 0.02 > 1.0 + 1e-9)
            .count();
        let applied_limit = if name == "torque limit and slew" {
            1.0
        } else if name == "combined" {
            2.5
        } else {
            10.0
        };
        let torque_violations = trace
            .iter()
            .filter(|s| s.applied_torque.abs() > applied_limit + 1e-9)
            .count();
        summary.push_str(&format!("{name},{delay:.12},{following:.12},{settled},{final_error},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{velocity_violations},{acceleration_violations},{torque_violations},{decision}\n",
            variation(|s| s.requested_torque), variation(|s| s.applied_torque),
            trace.iter().filter(|s| s.saturation).count() as f64 * 0.02,
            trace.iter().filter(|s| s.slew_limit).count() as f64 * 0.02,
            trace.iter().filter(|s| s.deadband).count() as f64 * 0.02,
            trace.iter().filter(|s| s.stiction).count() as f64 * 0.02));
    }
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/02-derivative-action-and-dynamic-response/24-servo-mechanical-nonidealities/figures");
    evidence::retain(
        &artifact_root,
        "mechanical-nonidealities",
        "Mechanical limits can look like poor controller tuning",
        "Configured Simulation evidence · identical controller and reversal; only plant nonidealities change",
        0.02,
        &[
            evidence::Panel {
                title: "Load and profiled position",
                unit: "rad",
                minimum: -0.6,
                maximum: 0.6,
                series: vec![
                    evidence::Series { name: "ideal load", color: "#009e73", dashed: true, values: &ideal_position },
                    evidence::Series { name: "profile only", color: "#0072b2", dashed: false, values: &profile_position },
                    evidence::Series { name: "torque limits", color: "#e69f00", dashed: false, values: &torque_position },
                    evidence::Series { name: "friction", color: "#56b4e9", dashed: false, values: &friction_position },
                    evidence::Series { name: "deadband", color: "#cc79a7", dashed: false, values: &deadband_position },
                    evidence::Series { name: "backlash", color: "#f0e442", dashed: false, values: &backlash_position },
                    evidence::Series { name: "stiction", color: "#000000", dashed: false, values: &stiction_position },
                    evidence::Series { name: "nonideal load", color: "#d55e00", dashed: false, values: &nonideal_position },
                    evidence::Series { name: "profile", color: "#0072b2", dashed: false, values: &profiled_position },
                ],
            },
            evidence::Panel {
                title: "Requested and applied motor torque",
                unit: "N m",
                minimum: -4.0,
                maximum: 4.0,
                series: vec![
                    evidence::Series { name: "requested", color: "#d55e00", dashed: true, values: &requested_torque },
                    evidence::Series { name: "applied", color: "#0072b2", dashed: false, values: &applied_torque },
                ],
            },
        ],
        &[
            evidence::Series { name: "profile requested torque", color: "#000000", dashed: false, values: &profile_requested },
            evidence::Series { name: "profile applied torque", color: "#000000", dashed: false, values: &profile_applied },
            evidence::Series { name: "torque-case requested torque", color: "#000000", dashed: false, values: &torque_requested },
            evidence::Series { name: "torque-case applied torque", color: "#000000", dashed: false, values: &torque_applied },
            evidence::Series { name: "torque-case saturation", color: "#000000", dashed: false, values: &torque_saturation },
            evidence::Series { name: "torque-case slew limit", color: "#000000", dashed: false, values: &torque_slew },
            evidence::Series { name: "friction requested torque", color: "#000000", dashed: false, values: &friction_requested },
            evidence::Series { name: "friction applied torque", color: "#000000", dashed: false, values: &friction_applied },
            evidence::Series { name: "deadband requested torque", color: "#000000", dashed: false, values: &deadband_requested },
            evidence::Series { name: "deadband applied torque", color: "#000000", dashed: false, values: &deadband_applied },
            evidence::Series { name: "deadband isolated state", color: "#000000", dashed: false, values: &deadband_state },
            evidence::Series { name: "backlash requested torque", color: "#000000", dashed: false, values: &backlash_requested },
            evidence::Series { name: "backlash applied torque", color: "#000000", dashed: false, values: &backlash_applied },
            evidence::Series { name: "backlash isolated state", color: "#000000", dashed: false, values: &backlash_state },
            evidence::Series { name: "stiction requested torque", color: "#000000", dashed: false, values: &stiction_requested },
            evidence::Series { name: "stiction applied torque", color: "#000000", dashed: false, values: &stiction_applied },
            evidence::Series { name: "stiction isolated state", color: "#000000", dashed: false, values: &stiction_state },
            evidence::Series { name: "saturation state", color: "#000000", dashed: false, values: &saturation },
            evidence::Series { name: "slew-limit state", color: "#000000", dashed: false, values: &slew_limit },
            evidence::Series { name: "deadband state", color: "#000000", dashed: false, values: &deadband },
            evidence::Series { name: "stiction state", color: "#000000", dashed: false, values: &stiction },
            evidence::Series { name: "backlash state", color: "#000000", dashed: false, values: &backlash },
        ],
        &[evidence::Marker { at_seconds: 10.0, label: "direction reversal" }],
        &summary,
    );

    assert!(nonideal_delay >= ideal_delay + 0.04);
    assert!(requested_applied_gap >= 0.5);
    assert!(maximum_profile_velocity <= 0.5 + 1.0e-9);
    assert!(maximum_profile_acceleration <= 1.0 + 1.0e-9);
    assert!(nonideal
        .iter()
        .all(|sample| sample.profiled_position.is_finite()));
    assert!(nonideal.iter().any(|sample| sample.saturation));
    assert!(nonideal.iter().any(|sample| sample.slew_limit));
    assert!(nonideal.iter().any(|sample| sample.deadband));
    assert!(nonideal.iter().any(|sample| sample.stiction));
    assert!(nonideal.iter().all(|sample| sample.backlash_active));
}

#[test]
fn control_curriculum_digital_realization_exposes_method_sampling_alias_and_delay() {
    let forward_20_ms = run_digital_realization(0, 1, false);
    let backward_20_ms = run_digital_realization(1, 1, false);
    let tustin_10_ms = run_digital_realization(2, 0, false);
    let tustin_20_ms = run_digital_realization(2, 1, false);
    let tustin_100_ms = run_digital_realization(2, 2, false);
    let delayed_10_ms = run_digital_realization(2, 0, true);

    let update_indices = |trace: &[DigitalSample]| {
        trace
            .iter()
            .enumerate()
            .filter_map(|(index, sample)| sample.update.then_some(index))
            .collect::<Vec<_>>()
    };
    assert!(update_indices(&tustin_10_ms)
        .windows(2)
        .all(|pair| pair[1] - pair[0] == 1));
    assert!(update_indices(&forward_20_ms)
        .windows(2)
        .all(|pair| pair[1] - pair[0] == 2));
    assert!(update_indices(&tustin_100_ms)
        .windows(2)
        .all(|pair| pair[1] - pair[0] == 10));
    assert!(
        tustin_100_ms.windows(2).all(|samples| {
            samples[1].update || (samples[1].torque - samples[0].torque).abs() <= 1.0e-12
        }),
        "zero-order hold must retain torque between controller updates"
    );

    let first_nonzero = |trace: &[DigitalSample]| {
        trace
            .iter()
            .position(|sample| sample.torque.abs() > 1.0e-9)
            .unwrap()
    };
    assert!(first_nonzero(&delayed_10_ms) >= first_nonzero(&tustin_10_ms) + 1);

    let maximum_method_difference = forward_20_ms
        .iter()
        .zip(&backward_20_ms)
        .map(|(forward, backward)| (forward.torque - backward.torque).abs())
        .fold(0.0_f64, f64::max);
    assert!(
        maximum_method_difference >= 0.01,
        "Euler choices must execute different recurrences, gap={maximum_method_difference}"
    );

    let slow_updates = tustin_100_ms
        .iter()
        .filter(|sample| sample.update)
        .collect::<Vec<_>>();
    let alias_amplitude = {
        let sine = slow_updates
            .iter()
            .map(|sample| {
                (sample.position - sample.ideal_position)
                    * (2.0 * std::f64::consts::PI * 2.0 * sample.time_s).sin()
            })
            .sum::<f64>();
        let cosine = slow_updates
            .iter()
            .map(|sample| {
                (sample.position - sample.ideal_position)
                    * (2.0 * std::f64::consts::PI * 2.0 * sample.time_s).cos()
            })
            .sum::<f64>();
        2.0 * sine.hypot(cosine) / slow_updates.len() as f64
    };
    let spectral_updates = slow_updates
        .iter()
        .copied()
        .filter(|sample| sample.time_s >= 5.0)
        .collect::<Vec<_>>();
    let frequency_step_hz = 0.05;
    let alias_spectrum = (1..=100)
        .map(|bin| {
            let frequency = bin as f64 * frequency_step_hz;
            let sine = spectral_updates
                .iter()
                .map(|sample| {
                    (sample.position - sample.ideal_position)
                        * (2.0 * std::f64::consts::PI * frequency * sample.time_s).sin()
                })
                .sum::<f64>();
            let cosine = spectral_updates
                .iter()
                .map(|sample| {
                    (sample.position - sample.ideal_position)
                        * (2.0 * std::f64::consts::PI * frequency * sample.time_s).cos()
                })
                .sum::<f64>();
            2.0 * sine.hypot(cosine) / spectral_updates.len() as f64
        })
        .collect::<Vec<_>>();
    let alias_bin = alias_spectrum
        .iter()
        .enumerate()
        .skip(9)
        .max_by(|left, right| left.1.total_cmp(right.1))
        .expect("alias spectrum peak")
        .0;
    let estimated_alias_frequency_hz = (alias_bin + 1) as f64 * frequency_step_hz;
    assert!(
        (estimated_alias_frequency_hz - 2.0).abs() <= 0.1,
        "independently estimated alias frequency={estimated_alias_frequency_hz}Hz"
    );
    assert!(alias_amplitude >= 0.001,
        "8 Hz encoder interference sampled at 10 Hz must appear near 2 Hz; amplitude={alias_amplitude}");
    assert!(
        tustin_10_ms.iter().all(|sample| {
            (sample.position / 0.001 - (sample.position / 0.001).round()).abs() <= 1.0e-9
        }),
        "controller-facing position must expose the configured 0.001 rad quantization"
    );
    for trace in [
        &forward_20_ms,
        &backward_20_ms,
        &tustin_10_ms,
        &tustin_20_ms,
        &tustin_100_ms,
        &delayed_10_ms,
    ] {
        assert!(trace.iter().all(|sample| {
            sample.position.is_finite()
                && sample.ideal_position.is_finite()
                && sample.torque.is_finite()
                && sample.torque.abs() <= 10.0
        }));
    }
    println!(
        "method torque gap={maximum_method_difference:.6}Nm, 100 ms sampled 8 Hz alias amplitude={alias_amplitude:.6}rad, first torque fast/delayed={:.3}/{:.3}s",
        tustin_10_ms[first_nonzero(&tustin_10_ms)].time_s,
        delayed_10_ms[first_nonzero(&delayed_10_ms)].time_s,
    );
    let forward_position = forward_20_ms
        .iter()
        .map(|sample| sample.position)
        .collect::<Vec<_>>();
    let backward_position = backward_20_ms
        .iter()
        .map(|sample| sample.position)
        .collect::<Vec<_>>();
    let tustin_20_position = tustin_20_ms
        .iter()
        .map(|sample| sample.position)
        .collect::<Vec<_>>();
    let fast_position = tustin_10_ms
        .iter()
        .map(|sample| sample.position)
        .collect::<Vec<_>>();
    let slow_position = tustin_100_ms
        .iter()
        .map(|sample| sample.position)
        .collect::<Vec<_>>();
    let delayed_position = delayed_10_ms
        .iter()
        .map(|sample| sample.position)
        .collect::<Vec<_>>();
    let ideal_position = tustin_10_ms
        .iter()
        .map(|sample| sample.ideal_position)
        .collect::<Vec<_>>();
    let forward_torque = forward_20_ms
        .iter()
        .map(|sample| sample.torque)
        .collect::<Vec<_>>();
    let backward_torque = backward_20_ms
        .iter()
        .map(|sample| sample.torque)
        .collect::<Vec<_>>();
    let tustin_20_torque = tustin_20_ms
        .iter()
        .map(|sample| sample.torque)
        .collect::<Vec<_>>();
    let fast_torque = tustin_10_ms
        .iter()
        .map(|sample| sample.torque)
        .collect::<Vec<_>>();
    let slow_torque = tustin_100_ms
        .iter()
        .map(|sample| sample.torque)
        .collect::<Vec<_>>();
    let delayed_torque = delayed_10_ms
        .iter()
        .map(|sample| sample.torque)
        .collect::<Vec<_>>();
    let fast_update = tustin_10_ms
        .iter()
        .map(|sample| f64::from(sample.update))
        .collect::<Vec<_>>();
    let slow_update = tustin_100_ms
        .iter()
        .map(|sample| f64::from(sample.update))
        .collect::<Vec<_>>();
    let setpoint = vec![0.5; tustin_10_ms.len()];
    let alias_reference = alias_spectrum
        .iter()
        .enumerate()
        .map(|(index, _)| if index == 39 { alias_amplitude } else { 0.0 })
        .collect::<Vec<_>>();
    let fast_start = tustin_10_ms[first_nonzero(&tustin_10_ms)].time_s;
    let delayed_start = delayed_10_ms[first_nonzero(&delayed_10_ms)].time_s;
    let mut summary = String::from("case,max_absolute_position_error_rad,overshoot_rad,settled,settling_time_s,final_position_span_rad,final_torque_span_nm,torque_total_variation_nm,max_held_output_duration_s,max_sample_interval_s,sample_interval_jitter_s,alias_amplitude_rad,estimated_alias_frequency_hz,bounded,decision\n");
    for (name, trace, decision) in [
        (
            "forward Euler 20 ms",
            forward_20_ms.as_slice(),
            "method candidate",
        ),
        (
            "backward Euler 20 ms",
            backward_20_ms.as_slice(),
            "method candidate",
        ),
        ("Tustin 20 ms", tustin_20_ms.as_slice(), "method reference"),
        (
            "Tustin 10 ms",
            tustin_10_ms.as_slice(),
            "fast interval reference",
        ),
        (
            "Tustin 100 ms",
            tustin_100_ms.as_slice(),
            "slow interval exposes hold and alias",
        ),
        (
            "Tustin 10 ms plus delay",
            delayed_10_ms.as_slice(),
            "one-update delay",
        ),
    ] {
        let maximum_error = trace
            .iter()
            .map(|s| (0.5 - s.position).abs())
            .fold(0.0_f64, f64::max);
        let overshoot = (trace
            .iter()
            .map(|s| s.position)
            .fold(f64::NEG_INFINITY, f64::max)
            - 0.5)
            .max(0.0);
        let final_window = &trace[1800..];
        let span = |field: fn(&DigitalSample) -> f64| {
            final_window
                .iter()
                .map(field)
                .fold(f64::NEG_INFINITY, f64::max)
                - final_window.iter().map(field).fold(f64::INFINITY, f64::min)
        };
        let final_position_span = span(|s| s.position);
        let final_torque_span = span(|s| s.torque);
        let entry = (0..trace.len().saturating_sub(50)).find(|start| {
            trace[*start..]
                .iter()
                .all(|s| (s.position - 0.5).abs() <= 0.02)
        });
        let settled = entry.is_some() && final_position_span <= 0.01 && final_torque_span <= 0.5;
        let settling = if settled {
            entry
                .map(|i| format!("{:.12}", trace[i].time_s))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let variation = trace
            .windows(2)
            .map(|w| (w[1].torque - w[0].torque).abs())
            .sum::<f64>();
        let updates = update_indices(trace);
        let intervals = updates
            .windows(2)
            .map(|w| (w[1] - w[0]) as f64 * 0.01)
            .collect::<Vec<_>>();
        let maximum_interval = intervals.iter().copied().fold(0.0_f64, f64::max);
        let minimum_interval = intervals.iter().copied().fold(f64::INFINITY, f64::min);
        let maximum_hold = maximum_interval;
        let bounded = trace
            .iter()
            .all(|s| s.position.is_finite() && s.torque.is_finite() && s.torque.abs() <= 10.0);
        summary.push_str(&format!("{name},{maximum_error:.12},{overshoot:.12},{settled},{settling},{final_position_span:.12},{final_torque_span:.12},{variation:.12},{maximum_hold:.12},{maximum_interval:.12},{:.12},,,{bounded},{decision}\n", maximum_interval-minimum_interval));
    }
    summary.push_str(&format!("alias measurement,,,,,,,,,,,{alias_amplitude:.12},{estimated_alias_frequency_hz:.12},true,estimated spectrum peak from retained 100 ms samples\ncomputation delay,{fast_start:.12},{delayed_start:.12},,,,,,,,,,,true,first action is delayed by one update\n"));
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/02-derivative-action-and-dynamic-response/25-servo-digital-realization/figures");
    evidence::retain(
        &artifact_root,
        "digital-realization",
        "Digital choices change the implemented controller",
        "Configured Simulation evidence · same servo and gains; method, sample interval or delay changes",
        0.01,
        &[
            evidence::Panel {
                title: "Measured table position",
                unit: "rad",
                minimum: -0.1,
                maximum: 0.6,
                series: vec![
                    evidence::Series { name: "SP", color: "#555555", dashed: true, values: &setpoint },
                    evidence::Series { name: "ideal", color: "#555555", dashed: true, values: &ideal_position },
                    evidence::Series { name: "10 ms", color: "#0072b2", dashed: false, values: &fast_position },
                    evidence::Series { name: "100 ms", color: "#d55e00", dashed: false, values: &slow_position },
                    evidence::Series { name: "10 ms + delay", color: "#009e73", dashed: false, values: &delayed_position },
                ],
            },
            evidence::Panel {
                title: "Requested motor torque",
                unit: "N m",
                minimum: -10.0,
                maximum: 10.0,
                series: vec![
                    evidence::Series { name: "10 ms torque", color: "#0072b2", dashed: false, values: &fast_torque },
                    evidence::Series { name: "100 ms torque", color: "#d55e00", dashed: false, values: &slow_torque },
                    evidence::Series { name: "delayed torque", color: "#009e73", dashed: false, values: &delayed_torque },
                ],
            },
        ],
        &[
            evidence::Series { name: "forward Euler position", color: "#000000", dashed: false, values: &forward_position },
            evidence::Series { name: "backward Euler position", color: "#000000", dashed: false, values: &backward_position },
            evidence::Series { name: "Tustin 20 ms position", color: "#000000", dashed: false, values: &tustin_20_position },
            evidence::Series { name: "forward Euler torque", color: "#000000", dashed: false, values: &forward_torque },
            evidence::Series { name: "backward Euler torque", color: "#000000", dashed: false, values: &backward_torque },
            evidence::Series { name: "Tustin 20 ms torque", color: "#000000", dashed: false, values: &tustin_20_torque },
            evidence::Series { name: "10 ms update pulse", color: "#000000", dashed: false, values: &fast_update },
            evidence::Series { name: "100 ms update pulse", color: "#000000", dashed: false, values: &slow_update },
        ],
        &[],
        &summary,
    );
    evidence::retain(
        &artifact_root,
        "digital-method-comparison",
        "Discretization methods differ at the same 20 ms interval",
        "Configured Simulation evidence · identical servo, gains, 20 ms interval, quantization and noise",
        0.01,
        &[
            evidence::Panel { title: "Setpoint and measured position", unit: "rad", minimum: -0.1, maximum: 0.6, series: vec![
                evidence::Series { name: "SP", color: "#555555", dashed: true, values: &setpoint[..250] },
                evidence::Series { name: "FE position", color: "#d55e00", dashed: false, values: &forward_position[..250] },
                evidence::Series { name: "BE position", color: "#009e73", dashed: false, values: &backward_position[..250] },
                evidence::Series { name: "Tustin position", color: "#0072b2", dashed: false, values: &tustin_20_position[..250] },
            ]},
            evidence::Panel { title: "Held motor torque", unit: "N m", minimum: -10.0, maximum: 10.0, series: vec![
                evidence::Series { name: "FE torque", color: "#d55e00", dashed: false, values: &forward_torque[..250] },
                evidence::Series { name: "BE torque", color: "#009e73", dashed: false, values: &backward_torque[..250] },
                evidence::Series { name: "Tustin torque", color: "#0072b2", dashed: false, values: &tustin_20_torque[..250] },
            ]},
        ],
        &[], &[], &summary,
    );
    evidence::retain(
        &artifact_root,
        "digital-update-timing",
        "Controller updates and zero-order holds are observable",
        "Configured Simulation evidence · 10 ms plant step; Tustin controller at 10 and 100 ms",
        0.01,
        &[
            evidence::Panel {
                title: "Controller update pulse",
                unit: "state",
                minimum: 0.0,
                maximum: 1.1,
                series: vec![
                    evidence::Series {
                        name: "10 ms updates",
                        color: "#0072b2",
                        dashed: false,
                        values: &fast_update[..100],
                    },
                    evidence::Series {
                        name: "100 ms updates",
                        color: "#d55e00",
                        dashed: false,
                        values: &slow_update[..100],
                    },
                ],
            },
            evidence::Panel {
                title: "Held motor torque",
                unit: "N m",
                minimum: -10.0,
                maximum: 10.0,
                series: vec![
                    evidence::Series {
                        name: "10 ms torque",
                        color: "#0072b2",
                        dashed: false,
                        values: &fast_torque[..100],
                    },
                    evidence::Series {
                        name: "100 ms torque",
                        color: "#d55e00",
                        dashed: false,
                        values: &slow_torque[..100],
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );
    evidence::retain_frequency(
        &artifact_root,
        "digital-alias-spectrum",
        "The sampled encoder residual contains a 2 Hz alias",
        "Configured Simulation evidence · retained 100 ms samples after 5 s",
        frequency_step_hz,
        &[
            evidence::Panel {
                title: "Measured residual spectrum",
                unit: "rad",
                minimum: 0.0,
                maximum: 0.003,
                series: vec![evidence::Series {
                    name: "measured amplitude",
                    color: "#0072b2",
                    dashed: false,
                    values: &alias_spectrum,
                }],
            },
            evidence::Panel {
                title: "Predicted alias reference",
                unit: "rad",
                minimum: 0.0,
                maximum: 0.003,
                series: vec![evidence::Series {
                    name: "2 Hz prediction",
                    color: "#d55e00",
                    dashed: false,
                    values: &alias_reference,
                }],
            },
        ],
        &summary,
    );
    let alias_svg = std::fs::read_to_string(artifact_root.join("digital-alias-spectrum.svg"))
        .expect("retained alias spectrum");
    assert!(
        alias_svg.contains(">0.0030</text>"),
        "milliradian spectrum ticks must not all round to 0.00"
    );
}

#[test]
fn control_curriculum_combined_pid_selects_the_valid_form_from_integral_action() {
    let source = project_file("pid-forms", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace("AutoMode : BOOL :=", "AutoMode AT %IX120.0 : BOOL :=")
        .replace(
            "PositionSetpoint : LREAL :=",
            "PositionSetpoint AT %IL32 : LREAL :=",
        )
        .replace(
            "PositionProportionalGain : LREAL :=",
            "PositionProportionalGain AT %IL40 : LREAL :=",
        )
        .replace(
            "HeaterSetpoint : LREAL :=",
            "HeaterSetpoint AT %IL48 : LREAL :=",
        )
        .replace(
            "HeaterProportionalGain : LREAL :=",
            "HeaterProportionalGain AT %IL56 : LREAL :=",
        )
        .replace(
            "HeaterIntegralGain : LREAL :=",
            "HeaterIntegralGain AT %IL64 : LREAL :=",
        )
        .replace(
            "DiagnosticError : LREAL :=",
            "DiagnosticError AT %IL72 : LREAL :=",
        )
        .replace(
            "RejectedIncrementalPGain : LREAL :=",
            "RejectedIncrementalPGain AT %IL80 : LREAL :=",
        )
        .replace("Position : LREAL :=", "Position AT %IL0 : LREAL :=")
        .replace("Velocity : LREAL :=", "Velocity AT %IL8 : LREAL :=")
        .replace(
            "HeaterTemperature : LREAL :=",
            "HeaterTemperature AT %IL16 : LREAL :=",
        )
        .replace(
            "PositionTorque : LREAL :=",
            "PositionTorque AT %QL0 : LREAL :=",
        )
        .replace("HeaterPower : LREAL :=", "HeaterPower AT %QL8 : LREAL :=")
        .replace(
            "RejectedIncrementalPOutput : LREAL :=",
            "RejectedIncrementalPOutput AT %ML88 : LREAL :=",
        )
        .replace(
            "PositionalPOutput : LREAL :=",
            "PositionalPOutput AT %ML96 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("pid-forms/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped positional/incremental forms lesson");

    let auto = IoAddress::parse("%IX120.0").unwrap();
    let position_sp = IoAddress::parse("%IL32").unwrap();
    let position_kp = IoAddress::parse("%IL40").unwrap();
    let heater_sp = IoAddress::parse("%IL48").unwrap();
    let heater_kp = IoAddress::parse("%IL56").unwrap();
    let heater_ki = IoAddress::parse("%IL64").unwrap();
    let diagnostic_error = IoAddress::parse("%IL72").unwrap();
    let rejected_kp = IoAddress::parse("%IL80").unwrap();
    let position = IoAddress::parse("%IL0").unwrap();
    let heater_temperature = IoAddress::parse("%IL16").unwrap();
    let position_torque = IoAddress::parse("%QL0").unwrap();
    let heater_power = IoAddress::parse("%QL8").unwrap();
    let rejected_output = IoAddress::parse("%ML88").unwrap();
    let positional_output = IoAddress::parse("%ML96").unwrap();

    write_bool(&mut runtime, &auto, true);
    write_lreal(&mut runtime, &position_sp, 0.5);
    write_lreal(&mut runtime, &position_kp, 4.0);
    write_lreal(&mut runtime, &heater_sp, 35.0);
    write_lreal(&mut runtime, &heater_kp, 4.0);
    write_lreal(&mut runtime, &heater_ki, 0.8);
    write_lreal(&mut runtime, &diagnostic_error, 0.5);
    write_lreal(&mut runtime, &rejected_kp, 4.0);
    write_lreal(&mut runtime, &position, 0.0);
    write_lreal(&mut runtime, &heater_temperature, 20.0);
    runtime
        .execute_cycle()
        .expect("execute initial form comparison");

    let position_before = read_lreal(&runtime, &position_torque);
    let heater_before = read_lreal(&runtime, &heater_power);
    assert!(
        (position_before - 2.0).abs() <= 1.0e-12,
        "initial positional P torque was {position_before}"
    );
    assert!(
        (read_lreal(&runtime, &positional_output) - 2.0).abs() <= 1.0e-12,
        "initial positional diagnostic was {}",
        read_lreal(&runtime, &positional_output)
    );
    assert!(
        (read_lreal(&runtime, &rejected_output) - 1.0).abs() <= 1.0e-12,
        "initial rejected incremental P state was {}",
        read_lreal(&runtime, &rejected_output)
    );

    write_lreal(&mut runtime, &position_kp, 8.0);
    write_lreal(&mut runtime, &heater_kp, 8.0);
    write_lreal(&mut runtime, &rejected_kp, 8.0);
    runtime
        .execute_cycle()
        .expect("execute unchanged-error gain change");

    let position_after = read_lreal(&runtime, &position_torque);
    let heater_after = read_lreal(&runtime, &heater_power);
    let expected_integral_increment = 0.8 * 0.02 * 15.0;
    let pi_discontinuity = (heater_after - heater_before) - expected_integral_increment;
    println!(
        "P torque before/after={position_before:.3}/{position_after:.3}Nm, rejected incremental P={:.3}Nm, PI gain-change discontinuity={pi_discontinuity:.12}%",
        read_lreal(&runtime, &rejected_output),
    );

    assert!(
        (position_after - 4.0).abs() <= 1.0e-12,
        "positional P must respond immediately to changed Kp at unchanged error"
    );
    assert!((read_lreal(&runtime, &positional_output) - 4.0).abs() <= 1.0e-12);
    assert!(
        (read_lreal(&runtime, &rejected_output) - 1.0).abs() <= 1.0e-12,
        "rejected incremental P recurrence must expose its arbitrary retained state"
    );
    assert!(
        pi_discontinuity.abs() <= 1.0e-12,
        "incremental PI must retain output and add only the expected integral increment"
    );

    let config_path = temporary_config_path("pid-forms");
    std::fs::write(&config_path, project_file("pid-forms", "simulation.toml"))
        .expect("write PID-forms simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load both PID-forms plants"),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish both initial plant states");
    let mut run = String::from(
        "sample_index,sample_time_s,plant,sp,pv,error,error_change,kp,ki,previous_requested_output,requested_output,applied_output,selected_form,limit_state,evidence_class\n",
    );
    let mut previous_position_error = 0.5;
    let mut previous_heater_error = 15.0;
    let mut previous_position_output = position_after;
    let mut previous_heater_output = heater_after;
    for sample in 1..=250 {
        let position_pv = read_lreal(&runtime, &position);
        let heater_pv = read_lreal(&runtime, &heater_temperature);
        let position_error = 0.5 - position_pv;
        let heater_error = 35.0 - heater_pv;
        runtime
            .execute_cycle()
            .expect("execute both product controllers against plants");
        let position_request = read_lreal(&runtime, &position_torque);
        let heater_request = read_lreal(&runtime, &heater_power);
        let now = Duration::from_millis((sample * 20) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance both PID-forms plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish both plant responses");
        let position_limit = if position_request.abs() >= 10.0 - 1.0e-12 {
            "amplitude"
        } else {
            "none"
        };
        let heater_limit = if heater_request <= 1.0e-12 || heater_request >= 100.0 - 1.0e-12 {
            "amplitude"
        } else {
            "none"
        };
        run.push_str(&format!(
            "{sample},{:.3},positioning-table,0.500000000000,{position_pv:.12},{position_error:.12},{:.12},8.000000000000,0.000000000000,{previous_position_output:.12},{position_request:.12},{position_request:.12},positional,{position_limit},configured_simulation\n",
            sample as f64 * 0.02,
            position_error - previous_position_error,
        ));
        run.push_str(&format!(
            "{sample},{:.3},recirculating-water-heater,35.000000000000,{heater_pv:.12},{heater_error:.12},{:.12},8.000000000000,0.800000000000,{previous_heater_output:.12},{heater_request:.12},{heater_request:.12},incremental,{heater_limit},configured_simulation\n",
            sample as f64 * 0.02,
            heater_error - previous_heater_error,
        ));
        previous_position_error = position_error;
        previous_heater_error = heater_error;
        previous_position_output = position_request;
        previous_heater_output = heater_request;
    }
    let _ = std::fs::remove_file(config_path);
    assert!(
        read_lreal(&runtime, &position).abs() > 0.01,
        "positioning-table plant did not advance"
    );
    assert!(
        (read_lreal(&runtime, &heater_temperature) - 20.0).abs() > 0.01,
        "recirculating-water-heater plant did not advance"
    );

    let position_torque_values = [position_before, position_after];
    let positional_prediction = [2.0, 4.0];
    let rejected_incremental_p = [1.0, read_lreal(&runtime, &rejected_output)];
    let heater_output_values = [heater_before, heater_after];
    let expected_heater_output = [heater_before, heater_before + expected_integral_increment];
    let summary = format!(
        "case,kp_before,kp_after,output_before,output_after,decision\npositional P,4.0,8.0,{position_before:.12},{position_after:.12},valid positional realization\nrejected incremental P,4.0,8.0,1.000000000000,{:.12},reject arbitrary retained state\nincremental PI,4.0,8.0,{heater_before:.12},{heater_after:.12},retains output plus expected integral increment\n",
        read_lreal(&runtime, &rejected_output)
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/03-production-quality-digital-pid/26-pid-forms/figures");
    write_or_compare_curriculum_artifact(&artifact_root.join("pid-forms-run.csv"), &run);
    evidence::retain_updates(
        &artifact_root,
        "pid-forms",
        "Valid realization follows controller structure",
        "Configured Simulation evidence · same inputs; proportional gain changes at update 2",
        &[
            evidence::Panel {
                title: "Position-controller torque",
                unit: "N m",
                minimum: 0.0,
                maximum: 5.0,
                series: vec![
                    evidence::Series {
                        name: "Control.PID positional P",
                        color: "#0072b2",
                        dashed: false,
                        values: &position_torque_values,
                    },
                    evidence::Series {
                        name: "positional prediction",
                        color: "#555555",
                        dashed: true,
                        values: &positional_prediction,
                    },
                    evidence::Series {
                        name: "rejected incremental P",
                        color: "#d55e00",
                        dashed: false,
                        values: &rejected_incremental_p,
                    },
                ],
            },
            evidence::Panel {
                title: "Heater PI retained output",
                unit: "%",
                minimum: heater_before - 0.1,
                maximum: heater_after + 0.1,
                series: vec![
                    evidence::Series {
                        name: "Control.PID incremental PI",
                        color: "#009e73",
                        dashed: false,
                        values: &heater_output_values,
                    },
                    evidence::Series {
                        name: "expected retained plus I",
                        color: "#555555",
                        dashed: true,
                        values: &expected_heater_output,
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );
}

#[test]
fn control_curriculum_pid_forms_retains_both_executable_plant_runs() {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        return;
    }
    let run = project_file("pid-forms", "figures/pid-forms-run.csv");
    let header = run.lines().next().expect("PID forms retained-run header");
    for field in [
        "sample_index",
        "sample_time_s",
        "plant",
        "sp",
        "pv",
        "error",
        "error_change",
        "kp",
        "ki",
        "previous_requested_output",
        "requested_output",
        "applied_output",
        "selected_form",
        "limit_state",
        "evidence_class",
    ] {
        assert!(
            header.split(',').any(|column| column == field),
            "PID forms retained run is missing {field}"
        );
    }
    assert!(
        run.lines()
            .filter(|line| line.contains("positioning-table"))
            .count()
            >= 200
    );
    assert!(
        run.lines()
            .filter(|line| line.contains("recirculating-water-heater"))
            .count()
            >= 200
    );
}
