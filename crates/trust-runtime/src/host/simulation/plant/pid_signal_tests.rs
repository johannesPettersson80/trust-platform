//! Control.PID measurement targets (Specification 33 §13.3): `pid:<instance>` gives the plant's
//! measurement to that controller's `Update` without writing the program's own variables.
use super::*;
use crate::harness::TestHarness;
use crate::simulation::SimulationConfig;

const CONTROL_LIBRARY: &str = include_str!("../../../../../../libraries/control/src/control.st");

const PROGRAM: &str = r#"
PROGRAM Main
VAR
    Command : LREAL := 84.0;
    Controller : Control.PID;
    Other : Control.PID;
    Settings : Control.PIDParameters := (ProportionalGain := 1.0, OutputMaximum := 100.0,
        RateMinimum := -1.0E9, RateMaximum := 1.0E9, TrackingTime := 1.0);
    Input : Control.PIDInput;
    OtherInput : Control.PIDInput;
    Output : LREAL;
    OtherOutput : LREAL;
    Timer : TON;
END_VAR
    Controller.Configure(Parameters := Settings);
    Other.Configure(Parameters := Settings);
    Input.Setpoint := 50.0;
    Input.ProcessValue := 1.0;
    Input.CycleTime := 0.1;
    OtherInput.Setpoint := 50.0;
    OtherInput.ProcessValue := 3.0;
    OtherInput.CycleTime := 0.1;
    Output := Controller.Update(Input := Input);
    OtherOutput := Other.Update(Input := OtherInput);
END_PROGRAM
"#;

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

fn harness() -> TestHarness {
    TestHarness::from_sources(&[CONTROL_LIBRARY, PROGRAM])
        .expect("the program compiles with the control library")
}

#[test]
fn the_plant_gives_its_measurement_to_the_named_controller_only() {
    let config = SimulationConfig::from_toml_text(&plants(&["pid:Main.Controller"]), "pid test")
        .expect("a Control.PID measurement is a valid feedback target");
    let mut harness = harness();
    let mut system = PlantSystem::new(&config.plant_system);

    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        1.0,
        "without a published measurement the controller sees the program's value"
    );

    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        42.0,
        "the controller's Update receives the plant's measurement"
    );
    assert_eq!(
        value(harness.runtime(), "Main.Input.ProcessValue"),
        1.0,
        "the program's own variable keeps what the program wrote"
    );
    assert_eq!(
        value(harness.runtime(), "Main.Other.StateValue.ProcessValue"),
        3.0,
        "another controller is not touched"
    );

    let target =
        signal::PlantSignal::parse("pid:Main.Controller", &[IoArea::Input], "feedback").unwrap();
    assert_eq!(target.read(harness.runtime()).unwrap(), 42.0);

    harness
        .runtime_mut()
        .restart(crate::RestartMode::Cold)
        .expect("the program restarts");
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        1.0,
        "a restart removes the measurement until the plant publishes again"
    );
}

#[test]
fn a_controller_measurement_is_only_a_feedback_target_of_one_plant() {
    let command = "[simulation]\nenabled = true\nplant_step_ms = 100\n\n[[plants]]\nid = \"plant\"\nmodel = \"static_gain\"\ncommand_sources = [\"pid:Main.Controller\"]\nfeedback_targets = [\"Main.Output\"]\n[plants.parameters]\ngain = 0.5\n";
    let error = SimulationConfig::from_toml_text(command, "pid test")
        .expect_err("a controller's measurement is not a command source");
    assert!(
        error
            .to_string()
            .contains("cannot be a Control.PID measurement"),
        "{error}"
    );
    let error = SimulationConfig::from_toml_text(
        &plants(&["pid:Main.Controller", "pid:main.controller"]),
        "pid test",
    )
    .expect_err("a controller's measurement has one plant");
    assert!(error.to_string().contains("already owned"), "{error}");
    let error = SimulationConfig::from_toml_text(&plants(&["pid:Main..Controller"]), "pid test")
        .expect_err("the instance path is checked");
    assert!(error.to_string().contains("Control.PID"), "{error}");
}

#[test]
fn a_measurement_target_that_names_no_control_pid_faults_the_plant_with_its_name() {
    for (target, reason) in [
        ("pid:Main.Timer", "is not an instance of Control.PID"),
        ("pid:Main.Command", "names no function block instance"),
        ("pid:Main.Missing", "pid:Main.Missing"),
    ] {
        let config = SimulationConfig::from_toml_text(&plants(&[target]), "pid test").unwrap();
        let mut harness = harness();
        let mut system = PlantSystem::new(&config.plant_system);
        system
            .advance(Duration::from_millis(100), harness.runtime())
            .unwrap();
        let error = system
            .publish(harness.runtime_mut())
            .expect_err("the target is not a Control.PID");
        assert!(error.to_string().contains(reason), "{target}: {error}");
    }
}
