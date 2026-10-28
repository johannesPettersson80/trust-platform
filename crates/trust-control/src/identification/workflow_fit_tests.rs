//! Tests of the fits of Identify's review workflow, ported from the workflow assertions of
//! `control-studio-step-response.test.ts`, `control-studio-experiment.test.ts`,
//! `control-studio-operating-point.test.ts` and `control-studio-general-model-session.test.ts`. The parts that drive a
//! webview, the analysis sessions or the selected-model bridge are not ported.
// Grid values, measured levels and frozen parameters are compared exactly, as in the `TypeScript`.
#![allow(clippy::float_cmp)]

use super::fixtures::{
    delayed_samples, encoded, fit_evidence, ledger_samples, names_validation_bound_or_review,
    operating_point_samples, record, request, restored, selected, snapshot, step_analysis,
    step_request, step_settings, workflow,
};
use super::*;
use crate::experiment::plan::{
    build_control_experiment_plan, ControlExperimentPlan, ControlExperimentPlanResult,
    ControlExperimentRequestKind, ControllerMode, ExperimentSample, PlanInput, PlanSignal,
};
use crate::experiment::SampleQuality;
use crate::experiment::{
    identification_record_for_role, ExperimentIdentificationLedger,
    ExperimentIdentificationTerminal, ExperimentTerminalStatus, IdentificationRole,
    TerminalRestoration,
};
use crate::identification::data::identification_operating_point;
use crate::identification::plant::ResultSource;
use crate::recording::fixtures::loop_definition;
use crate::recording::EvidenceSource;
use crate::signals::Quality;
use trust_control_math::identify::LowOrderModelKind;

// control-studio-experiment.test.ts, control-studio-operating-point.test.ts,
// control-studio-general-model-session.test.ts

#[test]
fn the_first_test_record_estimates_and_the_next_one_validates() {
    let mut workflow = workflow();
    assert_eq!(workflow.next_fit_role(), FitRole::Estimation);
    let estimation = workflow.qualify(request(
        ledger_samples(5.0),
        IdentificationModelSelection::Automatic,
        None,
        None,
    ));
    assert!(estimation.suitable);
    let provisional = workflow.fit().expect("the estimation fits");
    assert!(matches!(
        provisional,
        IdentificationReview::ValidationRequired(_)
    ));
    assert_eq!(workflow.next_fit_role(), FitRole::Validation);
    let validation = workflow.qualify(request(
        ledger_samples(7.0),
        IdentificationModelSelection::Automatic,
        None,
        None,
    ));
    assert!(validation.suitable);
    let review = workflow.fit().expect("the validation fits");
    assert!(matches!(
        review,
        IdentificationReview::AcceptanceRequired(_)
    ));
}

/// The reviewed setpoint-step Test of `control-studio-experiment.test.ts` (line 611) at `scan`: the fixture loop, a setpoint
/// step of 5 for 30 s in Auto, from a sample with the setpoint 50, the measurement 48 and the manipulated variable 32.
fn reviewed_setpoint_step(scan: u64) -> ControlExperimentPlan {
    let loop_definition = loop_definition();
    let live = |symbol: &str, address: &str, value: f64| PlanSignal {
        symbol: symbol.to_string(),
        address: Some(address.to_string()),
        value,
        unit: "%".to_string(),
        quality: Quality::Good,
    };
    let sample = ExperimentSample {
        scan,
        source: EvidenceSource::SimulatedPlant,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(live("LevelSP", "%ML0", 50.0)),
        process_variable: live("LevelPV", "%IL0", 48.0),
        manipulated_variable: live("ValveMV", "%QL0", 32.0),
        additional: Vec::new(),
    };
    match build_control_experiment_plan(&PlanInput {
        loop_definition: &loop_definition,
        sample: Some(&sample),
        kind: ControlExperimentRequestKind::SetpointStep,
        amplitude: 5.0,
        duration_seconds: 30.0,
        physical_topology_reviewed: false,
        seed: None,
        process_center: None,
        process_noise: None,
        hold_seconds: None,
    }) {
        ControlExperimentPlanResult::Accepted { plan } => *plan,
        ControlExperimentPlanResult::Rejected { reason } => {
            panic!("the plan is refused: {reason}")
        }
    }
}

