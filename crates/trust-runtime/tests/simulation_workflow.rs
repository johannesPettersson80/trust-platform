use serde_json::json;
use sha2::{Digest, Sha256};
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoAddress;
use trust_runtime::scheduler::{ResourceRunner, ResourceState, ScaledClock};
use trust_runtime::simulation::{
    PhysicsBackend, PhysicsConfig, PhysicsJointConfig, PhysicsJointKind, SignalCouplingRule,
    SimulationConfig, SimulationController, SimulationDisturbance, SimulationDisturbanceKind,
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
fn simulation_toml_model_parses_physics_joints() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-simulation-physics-config-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create temp dir");
    let path = root.join("simulation.toml");
    std::fs::write(
        &path,
        r#"
[simulation]
seed = 99

[physics]
enabled = true
backend = "in_tree_rapier"
step_ms = 10
encoder_counts_per_radian = 1000.0

[[physics.joints]]
id = "axis-1"
kind = "revolute"
enable_source = "%QX0.0"
feedback_target = "%IW0"
velocity_rad_per_s = 1.0
lower_rad = 0.0
upper_rad = 1.570796
"#,
    )
    .expect("write simulation.toml");

    let config = SimulationConfig::load(&path).expect("load config");
    let physics = config.physics.expect("physics config");
    assert!(config.enabled);
    assert!(physics.enabled);
    assert_eq!(physics.backend, PhysicsBackend::InTreeRapier);
    assert_eq!(physics.step, Duration::from_millis(10));
    assert_eq!(physics.joints.len(), 1);
    assert_eq!(physics.joints[0].kind, PhysicsJointKind::Revolute);
    assert_eq!(
        physics.joints[0].enable_source,
        IoAddress::parse("%QX0.0").unwrap()
    );
    assert_eq!(
        physics.joints[0].feedback_target,
        IoAddress::parse("%IW0").unwrap()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn physics_feedback_target_conflicts_are_rejected() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-simulation-physics-conflict-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create temp dir");
    let path = root.join("simulation.toml");
    std::fs::write(
        &path,
        r#"
[physics]
enabled = true

[[physics.joints]]
id = "axis-1"
kind = "revolute"
enable_source = "%QX0.0"
feedback_target = "%IW0"

[[couplings]]
source = "%QW2"
target = "%IW0"
"#,
    )
    .expect("write simulation.toml");

    let err = SimulationConfig::load(&path).expect_err("duplicate physics target must fail");
    let text = err.to_string();
    assert!(
        text.contains("physics feedback target %IW0 conflicts"),
        "unexpected error: {text}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn physics_revolute_joint_queues_encoder_feedback_through_io_boundary() {
    let enable = IoAddress::parse("%QX0.0").expect("enable address");
    let feedback = IoAddress::parse("%IW0").expect("feedback address");
    let mut runtime = Runtime::new();
    let mut simulation = SimulationController::new(SimulationConfig {
        enabled: true,
        seed: 1,
        time_scale: 1,
        couplings: Vec::new(),
        disturbances: Vec::new(),
        physics: Some(PhysicsConfig {
            enabled: true,
            backend: PhysicsBackend::InTreeRapier,
            step: Duration::from_millis(10),
            encoder_counts_per_radian: 1000.0,
            joints: vec![PhysicsJointConfig {
                id: "axis-1".into(),
                kind: PhysicsJointKind::Revolute,
                enable_source: enable.clone(),
                feedback_target: feedback.clone(),
                velocity_rad_per_s: 1.0,
                lower_rad: 0.0,
                upper_rad: std::f64::consts::FRAC_PI_2,
                encoder_counts_per_radian: 1000.0,
            }],
        }),
    });

    runtime
        .io_mut()
        .write(&enable, Value::Bool(true))
        .expect("write enable");
    simulation
        .apply_post_cycle(Duration::from_millis(0), &runtime)
        .expect("physics post cycle");
    assert_eq!(
        runtime
            .io()
            .read(&feedback)
            .expect("feedback before pre-cycle"),
        Value::Word(0),
        "physics output must be queued, not written during post-cycle"
    );

    simulation
        .apply_pre_cycle(Duration::from_millis(10), &mut runtime)
        .expect("physics pre cycle applies pending encoder");
    assert_eq!(
        runtime
            .io()
            .read(&feedback)
            .expect("feedback after pre-cycle"),
        Value::Word(10)
    );
}

#[test]
fn physics_revolute_trace_is_deterministic_for_same_seed() {
    let trace_a = run_physics_trace();
    let trace_b = run_physics_trace();
    assert_eq!(trace_a, trace_b);
    assert_eq!(trace_a.len(), 1000);
    assert_eq!(trace_a[0], 10);
    assert_eq!(trace_a[999], 1571);
    let trace_hash = hash_u16_trace(&trace_a);
    assert_eq!(
        trace_hash,
        "a9ef39925272a3c450aae99624c072ed2ad5799210ecb6c0166d818b92461c5c"
    );
    write_trust_twin_p2_artifact(&trace_hash, trace_a[0], trace_a[999])
        .expect("write trust-twin P2 gate artifact");
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
        physics: None,
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
        physics: None,
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
        physics: None,
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

fn run_physics_trace() -> Vec<u16> {
    let enable = IoAddress::parse("%QX0.0").expect("enable address");
    let feedback = IoAddress::parse("%IW0").expect("feedback address");
    let mut runtime = Runtime::new();
    let mut simulation = SimulationController::new(SimulationConfig {
        enabled: true,
        seed: 42,
        time_scale: 1,
        couplings: Vec::new(),
        disturbances: Vec::new(),
        physics: Some(PhysicsConfig {
            enabled: true,
            backend: PhysicsBackend::InTreeRapier,
            step: Duration::from_millis(10),
            encoder_counts_per_radian: 1000.0,
            joints: vec![PhysicsJointConfig {
                id: "axis-1".into(),
                kind: PhysicsJointKind::Revolute,
                enable_source: enable.clone(),
                feedback_target: feedback.clone(),
                velocity_rad_per_s: 1.0,
                lower_rad: 0.0,
                upper_rad: std::f64::consts::FRAC_PI_2,
                encoder_counts_per_radian: 1000.0,
            }],
        }),
    });
    runtime
        .io_mut()
        .write(&enable, Value::Bool(true))
        .expect("write enable");

    let mut trace = Vec::with_capacity(1000);
    for tick in 0..1000 {
        let now = Duration::from_millis((tick as i64) * 10);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("physics post cycle");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("physics pre cycle");
        match runtime.io().read(&feedback).expect("read feedback") {
            Value::Word(value) => trace.push(value),
            other => panic!("expected word feedback, got {other:?}"),
        }
    }
    trace
}

fn hash_u16_trace(trace: &[u16]) -> String {
    let mut hasher = Sha256::new();
    for value in trace {
        hasher.update(value.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn write_trust_twin_p2_artifact(
    trace_hash: &str,
    first_encoder: u16,
    final_encoder: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let artifact = json!({
        "phase": "P2",
        "driver": "SimulationController",
        "physics": {
            "backend": "in_tree_rapier",
            "deterministic_single_threaded": true,
            "step_ms": 10,
            "joint": {
                "id": "axis-1",
                "kind": "revolute",
                "enable_source": "%QX0.0",
                "feedback_target": "%IW0",
                "velocity_rad_per_s": 1.0,
                "lower_rad": 0.0,
                "upper_rad": std::f64::consts::FRAC_PI_2,
                "encoder_counts_per_radian": 1000.0
            }
        },
        "acceptance": {
            "plc_observes_encoder_value": true,
            "queued_feedback_via_simulation_controller": true,
            "no_direct_feedback_write_in_post_cycle": true,
            "sidecar_decision": "in_tree_rapier_for_p2; sidecar_split_future_work"
        },
        "trace": {
            "ticks": 1000,
            "first_encoder": first_encoder,
            "final_encoder": final_encoder,
            "sha256_le_u16": trace_hash
        },
        "preempt_rt": {
            "p2_hosting": "in_tree_no_sidecar",
            "validation_lane": "scripts/runtime_preempt_rt_validate.sh baseline evidence; sidecar-active leg is future work because no P2 sidecar exists"
        },
        "evidence_blockers": []
    });

    let path = workspace_root()
        .join("target")
        .join("gate-artifacts")
        .join("trust-twin-p2-physics.json");
    let parent = path
        .parent()
        .ok_or_else(|| format!("artifact path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&artifact)?),
    )?;
    Ok(())
}

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_input_bit(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read input bit") {
        Value::Bool(value) => value,
        other => panic!("expected bool input, got {other:?}"),
    }
}
