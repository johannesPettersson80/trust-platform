use super::*;
use crate::commissioning::fixture_loops::control_pid_loop;
use crate::loop_definition::{
    AuxiliaryRole, AuxiliarySignal, Direction, InitialSource, InitialValue, PostScan, Range,
    StateType,
};

fn state(symbol: &str) -> InitializedState {
    InitializedState {
        symbol: symbol.to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: -10.0,
        maximum: 10.0,
        initial: InitialValue::From(InitialSource::ModelOutput { index: 0 }),
        post_scan: PostScan::Bounded,
    }
}

const OWN: &str = "Main.TankController.StateValue.ProcessValue";

#[test]
fn a_library_pids_own_measurement_is_its_process_value_state() {
    assert_eq!(pid_own_measurement("Main.TankController"), OWN);
}

#[test]
fn the_loop_measures_its_pid_without_regard_to_case_and_another_signal_is_not_a_pid() {
    let loop_definition = control_pid_loop();
    assert_eq!(
        measured_pid(
            &loop_definition,
            "main.tankcontroller.statevalue.processvalue"
        ),
        Some("Main.TankController".to_owned())
    );
    assert_eq!(measured_pid(&loop_definition, "LevelPV"), None);
}

#[test]
fn a_plant_gives_a_pids_own_measurement_to_the_pid_and_writes_any_other_variable_itself_unless_it_has_an_address(
) {
    let loop_definition = control_pid_loop();
    assert_eq!(
        plant_feedback_target(&loop_definition, OWN, None),
        "pid:Main.TankController"
    );
    assert_eq!(
        plant_feedback_target(&loop_definition, "LevelPV", Some("%IL0")),
        "%IL0",
        "an address is the target"
    );
    assert_eq!(
        plant_feedback_target(&loop_definition, "LevelPV", None),
        "LevelPV"
    );
}

#[test]
fn the_measured_state_is_read_at_the_boundary_and_never_initialized() {
    let mut loop_definition = control_pid_loop();
    loop_definition.signals.process_variable.symbol = OWN.to_owned();
    let definition = ControllerInitialization::new(vec![
        state(OWN),
        state("Main.TankController.StateValue.Setpoint"),
    ])
    .expect("the states are valid");
    let initialized: Vec<String> = initialized_states(&loop_definition, &definition)
        .iter()
        .map(|state| state.symbol.clone())
        .collect();
    assert_eq!(
        initialized,
        vec!["Main.TankController.StateValue.Setpoint".to_owned()]
    );
}

#[test]
fn a_further_process_variable_role_is_also_read_and_not_initialized_while_other_states_are() {
    let mut loop_definition = control_pid_loop();
    loop_definition.signals.additional = Some(vec![AuxiliarySignal {
        id: "pv2".to_owned(),
        channel: "second".to_owned(),
        role: AuxiliaryRole::ProcessVariable,
        symbol: "main.second.pv".to_owned(),
        address: None,
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: "%".to_owned(),
        range: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
    }]);
    let definition =
        ControllerInitialization::new(vec![state("Main.Second.PV"), state("Main.Other")])
            .expect("the states are valid");
    let initialized: Vec<String> = initialized_states(&loop_definition, &definition)
        .iter()
        .map(|state| state.symbol.clone())
        .collect();
    assert_eq!(
        initialized,
        vec!["Main.Other".to_owned()],
        "the symbols compare without regard to case"
    );
}
