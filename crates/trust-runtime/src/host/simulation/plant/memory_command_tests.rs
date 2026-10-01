//! Memory command sources (Specification 33 §13.1): a plant may take a program variable as its input.
use super::*;
use crate::simulation::SimulationConfig;
use crate::value::Value;
use trust_hir::TypeId;

const MEMORY_COMMANDED: &str = r#"
[simulation]
enabled = true
plant_step_ms = 100

[[plants]]
id = "inner-loop"
model = "static_gain"
command_sources = ["%ML8"]
feedback_targets = ["%IL8"]
[plants.parameters]
gain = 0.5
"#;

fn load(config: &str) -> Result<SimulationConfig, RuntimeError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-memory-command-{}-{stamp}.toml",
        std::process::id()
    ));
    std::fs::write(&path, config).unwrap();
    let loaded = SimulationConfig::load(&path);
    let _ = std::fs::remove_file(path);
    loaded
}

#[test]
fn a_memory_command_source_feeds_the_plant_from_the_program_variable() {
    let config = load(MEMORY_COMMANDED).expect("a %M command source is a valid plant input");
    let mut runtime = Runtime::new();
    let command = IoAddress::parse("%ML8").unwrap();
    runtime
        .io_mut()
        .bind_typed("setpoint", command.clone(), TypeId::LREAL);
    runtime.io_mut().bind_typed(
        "measurement",
        IoAddress::parse("%IL8").unwrap(),
        TypeId::LREAL,
    );
    runtime
        .io_mut()
        .write(&command, Value::LWord(38.0f64.to_bits()))
        .unwrap();
    let mut system = PlantSystem::new(&config.plant_system);

    system
        .advance(Duration::from_millis(100), &runtime)
        .unwrap();
    system.publish(&mut runtime).unwrap();

    match runtime
        .io()
        .read(&IoAddress::parse("%IL8").unwrap())
        .unwrap()
    {
        Value::LWord(bits) => assert!((f64::from_bits(bits) - 19.0).abs() < 1e-12),
        other => panic!("expected LREAL feedback, got {other:?}"),
    }
}

#[test]
fn a_command_source_outside_outputs_and_memory_is_refused() {
    let error = load(&MEMORY_COMMANDED.replace("%ML8", "%IL0"))
        .expect_err("an input cannot command a plant");
    assert!(
        error
            .to_string()
            .contains("plant command address has wrong area: %IL0"),
        "{error}"
    );
}
