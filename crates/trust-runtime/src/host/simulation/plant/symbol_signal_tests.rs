//! Symbolic plant signals (Specification 46 §13.1–§13.3): program variables without an address.
use super::*;
use crate::harness::TestHarness;
use crate::simulation::SimulationConfig;

const PROGRAM: &str =
    "PROGRAM Main VAR Command : LREAL := 38.0; Measurement : LREAL; END_VAR END_PROGRAM";

fn load(config: &str) -> Result<SimulationConfig, RuntimeError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-symbol-signal-{}-{stamp}.toml",
        std::process::id()
    ));
    std::fs::write(&path, config).unwrap();
    let loaded = SimulationConfig::load(&path);
    let _ = std::fs::remove_file(path);
    loaded
}

fn plants(targets: &[&str]) -> String {
    let mut config = "[simulation]\nenabled = true\nplant_step_ms = 100\n".to_string();
    for (index, target) in targets.iter().enumerate() {
        config.push_str(&format!(
            "\n[[plants]]\nid = \"plant-{index}\"\nmodel = \"static_gain\"\ncommand_sources = [\"Main.Command\"]\nfeedback_targets = [\"{target}\"]\n[plants.parameters]\ngain = 0.5\n"
        ));
    }
    config
}

fn value(runtime: &Runtime, symbol: &str) -> f64 {
    match crate::debug::resolve_control_parameter_path(runtime.storage(), symbol)
        .unwrap()
        .value
    {
        crate::value::Value::LReal(value) => value,
        other => panic!("expected LREAL, got {other:?}"),
    }
}

#[test]
fn a_program_variable_without_an_address_commands_the_plant_and_receives_its_feedback() {
    let config = load(&plants(&["Main.Measurement"]))
        .expect("an unaddressed program variable is a valid command source and feedback target");
    let mut harness = TestHarness::from_source(PROGRAM).unwrap();
    let mut system = PlantSystem::new(&config.plant_system);

    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();

    assert!((value(harness.runtime(), "Main.Measurement") - 19.0).abs() < 1e-12);
}

#[test]
fn two_plants_cannot_publish_to_the_same_program_variable() {
    let error = load(&plants(&["Main.Measurement", "main.measurement"]))
        .expect_err("a program variable has one feedback owner");
    assert!(error.to_string().contains("already owned"), "{error}");
}

#[test]
fn an_unknown_program_variable_faults_the_plant_with_its_name() {
    let config = load(&plants(&["Main.Missing"])).unwrap();
    let mut harness = TestHarness::from_source(PROGRAM).unwrap();
    let mut system = PlantSystem::new(&config.plant_system);
    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    let error = system
        .publish(harness.runtime_mut())
        .expect_err("the target does not exist");
    assert!(error.to_string().contains("Main.Missing"), "{error}");
}
