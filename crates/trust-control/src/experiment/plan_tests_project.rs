//! Plan review of the project-scripted experiments (spec C12): the project's own sequence owns the excitation, and Control
//! Studio writes only the activation. Split from `plan_tests.rs` by concept.
#![allow(clippy::float_cmp)] // The plans carry exact values on purpose.

use serde_json::{json, Value};

use super::super::{
    build_control_experiment_plan, ControlExperimentKind, ControlExperimentPlan,
    ControlExperimentPlanResult, ControlExperimentRequestKind, EvidenceSource,
    ExperimentModeBinding, ExperimentValue, LoopDefinition, PlanAbortBinding, PlanAuxiliarySignal,
    PlanInput, PlanSource, Quality,
};
use super::{
    accepted, point, refusal, request, sample, setpoint_stop_conditions, signal, with_experiment,
};
use crate::experiment::project_step_recording_ms;
use crate::recording::fixtures::{bound_loop, hold_loop};

fn project_observation(duration_seconds: f64, baseline_seconds: Option<f64>) -> Value {
    let mut experiment = json!({
        "execution": "project-scripted",
        "kind": "observation",
        "amplitude": 0.0,
        "durationSeconds": duration_seconds,
        "seed": 1,
        "activation": { "symbol": "P1.Run", "address": "%MX64.0", "iecType": "BOOL", "direction": "write" }
    });
    if let Some(baseline) = baseline_seconds {
        experiment["baselineSeconds"] = json!(baseline);
    }
    experiment
}

fn project_loop(duration_seconds: f64, baseline_seconds: Option<f64>) -> LoopDefinition {
    with_experiment(
        bound_loop(),
        project_observation(duration_seconds, baseline_seconds),
    )
}

#[test]
fn a_project_baseline_is_recorded_before_the_same_boolean_authority_is_enabled() {
    let loop_definition = project_loop(35.1, Some(5.0));
    let sample = sample();
    let plan = accepted(request(
        &loop_definition,
        &sample,
        ControlExperimentRequestKind::Observation,
        0.0,
        35.1,
    ));
    assert_eq!(plan.trajectory, [point(0.0, 0.0), point(5000.0, 1.0)]);
    assert_eq!(
        plan.duration_ms, 35_100.0,
        "the reviewed maximum includes baseline and active intervals"
    );
    assert_eq!(plan.from, ExperimentValue::Boolean(false));
    assert_eq!(plan.restoration_value, ExperimentValue::Boolean(false));
    for duration in [1.0, 5.0] {
        let result = request(
            &loop_definition,
            &sample,
            ControlExperimentRequestKind::Observation,
            0.0,
            duration,
        );
        assert!(
            matches!(result, ControlExperimentPlanResult::Rejected { .. }),
            "{duration} s is too short"
        );
    }
    for baseline in [-1.0, f64::NAN, f64::INFINITY, 35.1] {
        let mut loop_definition = project_loop(35.1, None);
        loop_definition
            .experiment
            .as_mut()
            .expect("the experiment is set")
            .baseline_seconds = Some(baseline);
        let result = request(
            &loop_definition,
            &sample,
            ControlExperimentRequestKind::Observation,
            0.0,
            35.1,
        );
        assert!(
            matches!(result, ControlExperimentPlanResult::Rejected { .. }),
            "baseline {baseline} s is refused"
        );
    }
}

