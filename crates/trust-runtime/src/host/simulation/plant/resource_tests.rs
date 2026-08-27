use super::*;
use crate::simulation::SimulationConfig;
use trust_hir::TypeId;

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
    let mut runtime = Runtime::new();
    runtime
        .io_mut()
        .bind_typed("command", IoAddress::parse("%QL0").unwrap(), TypeId::LREAL);
    runtime
        .io_mut()
        .bind_typed("feedback", IoAddress::parse("%IL0").unwrap(), TypeId::LREAL);

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
