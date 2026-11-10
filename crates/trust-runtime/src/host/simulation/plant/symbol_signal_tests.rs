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
    let path = trust_test_support::unique_temp_path(&format!(
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

fn value(runtime: &CompiledRuntime, symbol: &str) -> f64 {
    match runtime.with_program_view(|program| program.value(symbol)) {
        Some(crate::value::Value::LReal(value)) => value,
        other => panic!("expected LREAL at {symbol}, got {other:?}"),
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

/// Specification 46 §13.1 and §13.2 (the CODESYS toolkit's B10): a plant is wired to a global of a
/// namespace, a CODESYS GVL member, by its qualified name, as to a program variable. Worked by hand:
/// the static gain 0.5 of the command `GVL_IO.Command`, 38.0, is 19.0 in `GVL_IO.Measurement`.
/// Catches a path whose first name had to be a global or a program instance, which refused
/// `GVL_IO.Measurement` as "program instance not found".
#[test]
fn a_global_of_a_namespace_commands_the_plant_and_receives_its_feedback() {
    let source =
        "NAMESPACE GVL_IO VAR_GLOBAL Command : LREAL := 38.0; Measurement : LREAL; END_VAR \
                  END_NAMESPACE PROGRAM Main END_PROGRAM";
    let config = load(
        "[simulation]\nenabled = true\nplant_step_ms = 100\n\n[[plants]]\nid = \"gvl\"\n\
         model = \"static_gain\"\ncommand_sources = [\"GVL_IO.Command\"]\n\
         feedback_targets = [\"GVL_IO.Measurement\"]\n[plants.parameters]\ngain = 0.5\n",
    )
    .expect("a GVL member is a valid command source and feedback target");
    let mut harness = TestHarness::from_source(source).unwrap();
    let mut system = PlantSystem::new(&config.plant_system);

    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();

    assert!((value(harness.runtime(), "GVL_IO.Measurement") - 19.0).abs() < 1e-12);
}
