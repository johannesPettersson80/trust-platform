//! The open path's reasons, ported from `control-studio-command.test.ts` (the reasons its workspace-discovery assertions check) and
//! `projectLoop.ts` (specification 35, B1.2). The assertions that open a quick pick, an error message or an editor belong to the
//! VS Code host and are not ported.

use super::{invalid_loop_reason, missing_loop_definition_reason, InvalidLoop};
use crate::loop_definition::LoopRejection;

#[test]
fn a_project_with_no_loop_definition_says_what_to_configure() {
    // projectLoop.ts: the missing reason, offered with "Configure control loop"; control-studio-command.test.ts's missing-loop
    // assertions (the examples route and "discovery must never synthesize bindings") are the VS Code host's and are not ported.
    assert_eq!(
        missing_loop_definition_reason(),
        "This project has no loop definition in control-loop.json or control-loops/. Configure a loop to bind its signals."
    );
}

#[test]
fn a_file_that_is_not_json_or_not_a_definition_is_named_in_the_reason() {
    // projectLoop.ts: "<file> could not be read as valid JSON." and "<file> does not contain a complete Control Studio loop definition."
    assert_eq!(
        invalid_loop_reason("control-loop.json", InvalidLoop::NotValidJson),
        "control-loop.json could not be read as valid JSON."
    );
    assert_eq!(
        invalid_loop_reason("second.json", InvalidLoop::NotDefinition),
        "second.json does not contain a complete Control Studio loop definition."
    );
}

#[test]
fn a_loop_of_another_runtime_names_both_runtimes() {
    // projectLoop.ts: "The configured loop uses <loop runtime>, but <selected> is the selected target."
    assert_eq!(
        invalid_loop_reason(
            "control-loop.json",
            InvalidLoop::RuntimeMismatch {
                loop_runtime: "runtime:other",
                selected: "runtime:local",
            },
        ),
        "The configured loop uses runtime:other, but runtime:local is the selected target."
    );
}

#[test]
fn a_duplicate_id_in_the_project_asks_for_a_unique_id() {
    // control-studio-command.test.ts: "discovers additional loop files and rejects duplicate IDs within a project" (the reason matches
    // /duplicate.*loop:tank/i); the sentence is projectLoop.ts's.
    let reason = invalid_loop_reason("second.json", InvalidLoop::Duplicate { id: "loop:tank" });
    assert_eq!(
        reason,
        "Duplicate control loop ID loop:tank in this project. Give each loop a unique ID."
    );
    assert!(reason.to_lowercase().contains("duplicate") && reason.contains("loop:tank"));
}

#[test]
fn incompatible_units_name_the_setpoint_and_the_measurement() {
    // control-studio-command.test.ts: "workspace discovery rejects incompatible units with the failing signal names"
    // (`/setpoint.*processVariable/`): the sentence names both signals and the rule.
    assert_eq!(
        invalid_loop_reason(
            "control-loop.json",
            InvalidLoop::Validation(&LoopRejection::IncompatibleUnits),
        ),
        "control-loop.json: setpoint and processVariable must use matching engineering units."
    );
}

#[test]
fn a_rule_of_one_signal_names_the_signal_and_the_rule_in_words() {
    // projectLoop.ts: "<file>: <signal>: <rule in words>."
    assert_eq!(
        invalid_loop_reason(
            "control-loop.json",
            InvalidLoop::Validation(&LoopRejection::InvalidSignalDirection(
                "manipulatedVariable".into()
            )),
        ),
        "control-loop.json: manipulatedVariable: invalid signal direction."
    );
    assert_eq!(
        invalid_loop_reason(
            "control-loop.json",
            InvalidLoop::Validation(&LoopRejection::IncompatibleSignalType(
                "processVariable".into()
            )),
        ),
        "control-loop.json: processVariable: incompatible signal type."
    );
}

#[test]
fn two_ambiguous_signals_name_both_and_a_rule_without_a_signal_names_the_configuration() {
    // projectLoop.ts: `signals.join(" and ")`, and the configuration when the rule names no signal.
    assert_eq!(
        invalid_loop_reason(
            "control-loop.json",
            InvalidLoop::Validation(&LoopRejection::AmbiguousSignalAddress(
                "processVariable".into(),
                "manipulatedVariable".into(),
            )),
        ),
        "control-loop.json: processVariable and manipulatedVariable: ambiguous signal address."
    );
    assert_eq!(
        invalid_loop_reason(
            "control-loop.json",
            InvalidLoop::Validation(&LoopRejection::IncompatibleAuxiliaryUnits(
                "secondary".into()
            )),
        ),
        "control-loop.json: loop configuration: incompatible auxiliary units."
    );
}
