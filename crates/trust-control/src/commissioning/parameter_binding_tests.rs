//! Tests of the parameter record's binding built from the loop definition (annex E7.1; specification 35, B6.1 and B6.4).

use super::*;
use crate::commissioning::fixture_loops::parallel_pid_loop;
use crate::commissioning::texts;
use crate::failure::{Cause, NextAction};
use crate::loop_definition::RealType;

/// A gain of the fixture's parameter record as its loop file declares it: LREAL, in %/%, bounded to ±100.
fn gain(symbol: &str, convention: &str) -> RuntimeParameter {
    RuntimeParameter {
        symbol: symbol.to_owned(),
        iec_type: RealType::Lreal,
        unit: "%/%".to_owned(),
        convention: convention.to_owned(),
        minimum: -100.0,
        maximum: 100.0,
    }
}

/// Annex E7.1: the binding is the loop's declared parameter record, member for member: its loop, the task that owns it, each
/// parameter with its IEC type, unit, convention and bounds, and the mode symbol with its Manual value.
#[test]
fn the_binding_is_the_declared_parameter_record_of_the_loop() {
    let binding =
        runtime_binding_of(&parallel_pid_loop()).expect("the fixture declares its record");
    assert_eq!(
        binding,
        RuntimeBinding {
            loop_id: "loop:tank".to_owned(),
            task_name: "MainTask".to_owned(),
            parameters: vec![
                gain("Main.Parameters.Kp", "proportional gain"),
                gain("Main.Parameters.Ki", "integral gain"),
                gain("Main.Parameters.Kd", "derivative gain"),
            ],
            mode: RuntimeMode {
                symbol: "Main.Auto".to_owned(),
                manual_value: false,
            },
        }
    );
}

/// Annex E7.1 and E7.2: a request carries the object the loop file declares, in the annex's field names, so
/// `control.parameters.*` reads and writes the record that the loop setup bound.
#[test]
fn the_binding_is_sent_as_the_loop_file_declares_it() {
    let pid_loop = parallel_pid_loop();
    let binding = runtime_binding_of(&pid_loop).expect("the fixture declares its record");
    assert_eq!(
        serde_json::to_value(&binding).expect("the binding serializes"),
        serde_json::to_value(&pid_loop.parameter_binding).expect("the declaration serializes")
    );
}

/// Annex E7.1 (`connectRuntimeParameters`, runtimeParameterConnection.ts): a loop that declares no parameter record has nothing
/// for Apply or Restore to read or write, and the refusal opens its setup.
#[test]
fn a_loop_without_a_parameter_binding_is_refused() {
    let failure = runtime_binding_of(&crate::recording::fixtures::loop_definition())
        .expect_err("the loop declares no record");
    assert_eq!(failure, application_texts::parameter_binding_missing());
    assert_eq!(
        failure.message,
        "Declare the ST parameter binding before application."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenLoopSetup)
    );
}

/// Annex E7.1 and E11.8: the binding is checked as the parameter client checks it, before any request is sent: symbols that
/// differ only in case are one symbol to the runtime, and the binding must name its own loop.
#[test]
fn a_binding_that_breaks_the_annex_rules_is_refused_before_any_request() {
    let mut repeated = parallel_pid_loop();
    let declared = repeated
        .parameter_binding
        .as_mut()
        .expect("the fixture declares its record");
    let mut twin = declared.parameters[0].clone();
    twin.symbol = "MAIN.PARAMETERS.KP".to_owned();
    declared.parameters.push(twin);
    assert_eq!(
        runtime_binding_of(&repeated).expect_err("a symbol repeated in another case"),
        Failure::new(
            &texts::parameter_binding_symbol("MAIN.PARAMETERS.KP"),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )
    );

    let mut foreign = parallel_pid_loop();
    foreign
        .parameter_binding
        .as_mut()
        .expect("the fixture declares its record")
        .loop_id = "loop:other".to_owned();
    assert_eq!(
        runtime_binding_of(&foreign).expect_err("the binding names another loop"),
        Failure::new(
            &texts::parameter_binding_loop("loop:other", "loop:tank"),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )
    );
}
