//! A reset time that a law in standard form keeps as `TIME` (specification 35 C0, C2): the runtime binding declares it a TIME in
//! seconds (protocol annex E7.1), the Auto entry's transfer guards it as a TIME (E6.2), and a value Control Studio writes is rounded
//! to whole milliseconds.

#![allow(clippy::float_cmp)] // The values are compared with the exact values the rules give.

use std::collections::BTreeMap;

use serde_json::json;

use super::declared_initialization::declared_transfer_binding;
use super::fixture_loops::project_standard_pi_loop_on_p1;
use super::parameter_application::review_input;
use super::parameter_binding::runtime_binding_of;
use super::transfer::TransferValueType;
use crate::loop_definition::{LoopDefinition, ParameterType};

/// The standard PI loop on `P1`, its reset time `P1.Ti` bound as a TIME in seconds from 0.01 to 1,000. It is written and read back
/// as a loop file, so the loop's checks accept it.
fn time_reset_loop() -> LoopDefinition {
    let mut loop_definition = project_standard_pi_loop_on_p1();
    let binding = loop_definition
        .parameter_binding
        .as_mut()
        .expect("the binding");
    let reset_time = binding
        .parameters
        .iter_mut()
        .find(|parameter| parameter.symbol == "P1.Ti")
        .expect("the reset time is bound");
    reset_time.iec_type = ParameterType::Time;
    let file = serde_json::to_string(&loop_definition).expect("the loop serializes");
    LoopDefinition::decode(&file).expect("a standard law's TIME reset time is accepted")
}

/// C0 and E7.1: the runtime binding declares the reset time `TIME` in `s`; E6.2: the Auto entry's transfer reads it as a TIME guard
/// and writes no TIME. Catches the reset time sent as an LREAL, which the runtime refuses for a TIME variable, and a transfer that
/// would write it.
#[test]
fn a_time_reset_time_is_a_time_in_the_runtime_binding_and_a_time_guard_of_the_transfer() {
    let loop_definition = time_reset_loop();
    let binding = runtime_binding_of(&loop_definition).expect("the runtime binding");
    let wire = serde_json::to_value(&binding).expect("the binding serializes");
    assert_eq!(
        wire["parameters"][1],
        json!({ "symbol": "P1.Ti", "iec_type": "TIME", "unit": "s", "convention": "reset time", "minimum": 0.01, "maximum": 1000.0 })
    );
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the program's own law");
    let transfer = declared_transfer_binding(&loop_definition, &project.initialization)
        .expect("the transfer binding");
    let guard = transfer
        .guards
        .iter()
        .find(|guard| guard.symbol == "P1.Ti")
        .expect("the reset time is a guard");
    assert_eq!(guard.iec_type, TransferValueType::Time);
    assert!(
        transfer
            .states
            .iter()
            .chain(std::iter::once(&transfer.output))
            .all(|member| member.iec_type != TransferValueType::Time),
        "a transfer writes no TIME"
    );
}

/// C0: a TIME value that Control Studio writes is rounded to whole milliseconds, the resolution of a TIME in CODESYS and Beckhoff controllers,
/// as a REAL value is rounded to single precision: 1.2346 s is written as 1.235 s. A value read back is kept as the runtime holds
/// it, finer than a millisecond too. Catches a reset time written with a fraction of a millisecond, and a value read back refused or
/// rounded.
#[test]
fn a_time_value_is_written_in_whole_milliseconds_and_kept_as_read() {
    let binding = runtime_binding_of(&time_reset_loop()).expect("the runtime binding");
    let reviewed = review_input(
        &binding,
        &BTreeMap::from([("P1.Kp".to_owned(), 2.0), ("P1.Ti".to_owned(), 1.2346)]),
    )
    .expect("the values lie within their bounds");
    assert_eq!(reviewed["P1.Ti"], 1.235);
    assert_eq!(reviewed["P1.Kp"], 2.0);
    assert_eq!(ParameterType::Time.realize(4_194_792.263), 4_194_792.263);
    assert!(
        ParameterType::Time.keeps(1.0005),
        "a TIME read back may be finer than a millisecond"
    );
    assert_eq!(ParameterType::Real.realize(0.1), f64::from(0.1_f32));
    assert!(!ParameterType::Real.keeps(0.1));
}
