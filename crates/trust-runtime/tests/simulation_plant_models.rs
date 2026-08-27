use std::path::PathBuf;

use trust_hir::TypeId;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

fn temp_simulation_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-simulation-plant-{name}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL process-image bits");
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime
        .io()
        .read(address)
        .expect("read LREAL process image")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LWORD storage for LREAL binding, got {other:?}"),
    }
}

fn run_scalar_plant(
    model: &str,
    parameters: &str,
    initial_state: &[f64],
    input: f64,
    step_ms: i64,
) -> f64 {
    let path = temp_simulation_path(model);
    let initial = initial_state
        .iter()
        .map(f64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        &path,
        format!(
            r#"
[simulation]
enabled = true
plant_step_ms = {step_ms}

[[plants]]
id = "process"
model = "{model}"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [{initial}]

[plants.parameters]
{parameters}
"#
        ),
    )
    .expect("write scalar plant configuration");
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "model {model} must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, input);
    simulation
        .apply_post_cycle(Duration::from_millis(step_ms), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(step_ms), &mut runtime)
        .unwrap();
    let value = read_lreal(&runtime, &feedback);
    let _ = std::fs::remove_file(path);
    value
}

#[test]
fn first_order_plant_advances_through_process_image_boundaries() {
    let path = temp_simulation_path("first-order");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "process"
model = "first_order"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]