#[test]
fn a_project_step_test_records_its_baseline_its_applied_hold_and_one_restoration_sample() {
    let loop_definition = hold_loop();
    let sample = sample();
    let review = |hold: Option<f64>| {
        build_control_experiment_plan(&PlanInput {
            loop_definition: &loop_definition,
            sample: Some(&sample),
            kind: ControlExperimentRequestKind::Observation,
            amplitude: 0.0,
            duration_seconds: 45.1,
            physical_topology_reviewed: false,
            seed: None,
            process_center: None,
            process_noise: None,
            hold_seconds: hold,
        })
    };
    for (hold, duration_ms) in [
        (30.0, 35_100.0),
        (40.0, 45_100.0),
        (20.0, 25_100.0),
        (25.05, 30_200.0),
    ] {
        let plan = accepted(review(Some(hold)));
        assert_eq!(plan.duration_ms, duration_ms, "a {hold} s hold records the 5 s baseline, the whole hold and one 0.1 s restoration sample");
    }
    assert_eq!(
        refusal(review(None)),
        "Apply a P1.HoldSeconds value within 20–40 s before reviewing the project test.",
        "the applied hold must be read before the test is reviewed"
    );
    assert_eq!(
        refusal(review(Some(41.0))),
        "Apply a P1.HoldSeconds value within 20–40 s before reviewing the project test.",
        "a hold outside its declared bounds is refused"
    );
    let declared_30 = with_experiment(
        hold_loop(),
        json!({
            "execution": "project-scripted", "kind": "observation", "amplitude": 0.0, "durationSeconds": 30.0,
            "seed": 1, "baselineSeconds": 5.0, "holdParameter": "P1.HoldSeconds",
            "activation": { "symbol": "P1.Run", "address": "%MX64.0", "iecType": "BOOL", "direction": "write" }
        }),
    );
    let needs = build_control_experiment_plan(&PlanInput {
        loop_definition: &declared_30,
        sample: Some(&sample),
        kind: ControlExperimentRequestKind::Observation,
        amplitude: 0.0,
        duration_seconds: 30.0,
        physical_topology_reviewed: false,
        seed: None,
        process_center: None,
        process_noise: None,
        hold_seconds: Some(30.0),
    });
    assert_eq!(
        refusal(needs),
        "A 30 s hold needs 35.1 s, more than the declared 30 s maximum.",
        "a hold that does not fit the declared duration is refused with both durations"
    );
}

#[test]
fn a_project_hold_parameter_that_the_loop_does_not_declare_is_named_in_the_refusal() {
    let mut loop_definition = hold_loop();
    loop_definition
        .parameter_binding
        .as_mut()
        .expect("the binding is set")
        .parameters
        .clear();
    let reason = refusal(build_control_experiment_plan(&PlanInput {
        loop_definition: &loop_definition,
        sample: Some(&sample()),
        kind: ControlExperimentRequestKind::Observation,
        amplitude: 0.0,
        duration_seconds: 45.1,
        physical_topology_reviewed: false,
        seed: None,
        process_center: None,
        process_noise: None,
        hold_seconds: Some(30.0),
    }));
    assert_eq!(
        reason,
        "Apply a P1.HoldSeconds value within its–declared s before reviewing the project test."
    );
}

#[test]
fn a_project_owned_excitation_rejects_forced_or_bad_primary_signals_and_keeps_auxiliary_diagnostics(
) {
    let loop_definition = project_loop(30.0, None);
    for key in ["setpoint", "processVariable", "manipulatedVariable"] {
        for quality in [Quality::Forced, Quality::Bad] {
            let mut observed = sample();
            match key {
                "setpoint" => {
                    observed
                        .setpoint
                        .as_mut()
                        .expect("the setpoint is set")
                        .quality = quality;
                }
                "processVariable" => observed.process_variable.quality = quality,
                _ => observed.manipulated_variable.quality = quality,
            }
            let result = request(
                &loop_definition,
                &observed,
                ControlExperimentRequestKind::Observation,
                0.0,
                30.0,
            );
            assert!(
                matches!(result, ControlExperimentPlanResult::Rejected { .. }),
                "{key} {quality:?} must block Boolean activation"
            );
        }
    }
    let mut observed = sample();
    observed.additional = vec![PlanAuxiliarySignal {
        id: "diagnostic".to_string(),
        signal: signal("Diagnostic", None, 1.0, "", Quality::Forced),
    }];
    accepted(request(
        &loop_definition,
        &observed,
        ControlExperimentRequestKind::Observation,
        0.0,
        30.0,
    ));
}

