use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoAddress;
use trust_runtime::scheduler::{ResourceRunner, ResourceState, ScaledClock};
use trust_runtime::simulation::{
    SignalCouplingRule, SimulationConfig, SimulationController, SimulationDisturbance,
    SimulationDisturbanceKind,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};
use trust_runtime::Runtime;

#[test]
fn simulation_toml_model_parses_rules_and_disturbances() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-simulation-config-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create temp dir");
    let path = root.join("simulation.toml");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
seed = 99
time_scale = 6

[[couplings]]
source = "%QW0"
target = "%IX0.0"
threshold = 8.0
delay_ms = 25
on_true = "TRUE"
on_false = "FALSE"

[[disturbances]]
at_ms = 120
kind = "set"
target = "%IX0.1"
value = "TRUE"

[[disturbances]]
at_ms = 240
kind = "fault"
message = "fault-script"
"#,
    )
    .expect("write simulation.toml");

    let config = SimulationConfig::load(&path).expect("load config");
    assert!(config.enabled);
    assert_eq!(config.seed, 99);
    assert_eq!(config.time_scale, 6);
    assert_eq!(config.couplings.len(), 1);
    assert_eq!(config.disturbances.len(), 2);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn deterministic_trace_with_same_simulation_config() {
    let trace_a = run_simulation_trace();
    let trace_b = run_simulation_trace();
    assert_eq!(trace_a, trace_b);
}

#[test]
fn coupling_applies_threshold_with_delay() {
    let source = IoAddress::parse("%QW0").expect("source address");
    let target = IoAddress::parse("%IX0.1").expect("target address");
    let mut runtime = Runtime::new();
    let mut simulation = SimulationController::new(SimulationConfig {
        enabled: true,
        seed: 123,
        time_scale: 1,
        couplings: vec![SignalCouplingRule {
            source: source.clone(),
            target: target.clone(),
            threshold: Some(10.0),
            delay: Duration::from_millis(50),
            on_true: Some(Value::Bool(true)),
            on_false: Some(Value::Bool(false)),
        }],
        disturbances: Vec::new(),
    });

    runtime
        .io_mut()
        .write(&source, Value::Word(4))
        .expect("write below threshold");
    simulation
        .apply_post_cycle(Duration::from_millis(0), &runtime)
        .expect("post cycle below threshold");
    simulation
        .apply_pre_cycle(Duration::from_millis(49), &mut runtime)
        .expect("pre cycle before delay");
    assert!(!read_input_bit(&runtime, &target));

    simulation
        .apply_pre_cycle(Duration::from_millis(50), &mut runtime)
        .expect("pre cycle at delay");
    assert!(!read_input_bit(&runtime, &target));

    runtime
        .io_mut()
        .write(&source, Value::Word(20))
        .expect("write above threshold");
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .expect("post cycle above threshold");
    simulation
        .apply_pre_cycle(Duration::from_millis(149), &mut runtime)
        .expect("pre cycle before delayed true");
    assert!(!read_input_bit(&runtime, &target));

    simulation
        .apply_pre_cycle(Duration::from_millis(150), &mut runtime)
        .expect("pre cycle at delayed true");
    assert!(read_input_bit(&runtime, &target));
}

#[test]
fn scripted_fault_disturbance_faults_runtime() {
    let mut runtime = Runtime::new();
    let mut simulation = SimulationController::new(SimulationConfig {
        enabled: true,
        seed: 7,
        time_scale: 1,
        couplings: Vec::new(),
        disturbances: vec![SimulationDisturbance {
            at: Duration::from_millis(20),
            kind: SimulationDisturbanceKind::Fault {
                message: "inject-fault".into(),
            },
        }],
    });

    simulation
        .apply_pre_cycle(Duration::from_millis(19), &mut runtime)
        .expect("disturbance should not trigger yet");
    assert!(!runtime.faulted());

    let err = simulation
        .apply_pre_cycle(Duration::from_millis(20), &mut runtime)
        .expect_err("fault disturbance should trigger");
    assert!(matches!(err, RuntimeError::SimulationFault(_)));
    assert!(runtime.faulted());
}

#[test]
fn accelerated_clock_keeps_watchdog_semantics() {
    let mut runtime = Runtime::new();
    runtime.set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        timeout: Duration::from_millis(200),
        action: WatchdogAction::Halt,
    });
    let runner = ResourceRunner::new(runtime, ScaledClock::new(1000), Duration::from_millis(10));
    let mut handle = runner.spawn("simulation-watchdog").expect("spawn runner");
    std::thread::sleep(std::time::Duration::from_millis(30));
    handle.stop();
    handle.join().expect("join runner");

    assert_eq!(handle.state(), ResourceState::Stopped);
    assert!(handle.last_error().is_none());
}

fn run_simulation_trace() -> Vec<bool> {
    let source = IoAddress::parse("%QW0").expect("source address");
    let target = IoAddress::parse("%IX0.0").expect("target address");
    let mut runtime = Runtime::new();
    let mut simulation = SimulationController::new(SimulationConfig {
        enabled: true,
        seed: 42,
        time_scale: 1,
        couplings: vec![SignalCouplingRule {
            source: source.clone(),
            target: target.clone(),
            threshold: Some(8.0),
            delay: Duration::from_millis(10),
            on_true: Some(Value::Bool(true)),
            on_false: Some(Value::Bool(false)),
        }],
        disturbances: vec![SimulationDisturbance {
            at: Duration::from_millis(25),
            kind: SimulationDisturbanceKind::SetInput {
                target: target.clone(),
                value: Value::Bool(true),
            },
        }],
    });

    let output_pattern = [1u16, 12u16, 4u16, 18u16, 0u16, 25u16];
    let mut trace = Vec::new();
    for (idx, output) in output_pattern.into_iter().enumerate() {
        let now = Duration::from_millis((idx as i64) * 10);
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("pre cycle trace");
        trace.push(read_input_bit(&runtime, &target));
        runtime
            .io_mut()
            .write(&source, Value::Word(output))
            .expect("set output");
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("post cycle trace");
    }
    trace
}

fn read_input_bit(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read input bit") {
        Value::Bool(value) => value,
        other => panic!("expected bool input, got {other:?}"),
    }
}
