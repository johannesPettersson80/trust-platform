//! Control.PID measurement targets (Specification 46 §13.2): `pid:<instance>` gives the plant's
//! measurement to that controller's `Update` without writing the program's own variables.
use super::*;
use crate::harness::TestHarness;
use crate::simulation::SimulationConfig;
use crate::value::Value;

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

/// The same loop with a controller whose instance is declared `RETAIN`.
const RETAINED_PROGRAM: &str = r#"
PROGRAM Main
VAR
    Command : LREAL := 84.0;
    Settings : Control.PIDParameters := (ProportionalGain := 1.0, OutputMaximum := 100.0,
        RateMinimum := -1.0E9, RateMaximum := 1.0E9, TrackingTime := 1.0);
    Input : Control.PIDInput;
    Output : LREAL;
END_VAR
VAR RETAIN
    Controller : Control.PID;
END_VAR
    Controller.Configure(Parameters := Settings);
    Input.Setpoint := 50.0;
    Input.ProcessValue := 1.0;
    Input.CycleTime := 0.1;
    Output := Controller.Update(Input := Input);
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

fn value(runtime: &CompiledRuntime, symbol: &str) -> f64 {
    match runtime.with_program_view(|program| program.value(symbol)) {
        Some(Value::LReal(value)) => value,
        other => panic!("expected LREAL at {symbol}, got {other:?}"),
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
fn a_later_publish_replaces_the_held_measurement_of_that_controller_only() {
    // Specification 46 §13.2: each time the plant publishes, the value is held for that instance,
    // and other instances are untouched. The plant's command goes from 84 to 100, so the plant
    // holds 0.5 × 84 = 42 and then 0.5 × 100 = 50. Catches a held value that the first publish
    // keeps, and a later value that reaches another controller.
    let config =
        SimulationConfig::from_toml_text(&plants(&["pid:Main.Controller"]), "pid test").unwrap();
    let mut harness = harness();
    let mut system = PlantSystem::new(&config.plant_system);
    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        42.0
    );

    harness.set_input("Command", Value::LReal(100.0));
    system
        .advance(Duration::from_millis(200), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        50.0,
        "the later publish replaces the held measurement"
    );
    assert_eq!(
        value(harness.runtime(), "Main.Other.StateValue.ProcessValue"),
        3.0,
        "another controller is not touched"
    );
}

#[test]
fn a_newly_loaded_program_takes_no_measurement_until_the_plant_publishes_again() {
    // Specification 46 §13.2: a held value is removed when the runtime loads another program, and
    // the plant gives it again at its next publish. The plant publishes 0.5 × 84 = 42 once; after
    // an online change of the program the controller works on the program's own 1.0 until the
    // plant completes its next step. Catches the online change carrying the held measurement into
    // the new program's instance (an online change carries every member by name, specification 44
    // §12.2).
    let config =
        SimulationConfig::from_toml_text(&plants(&["pid:Main.Controller"]), "pid test").unwrap();
    let mut harness = harness();
    let mut system = PlantSystem::new(&config.plant_system);
    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        42.0
    );

    harness
        .reload_sources(&[CONTROL_LIBRARY, PROGRAM])
        .expect("the program changes online");
    system.publish(harness.runtime_mut()).unwrap();
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        1.0,
        "the newly loaded program's controller holds no measurement"
    );
    let target =
        signal::PlantSignal::parse("pid:Main.Controller", &[IoArea::Input], "feedback").unwrap();
    let error = target
        .read(harness.runtime())
        .expect_err("nothing is held after the change");
    assert!(
        error
            .to_string()
            .contains("has no published measurement yet"),
        "{error}"
    );

    system
        .advance(Duration::from_millis(200), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        42.0,
        "the plant's next publish gives the measurement again"
    );
}

#[test]
fn a_warm_restart_removes_the_measurement_of_a_retained_controller() {
    // Specification 46 §13.2: a held value is removed when the runtime restarts. A warm restart
    // keeps the RETAIN instance's parameters and history (IEC 61131-3 §6.5.6.1) but starts its
    // held measurement over, so the controller works on the program's own 1.0 again. Catches the
    // library declaring the held measurement in a plain VAR block, which a RETAIN instance keeps
    // through a warm restart.
    let config =
        SimulationConfig::from_toml_text(&plants(&["pid:Main.Controller"]), "pid test").unwrap();
    let mut harness = TestHarness::from_sources(&[CONTROL_LIBRARY, RETAINED_PROGRAM])
        .expect("the program compiles with the control library");
    let mut system = PlantSystem::new(&config.plant_system);
    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        42.0
    );

    harness
        .restart(crate::RestartMode::Warm)
        .expect("the program restarts warm");
    assert_eq!(
        value(
            harness.runtime(),
            "Main.Controller.ParametersValue.ProportionalGain"
        ),
        1.0,
        "the RETAIN instance keeps its parameters"
    );
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        value(harness.runtime(), "Main.Controller.StateValue.ProcessValue"),
        1.0,
        "the warm restart removes the held measurement"
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

#[test]
fn a_controller_without_the_simulation_members_faults_the_plant_with_its_name() {
    // Specification 46 §13.2: a `PID` of a Control library older than the simulation extension has
    // no `HeldMeasurement` or `MeasurementHeld`, so it cannot take a measurement; the plant faults
    // at its first publish, naming the target. Catches a plant that reports the publish done while
    // nothing reaches the controller.
    const OLD_LIBRARY: &str = r#"
NAMESPACE Control
TYPE PIDParameters :
STRUCT
    ProportionalGain : LREAL;
END_STRUCT
END_TYPE
TYPE PIDInitialState :
STRUCT
    ProcessValue : LREAL;
END_STRUCT
END_TYPE
FUNCTION_BLOCK PID
VAR
    ParametersValue : PIDParameters;
    StateValue : PIDInitialState;
END_VAR
END_FUNCTION_BLOCK
END_NAMESPACE
"#;
    const OLD_PROGRAM: &str = r#"
PROGRAM Main
VAR
    Command : LREAL := 84.0;
    Controller : Control.PID;
END_VAR
END_PROGRAM
"#;
    let config =
        SimulationConfig::from_toml_text(&plants(&["pid:Main.Controller"]), "pid test").unwrap();
    let mut harness = TestHarness::from_sources(&[OLD_LIBRARY, OLD_PROGRAM])
        .expect("the program compiles with the old library");
    let mut system = PlantSystem::new(&config.plant_system);
    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    let error = system
        .publish(harness.runtime_mut())
        .expect_err("the old controller takes no measurement");
    let message = error.to_string();
    assert!(message.contains("pid:Main.Controller"), "{message}");
    assert!(message.contains("HeldMeasurement"), "{message}");
}
