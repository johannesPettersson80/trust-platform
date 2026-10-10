// Ported from `control-studio-identify-tune.test.ts`: the `prepareIdentificationReview` assertions, with their numbers.

use super::*;
use crate::identification::plant::tests::fopdt_samples;
use crate::identification::plant::Partition;

/// A review input of a simulated estimation recording, and of a validation recording when one is given, asking for the
/// recommended model with no other statement.
fn review_input(
    estimation: Vec<IdentificationSample>,
    validation: Option<Vec<IdentificationSample>>,
) -> IdentificationReviewInput {
    IdentificationReviewInput {
        recording: None,
        step_response_settings: None,
        first_order_step_response_settings: None,
        maximum_normalized_error: None,
        estimation,
        validation,
        estimation_recording_id: None,
        validation_recording_id: None,
        excluded: Vec::new(),
        source: DataSource::Simulation,
        requested_kind: IdentificationModelSelection::Automatic,
        declared_process: None,
        closed_loop: None,
    }
}

#[test]
fn a_distinct_validation_run_and_an_explicit_review_are_required_before_design_use() {
    let estimation = fopdt_samples(2.0);
    let preview = prepare_identification_review(&review_input(estimation.clone(), None))
        .expect("the estimation run is previewed");
    let IdentificationReview::ValidationRequired(preview) = preview else {
        panic!("a preview waits for validation");
    };
    assert!(preview.estimation_qualification.suitable);
    assert!(!preview.estimation_qualification.suitable_for_design);
    assert_eq!(preview.preview.partition, Some(Partition::SingleRun));

    let same =
        prepare_identification_review(&review_input(estimation.clone(), Some(estimation.clone())))
            .expect_err("the estimation run is no validation run");
    assert!(
        same.0
            .to_lowercase()
            .contains("distinct immutable validation run"),
        "{same}"
    );

    let qualified = prepare_identification_review(&IdentificationReviewInput {
        maximum_normalized_error: Some(0.02),
        ..review_input(estimation, Some(fopdt_samples(1.25)))
    })
    .expect("the validated review");
    let IdentificationReview::AcceptanceRequired(qualified) = qualified else {
        panic!("a validated review waits for acceptance");
    };
    assert!(
        qualified
            .validation_qualification
            .metrics
            .independent_validation_available
    );
    assert_eq!(qualified.result.partition, Some(Partition::IndependentRuns));
    // Specification N3.3: on this record the first order and the first order with delay are both acceptable at the limit
    // 0.02, and the resolved delay is selected (round 1 expected the first order).
    assert_eq!(
        qualified.selected.kind,
        LowOrderModelKind::FirstOrderDeadTime
    );
    assert_ne!(
        qualified.fit.points,
        [] as [crate::identification::evidence::ModelFitPoint; 0]
    );
}

#[test]
fn a_requested_wrong_model_order_is_refused_on_independent_validation() {
    let refusal = prepare_identification_review(&IdentificationReviewInput {
        requested_kind: IdentificationModelSelection::Integrating,
        maximum_normalized_error: Some(0.02),
        ..review_input(fopdt_samples(2.0), Some(fopdt_samples(1.25)))
    })
    .expect_err("the integrating model fails independent validation");
    let text = refusal.0.to_lowercase();
    let after_request = text
        .find("requested integrating model")
        .map(|start| &text[start..]);
    assert!(
        after_request.is_some_and(|rest| rest.contains("independent validation")),
        "{refusal}"
    );
}

#[test]
fn equal_deterministic_samples_validate_when_their_recording_identities_differ() {
    // From the workflow test "accepts equal deterministic samples only when their immutable Test recording identities
    // differ": its validation fit reaches this review with the two identities and no error limit of its own.
    let deterministic = fopdt_samples(2.0);
    let review = prepare_identification_review(&IdentificationReviewInput {
        estimation_recording_id: Some("test-recording-estimation".to_string()),
        validation_recording_id: Some("test-recording-validation".to_string()),
        ..review_input(deterministic.clone(), Some(deterministic))
    })
    .expect(
        "separate Test recording identities establish independent validation even when a deterministic simulator \
         returns equal values",
    );
    assert!(matches!(
        review,
        IdentificationReview::AcceptanceRequired(_)
    ));
}

#[test]
fn a_two_lag_request_with_step_response_settings_is_refused_while_the_sensor_lag_comparison_is_parked(
) {
    let settings: StepResponseSettings = serde_json::from_value(serde_json::json!({
        "baselineDurationSeconds": 5.0,
        "finalDurationSeconds": 5.0,
        "maximumWindowRange": 0.1,
        "maximumWindowSlopePerSecond": 0.01,
        "measurementBound": 0.05,
        "onsetThreshold": 0.15,
        "onsetSamples": 3,
        "delay": { "minimum": 0.0, "maximum": 3.0, "step": 0.5 },
        "timeConstant": { "minimum": 1.0, "maximum": 9.0, "step": 0.5 },
        "baselineStep": 0.01,
        "gainStep": 0.001,
        "timestampShiftSeconds": 0.0
    }))
    .expect("the step-response settings decode");
    let refusal = prepare_identification_review(&IdentificationReviewInput {
        step_response_settings: Some(settings),
        requested_kind: IdentificationModelSelection::Sopdt,
        estimation_recording_id: Some("retained-estimation".to_string()),
        ..review_input(fopdt_samples(2.0), None)
    })
    .expect_err("the sensor-lag comparison is parked");
    assert_eq!(
        refusal.0,
        "The sensor-lag comparison (second-order with dead time and a sensor lag) is not available yet."
    );
}