[plants.parameters]
gain = 2.0
time_constant_s = 1.0
input_bias = 0.0
output_bias = 0.0
"#,
    )
    .expect("write plant configuration");

    let config = SimulationConfig::load(&path).expect("load first-order plant");
    let mut simulation = SimulationController::new(config);
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").expect("command address");
    let feedback = IoAddress::parse("%IL0").expect("feedback address");
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);

    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial feedback");
    assert_eq!(read_lreal(&runtime, &feedback), 0.0);

    write_lreal(&mut runtime, &command, 3.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .expect("advance first-order plant");
    assert_eq!(
        read_lreal(&runtime, &feedback),
        0.0,
        "post-cycle calculation must remain queued"
    );

    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .expect("publish completed feedback");
    let expected = 6.0 * (1.0 - (-0.1_f64).exp());
    let actual = read_lreal(&runtime, &feedback);
    assert!(
        (actual - expected).abs() <= 1e-12 * expected.abs().max(1.0),
        "expected {expected}, got {actual}"
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn integrator_plant_matches_specified_update() {
    let actual = run_scalar_plant(
        "integrator",
        "gain = 2.0\ninput_bias = 1.0\noutput_bias = 3.0",
        &[4.0],
        5.0,
        250,
    );
    assert!((actual - 9.0).abs() <= 1e-12, "expected 9, got {actual}");
}

#[test]
fn static_and_piecewise_plants_match_their_maps() {
    let static_value = run_scalar_plant(
        "static_gain",
        "gain = 2.0\ninput_bias = 1.0\noutput_bias = 3.0",
        &[],
        5.0,
        100,
    );
    assert!((static_value - 11.0).abs() <= 1e-12);

    let mapped = run_scalar_plant(
        "piecewise_linear",
        "input_points = [0.0, 1.0, 2.0]\noutput_points = [0.0, 10.0, 20.0]",
        &[],
        1.5,
        100,
    );
    assert!((mapped - 15.0).abs() <= 1e-12);
}

#[test]
fn second_order_plant_uses_classical_rk4() {
    let actual = run_scalar_plant(
        "second_order",
        "gain = 1.0\nnatural_frequency_rad_s = 2.0\ndamping_ratio = 0.5\ninput_bias = 0.0\noutput_bias = 0.0",
        &[0.0, 0.0],
        1.0,
        100,
    );
    // Independent expansion of the four RK4 stages for x'' = 4 - 2*x' - 4*x.
    let expected = 0.018666666666666668;
    assert!(
        (actual - expected).abs() <= 1e-12,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn state_space_plant_matches_equivalent_second_order_model() {
    let actual = run_scalar_plant(
        "state_space",
        "a = [[0.0, 1.0], [-4.0, -2.0]]\nb = [[0.0], [4.0]]\nc = [[1.0, 0.0]]\nd = [[0.0]]",
        &[0.0, 0.0],
        1.0,
        100,
    );
    let expected = 0.018666666666666668;
    assert!(
        (actual - expected).abs() <= 1e-12,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn state_space_supports_multiple_scaled_inputs_and_outputs() {
    let path = temp_simulation_path("mimo-state-space");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "mimo"
model = "state_space"
command_sources = ["%QL0", "%QL8"]
feedback_targets = ["%IL0", "%IL8"]
initial_state = [0.0, 0.0]
input_scale = [2.0, 1.0]
input_offset = [1.0, -1.0]
output_scale = [10.0, 2.0]
output_offset = [1.0, -1.0]

[plants.parameters]
a = [[0.0, 0.0], [0.0, 0.0]]
b = [[1.0, 0.0], [0.0, 1.0]]
c = [[1.0, 0.0], [0.0, 1.0]]
d = [[0.0, 0.0], [0.0, 0.0]]
"#,
    )
    .unwrap();

    let loaded = SimulationConfig::load(&path);
    assert!(
        loaded.is_ok(),
        "MIMO state-space model must load: {loaded:?}"
    );
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command_0 = IoAddress::parse("%QL0").unwrap();
    let command_1 = IoAddress::parse("%QL8").unwrap();
    let feedback_0 = IoAddress::parse("%IL0").unwrap();
    let feedback_1 = IoAddress::parse("%IL8").unwrap();
    for (name, address) in [
        ("command_0", command_0.clone()),
        ("command_1", command_1.clone()),
        ("feedback_0", feedback_0.clone()),
        ("feedback_1", feedback_1.clone()),
    ] {
        runtime.io_mut().bind_typed(name, address, TypeId::LREAL);
    }

    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command_0, 2.0);
    write_lreal(&mut runtime, &command_1, 3.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();

    assert!((read_lreal(&runtime, &feedback_0) - 6.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &feedback_1) + 0.6).abs() <= 1e-12);
    let _ = std::fs::remove_file(path);
}

#[test]
fn integer_process_image_bindings_use_numeric_values_and_ties_to_even() {
    let path = temp_simulation_path("integer-bindings");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "integer_io"
model = "static_gain"
command_sources = ["%QD0"]
feedback_targets = ["%IW0"]

[plants.parameters]
gain = 0.5
"#,
    )
    .unwrap();
    let mut simulation =
        SimulationController::new(SimulationConfig::load(&path).expect("integer plant config"));
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QD0").unwrap();
    let feedback = IoAddress::parse("%IW0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::DINT);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::INT);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();

    for (step, command_value, expected) in [(1, 5_i32, 2_i16), (2, 7, 4), (3, -5, -2)] {
        runtime
            .io_mut()
            .write(&command, Value::DWord(command_value as u32))
            .unwrap();
        let now = Duration::from_millis(step * 100);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        assert_eq!(
            runtime.io().read(&feedback).unwrap(),
            Value::Word(expected as u16)
        );
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn multi_output_conversion_fault_preserves_the_complete_previous_feedback() {
    let path = temp_simulation_path("atomic-feedback");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "atomic"
model = "state_space"
command_sources = ["%QL0"]
feedback_targets = ["%IL0", "%IW8"]
initial_state = [0.0]

[plants.parameters]
a = [[0.0]]
b = [[0.0]]
c = [[0.0], [0.0]]
d = [[1.0], [100000.0]]
"#,
    )
    .unwrap();
    let mut simulation =
        SimulationController::new(SimulationConfig::load(&path).expect("atomic plant config"));
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback_real = IoAddress::parse("%IL0").unwrap();
    let feedback_int = IoAddress::parse("%IW8").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback_real", feedback_real.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback_int", feedback_int.clone(), TypeId::INT);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 1.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();

    let error = simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .expect_err("second feedback conversion must overflow");
    let message = error.to_string();
    assert!(message.contains("atomic"), "{message}");
    assert!(message.contains("state_space"), "{message}");
    assert!(message.contains("feedback publish"), "{message}");
    assert_eq!(read_lreal(&runtime, &feedback_real), 0.0);
    assert_eq!(runtime.io().read(&feedback_int).unwrap(), Value::Word(0));
    let _ = std::fs::remove_file(path);
}

#[test]
fn fopdt_preserves_fractional_dead_time_inside_a_fixed_step() {
    let path = temp_simulation_path("fopdt");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "process"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]

[plants.parameters]
gain = 1.0
time_constant_s = 1.0
dead_time_s = 0.15
initial_delayed_input = 0.0
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "FOPDT configuration must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 1.0);
    simulation
        .apply_post_cycle(Duration::ZERO, &runtime)
        .unwrap();
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 0.0);
    simulation
        .apply_post_cycle(Duration::from_millis(200), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(200), &mut runtime)
        .unwrap();
    let expected = 1.0 - (-0.05_f64).exp();
    let actual = read_lreal(&runtime, &feedback);
    assert!(
        (actual - expected).abs() <= 1e-12,
        "expected {expected}, got {actual}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn fopdt_delay_history_records_the_applied_process_input() {
    let path = temp_simulation_path("fopdt-actuator-input");
    std::fs::write(
        &path,
        r#"
[simulation]
plant_step_ms = 100

[[plants]]
id = "limited"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
time_constant_s = 0.1
dead_time_s = 0.0

[plants.actuator]
minimum = 0.0
maximum = 1.0
"#,
    )
    .unwrap();
    let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 10.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    let expected = 1.0 - (-1.0_f64).exp();
    assert!((read_lreal(&runtime, &feedback) - expected).abs() <= 1e-12);
    let _ = std::fs::remove_file(path);
}

#[test]
fn actuator_applies_deadband_saturation_slew_and_lag_in_order() {
    let path = temp_simulation_path("actuator");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 1000

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]

[plants.parameters]
gain = 1.0

[plants.actuator]
minimum = 0.0
maximum = 5.0
rate_up_per_s = 2.0
rate_down_per_s = 3.0
deadband = 0.1
lag_s = 0.0
initial_applied = 0.0
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(
        loaded.is_ok(),
        "actuator configuration must load: {loaded:?}"
    );
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 10.0);
    simulation
        .apply_post_cycle(Duration::from_millis(1000), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(1000), &mut runtime)
        .unwrap();
    let actual = read_lreal(&runtime, &feedback);
    assert!(
        (actual - 2.0).abs() <= 1e-12,
        "expected slew-limited 2, got {actual}"
    );
    let _ = std::fs::remove_file(path);
}

fn seeded_uniform_trace(seed: u64) -> Vec<f64> {
    let path = temp_simulation_path("seeded-uniform");
    std::fs::write(
        &path,
        format!(
            r#"
[simulation]
enabled = true
seed = {seed}
plant_step_ms = 100

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]

[plants.parameters]
gain = 0.0

[[plants.uncertainties]]
id = "sensor-noise"
target = "measurement"
channel = 0
mode = "add"
generator = "uniform_white"
minimum = -1.0
maximum = 1.0
"#
        ),
    )
    .unwrap();
    let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command, TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let mut trace = Vec::new();
    for step in 1..=4 {
        let now = Duration::from_millis(step * 100);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        trace.push(read_lreal(&runtime, &feedback));
    }
    let _ = std::fs::remove_file(path);
    trace
}

