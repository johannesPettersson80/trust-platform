use serde_json::json;

use super::*;
use crate::experiment::plan::PlanSignal;
use crate::failure::NextAction;
use crate::identification::texts::policy_error_text;
use crate::loop_definition::{Activation, Direction};
use crate::recording::EvidenceSource;
use crate::signals::Quality;

/// The tank of the long-process campaign test: a setpoint of 45 degC, measured in 0 to 100 degC, driven by a 0 to
/// 100 % output in Auto, with a Manual command.
fn tank_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2,
        "id": "tank",
        "name": "Tank",
        "revision": "rev-1",
        "projectId": "project",
        "runtimeId": "runtime",
        "topology": "closed-loop",
        "controller": {
            "instance": "Main.TIC101",
            "type": "Control.PID",
            "revision": "sha256:test",
            "action": "direct",
            "taskPeriodMs": 100.0,
            "manualOutputSignal": "Main.TIC101_ManualOutput",
            "manualOutputType": "REAL"
        },
        "signals": {
            "setpoint": {
                "symbol": "Main.TIC101_Setpoint", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "processVariable": {
                "symbol": "Main.TT101.Value", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "manipulatedVariable": {
                "symbol": "Main.TIC101_Output", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            }
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "experiment": {
            "execution": "control-studio", "kind": "observation", "amplitude": 0.0,
            "durationSeconds": 600.0, "seed": 1
        }
    }))
    .expect("the tank decodes")
}

/// The tank of the project-scripted tests: the same loop, run by its project (`execution: project-scripted`, C12). With
/// `activated`, the project declares the Boolean activation that Control Studio writes.
fn project_tank(activated: bool) -> LoopDefinition {
    let mut project = tank_loop();
    if let Some(experiment) = project.experiment.as_mut() {
        experiment.execution = Execution::ProjectScripted;
        experiment.activation = activated.then(|| Activation {
            symbol: "Main.Scenario_Active".to_string(),
            address: "%QX0.0".to_string(),
            iec_type: "BOOL".to_string(),
            direction: Direction::Write,
            asserted_value: None,
        });
    }
    project
}

/// The live sample the runtime reports: the controller mode, the measured output, and the manipulated variable.
fn live_sample(mode: ControllerMode, process_value: f64, output: f64) -> ExperimentSample {
    ExperimentSample {
        scan: 7,
        source: EvidenceSource::SimulatedPlant,
        controller_mode: mode,
        setpoint: Some(PlanSignal {
            symbol: "Main.TIC101_Setpoint".to_string(),
            address: None,
            value: 45.0,
            unit: "degC".to_string(),
            quality: Quality::Good,
        }),
        process_variable: PlanSignal {
            symbol: "Main.TT101.Value".to_string(),
            address: None,
            value: process_value,
            unit: "degC".to_string(),
            quality: Quality::Good,
        },
        manipulated_variable: PlanSignal {
            symbol: "Main.TIC101_Output".to_string(),
            address: None,
            value: output,
            unit: "%".to_string(),
            quality: Quality::Good,
        },
        additional: Vec::new(),
    }
}

fn automatic_input<'a>(
    loop_definition: &'a LoopDefinition,
    sample: &'a ExperimentSample,
    kind: ControlExperimentKind,
    noise: f64,
    validation: bool,
) -> AmplitudeInput<'a> {
    AmplitudeInput {
        loop_definition,
        sample: Some(sample),
        kind,
        automatic: true,
        noise,
        validation,
        fallback_amplitude: 10.0,
        on_off: false,
    }
}

/// The size of an automatic excitation, which must exist.
fn amplitude(input: &AmplitudeInput<'_>) -> f64 {
    identification_amplitude(input).expect("the excitation is sized")
}

#[track_caller]
fn near(actual: f64, expected: f64, reason: &str) {
    assert!(
        (actual - expected).abs() <= 1e-9,
        "{actual} is not {expected}: {reason}"
    );
}

