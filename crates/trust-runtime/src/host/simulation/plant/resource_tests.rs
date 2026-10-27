use super::*;
use crate::simulation::SimulationConfig;
use crate::value::Value;

fn integrator_dead_time_system(config: &str) -> (PlantSystem, CompiledRuntime) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-integrator-dead-time-{}-{stamp}.toml",
        std::process::id()
    ));
    std::fs::write(&path, config).unwrap();
    let loaded = SimulationConfig::load(&path);
    let _ = std::fs::remove_file(path);
    let config = loaded.expect("integrator_dead_time configuration must load");
    let runtime = super::lreal_bound_runtime(&[("command", "%QL0"), ("feedback", "%IL0")]);
    (PlantSystem::new(&config.plant_system), runtime)
}

fn write_lreal(runtime: &mut CompiledRuntime, address: &str, value: f64) {
    runtime
        .write_io(
            &IoAddress::parse(address).unwrap(),
            Value::LWord(value.to_bits()),
        )
        .unwrap();
}

fn read_lreal(runtime: &CompiledRuntime, address: &str) -> f64 {
    match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL {address}, got {other:?}"),
    }
}

const DELAYED_INTEGRATOR: &str = r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "delayed-integrator"
model = "integrator_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [2.0]
[plants.parameters]
gain = 2.0
dead_time_s = 0.15
initial_delayed_input = 3.0
input_bias = 1.0
output_bias = 3.0
"#;

#[test]
fn integrator_dead_time_splits_non_aligned_delay_at_the_exact_input_area() {
    let (mut system, mut runtime) = integrator_dead_time_system(DELAYED_INTEGRATOR);
    write_lreal(&mut runtime, "%QL0", 5.0);

    system
        .advance(Duration::from_millis(200), &runtime)
        .unwrap();
    system.publish(&mut runtime).unwrap();

    assert!((system.plants[0].state[0] - 3.0).abs() < 1e-12);
    assert!((read_lreal(&runtime, "%IL0") - 6.0).abs() < 1e-12);
}

#[test]
fn integrator_dead_time_uses_initial_history_and_reset_restores_initial_output() {
    let config = format!(
        "{}\n[[plant_events]]\nat_ms = 200\nplant = \"delayed-integrator\"\nkind = \"reset_plant\"\n",
        DELAYED_INTEGRATOR.replace("initial_delayed_input = 3.0", "initial_delayed_input = 1.0")
    );
    let (mut system, mut runtime) = integrator_dead_time_system(&config);
    write_lreal(&mut runtime, "%QL0", 5.0);

    system
        .advance(Duration::from_millis(200), &runtime)
        .unwrap();
    system.publish(&mut runtime).unwrap();

    assert!((system.plants[0].state[0] - 2.0).abs() < 1e-12);
    assert!((read_lreal(&runtime, "%IL0") - 5.0).abs() < 1e-12);
    assert_eq!(system.plants[0].input_history.len(), 1);
}

#[test]
fn integrator_dead_time_history_stays_bounded_by_its_delay_window() {
    let (mut system, mut runtime) = integrator_dead_time_system(DELAYED_INTEGRATOR);
    write_lreal(&mut runtime, "%QL0", 5.0);

    for step in 1..=10_000 {
        system
            .advance(Duration::from_millis(step * 100), &runtime)
            .unwrap();
    }

    assert!(
        system.plants[0].input_history.len() <= 5,
        "history retained {} samples for a two-step delay",
        system.plants[0].input_history.len()
    );
}

#[test]
fn integrator_dead_time_rejects_missing_or_invalid_delay_history_parameters() {
    for (field, replacement) in [
        ("initial_delayed_input", ""),
        ("initial_delayed_input", "initial_delayed_input = nan\n"),
        ("dead_time_s", "dead_time_s = -0.1\n"),
        ("dead_time_s", "dead_time_s = nan\n"),
    ] {
        let config = match field {
            "initial_delayed_input" => {
                DELAYED_INTEGRATOR.replace("initial_delayed_input = 3.0\n", replacement)
            }
            "dead_time_s" => DELAYED_INTEGRATOR.replace("dead_time_s = 0.15\n", replacement),
            _ => unreachable!(),
        };
        let result = SimulationConfig::from_toml_text(&config, "integrator-dead-time-invalid");
        assert!(result.is_err(), "{field}={replacement:?} must be rejected");
    }
}

#[test]
fn fopdt_command_history_stays_bounded_by_dead_time_window() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-plant-resource-{}-{stamp}.toml",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "bounded-delay"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
time_constant_s = 1.0
dead_time_s = 0.5
"#,
    )
    .unwrap();
    let config = SimulationConfig::load(&path).unwrap();
    let mut system = PlantSystem::new(&config.plant_system);
    let runtime = super::lreal_bound_runtime(&[("command", "%QL0"), ("feedback", "%IL0")]);

    for step in 1..=10_000 {
        system
            .advance(Duration::from_millis(step * 100), &runtime)
            .unwrap();
    }

    assert!(
        system.plants[0].input_history.len() <= 7,
        "history retained {} samples for a five-step delay",
        system.plants[0].input_history.len()
    );
    let _ = std::fs::remove_file(path);
}
