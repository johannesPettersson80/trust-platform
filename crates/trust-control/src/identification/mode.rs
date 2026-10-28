//! Whether Identify must move the loop to the other controller mode before a method runs, and the refusal of a mode it cannot
//! read (the decision of `connectedIdentification.ts`; specification 35, A6.2, B3.4 step 1 and B3.9). A Manual method writes the
//! plant input, so the loop must be in Manual while it runs, and any other method needs Auto. The move itself is a controller
//! transfer (`mode_transfer`), which the engine makes before anything is recorded and reverses once the campaign has ended,
//! failed or been cancelled (A6.2). A mode that cannot be read refuses the run: Identify cannot tell whether a step is safe
//! without it.

use crate::experiment::plan::ControllerMode;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::policy::{project_scripted, IdentificationMethod};
use crate::loop_definition::{LoopDefinition, Topology};

/// Whether a loop has no controller mode that its run changes: an open-loop loop has none, and a project-scripted loop runs
/// its project's scenario whatever the mode (C12). Neither is refused for its mode.
fn has_no_mode_to_change(loop_definition: &LoopDefinition) -> bool {
    loop_definition.topology == Some(Topology::OpenLoop)
        || project_scripted(loop_definition).is_some()
}

/// Whether the loop must change to the mode the method needs before it runs (`needsMode` of `connectedIdentification.ts`).
/// An open-loop loop has no controller mode to change, and neither does a project-scripted loop, which its project runs
/// whatever the mode (C12). Otherwise a Manual method (`manual`: it writes the plant input in Manual) needs Manual, and any
/// other method needs Auto. A loop in another mode needs the change, and so does a loop whose mode cannot be read
/// (`ControllerMode::Unavailable`); [`unreadable_mode_refusal`] refuses such a mode before this is asked.
///
/// The method decides the mode, not the setpoint binding itself. Automatic chooses its method from the mode and the policy
/// (`automatic_method`): a loop in Auto without a setpoint binding, where open-loop tests are permitted, gets a Manual
/// plant-input step, which needs the change (B3.3). A loop in Auto with a setpoint gets a setpoint step, which needs none.
#[must_use]
pub fn needs_mode_change(
    loop_definition: &LoopDefinition,
    method: IdentificationMethod,
    mode: ControllerMode,
) -> bool {
    if has_no_mode_to_change(loop_definition) {
        return false;
    }
    let needed = if method.manual {
        ControllerMode::Manual
    } else {
        ControllerMode::Auto
    };
    mode != needed
}