#[test]
fn automatic_identifies_a_loop_in_auto_with_a_setpoint_by_a_setpoint_step_sized_by_the_noise_and_the_range(
) {
    let loop_definition = tank_loop();
    let auto = live_sample(ControllerMode::Auto, 45.0, 30.0);
    assert_eq!(
        identification_method(&loop_definition, Some(&auto)),
        Ok(IdentificationMethod {
            kind: ControlExperimentKind::SetpointStep,
            manual: false,
            automatic: true,
        })
    );
    near(
        amplitude(&automatic_input(
            &loop_definition,
            &auto,
            ControlExperimentKind::SetpointStep,
            0.1,
            false,
        )),
        2.0,
        "20 times the noise is 2, the same as 2 % of the 100 degC range",
    );
    near(
        amplitude(&automatic_input(
            &loop_definition,
            &auto,
            ControlExperimentKind::SetpointStep,
            0.5,
            false,
        )),
        10.0,
        "20 times the noise is 10, capped at 10 % of the range",
    );
    let high = live_sample(ControllerMode::Auto, 99.0, 30.0);
    near(
        amplitude(&automatic_input(
            &loop_definition,
            &high,
            ControlExperimentKind::SetpointStep,
            0.1,
            false,
        )),
        -2.0,
        "with 99 degC measured the larger headroom is below, so the step goes down",
    );
    near(
        amplitude(&automatic_input(
            &loop_definition,
            &high,
            ControlExperimentKind::SetpointStep,
            0.1,
            true,
        )),
        1.0,
        "the model check goes the opposite way and is capped by the 1 degC headroom above 99",
    );
}

#[test]
fn automatic_identifies_a_loop_in_manual_by_a_plant_input_step_sized_by_the_input_range_and_its_headroom(
) {
    let loop_definition = tank_loop();
    let manual = live_sample(ControllerMode::Manual, 45.0, 30.0);
    assert_eq!(
        identification_method(&loop_definition, Some(&manual)),
        Ok(IdentificationMethod {
            kind: ControlExperimentKind::MvStep,
            manual: true,
            automatic: true,
        })
    );
    near(
        amplitude(&automatic_input(
            &loop_definition,
            &manual,
            ControlExperimentKind::MvStep,
            0.1,
            false,
        )),
        10.0,
        "10 % of the 100 % input range",
    );
    near(
        amplitude(&automatic_input(
            &loop_definition,
            &manual,
            ControlExperimentKind::MvStep,
            0.1,
            true,
        )),
        -7.5,
        "the model check steps the other way at 0.75 times the size",
    );
    let closed = live_sample(ControllerMode::Manual, 45.0, 0.0);
    near(
        amplitude(&automatic_input(&loop_definition, &closed, ControlExperimentKind::MvStep, 0.1, true)),
        7.5,
        "at 0 % there is no room below, so the model check keeps the direction at 0.75 times the size",
    );
}

#[test]
fn automatic_uses_a_pulse_when_sustained_holds_are_not_permitted_and_prbs_when_frequency_is_wanted()
{
    let manual = live_sample(ControllerMode::Manual, 45.0, 30.0);
    let mut pulse_only = tank_loop();
    pulse_only.identification_policy = Some(json!({
        "openLoopPermitted": true, "sustainedHoldPermitted": false, "information": "basic"
    }));
    assert_eq!(
        identification_method(&pulse_only, Some(&manual)).map(|method| method.kind),
        Ok(ControlExperimentKind::MvPulse)
    );
    let mut frequency = tank_loop();
    frequency.identification_policy = Some(json!({
        "openLoopPermitted": true, "sustainedHoldPermitted": true, "information": "frequency"
    }));
    assert_eq!(
        identification_method(&frequency, Some(&manual)).map(|method| method.kind),
        Ok(ControlExperimentKind::Prbs)
    );
}

#[test]
fn automatic_has_no_safe_method_for_a_loop_in_auto_without_a_setpoint_where_open_loop_tests_are_not_permitted(
) {
    let mut without_setpoint = tank_loop();
    without_setpoint.signals.setpoint = None;
    let auto = live_sample(ControllerMode::Auto, 45.0, 30.0);
    assert_eq!(
        identification_method(&without_setpoint, Some(&auto))
            .map(|method| (method.kind, method.manual)),
        Ok((ControlExperimentKind::MvStep, true)),
        "open-loop tests are permitted: a plant-input step in Manual"
    );
    let mut forbidden = without_setpoint;
    forbidden.identification_policy = Some(json!({
        "openLoopPermitted": false, "sustainedHoldPermitted": true, "information": "basic"
    }));
    let error = identification_method(&forbidden, Some(&auto)).expect_err("no safe method");
    assert_eq!(error, PolicyError::NoSafeMethodWithoutSetpoint);
    assert!(policy_error_text(&error).contains("no declared setpoint binding"));
}

