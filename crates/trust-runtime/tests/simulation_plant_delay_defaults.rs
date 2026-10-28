use trust_hir::TypeId;
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};

fn delayed_output(initial_history: Option<f64>) -> f64 {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let config_path = std::env::temp_dir().join(format!(
        "trust-plant-delay-default-{}-{stamp}.toml",
        std::process::id()
    ));
    let history = initial_history
        .map(|value| format!("initial_delayed_input = {value}"))
        .unwrap_or_default();
    std::fs::write(
        &config_path,
        format!(
            r#"
[simulation]
enabled = true
plant_step_ms = 100
plant_trace_capacity = 16

[[plants]]
id = "biased-process"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [20.0]

[plants.parameters]
gain = 1.0
time_constant_s = 1.0
dead_time_s = 10.0
input_bias = 10.0
output_bias = 20.0
{history}
"#
        ),
    )
    .unwrap();
    let config = SimulationConfig::load(&config_path).expect("valid biased FOPDT model");
    std::fs::remove_file(config_path).unwrap();
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
        .write_io(&command, Value::LWord(10.0_f64.to_bits()))
        .unwrap();
    simulation
        .apply_post_cycle(Duration::from_secs(1), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_secs(1), &mut runtime)
        .unwrap();
    let trace = simulation.trace_snapshot();
    assert_eq!(trace.rows.len(), 10);
    trace.rows.last().unwrap().ideal_output[0]
}

#[test]
fn omitted_delay_history_uses_the_input_operating_point_but_explicit_zero_is_retained() {
    // Specification 46 §7.3: the unknown past input defaults to input_bias.
    assert!(
        (delayed_output(None) - 20.0).abs() < 1e-12,
        "an omitted delay history must preserve the biased equilibrium"
    );
    // An explicitly specified zero is a deliberate non-equilibrium start.
    let explicit_zero = 10.0 + 10.0 * (-1.0_f64).exp();
    assert!((delayed_output(Some(0.0)) - explicit_zero).abs() < 1e-12);
    assert!((delayed_output(Some(10.0)) - 20.0).abs() < 1e-12);
}