/// The terminal answer of a Test that ended restored with `status` (the `terminal` objects of the `TypeScript` test).
fn restored_terminal(
    experiment_id: &str,
    status: ExperimentTerminalStatus,
) -> ExperimentIdentificationTerminal {
    ExperimentIdentificationTerminal {
        experiment_id: experiment_id.to_string(),
        status,
        stop_reason: None,
        restoration: TerminalRestoration::Restored,
        restoration_reason: None,
    }
}

#[test]
fn a_completed_test_of_each_role_is_fitted_in_turn_and_a_failed_test_never_reaches_identify() {
    // control-studio-experiment.test.ts line 611: a failed Test (scan 41), then an estimation (scan 42) and a validation
    // (scan 43) are recorded in the ledger, and Identify takes the completed record of each role. The role selection
    // itself is covered by experiment/identification_record_tests.rs; here the fits run on the records it selects.
    let mut ledger = ExperimentIdentificationLedger::default();
    let failed_plan = reviewed_setpoint_step(41);
    let failed = ledger
        .record(
            &failed_plan,
            &restored_terminal(&failed_plan.id, ExperimentTerminalStatus::Failed),
            &ledger_samples(5.0),
            None,
        )
        .expect("the failed Test is recorded");
    let estimation_plan = reviewed_setpoint_step(42);
    let estimation = ledger
        .record(
            &estimation_plan,
            &restored_terminal(&estimation_plan.id, ExperimentTerminalStatus::Completed),
            &ledger_samples(5.0),
            None,
        )
        .expect("the estimation is recorded");
    let validation_plan = reviewed_setpoint_step(43);
    let validation = ledger
        .record(
            &validation_plan,
            &restored_terminal(&validation_plan.id, ExperimentTerminalStatus::Completed),
            &ledger_samples(7.0),
            None,
        )
        .expect("the validation is recorded");
    assert_eq!(
        identification_record_for_role(ledger.records(), IdentificationRole::Estimation)
            .map(|record| record.experiment_id.as_str()),
        Some(estimation.experiment_id.as_str())
    );
    assert_eq!(
        identification_record_for_role(ledger.records(), IdentificationRole::Validation)
            .map(|record| record.experiment_id.as_str()),
        Some(validation.experiment_id.as_str())
    );
    assert_ne!(
        identification_record_for_role(ledger.records(), IdentificationRole::Estimation)
            .map(|record| record.experiment_id.as_str()),
        Some(failed.experiment_id.as_str()),
        "failed Test records must not reach Identify"
    );

    let mut workflow = workflow();
    let estimation_samples =
        identification_record_for_role(ledger.records(), IdentificationRole::Estimation)
            .expect("the completed estimation is selected")
            .samples
            .clone();
    let estimation_qualification = workflow.qualify(request(
        estimation_samples,
        IdentificationModelSelection::Automatic,
        None,
        None,
    ));
    assert!(estimation_qualification.suitable);
    let provisional = workflow.fit().expect("the estimation fits");
    assert!(matches!(
        provisional,
        IdentificationReview::ValidationRequired(_)
    ));
    assert_eq!(workflow.next_fit_role(), FitRole::Validation);
    let validation_samples =
        identification_record_for_role(ledger.records(), IdentificationRole::Validation)
            .expect("the completed validation is selected")
            .samples
            .clone();
    let validation_qualification = workflow.qualify(request(
        validation_samples,
        IdentificationModelSelection::Automatic,
        None,
        None,
    ));
    assert!(validation_qualification.suitable);
    let review = workflow.fit().expect("the validation fits");
    assert!(matches!(
        review,
        IdentificationReview::AcceptanceRequired(_)
    ));
}