#[test]
fn seeded_uniform_uncertainty_has_exact_replayable_samples() {
    let expected = vec![
        -0.9442838356859153,
        0.20597750832386952,
        0.8459411154271397,
        -0.2235096044444369,
    ];
    let first = seeded_uniform_trace(42);
    let replay = seeded_uniform_trace(42);
    let different_seed = seeded_uniform_trace(43);
    assert_eq!(first, expected, "splitmix64-v1 known-answer trace");
    assert_eq!(replay, expected, "same seed must replay bit-for-bit");
    assert_ne!(
        different_seed, expected,
        "different seed must change samples"
    );
}

#[test]
fn stochastic_results_do_not_depend_on_elapsed_time_partitioning() {
    fn final_feedback(partitions_ms: &[i64]) -> f64 {
        let path = temp_simulation_path("partition-invariance");
        std::fs::write(
            &path,
            r#"
[simulation]
seed = 42
plant_step_ms = 100

[[plants]]
id = "partitioned"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 0.0

[[plants.uncertainties]]
id = "noise"
target = "measurement"
channel = 0
mode = "replace"
generator = "uniform_white"
minimum = -1.0
maximum = 1.0
"#,
        )
        .unwrap();
        let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
        let mut runtime = Runtime::new();
        let feedback = IoAddress::parse("%IL0").unwrap();
        runtime
            .io_mut()
            .bind_typed("command", IoAddress::parse("%QL0").unwrap(), TypeId::LREAL);
        runtime
            .io_mut()
            .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        let mut elapsed = 0;
        for partition in partitions_ms {
            elapsed += partition;
            let now = Duration::from_millis(elapsed);
            simulation.apply_post_cycle(now, &runtime).unwrap();
            simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        }
        let result = read_lreal(&runtime, &feedback);
        let _ = std::fs::remove_file(path);
        result
    }

    assert_eq!(final_feedback(&[300]), final_feedback(&[100, 100, 100]));
}