#[test]
fn a_distinct_latched_project_abort_is_reviewed_with_its_assertion() {
    let loop_definition = with_experiment(
        bound_loop(),
        json!({
            "execution": "project-scripted", "kind": "observation", "amplitude": 0.0, "durationSeconds": 30.0,
            "seed": 1,
            "activation": { "symbol": "P1.Run", "address": "%MX64.0", "iecType": "BOOL", "direction": "write" },
            "abort": { "symbol": "P1.Abort", "address": "%MX64.1", "iecType": "BOOL", "direction": "write", "assertedValue": true }
        }),
    );
    let plan = accepted(request(
        &loop_definition,
        &sample(),
        ControlExperimentRequestKind::Observation,
        0.0,
        30.0,
    ));
    assert_eq!(
        plan.abort_binding,
        Some(PlanAbortBinding {
            symbol: "P1.Abort".to_string(),
            address: "%MX64.1".to_string(),
            value_type: "BOOL".to_string(),
            asserted_value: true,
        })
    );
    assert!(plan
        .stop_conditions
        .iter()
        .any(|reason| reason.contains("P1.Abort") && reason.contains("true")));
    assert_eq!(
        plan.stop_conditions.last().map(String::as_str),
        Some("Abort or failure asserts P1.Abort = true; normal completion does not.")
    );
}

#[test]
fn an_invalid_project_duration_is_refused_before_approval_is_presented() {
    let loop_definition = project_loop(30.0, None);
    for duration in [0.0, 0.5, f64::NAN, f64::INFINITY, 86_401.0] {
        let result = request(
            &loop_definition,
            &sample(),
            ControlExperimentRequestKind::Observation,
            0.0,
            duration,
        );
        assert_eq!(
            refusal(result),
            "Duration must be between 1 s and 24 h.",
            "{duration} s"
        );
    }
}

#[test]
fn a_project_owned_lesson_is_started_and_restored_through_its_single_boolean_authority() {
    let loop_definition = with_experiment(
        bound_loop(),
        json!({
            "execution": "project-scripted", "kind": "observation", "amplitude": 0.0, "durationSeconds": 40.0,
            "seed": 20_260_829_u64,
            "activation": { "symbol": "P1.ExperimentEnabled", "address": "%MX96.0", "iecType": "BOOL", "direction": "write" }
        }),
    );
    let plan = accepted(request(
        &loop_definition,
        &sample(),
        ControlExperimentRequestKind::Observation,
        0.0,
        40.0,
    ));
    assert_eq!(
        plan,
        ControlExperimentPlan {
            id: "experiment:loop:tank:41:project_sequence".to_string(),
            source: PlanSource::Simulation,
            kind: ControlExperimentKind::ProjectSequence,
            affected_signal: "P1.ExperimentEnabled".to_string(),
            address: Some("%MX96.0".to_string()),
            value_type: "BOOL".to_string(),
            from: ExperimentValue::Boolean(false),
            to: ExperimentValue::Boolean(true),
            unit: "authority".to_string(),
            minimum: 0.0,
            maximum: 1.0,
            duration_ms: 40_000.0,
            expected_scan: 41,
            task: Some("MainTask".to_string()),
            mode_binding: Some(ExperimentModeBinding {
                symbol: "P1.Run".to_string(),
                manual_value: false,
            }),
            stop_conditions: setpoint_stop_conditions("40"),
            restoration_value: ExperimentValue::Boolean(false),
            requires_physical_approval: false,
            trajectory: Vec::new(),
            abort_binding: None,
            relay_feedback: None,
            interval_ms: None,
            seed: None,
            frequencies_hz: None,
        }
    );
    let mut physical = sample();
    physical.source = EvidenceSource::PhysicalPlant;
    assert_eq!(
        refusal(request(
            &loop_definition,
            &physical,
            ControlExperimentRequestKind::Observation,
            0.0,
            40.0
        )),
        "A reviewed physical topology is required before approval can be requested."
    );
}

#[test]
fn a_project_step_recording_holds_the_baseline_every_scan_of_the_hold_and_one_restoration_sample() {
    let loop_definition = hold_loop();
    assert_eq!(project_step_recording_ms(&loop_definition, 30.0), 35_100.0);
    assert_eq!(project_step_recording_ms(&loop_definition, 40.0), 45_100.0);
    assert_eq!(project_step_recording_ms(&loop_definition, 25.05), 30_200.0);
}