#[test]
fn the_accepted_review_keeps_the_estimation_operating_point() {
    let mut workflow = workflow();
    workflow.qualify(request(
        operating_point_samples(1.0, 10.0),
        IdentificationModelSelection::FirstOrder,
        None,
        None,
    ));
    workflow.fit().expect("the estimation fits");
    workflow.qualify(request(
        operating_point_samples(0.75, 11.0),
        IdentificationModelSelection::FirstOrder,
        None,
        None,
    ));
    workflow.fit().expect("the validation fits");
    let accepted = workflow.accept().expect("the validated model is accepted");
    let point = identification_operating_point(&accepted.review.estimation)
        .expect("the estimation has an operating point");
    assert_eq!(
        (point.input0, point.output0),
        (2.0, 10.0),
        "Independent validation coordinates must not replace the fitted estimation point"
    );
}

#[test]
fn an_independently_validated_fopdt_is_accepted_with_its_delay() {
    let mut workflow = workflow();
    workflow.qualify(request(
        delayed_samples(1.0),
        IdentificationModelSelection::Fopdt,
        None,
        None,
    ));
    workflow.fit().expect("the estimation fits");
    workflow.qualify(request(
        delayed_samples(1.5),
        IdentificationModelSelection::Fopdt,
        None,
        None,
    ));
    let review = workflow.fit().expect("the validation fits").clone();
    let accepted = workflow.accept().expect("the validated model is accepted");
    assert_eq!(accepted.model.kind, LowOrderModelKind::FirstOrderDeadTime);
    assert!(accepted.model.time_constant_seconds.is_some());
    assert!(accepted.model.dead_time_seconds.is_some());
    assert_eq!(accepted.evidence, ResultSource::FromSimulation);
    assert_eq!(encoded(&accepted.model), encoded(selected(&review)));
    assert_ne!(
        encoded(&accepted.review.estimation),
        encoded(&accepted.review.validation),
        "independent record identities remain distinct"
    );
}

// control-studio-step-response.test.ts

#[test]
fn the_workflow_keeps_the_measured_windows_and_the_consistent_grid() {
    let mut workflow = workflow();
    let qualification = workflow.qualify(step_request(record(10.0), "estimation"));
    assert!(qualification.suitable);
    let review = workflow.fit().expect("the estimation fits");
    assert!(matches!(
        review,
        IdentificationReview::ValidationRequired(_)
    ));
    let analysis = step_analysis(review)
        .expect("the requested measured-step analysis must be retained by the existing workflow");
    assert_eq!(analysis.baseline.mean, 35.0);
    assert!(analysis.final_window.mean > 36.99);
    assert_eq!(analysis.input_change, 10.0);
    assert!(analysis.consistency.count > 0);
    let gain = analysis
        .consistency
        .gain
        .expect("the consistent set has a gain interval");
    assert!(gain.minimum <= 0.2 && gain.maximum >= 0.2);
    assert_eq!(analysis.best.delay_seconds, 1.0);
    assert_eq!(analysis.best.time_constant_seconds, 5.0);
    assert!(workflow.accepted_model().is_none());
}

#[test]
fn a_drifting_final_window_is_a_partial_response_and_a_bad_hold_sample_is_refused() {
    for drift in [true, false] {
        let mut data = record(10.0);
        if drift {
            for sample in &mut data {
                if sample.time_seconds >= 35.0 {
                    sample.output += 0.03 * (sample.time_seconds - 35.0);
                }
            }
        } else {
            data[100].quality = SampleQuality::Bad;
        }
        let mut workflow = workflow();
        let qualification = workflow.qualify(step_request(data, "estimation"));
        if drift {
            // Only the baseline must be steady (N2.4); a final window that is still moving is a partial response
            // (A10.3), reported as unsteady.
            assert!(
                qualification.suitable,
                "a drifting final window must not refuse the bounded step"
            );
            let measurement = qualification
                .step_response
                .as_ref()
                .expect("the held step is measured");
            assert!(
                measurement.final_window.range > step_settings().maximum_window_range,
                "the drifting final window stays reported as unsteady"
            );
            assert!(
                workflow.fit().is_ok(),
                "a partial response with a moving final window is still fitted (A10.3)"
            );
        } else {
            assert!(
                !qualification.suitable,
                "a bad sample must prevent bounded step qualification"
            );
            assert!(workflow.fit().is_err());
        }
    }
}