#[test]
fn automatic_never_switches_a_loop_in_manual_into_auto_so_without_open_loop_tests_it_has_no_safe_method(
) {
    let mut closed_only = tank_loop();
    closed_only.identification_policy = Some(json!({
        "openLoopPermitted": false, "sustainedHoldPermitted": true, "information": "basic"
    }));
    let auto = live_sample(ControllerMode::Auto, 45.0, 30.0);
    let manual = live_sample(ControllerMode::Manual, 45.0, 30.0);
    assert_eq!(
        identification_method(&closed_only, Some(&auto)).map(|method| (method.kind, method.manual)),
        Ok((ControlExperimentKind::SetpointStep, false))
    );
    let error =
        identification_method(&closed_only, Some(&manual)).expect_err("no safe method in Manual");
    assert_eq!(error, PolicyError::NoSafeMethodInManual);
    let text = policy_error_text(&error);
    let loop_in_manual = text
        .find("loop is in Manual")
        .expect("the sentence names Manual");
    assert!(text[loop_in_manual..].contains("Put the loop in Auto"));
}

#[test]
fn an_explicit_method_is_kept_as_declared_and_a_plant_input_method_in_auto_switches_to_manual() {
    let mut explicit = tank_loop();
    if let Some(experiment) = explicit.experiment.as_mut() {
        experiment.kind = ExperimentKind::MvStep;
        experiment.amplitude = 10.0;
    }
    let auto = live_sample(ControllerMode::Auto, 45.0, 30.0);
    assert_eq!(
        identification_method(&explicit, Some(&auto)),
        Ok(IdentificationMethod {
            kind: ControlExperimentKind::MvStep,
            manual: true,
            automatic: false,
        })
    );
}

#[test]
fn a_pinned_method_is_checked_against_the_policy_and_a_load_step_needs_a_disturbance() {
    let mut pinned = tank_loop();
    pinned.identification_policy = Some(json!({
        "openLoopPermitted": false, "sustainedHoldPermitted": true, "information": "basic"
    }));
    if let Some(experiment) = pinned.experiment.as_mut() {
        experiment.kind = ExperimentKind::Prbs;
    }
    assert_eq!(
        identification_method(&pinned, None),
        Err(PolicyError::PinnedClosedLoopOnly)
    );

    let mut hold_restricted = tank_loop();
    hold_restricted.identification_policy = Some(json!({
        "openLoopPermitted": true, "sustainedHoldPermitted": false, "information": "basic"
    }));
    if let Some(experiment) = hold_restricted.experiment.as_mut() {
        experiment.kind = ExperimentKind::MvStep;
    }
    assert_eq!(
        identification_method(&hold_restricted, None),
        Err(PolicyError::PinnedHoldRestriction)
    );

    let mut load = tank_loop();
    if let Some(experiment) = load.experiment.as_mut() {
        experiment.kind = ExperimentKind::LoadStep;
    }
    assert_eq!(
        identification_method(&load, None),
        Err(PolicyError::PinnedLoadStep)
    );
}

#[test]
fn an_excitation_needs_a_fresh_sample_and_a_step_needs_room_to_move() {
    let tank = tank_loop();
    let no_sample = AmplitudeInput {
        loop_definition: &tank,
        sample: None,
        kind: ControlExperimentKind::MvStep,
        automatic: true,
        noise: 0.1,
        validation: false,
        fallback_amplitude: 10.0,
        on_off: false,
    };
    assert_eq!(
        identification_amplitude(&no_sample),
        Err(PolicyError::SampleNotAvailable)
    );

    // A measured range of one value leaves no room in either direction, so the step has no headroom.
    let mut flat = tank_loop();
    flat.signals.process_variable.range = crate::loop_definition::Range {
        minimum: 45.0,
        maximum: 45.0,
    };
    let sample = live_sample(ControllerMode::Auto, 45.0, 30.0);
    let error = identification_amplitude(&automatic_input(
        &flat,
        &sample,
        ControlExperimentKind::SetpointStep,
        0.1,
        false,
    ))
    .expect_err("no headroom");
    assert!(
        matches!(error, PolicyError::NoHeadroom { ref symbol, .. } if symbol == "Main.TT101.Value")
    );
    assert_eq!(error.failure().next_action, NextAction::OpenLimits);
    assert!(
        policy_error_text(&error).contains("has no headroom to step Main.TT101.Value from 45 degC")
    );
}

#[test]
fn an_identification_policy_must_name_a_usable_error_limit_and_order() {
    let base = json!({ "openLoopPermitted": true, "sustainedHoldPermitted": true, "information": "basic" });
    assert!(decode_identification_policy(&base).is_some());
    let mut order_too_high = base.clone();
    order_too_high["maximumOrder"] = json!(9);
    assert!(decode_identification_policy(&order_too_high).is_none());
    let mut error_too_large = base.clone();
    error_too_large["maximumNormalizedError"] = json!(1.0);
    assert!(decode_identification_policy(&error_too_large).is_none());
    let mut usable = base;
    usable["maximumNormalizedError"] = json!(0.15);
    usable["maximumOrder"] = json!(4);
    assert!(decode_identification_policy(&usable).is_some());
}

