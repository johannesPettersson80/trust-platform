use std::sync::atomic::{AtomicU64, Ordering};

use trust_hir::TypeId;
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController, SimulationTraceRow};
use trust_runtime::value::{Duration, Value};

static CASE_ID: AtomicU64 = AtomicU64::new(0);

fn temperature_trace(gain: f64, starting_temperature: f64, reset: bool) -> Vec<SimulationTraceRow> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-temperature-example-{}-{stamp}-{}.toml",
        std::process::id(),
        CASE_ID.fetch_add(1, Ordering::Relaxed),
    ));
    let event = if reset {
        "[[plant_events]]\nat_ms = 34000\nplant = \"temperature\"\nkind = \"reset_plant\""
    } else {
        ""
    };
    std::fs::write(
        &path,
        format!(
            r#"
[simulation]
enabled = true
plant_step_ms = 100
plant_trace_capacity = 1000

[[plants]]
id = "temperature"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_input = [0.0]
initial_state = [{starting_temperature}]

[plants.parameters]
gain = {gain}
time_constant_s = 30.0
dead_time_s = 2.0
input_bias = 0.0
output_bias = 20.0
initial_delayed_input = 0.0

[plants.actuator]
minimum = 0.0
maximum = 100.0
initial_applied = 0.0

{event}
"#
        ),
    )
    .unwrap();
    let config = SimulationConfig::load(&path).expect("temperature example is runtime-valid");
    std::fs::remove_file(path).unwrap();
    let mut simulation = SimulationController::new(config);
    let command = IoAddress::parse("%QL0").unwrap();
    let mut runtime = trust_runtime::harness::runtime_with_bindings(&[
        (command.clone(), TypeId::LREAL),
        (IoAddress::parse("%IL0").unwrap(), TypeId::LREAL),
    ])
    .expect("compile the plant's bindings");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    runtime
        .write_io(&command, Value::LWord(50.0_f64.to_bits()))
        .unwrap();
    let end = Duration::from_secs(if reset { 66 } else { 32 });
    simulation.apply_post_cycle(end, &runtime).unwrap();
    simulation.apply_pre_cycle(end, &mut runtime).unwrap();
    simulation.trace_snapshot().rows
}

#[test]
fn temperature_example_uses_absolute_temperature_and_exact_delayed_response() {
    // Specifications 46 §7.3 and 47 §7.1: 50% power, 2 s delay, 30 s time constant.
    for gain in [1.0, -2.0] {
        let trace = temperature_trace(gain, 20.0, false);
        assert_eq!(trace.len(), 320);
        assert!(trace.iter().take(20).all(|row| row.ideal_output == [20.0]));
        let expected = 20.0 + gain * 50.0 * (1.0 - (-1.0_f64).exp());
        let at_32 = trace.last().unwrap();
        assert!((at_32.ideal_output[0] - expected).abs() < 1e-9);
        assert_eq!(at_32.measured_feedback, at_32.ideal_output);
        assert_eq!(at_32.state, at_32.ideal_output);
        assert_eq!(at_32.applied_input, [50.0]);
    }
    let warm_start = temperature_trace(1.0, 30.0, false);
    let expected = 70.0 + (20.0 + 10.0 * (-2.0_f64 / 30.0).exp() - 70.0) * (-1.0_f64).exp();
    assert!(
        (warm_start.last().unwrap().ideal_output[0] - expected).abs() < 1e-9,
        "an explicitly warmer start is not silently reset to the equilibrium"
    );
}

#[test]
fn temperature_example_reset_restores_state_and_delay_history() {
    let trace = temperature_trace(1.0, 20.0, true);
    let reset_index = trace
        .iter()
        .position(|row| row.fired_events.contains(&"reset_plant"))
        .unwrap();
    let reset = &trace[reset_index];
    assert_eq!(reset.state, [20.0]);
    // Spec 46 §3.6: due events run before integration for their step. The reset
    // row is already the first post-reset step, not a new zero-duration sample.
    let original = temperature_trace(1.0, 20.0, false);
    for (before, after) in original.iter().zip(&trace[reset_index..]) {
        assert_eq!(
            before.state, after.state,
            "reset must replay the absolute state"
        );
        assert_eq!(
            before.ideal_output, after.ideal_output,
            "reset must replay delay history"
        );
        assert_eq!(
            before.applied_input, after.applied_input,
            "reset must replay actuator state"
        );
    }
}