#[test]
fn a_rejected_independent_prediction_stays_inspectable_through_reopen_without_acceptance() {
    let mut workflow = workflow();
    workflow.qualify(step_request(record(10.0), "estimation"));
    let frozen = selected(workflow.fit().expect("the estimation fits")).clone();
    let holdout = record(12.0)
        .into_iter()
        .map(|sample| IdentificationSample {
            output: 35.0 + (sample.output - 35.0) * 1.2,
            ..sample
        })
        .collect();
    assert!(
        workflow
            .qualify(step_request(holdout, "validation"))
            .suitable
    );
    // The validation fit may fail or return a failed review; both leave the review for inspection.
    let _ = workflow.fit();
    let state = workflow.current_state();
    let review = state
        .review
        .as_ref()
        .expect("A rejected independent prediction must remain inspectable");
    assert_ne!(
        fit_evidence(review).points,
        [] as [crate::identification::evidence::ModelFitPoint; 0]
    );
    assert_eq!(state.phase, IdentificationPhase::Failed);
    assert!(workflow.accepted_model().is_none());
    let refusal = workflow
        .accept()
        .expect_err("a failed validation is not accepted");
    assert!(names_validation_bound_or_review(&refusal.0));
    assert_eq!(selected(review).gain, frozen.gain);
    assert_eq!(
        selected(review).time_constant_seconds,
        frozen.time_constant_seconds
    );
    assert_eq!(selected(review).dead_time_seconds, frozen.dead_time_seconds);
    let bound = step_settings().measurement_bound;
    assert!(fit_evidence(review)
        .points
        .iter()
        .any(|point| point.residual.abs() > bound));

    let mut reopened = restored(&snapshot(&workflow));
    let reopened_state = reopened.current_state();
    assert_eq!(reopened_state.phase, IdentificationPhase::Failed);
    let reopened_review = reopened_state
        .review
        .as_ref()
        .expect("the reopened failure keeps its review");
    assert_eq!(
        encoded(&fit_evidence(reopened_review).points),
        encoded(&fit_evidence(review).points)
    );
    let refusal = reopened
        .accept()
        .expect_err("a reopened failure is not accepted");
    assert!(names_validation_bound_or_review(&refusal.0));
    assert!(reopened_state
        .failure
        .as_deref()
        .is_some_and(|failure| failure.contains("independent measurement bound")));
}

#[test]
fn a_changed_validation_step_is_scored_with_the_frozen_estimation_parameters() {
    let mut workflow = workflow();
    workflow.qualify(step_request(record(10.0), "estimation"));
    let provisional = workflow.fit().expect("the estimation fits").clone();
    workflow.qualify(step_request(record(12.0), "validation"));
    let validated = workflow.fit().expect("the validation fits").clone();
    assert_eq!(
        selected(&validated).gain,
        selected(&provisional).gain,
        "independent validation must not refit the estimation gain"
    );
    let (Some(validated_step), Some(provisional_step)) =
        (step_analysis(&validated), step_analysis(&provisional))
    else {
        panic!("both reviews keep the measured-step analysis");
    };
    assert_eq!(
        encoded(&validated_step.best),
        encoded(&provisional_step.best)
    );
    assert_eq!(
        selected(&validated).dead_time_seconds,
        selected(&provisional).dead_time_seconds
    );
    assert!(matches!(
        validated,
        IdentificationReview::AcceptanceRequired(_)
    ));
    assert!(workflow.accepted_model().is_none());
    let accepted = workflow.accept().expect("the validated model is accepted");
    assert_eq!(encoded(&accepted.model), encoded(selected(&validated)));
}

#[test]
fn the_validation_fit_keeps_the_source_of_the_frozen_estimation_grid() {
    let mut workflow = workflow();
    workflow.qualify(step_request(record(10.0), "estimation"));
    workflow.fit().expect("the estimation fits");
    workflow.qualify(step_request(record(12.0), "validation"));
    let review = workflow.fit().expect("the validation fits");
    let analysis = fit_evidence(review)
        .step_response
        .as_ref()
        .expect("the fit keeps the estimation analysis");
    assert_eq!(
        analysis.input_change, 10.0,
        "the frozen estimation grid keeps its original source"
    );
}
