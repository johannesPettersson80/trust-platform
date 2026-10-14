use super::*;
use crate::commissioning::fixture_loops;
use crate::loop_definition::{AuxiliaryRole, AuxiliarySignal, Direction, Range};

/// The controller-level loop of the fixtures, with the given mode signal and at most one further signal.
fn loop_with_mode(mode: Option<&str>, auxiliary: Option<AuxiliarySignal>) -> LoopDefinition {
    let mut loop_definition = fixture_loops::control_pid_loop();
    loop_definition.controller.mode_signal = mode.map(str::to_owned);
    loop_definition.signals.additional = auxiliary.map(|signal| vec![signal]);
    loop_definition
}

/// A further signal of the loop, named `symbol`, at `address`, of the given IEC type.
fn signal(symbol: &str, address: Option<&str>, iec_type: &str) -> AuxiliarySignal {
    AuxiliarySignal {
        id: "mode".into(),
        channel: "tank".into(),
        role: AuxiliaryRole::State,
        symbol: symbol.into(),
        address: address.map(str::to_owned),
        iec_type: iec_type.into(),
        direction: Direction::Read,
        unit: "1".into(),
        range: Range {
            minimum: 0.0,
            maximum: 1.0,
        },
    }
}

#[test]
fn a_declared_bool_signal_gives_the_mode_its_address_whatever_the_case() {
    // controllerModeAddress: the symbol matches without regard to case, and the signal's address is the mode's address.
    let loop_definition = loop_with_mode(
        Some("main.auto"),
        Some(signal("Main.Auto", Some("%MX0.2"), "BOOL")),
    );
    assert_eq!(
        mode_address(&loop_definition, None),
        Some("%MX0.2".to_owned())
    );
}

#[test]
fn a_direct_bit_address_is_its_own_address_in_capitals() {
    let loop_definition = loop_with_mode(Some("%mx0.1"), None);
    assert_eq!(
        mode_address(&loop_definition, None),
        Some("%MX0.1".to_owned())
    );
}

#[test]
fn a_signal_that_is_not_bool_does_not_name_the_mode() {
    let loop_definition = loop_with_mode(
        Some("Main.Auto"),
        Some(signal("Main.Auto", Some("%IX0.0"), "REAL")),
    );
    assert_eq!(mode_address(&loop_definition, None), None);
}

#[test]
fn an_undeclared_name_that_is_not_a_bit_address_has_no_address() {
    let loop_definition = loop_with_mode(Some("Main.Auto"), None);
    assert_eq!(mode_address(&loop_definition, None), None);
}

#[test]
fn a_declared_signal_without_an_address_gives_no_address() {
    let loop_definition =
        loop_with_mode(Some("Main.Auto"), Some(signal("Main.Auto", None, "BOOL")));
    assert_eq!(mode_address(&loop_definition, None), None);
}

#[test]
fn an_empty_mode_has_no_address() {
    let loop_definition = loop_with_mode(Some(""), None);
    assert_eq!(mode_address(&loop_definition, None), None);
}

#[test]
fn an_explicit_mode_takes_the_place_of_the_controllers_default() {
    let loop_definition = loop_with_mode(Some("Main.Auto"), None);
    assert_eq!(
        mode_address(&loop_definition, Some("%qx1.0")),
        Some("%QX1.0".to_owned())
    );
}

#[test]
fn without_a_controller_mode_or_an_argument_there_is_no_address() {
    let loop_definition = loop_with_mode(None, None);
    assert_eq!(mode_address(&loop_definition, None), None);
}