#[test]
fn an_on_off_actuator_is_read_from_the_controller() {
    let mut tank = tank_loop();
    assert!(!on_off_actuator(&tank));
    tank.controller.on_off = Some(json!({ "lowOutput": 0.0, "highOutput": 100.0 }));
    assert!(on_off_actuator(&tank));
}

#[test]
fn a_project_scripted_loop_takes_the_project_scenario_in_any_mode() {
    // C12 and B3.3: the project owns the excitation. Control Studio writes only the activation, as a project_sequence, so no
    // Manual output is needed and the mode is not changed, whatever the mode. Without an activation it only observes.
    let activated = project_tank(true);
    let auto = live_sample(ControllerMode::Auto, 45.0, 30.0);
    let manual = live_sample(ControllerMode::Manual, 45.0, 30.0);
    let scenario = IdentificationMethod {
        kind: ControlExperimentKind::ProjectSequence,
        manual: false,
        automatic: true,
    };
    assert_eq!(identification_method(&activated, Some(&auto)), Ok(scenario));
    assert_eq!(
        identification_method(&activated, Some(&manual)),
        Ok(scenario)
    );
    assert_eq!(
        identification_method(&activated, None),
        Ok(scenario),
        "without a sample the scenario does not need the mode"
    );
    let observed = IdentificationMethod {
        kind: ControlExperimentKind::Observation,
        ..scenario
    };
    assert_eq!(
        identification_method(&project_tank(false), Some(&manual)),
        Ok(observed)
    );
}

#[test]
fn an_explicit_excitation_of_a_project_scripted_loop_is_refused_as_a_duplicate() {
    // C12: the project owns the excitation, so an explicit Step, or any other excitation, would duplicate it. The refusal
    // offers Automatic, which observes the project's scenario, as its next action.
    let manual = live_sample(ControllerMode::Manual, 45.0, 30.0);
    let excitations = [
        ExperimentKind::MvStep,
        ExperimentKind::MvPulse,
        ExperimentKind::SetpointStep,
        ExperimentKind::LoadStep,
        ExperimentKind::Relay,
        ExperimentKind::Prbs,
        ExperimentKind::Multisine,
    ];
    for kind in excitations {
        let mut explicit = project_tank(true);
        if let Some(experiment) = explicit.experiment.as_mut() {
            experiment.kind = kind;
        }
        let error = identification_method(&explicit, Some(&manual))
            .expect_err("a duplicate excitation is refused");
        assert_eq!(error, PolicyError::DuplicateExcitation, "{kind:?}");
        assert_eq!(error.failure().next_action, NextAction::ChooseMethod);
        assert!(policy_error_text(&error).contains("no duplicate excitation"));
    }
}

#[test]
fn a_project_scenario_and_an_observation_write_no_amplitude() {
    // C12: Control Studio observes a project's scenario with zero external amplitude. The same loop in Manual sizes a
    // plant-input step at 10 (the Manual test above), so the zero comes from the scenario, not from the sample.
    let tank = tank_loop();
    let manual = live_sample(ControllerMode::Manual, 45.0, 30.0);
    near(
        amplitude(&automatic_input(
            &tank,
            &manual,
            ControlExperimentKind::MvStep,
            0.1,
            false,
        )),
        10.0,
        "a plant-input step is sized from the input range",
    );
    for kind in [
        ControlExperimentKind::ProjectSequence,
        ControlExperimentKind::Observation,
    ] {
        near(
            amplitude(&automatic_input(&tank, &manual, kind, 0.1, false)),
            0.0,
            "a project scenario and an observation write no amplitude",
        );
    }
}

#[test]
fn an_automatic_plant_input_step_names_a_missing_output_limit() {
    // B3.4 step 1 and B3.9: an automatic plant-input step needs both output limits. The refusal is the campaign's failure,
    // which names the missing field, and its next action opens Limits.
    let mut tank = tank_loop();
    tank.limits.output_minimum = None;
    let manual = live_sample(ControllerMode::Manual, 45.0, 30.0);
    let error = identification_amplitude(&automatic_input(
        &tank,
        &manual,
        ControlExperimentKind::MvStep,
        0.1,
        false,
    ))
    .expect_err("a plant-input step needs the output limits");
    assert_eq!(error.failure().next_action, NextAction::OpenLimits);
    assert!(policy_error_text(&error).contains("limits.outputMinimum"));
}