#[test]
fn status_and_bounded_trace_expose_replay_and_plant_state() {
    let path = temp_simulation_path("status-trace");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
seed = 42
time_scale = 3
plant_step_ms = 100
plant_trace_capacity = 2

[[plants]]
id = "observed"
model = "integrator"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
"#,
    )
    .unwrap();
    let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback, TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 2.0);
    for step in 1..=3 {
        let now = Duration::from_millis(step * 100);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
    }

    let status = simulation.status();
    assert!(status.active);
    assert_eq!(status.time_scale, 3);
    assert_eq!(status.effective_seed, 42);
    assert_eq!(status.random_algorithm, "splitmix64");
    assert_eq!(status.random_algorithm_version, 1);
    assert_eq!(status.restart_policy, "reset_on_cold_and_warm_restart");
    assert_eq!(status.plants.len(), 1);
    let plant = &status.plants[0];
    assert_eq!(plant.id, "observed");
    assert_eq!(plant.model, "integrator");
    assert_eq!(plant.simulated_time, Duration::from_millis(300));
    assert!((plant.state[0] - 0.6).abs() <= 1e-12);
    assert_eq!(plant.transformed_command, vec![2.0]);
    assert_eq!(plant.applied_input, vec![2.0]);
    assert!((plant.published_feedback[0] - 0.6).abs() <= 1e-12);
    assert_eq!(plant.last_substeps, 1);

    let trace = simulation.trace_snapshot();
    assert_eq!(trace.effective_seed, 42);
    assert_eq!(trace.capacity_per_plant, 2);
    assert_eq!(trace.dropped_rows, 1);
    assert_eq!(trace.rows.len(), 2);
    assert_eq!(trace.rows[0].simulation_time, Duration::from_millis(200));
    assert_eq!(trace.rows[1].simulation_time, Duration::from_millis(300));
    assert!((trace.rows[1].state[0] - 0.6).abs() <= 1e-12);
    assert!((trace.rows[1].queued_feedback[0] - 0.6).abs() <= 1e-12);
    simulation.clear_trace();
    assert!(simulation.trace_snapshot().rows.is_empty());
    assert_eq!(simulation.trace_snapshot().dropped_rows, 0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn parameter_uncertainty_changes_effective_gain_without_mutating_nominal_model() {
    let path = temp_simulation_path("gain-mismatch");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "nominal"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]

[plants.parameters]
gain = 1.0

[[plants]]
id = "mismatch"
model = "static_gain"
command_sources = ["%QL8"]
feedback_targets = ["%IL8"]

[plants.parameters]
gain = 1.0

[[plants.uncertainties]]
id = "gain-multiplier"
target = "parameter"
parameter = "gain"
mode = "multiply"
generator = "constant"
value = 1.1
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "parameter effect must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let nominal_command = IoAddress::parse("%QL0").unwrap();
    let mismatch_command = IoAddress::parse("%QL8").unwrap();
    let nominal_feedback = IoAddress::parse("%IL0").unwrap();
    let mismatch_feedback = IoAddress::parse("%IL8").unwrap();
    for (name, address) in [
        ("nominal-command", nominal_command.clone()),
        ("mismatch-command", mismatch_command.clone()),
        ("nominal-feedback", nominal_feedback.clone()),
        ("mismatch-feedback", mismatch_feedback.clone()),
    ] {
        runtime.io_mut().bind_typed(name, address, TypeId::LREAL);
    }
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &nominal_command, 2.0);
    write_lreal(&mut runtime, &mismatch_command, 2.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &nominal_feedback), 2.0);
    assert_eq!(read_lreal(&runtime, &mismatch_feedback), 2.2);
    let _ = std::fs::remove_file(path);
}

