use super::*;
use crate::commissioning::fixture_loops::{
    library_pid_loop as library_loop, library_pid_states as seven_states,
};

/// The loop's retained states, for the tests that change one of them.
fn states_of(loop_definition: &mut LoopDefinition) -> &mut Vec<InitializedState> {
    match loop_definition.controller.pid_initialization.as_mut() {
        Some(PidInitialization::ControlPid { states, .. }) => states,
        _ => panic!("the library loop declares control-pid-v1"),
    }
}

#[test]
fn a_control_pid_with_its_seven_states_declares_them_as_one_initialization() {
    let definition =
        library_pid_initialization(&library_loop()).expect("the seven states are bound");
    assert_eq!(definition.states().len(), 7);
}

#[test]
fn a_retained_state_bound_to_another_coordinate_or_unit_is_refused() {
    let mut wrong_unit = library_loop();
    states_of(&mut wrong_unit)[5].unit = "degC".to_owned();
    assert!(
        library_pid_initialization(&wrong_unit).is_err(),
        "the nominal output takes the actuator's unit"
    );
    let mut wrong_coordinate = library_loop();
    states_of(&mut wrong_coordinate)[2].initial =
        InitialValue::From(InitialSource::ModelInput { index: 0 });
    assert!(
        library_pid_initialization(&wrong_coordinate).is_err(),
        "the process value starts at the output"
    );
    let mut not_bounded = library_loop();
    states_of(&mut not_bounded)[0].post_scan = PostScan::Unchanged;
    assert!(
        library_pid_initialization(&not_bounded).is_err(),
        "a retained state may move within its bounds"
    );
}

#[test]
fn the_snapshot_age_fits_the_task_period_and_stays_within_ten_seconds() {
    let mut too_young = library_loop();
    too_young.controller.pid_initialization = Some(PidInitialization::ControlPid {
        maximum_snapshot_age_ms: 5,
        states: seven_states(),
        application_states: None,
    });
    assert!(
        library_pid_initialization(&too_young).is_err(),
        "the snapshot is at least one task period old at most"
    );
    let mut too_old = library_loop();
    too_old.controller.pid_initialization = Some(PidInitialization::ControlPid {
        maximum_snapshot_age_ms: 20_000,
        states: seven_states(),
        application_states: None,
    });
    assert!(
        library_pid_initialization(&too_old).is_err(),
        "the snapshot is at most 10 s old"
    );
}

#[test]
fn application_states_of_the_owning_program_are_added_and_others_or_duplicates_are_refused() {
    let mut owned = library_loop();
    owned.controller.pid_initialization = Some(PidInitialization::ControlPid {
        maximum_snapshot_age_ms: 100,
        states: seven_states(),
        application_states: Some(vec![InitializedState {
            symbol: "Main.Startup.Done".to_owned(),
            ..seven_states()[4].clone()
        }]),
    });
    let definition =
        library_pid_initialization(&owned).expect("the owning program's state is accepted");
    assert_eq!(definition.states().len(), 8);
    let mut foreign = library_loop();
    foreign.controller.pid_initialization = Some(PidInitialization::ControlPid {
        maximum_snapshot_age_ms: 100,
        states: seven_states(),
        application_states: Some(vec![InitializedState {
            symbol: "Other.Flag".to_owned(),
            ..seven_states()[4].clone()
        }]),
    });
    assert!(
        library_pid_initialization(&foreign).is_err(),
        "application states belong to the owning program"
    );
    let mut duplicate = library_loop();
    duplicate.controller.pid_initialization = Some(PidInitialization::ControlPid {
        maximum_snapshot_age_ms: 100,
        states: seven_states(),
        application_states: Some(vec![seven_states()[0].clone()]),
    });
    assert!(
        library_pid_initialization(&duplicate).is_err(),
        "a symbol is declared once"
    );
}

#[test]
fn a_list_that_is_not_exactly_seven_states_is_refused_by_name() {
    let refusal = assert_library_pid_states("Main.TankController", &seven_states()[..6], "%", "%")
        .expect_err("six states are not seven");
    assert!(refusal
        .message
        .contains("seven retained states for Main.TankController"));
    assert!(assert_library_pid_states("Main.TankController", &seven_states(), "%", "%").is_ok());
}