/// The refusal of a loop whose controller mode cannot be read in an available sample (B3.4 step 1, B3.9): the mode signal is
/// stale or bad, so Identify waits for a fresh value before it records anything. `mode` is the sample's mode, or `None` when no
/// sample is available: the connection or the scan is the failure then, not the mode, so nothing is refused here. An open-loop
/// loop has no controller mode and is never refused, and neither is a project-scripted loop, whatever its mode (C12).
#[must_use]
pub fn unreadable_mode_refusal(
    loop_definition: &LoopDefinition,
    mode: Option<ControllerMode>,
) -> Option<Failure> {
    let unreadable = mode == Some(ControllerMode::Unavailable);
    if !unreadable || has_no_mode_to_change(loop_definition) {
        return None;
    }
    let text = match loop_definition.controller.mode_signal.as_deref() {
        Some(signal) => format!(
            "Identify cannot read the loop's mode from {signal}: the signal is stale or bad, so it cannot tell whether the step is safe. Wait for a fresh value, normally within a few executions of the loop's task, then identify again."
        ),
        None => "Identify cannot read the loop's mode: its mode signal is stale or bad, so it cannot tell whether the step is safe. Wait for a fresh value, normally within a few executions of the loop's task, then identify again.".to_owned(),
    };
    Some(Failure::new(&text, Cause::Runtime, NextAction::Wait))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::plan::ControlExperimentKind;
    use crate::identification::policy::{automatic_method, PolicyError};
    use crate::recording::fixtures::loop_definition;

    /// A method the engineer chose, with the kind it runs and whether it writes the plant input in Manual.
    fn chosen(kind: ControlExperimentKind, manual: bool) -> IdentificationMethod {
        IdentificationMethod {
            kind,
            manual,
            automatic: false,
        }
    }

    /// The fixture loop for Automatic (B3.3): with its setpoint binding when `setpoint` is set, with its Manual command, and with
    /// a policy that permits open-loop tests when `open_loop_permitted` is set.
    fn automatic_loop(setpoint: bool, open_loop_permitted: bool) -> LoopDefinition {
        let mut loop_def = loop_definition();
        if !setpoint {
            loop_def.signals.setpoint = None;
        }
        loop_def.controller.manual_output_signal = Some("Main.TIC101_ManualOutput".to_owned());
        loop_def.identification_policy = Some(serde_json::json!({
            "openLoopPermitted": open_loop_permitted,
            "sustainedHoldPermitted": true,
            "information": "basic"
        }));
        loop_def
    }

    /// The fixture loop run by its project (`execution: project-scripted`, C12): the project writes the excitation, so the
    /// loop declares an experiment that Control Studio does not run.
    fn project_scripted_loop() -> LoopDefinition {
        let experiment = serde_json::json!({
            "execution": "project-scripted",
            "kind": "observation",
            "amplitude": 0.0,
            "durationSeconds": 30.0,
            "seed": 1
        });
        let mut loop_def = loop_definition();
        loop_def.experiment = Some(
            serde_json::from_value(experiment).expect("the project-scripted experiment decodes"),
        );
        loop_def
    }

    #[test]
    fn a_manual_method_needs_manual_and_any_other_method_needs_auto() {
        let loop_def = loop_definition();
        let step = chosen(ControlExperimentKind::MvStep, true);
        let setpoint = chosen(ControlExperimentKind::SetpointStep, false);
        assert!(needs_mode_change(&loop_def, step, ControllerMode::Auto));
        assert!(!needs_mode_change(&loop_def, step, ControllerMode::Manual));
        assert!(needs_mode_change(
            &loop_def,
            setpoint,
            ControllerMode::Manual
        ));
        assert!(!needs_mode_change(
            &loop_def,
            setpoint,
            ControllerMode::Auto
        ));
    }

    #[test]
    fn an_open_loop_needs_no_change_and_an_unreadable_mode_is_never_taken_for_the_method_mode() {
        let mut loop_def = loop_definition();
        let step = chosen(ControlExperimentKind::MvStep, true);
        assert!(needs_mode_change(
            &loop_def,
            step,
            ControllerMode::Unavailable
        ));
        loop_def.topology = Some(Topology::OpenLoop);
        assert!(!needs_mode_change(&loop_def, step, ControllerMode::Auto));
        assert!(!needs_mode_change(
            &loop_def,
            step,
            ControllerMode::Unavailable
        ));
    }

    #[test]
    fn automatic_steps_the_setpoint_in_auto_and_the_plant_input_in_manual_with_no_change() {
        let loop_def = automatic_loop(true, true);
        let in_auto = automatic_method(&loop_def, ControllerMode::Auto).expect("a setpoint step");
        assert_eq!(
            (in_auto.kind, in_auto.manual, in_auto.automatic),
            (ControlExperimentKind::SetpointStep, false, true)
        );
        assert!(!needs_mode_change(&loop_def, in_auto, ControllerMode::Auto));
        let in_manual =
            automatic_method(&loop_def, ControllerMode::Manual).expect("a plant-input step");
        assert_eq!(
            (in_manual.kind, in_manual.manual),
            (ControlExperimentKind::MvStep, true)
        );
        assert!(!needs_mode_change(
            &loop_def,
            in_manual,
            ControllerMode::Manual
        ));
    }

    /// B3.3 and B3.4 step 1: Automatic gives a loop in Auto without a setpoint binding, where open-loop tests are permitted, a
    /// plant-input step in Manual, so the loop must change to Manual for it (the transfer of `mode_transfer`).
    #[test]
    fn automatic_in_auto_without_a_setpoint_needs_manual() {
        let loop_def = automatic_loop(false, true);
        let plant_input =
            automatic_method(&loop_def, ControllerMode::Auto).expect("a plant-input step");
        assert_eq!(
            (plant_input.kind, plant_input.manual, plant_input.automatic),
            (ControlExperimentKind::MvStep, true, true)
        );
        assert!(needs_mode_change(
            &loop_def,
            plant_input,
            ControllerMode::Auto
        ));
    }

    #[test]
    fn automatic_without_a_setpoint_and_without_open_loop_tests_has_no_safe_method() {
        let loop_def = automatic_loop(false, false);
        let refusal =
            automatic_method(&loop_def, ControllerMode::Auto).expect_err("no safe method");
        assert_eq!(refusal, PolicyError::NoSafeMethodWithoutSetpoint);
        let failure = refusal.failure();
        assert_eq!(
            (failure.cause, failure.next_action),
            (Cause::Implementation, NextAction::OpenLoopSetup)
        );
    }

    #[test]
    fn an_unreadable_mode_of_a_closed_loop_is_refused_naming_the_signal_and_waiting() {
        let mut loop_def = loop_definition();
        loop_def.controller.mode_signal = Some("Main.TIC101_Auto".to_owned());
        let refusal = unreadable_mode_refusal(&loop_def, Some(ControllerMode::Unavailable))
            .expect("an unreadable mode refuses the run");
        assert_eq!(
            refusal.detail.as_deref(),
            Some(
                "Identify cannot read the loop's mode from Main.TIC101_Auto: the signal is stale or bad, so it cannot tell whether the step is safe. Wait for a fresh value, normally within a few executions of the loop's task, then identify again."
            )
        );
        assert_eq!(
            (refusal.cause, refusal.next_action),
            (Cause::Runtime, NextAction::Wait)
        );
    }

    #[test]
    fn no_sample_and_an_open_loop_are_not_refused_for_their_mode() {
        let mut loop_def = loop_definition();
        assert!(unreadable_mode_refusal(&loop_def, None).is_none());
        assert!(unreadable_mode_refusal(&loop_def, Some(ControllerMode::Auto)).is_none());
        loop_def.topology = Some(Topology::OpenLoop);
        assert!(unreadable_mode_refusal(&loop_def, Some(ControllerMode::Unavailable)).is_none());
    }

    #[test]
    fn a_project_scripted_loop_needs_no_mode_change_in_auto_or_in_manual() {
        let loop_def = project_scripted_loop();
        for mode in [ControllerMode::Auto, ControllerMode::Manual] {
            let scenario = automatic_method(&loop_def, mode).expect("the project's scenario");
            assert!(
                !needs_mode_change(&loop_def, scenario, mode),
                "the project's scenario needs no mode change in {mode:?} (C12)"
            );
        }
    }

    #[test]
    fn an_unreadable_mode_does_not_refuse_a_project_scripted_loop() {
        let loop_def = project_scripted_loop();
        assert!(unreadable_mode_refusal(&loop_def, Some(ControllerMode::Unavailable)).is_none());
        let scenario = automatic_method(&loop_def, ControllerMode::Unavailable)
            .expect("the project's scenario, whatever the mode");
        assert!(!needs_mode_change(
            &loop_def,
            scenario,
            ControllerMode::Unavailable
        ));
    }
}
