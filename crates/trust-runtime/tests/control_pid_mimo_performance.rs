use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

fn temp_simulation_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-pid-coupled-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn read_lreal(runtime: &CompiledRuntime, address: &IoAddress) -> f64 {
    match runtime.read_io(address).expect("read LREAL") {
        Value::LWord(bits) => f64::from_bits(bits),
        Value::LReal(value) => value,
        other => panic!("expected LWORD storage for LREAL, got {other:?}"),
    }
}

#[test]
fn pid_advanced_benchmark_controls_a_coupled_two_loop_plant() {
    let source = r#"
USING Control;

PROGRAM Main
VAR
    ControllerA : PID;
    ControllerB : PID;
    Parameters : PIDParameters;
    StateA : PIDInitialState;
    StateB : PIDInitialState;
    InputA : PIDInput;
    InputB : PIDInput;
    ProcessValueA AT %IL0 : LREAL;
    ProcessValueB AT %IL8 : LREAL;
    CommandA AT %QL0 : LREAL;
    CommandB AT %QL8 : LREAL;
    Initialized : BOOL;
END_VAR

IF NOT Initialized THEN
    Parameters.ProportionalGain := LREAL#0.8;
    Parameters.IntegralGain := LREAL#0.4;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#-5.0;
    Parameters.OutputMaximum := LREAL#5.0;
    Parameters.RateMinimum := LREAL#-1.0e100;
    Parameters.RateMaximum := LREAL#1.0e100;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
    ControllerA.Configure(Parameters := Parameters);
    ControllerB.Configure(Parameters := Parameters);
    ControllerA.Initialize(State := StateA);
    ControllerB.Initialize(State := StateB);
    Initialized := TRUE;
END_IF;

InputA.Setpoint := LREAL#1.0;
InputA.ProcessValue := ProcessValueA;
InputA.CycleTime := LREAL#0.05;
InputA.Mode := PIDMode#Auto;
InputB.Setpoint := LREAL#-0.5;
InputB.ProcessValue := ProcessValueB;
InputB.CycleTime := LREAL#0.05;
InputB.Mode := PIDMode#Auto;
CommandA := ControllerA.Update(Input := InputA);
CommandB := ControllerB.Update(Input := InputB);
END_PROGRAM
"#;
    let mut runtime = TestHarness::from_session(
        CompileSession::from_sources(vec![
            SourceFile::with_path(
                "Control/control.st",
                include_str!("../../../libraries/control/src/control.st"),
            ),
            SourceFile::with_path("main.st", source),
        ])
        .label_errors(true),
    )
    .expect("compile coupled Control.PID fixture")
    .into_runtime();
    let path = temp_simulation_path();
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
seed = 42
plant_step_ms = 50

[[plants]]
id = "coupled-two-loop"
model = "state_space"
command_sources = ["%QL0", "%QL8"]
feedback_targets = ["%IL0", "%IL8"]
initial_state = [0.0, 0.0]
[plants.parameters]
a = [[-1.0, 0.2], [0.15, -1.2]]
b = [[1.0, 0.25], [0.2, 1.0]]
c = [[1.0, 0.0], [0.0, 1.0]]
d = [[0.0, 0.0], [0.0, 0.0]]
"#,
    )
    .expect("write coupled plant configuration");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path).expect("load coupled plant configuration"),
    );
    let feedback_a = IoAddress::parse("%IL0").unwrap();
    let feedback_b = IoAddress::parse("%IL8").unwrap();
    let command_a = IoAddress::parse("%QL0").unwrap();
    let command_b = IoAddress::parse("%QL8").unwrap();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let mut trace = Vec::with_capacity(800);
    for step in 1..=800 {
        runtime.execute_cycle().unwrap();
        let now = Duration::from_millis(step * 50);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        trace.push((
            step as f64 * 0.05,
            read_lreal(&runtime, &feedback_a),
            read_lreal(&runtime, &feedback_b),
            read_lreal(&runtime, &command_a),
            read_lreal(&runtime, &command_b),
        ));
    }
    let _ = std::fs::remove_file(path);
    assert!(trace.iter().all(|sample| {
        sample.1.is_finite()
            && sample.2.is_finite()
            && sample.3.is_finite()
            && sample.4.is_finite()
            && sample.1.abs() <= 5.0
            && sample.2.abs() <= 5.0
            && sample.3.abs() <= 5.0
            && sample.4.abs() <= 5.0
    }));
    let final_sample = trace.last().unwrap();
    let final_error_a = (1.0 - final_sample.1).abs();
    let final_error_b = (-0.5 - final_sample.2).abs();
    let mut final_window = trace.iter().filter(|sample| sample.0 >= 35.0);
    assert!(final_window
        .clone()
        .all(|sample| (1.0 - sample.1).abs() <= 0.02));
    assert!(final_window.all(|sample| (-0.5 - sample.2).abs() <= 0.01));
    eprintln!(
        "advanced coupled loops: final_error_a={final_error_a:.8}, final_error_b={final_error_b:.8}, final_mv_a={:.8}, final_mv_b={:.8}",
        final_sample.3, final_sample.4
    );
    assert!(final_error_a <= 0.02);
    assert!(final_error_b <= 0.01);
}