#[test]
fn scalar_and_matrix_parameters_accept_declared_effects_and_events() {
    let scalar_path = temp_simulation_path("time-constant-effect");
    std::fs::write(
        &scalar_path,
        r#"
[simulation]
plant_step_ms = 100

[[plants]]
id = "scalar"
model = "first_order"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
time_constant_s = 1.0

[[plants.uncertainties]]
id = "slower"
target = "parameter"
parameter = "time_constant_s"
mode = "multiply"
generator = "constant"
value = 2.0
"#,
    )
    .unwrap();
    let scalar = SimulationConfig::load(&scalar_path);
    assert!(scalar.is_ok(), "time-constant effect must load: {scalar:?}");
    let mut simulation = SimulationController::new(scalar.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 1.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert!((read_lreal(&runtime, &feedback) - (1.0 - (-0.05_f64).exp())).abs() <= 1e-12);

    let matrix_path = temp_simulation_path("matrix-parameter-event");
    std::fs::write(
        &matrix_path,
        r#"
[simulation]
plant_step_ms = 100

[[plants]]
id = "matrix"
model = "state_space"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
a = [[0.0]]
b = [[1.0]]
c = [[1.0]]
d = [[0.0]]

[[plant_events]]
at_ms = 100
plant = "matrix"
kind = "set_parameter"
parameter = "b[0,0]"
value = 2.0
"#,
    )
    .unwrap();
    let matrix = SimulationConfig::load(&matrix_path);
    assert!(
        matrix.is_ok(),
        "matrix parameter event must load: {matrix:?}"
    );
    let _ = std::fs::remove_file(scalar_path);
    let _ = std::fs::remove_file(matrix_path);
}

#[test]
fn model_specific_parameter_fields_are_rejected_for_the_wrong_model() {
    let path = temp_simulation_path("wrong-model-parameter");
    std::fs::write(
        &path,
        r#"
[[plants]]
id = "static"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0
time_constant_s = 2.0
"#,
    )
    .unwrap();
    let error = SimulationConfig::load(&path)
        .expect_err("a parameter owned by another model must be rejected");
    assert!(error.to_string().contains("time_constant_s"), "{error}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn plant_applies_initial_input_and_process_image_scaling_at_declared_boundaries() {
    let path = temp_simulation_path("scaling");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "scaled"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_input = [3.0]
input_scale = [2.0]
input_offset = [1.0]
output_scale = [10.0]
output_offset = [1.0]

[plants.parameters]
gain = 2.0
"#,
    )
    .unwrap();
    let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);

    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 61.0);

    write_lreal(&mut runtime, &command, 4.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 181.0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn plant_activation_rejects_duplicate_identity_and_feedback_ownership() {
    let duplicate_id = temp_simulation_path("duplicate-id");
    std::fs::write(
        &duplicate_id,
        r#"
[[plants]]
id = "same"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0

[[plants]]
id = "same"
model = "static_gain"
command_sources = ["%QL8"]
feedback_targets = ["%IL8"]
[plants.parameters]
gain = 1.0
"#,
    )
    .unwrap();
    assert!(SimulationConfig::load(&duplicate_id).is_err());

    let duplicate_target = temp_simulation_path("duplicate-target");
    std::fs::write(
        &duplicate_target,
        r#"
[[plants]]
id = "first"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0

[[plants]]
id = "second"
model = "static_gain"
command_sources = ["%QL8"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0
"#,
    )
    .unwrap();
    assert!(SimulationConfig::load(&duplicate_target).is_err());

    let disabled_owner = temp_simulation_path("disabled-owner");
    std::fs::write(
        &disabled_owner,
        r#"
[[plants]]
id = "disabled"
enabled = false
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0

[[plants]]
id = "active"
model = "static_gain"
command_sources = ["%QL8"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0
"#,
    )
    .unwrap();
    assert!(SimulationConfig::load(&disabled_owner).is_ok());

    let _ = std::fs::remove_file(duplicate_id);
    let _ = std::fs::remove_file(duplicate_target);
    let _ = std::fs::remove_file(disabled_owner);
}

#[test]
fn plant_feedback_ownership_rejects_coupling_and_physics_overlaps() {
    let coupling_path = temp_simulation_path("coupling-owner");
    std::fs::write(
        &coupling_path,
        r#"
[[couplings]]
source = "%QL8"
target = "%IW4"

[[plants]]
id = "plant"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0
"#,
    )
    .unwrap();
    let coupling_error = SimulationConfig::load(&coupling_path)
        .expect_err("overlapping coupling and plant feedback must be rejected");
    assert!(
        coupling_error.to_string().contains("conflicts"),
        "{coupling_error}"
    );

    let physics_path = temp_simulation_path("physics-owner");
    std::fs::write(
        &physics_path,
        r#"
[physics]
enabled = true

[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "%QX8.0"
feedback_target = "%IW6"

[[plants]]
id = "plant"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0
"#,
    )
    .unwrap();
    let physics_error = SimulationConfig::load(&physics_path)
        .expect_err("overlapping physics and plant feedback must be rejected");
    assert!(
        physics_error.to_string().contains("conflicts"),
        "{physics_error}"
    );
    let _ = std::fs::remove_file(coupling_path);
    let _ = std::fs::remove_file(physics_path);
}

#[test]
fn all_simulated_feedback_producers_reject_byte_range_overlaps() {
    let coupling_path = temp_simulation_path("coupling-physics-range-owner");
    std::fs::write(
        &coupling_path,
        r#"
[[couplings]]
source = "%QL0"
target = "%IB0"

[physics]
enabled = true
[[physics.joints]]
id = "axis"
kind = "revolute"
enable_source = "%QX8.0"
feedback_target = "%IW0"
"#,
    )
    .unwrap();
    let coupling_error = SimulationConfig::load(&coupling_path)
        .expect_err("coupling byte and physics word ownership overlap");
    assert!(coupling_error.to_string().contains("conflicts"));

    let physics_path = temp_simulation_path("physics-range-owner");
    std::fs::write(
        &physics_path,
        r#"
[physics]
enabled = true
[[physics.joints]]
id = "axis-a"
kind = "revolute"
enable_source = "%QX8.0"
feedback_target = "%IW0"

[[physics.joints]]
id = "axis-b"
kind = "revolute"
enable_source = "%QX8.1"
feedback_target = "%IW1"
"#,
    )
    .unwrap();
    let physics_error =
        SimulationConfig::load(&physics_path).expect_err("physics byte and word ownership overlap");
    assert!(
        physics_error.to_string().contains("conflicts"),
        "{physics_error}"
    );
    let _ = std::fs::remove_file(coupling_path);
    let _ = std::fs::remove_file(physics_path);
}

#[test]
fn measurement_effects_apply_bias_quantisation_and_clipping_in_declaration_order() {
    let path = temp_simulation_path("sensor-pipeline");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "sensor"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0

[[plants.uncertainties]]
id = "bias"
target = "measurement"
channel = 0
mode = "add"
generator = "constant"
value = 0.1

[[plants.uncertainties]]
id = "quantiser"
target = "measurement"
channel = 0
mode = "replace"
generator = "quantise"
quantum = 0.25

[[plants.uncertainties]]
id = "clipper"
target = "measurement"
channel = 0
mode = "replace"
generator = "clip"
minimum = 0.0
maximum = 0.2
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "sensor pipeline must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 0.26);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 0.2);
    let _ = std::fs::remove_file(path);
}

#[test]
fn colored_noise_and_bounded_random_walk_follow_exact_seeded_oracle() {
    let path = temp_simulation_path("imperfect-sensor");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
seed = 42
plant_step_ms = 100

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 0.0

[[plants.uncertainties]]
id = "normal"
target = "measurement"
channel = 0
mode = "add"
generator = "normal_approx_white"
mean = 0.2
standard_deviation = 0.1

[[plants.uncertainties]]
id = "colored"
target = "measurement"
channel = 0
mode = "add"
generator = "colored_noise"
standard_deviation = 0.05
correlation_time_s = 0.5

[[plants.uncertainties]]
id = "walk"
target = "measurement"
channel = 0
mode = "add"
generator = "random_walk"
diffusion_per_sqrt_s = 0.01
minimum = -0.02
maximum = 0.02

[[plants.uncertainties]]
id = "quantiser"
target = "measurement"
channel = 0
mode = "replace"
generator = "quantise"
quantum = 0.001
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "imperfect sensor must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command, TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let mut trace = Vec::new();
    for step in 1..=3 {
        let now = Duration::from_millis(step * 100);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        trace.push(read_lreal(&runtime, &feedback));
    }
    assert_eq!(trace, vec![0.085, 0.358, 0.335]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn hold_last_dropout_preserves_the_last_valid_nonzero_measurement() {
    let path = temp_simulation_path("dropout");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
seed = 7
plant_step_ms = 100

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_input = [5.0]
[plants.parameters]
gain = 1.0

[[plants.uncertainties]]
id = "radio-loss"
target = "measurement"
channel = 0
mode = "replace"
generator = "dropout"
rate_per_s = 1000000000.0
duration_s = 1.0
dropout_policy = "hold_last"
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "dropout must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 5.0);
    write_lreal(&mut runtime, &command, 10.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 5.0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn failed_substep_retains_the_last_complete_plant_state_and_feedback() {
    let path = temp_simulation_path("fault-rollback");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
seed = 7
plant_step_ms = 100
plant_trace_capacity = 4

[[plants]]
id = "process"
model = "integrator"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0

[[plants.uncertainties]]
id = "failed-sensor"
target = "measurement"
channel = 0
mode = "replace"
generator = "dropout"
rate_per_s = 1000000000.0
duration_s = 1.0
dropout_policy = "fault"
"#,
    )
    .unwrap();
    let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 1.0);

    let error = simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .expect_err("fault-policy dropout must reject the substep");
    assert!(error.to_string().contains("failed-sensor"));
    let plant = &simulation.status().plants[0];
    assert_eq!(plant.simulated_time, Duration::ZERO);
    assert_eq!(plant.state, vec![0.0]);
    assert_eq!(plant.published_feedback, vec![0.0]);
    assert!(plant.last_fault.is_some());
    assert!(simulation.trace_snapshot().rows.is_empty());
    assert_eq!(read_lreal(&runtime, &feedback), 0.0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn catchup_limit_fault_does_not_advance_or_publish_the_plant() {
    let path = temp_simulation_path("catchup-limit");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100
max_plant_substeps_per_cycle = 2

[[plants]]
id = "process"
model = "integrator"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
"#,
    )
    .unwrap();
    let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 1.0);

    let error = simulation
        .apply_post_cycle(Duration::from_millis(300), &runtime)
        .expect_err("three requested steps must exceed the two-step limit");
    assert!(error.to_string().contains("substep limit"));
    let plant = &simulation.status().plants[0];
    assert_eq!(plant.simulated_time, Duration::ZERO);
    assert_eq!(plant.state, vec![0.0]);
    assert_eq!(plant.published_feedback, vec![0.0]);
    assert_eq!(read_lreal(&runtime, &feedback), 0.0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn reset_event_clears_persistent_parameter_and_sensor_changes_without_replay() {
    let path = temp_simulation_path("reset-event");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0

[[plant_events]]
at_ms = 100
plant = "process"
kind = "set_parameter"
parameter = "gain"
value = 2.0

[[plant_events]]
at_ms = 100
plant = "process"
kind = "set_sensor_bias"
channel = 0
value = 1.0

[[plant_events]]
at_ms = 200
plant = "process"
kind = "reset_plant"
"#,
    )
    .unwrap();
    let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 3.0);

    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 7.0);
    simulation
        .apply_post_cycle(Duration::from_millis(200), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(200), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 3.0);
    simulation
        .apply_post_cycle(Duration::from_millis(300), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(300), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 3.0);
    assert_eq!(simulation.status().plants[0].pending_events, 0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn scheduled_state_event_executes_before_integration_at_due_boundary() {
    let path = temp_simulation_path("state-event");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "process"
model = "integrator"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [1.0]
[plants.parameters]
gain = 0.0

[[plant_events]]
at_ms = 100
plant = "process"
kind = "add_state"
channel = 0
value = 2.0
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "state event must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command, TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 1.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 3.0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn scheduled_actuator_parameter_and_sensor_events_are_persistent_and_ordered() {
    let path = temp_simulation_path("device-events");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0

[[plant_events]]
at_ms = 100
plant = "process"
kind = "stick_actuator"

[[plant_events]]
at_ms = 200
plant = "process"
kind = "release_actuator"

[[plant_events]]
at_ms = 300
plant = "process"
kind = "set_sensor_bias"
channel = 0
value = 1.0

[[plant_events]]
at_ms = 400
plant = "process"
kind = "set_parameter"
parameter = "gain"
value = 2.0
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "device events must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 4.0);
    let mut trace = Vec::new();
    for step in 1..=4 {
        let now = Duration::from_millis(step * 100);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        trace.push(read_lreal(&runtime, &feedback));
    }
    assert_eq!(trace, vec![0.0, 4.0, 5.0, 9.0]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn control_pid_closes_the_specified_loop_against_fopdt_plant() {
    let controller_source = r#"
USING Control;

PROGRAM Main
VAR
    Controller : PID;
    Parameters : PIDParameters;
    State : PIDInitialState;
    Input : PIDInput;
    ProcessValue AT %IL0 : LREAL;
    Command AT %QL0 : LREAL;
    Initialized : BOOL;
END_VAR

IF NOT Initialized THEN
    Parameters.ProportionalGain := LREAL#0.667;
    Parameters.IntegralGain := LREAL#0.667;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#-1.0e100;
    Parameters.OutputMaximum := LREAL#1.0e100;
    Parameters.RateMinimum := LREAL#-1.0e100;
    Parameters.RateMaximum := LREAL#1.0e100;
    Parameters.TrackingTime := LREAL#1.0;
    Controller.Configure(Parameters := Parameters);
    Controller.Initialize(State := State);
    Initialized := TRUE;
END_IF;

Input.Setpoint := LREAL#1.0;
Input.ProcessValue := ProcessValue;
Input.CycleTime := LREAL#0.1;
Input.Mode := PIDMode#Auto;
Command := Controller.Update(Input := Input);
END_PROGRAM
"#;
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("main.st", controller_source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile Control.PID closed-loop fixture");

    let path = temp_simulation_path("pid-closed-loop");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "paper-fopdt"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
time_constant_s = 1.0
dead_time_s = 0.5
initial_delayed_input = 0.0
"#,
    )
    .unwrap();
    let mut simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    let feedback = IoAddress::parse("%IL0").unwrap();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();

    let mut trace = Vec::new();
    let mut iae = 0.0;
    for step in 1..=200 {
        runtime.execute_cycle().expect("execute Control.PID cycle");
        let now = Duration::from_millis(step * 100);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        let process_value = read_lreal(&runtime, &feedback);
        trace.push(process_value);
        iae += 0.1 * (1.0 - process_value).abs();
    }

    for (step, expected) in [
        (10, 0.3464035355654934),
        (20, 0.8142074414846863),
        (30, 0.9490966777492305),
        (50, 0.9924202641525338),
        (100, 0.9998681292138564),
    ] {
        let actual = trace[step - 1];
        assert!((actual - expected).abs() <= 1e-12, "step {step}: {actual}");
    }
    assert!(trace
        .iter()
        .all(|value| (0.0..=1.000000000001).contains(value)));
    assert!(trace[38..].iter().all(|value| (1.0 - value).abs() <= 0.02));
    assert!((1.0 - trace[199]).abs() <= 1e-7);
    assert!(iae <= 1.4, "IAE was {iae}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn random_seed_selection_exposes_a_concrete_replay_seed() {
    let path = temp_simulation_path("random-seed");
    std::fs::write(
        &path,
        r#"
[simulation]
seed = "random"

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0
"#,
    )
    .unwrap();
    let selected = SimulationConfig::load(&path);
    assert!(
        selected.is_ok(),
        "random seed selection must load: {selected:?}"
    );
    let effective_seed = selected.unwrap().seed;

    let replay_path = temp_simulation_path("replay-seed");
    std::fs::write(
        &replay_path,
        format!(
            r#"
[simulation]
seed = {effective_seed}

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0
"#
        ),
    )
    .unwrap();
    assert_eq!(
        SimulationConfig::load(&replay_path).unwrap().seed,
        effective_seed
    );
    assert_eq!(
        seeded_uniform_trace(effective_seed),
        seeded_uniform_trace(effective_seed)
    );
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(replay_path);
}

#[test]
fn uncertainty_targets_execute_at_their_declared_pipeline_boundaries() {
    let path = temp_simulation_path("uncertainty-order");
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 2.0

[[plants.uncertainties]]
id = "command-add"
target = "command"
channel = 0
mode = "add"
generator = "constant"
value = 1.0

[[plants.uncertainties]]
id = "input-multiply"
target = "process_input"
channel = 0
mode = "multiply"
generator = "constant"
value = 2.0

[[plants.uncertainties]]
id = "output-add"
target = "process_output"
channel = 0
mode = "add"
generator = "constant"
value = 3.0

[[plants.uncertainties]]
id = "measurement-add"
target = "measurement"
channel = 0
mode = "add"
generator = "constant"
value = 4.0
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "uncertainty pipeline must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &command, 1.0);
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 15.0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn hold_mode_freezes_the_last_value_accepted_before_its_active_window() {
    let path = temp_simulation_path("hold-mode");
    std::fs::write(
        &path,
        r#"
[simulation]
plant_step_ms = 100

[[plants]]
id = "hold"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 1.0

[[plants.uncertainties]]
id = "freeze-output"
target = "process_output"
channel = 0
mode = "hold"
generator = "constant"
value = 999.0
start_s = 0.2
end_s = 0.4
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "hold effect must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command.clone(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let mut trace = Vec::new();
    for (step, value) in [(1, 1.0), (2, 2.0), (3, 3.0), (4, 4.0)] {
        write_lreal(&mut runtime, &command, value);
        let now = Duration::from_millis(step * 100);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        trace.push(read_lreal(&runtime, &feedback));
    }
    assert_eq!(trace, vec![1.0, 1.0, 1.0, 4.0]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn plant_configuration_rejects_unknown_fields_instead_of_ignoring_them() {
    let path = temp_simulation_path("unknown-field");
    std::fs::write(
        &path,
        r#"
[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_stat = [0.0]
[plants.parameters]
gain = 1.0
"#,
    )
    .unwrap();
    let error = SimulationConfig::load(&path).expect_err("unknown plant field must reject config");
    assert!(error.to_string().contains("unknown field"), "{error}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn state_uncertainty_applies_after_nominal_integration() {
    let path = temp_simulation_path("state-uncertainty");
    std::fs::write(
        &path,
        r#"
[simulation]
plant_step_ms = 100

[[plants]]
id = "process"
model = "integrator"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [1.0]
[plants.parameters]
gain = 0.0

[[plants.uncertainties]]
id = "state-step"
target = "state"
channel = 0
mode = "add"
generator = "constant"
value = 2.0
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "state uncertainty must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%QL0").unwrap();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", command, TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    simulation
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .unwrap();
    assert_eq!(read_lreal(&runtime, &feedback), 3.0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pulse_and_sine_generators_use_absolute_simulation_time() {
    let path = temp_simulation_path("analytic-effects");
    std::fs::write(
        &path,
        r#"
[simulation]
plant_step_ms = 100

[[plants]]
id = "process"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 0.0

[[plants.uncertainties]]
id = "pulse"
target = "measurement"
channel = 0
mode = "add"
generator = "pulse"
value = 4.0
duration_s = 0.5

[[plants.uncertainties]]
id = "sine"
target = "measurement"
channel = 0
mode = "add"
generator = "sine"
amplitude = 2.0
frequency_hz = 0.25
phase_rad = 0.0
bias = 1.0
"#,
    )
    .unwrap();
    let loaded = SimulationConfig::load(&path);
    assert!(loaded.is_ok(), "analytic effects must load: {loaded:?}");
    let mut simulation = SimulationController::new(loaded.unwrap());
    let mut runtime = Runtime::new();
    let feedback = IoAddress::parse("%IL0").unwrap();
    runtime
        .io_mut()
        .bind_typed("command", IoAddress::parse("%QL0").unwrap(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", feedback.clone(), TypeId::LREAL);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let mut trace = Vec::new();
    for step in 1..=5 {
        let now = Duration::from_millis(step * 100);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        trace.push(read_lreal(&runtime, &feedback));
    }
    let at_point_one = 5.0 + 2.0 * (0.05 * std::f64::consts::PI).sin();
    assert!((trace[0] - at_point_one).abs() <= 1e-12);
    let at_point_five = 1.0 + std::f64::consts::SQRT_2;
    assert!((trace[4] - at_point_five).abs() <= 1e-12);
    let _ = std::fs::remove_file(path);
}
