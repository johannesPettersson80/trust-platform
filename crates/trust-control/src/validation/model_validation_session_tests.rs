use super::*;
use crate::identification::automatic::CampaignStatus;
use crate::identification::independent_validation::{recording_fingerprint, RequirementStatus};
use crate::identification::record_fixtures::{fitted_model, setpoint_record, Row, Verdict};
use crate::model::{ModelChannel, OperatingRange};
use crate::recording::fixtures::loop_definition;
use serde_json::json;

/// A validation run of two scans on the loop's signals (`ValveMV` and `LevelPV`), with the given identifier.
fn run_record(id: &str) -> RuntimeVerificationResult<Verdict> {
    let rows: [Row; 2] = [
        (0.0, 1, vec![Some(0.0), Some(1.0)]),
        (0.1, 2, vec![Some(1.0), Some(2.0)]),
    ];
    setpoint_record(
        id,
        &[("ValveMV", "%"), ("LevelPV", "%")],
        &rows,
        RequirementStatus::Pass,
    )
}

fn campaign(
    runs: Vec<RuntimeVerificationResult<Verdict>>,
) -> IdentificationCampaignRecord<Verdict> {
    IdentificationCampaignRecord {
        status: CampaignStatus::Completed,
        reason: None,
        runs,
    }
}

/// The fixture loop with the given identification policy, as the loop file stores it.
fn loop_with_policy(policy: Value) -> LoopDefinition {
    let mut loop_definition = loop_definition();
    loop_definition.identification_policy = Some(policy);
    loop_definition
}

/// The refusal of a stored policy that does not hold (`identificationPolicy.ts`, line 17).
fn policy_refusal() -> Failure {
    Failure::new(
        &texts::identification_policy_invalid(),
        Cause::Settings,
        NextAction::OpenLoopSetup,
    )
}

#[test]
fn the_estimation_record_is_the_run_whose_fingerprint_the_model_names() {
    let first = run_record("first");
    let second = run_record("second");
    let campaigns = [campaign(vec![first, second.clone()])];
    let model = fitted_model(
        vec![recording_fingerprint(&second.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    assert_eq!(
        estimation_record(&model, &campaigns).map(|record| record.run.id.as_str()),
        Some("second")
    );
    let unnamed = fitted_model(vec!["sha256:unnamed".to_owned()], vec![1.0], vec![1.0, 1.0]);
    assert!(estimation_record(&unnamed, &campaigns).is_none());
}

#[test]
fn the_single_input_and_output_map_to_the_loop_signals_with_their_symbols_and_units() {
    let loop_definition = loop_definition();
    let model = fitted_model(Vec::new(), vec![1.0], vec![1.0, 1.0]);
    assert_eq!(
        model_validation_channels(&loop_definition, &model).expect("one input and one output map"),
        ModelValidationChannels {
            inputs: vec![ChannelIdentity {
                id: "MV".to_owned(),
                symbol: "ValveMV".to_owned(),
                unit: "%".to_owned(),
            }],
            outputs: vec![ChannelIdentity {
                id: "PV".to_owned(),
                symbol: "LevelPV".to_owned(),
                unit: "%".to_owned(),
            }],
        }
    );
}

#[test]
fn a_model_without_its_estimation_record_is_refused_and_asks_to_identify_again() {
    let loop_definition = loop_definition();
    let model = fitted_model(Vec::new(), vec![1.0], vec![1.0, 1.0]);
    let campaigns = [campaign(vec![run_record("first")])];
    let failure = check_independent_model(
        &loop_definition,
        &model,
        &campaigns,
        &run_record("validation"),
    )
    .expect_err("no campaign run is the model's estimation record");
    assert_eq!(
        failure,
        Failure::new(
            &texts::estimation_recording_missing(),
            Cause::ModelOrData,
            NextAction::Identify
        )
    );
}

#[test]
fn the_check_takes_the_estimation_record_the_loop_channels_and_the_default_error_limit() {
    let loop_definition = loop_definition();
    let estimation = run_record("first");
    let model = fitted_model(
        vec![recording_fingerprint(&estimation.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let campaigns = [campaign(vec![estimation])];
    let validation = check_independent_model(
        &loop_definition,
        &model,
        &campaigns,
        &run_record("validation"),
    )
    .expect("the estimation record is found");
    assert_eq!(validation.estimation_recording_id, "first");
    assert!(
        (validation.maximum_normalized_error - 0.15).abs() <= 1e-12,
        "the workflow's default limit is 0.15 (N3.2)"
    );
    assert_eq!(
        validation.channels,
        model_validation_channels(&loop_definition, &model).expect("the channels map")
    );
}

#[test]
fn a_model_with_more_than_one_input_is_refused_before_the_check() {
    // The TypeScript throws `channel_bindings_unmapped` in `connectedModelChannels` (loopChannels.ts).
    let loop_definition = loop_definition();
    let estimation = run_record("first");
    let mut model = fitted_model(
        vec![recording_fingerprint(&estimation.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let GeneralModel::TransferFunction { identity, .. } = &mut model else {
        panic!("the fixture is a transfer function");
    };
    identity.inputs.push(ModelChannel {
        id: "MV2".to_owned(),
        unit: "%".to_owned(),
        operating_range: OperatingRange {
            minimum: 0.0,
            maximum: 100.0,
        },
    });
    let campaigns = [campaign(vec![estimation])];
    let failure = check_independent_model(
        &loop_definition,
        &model,
        &campaigns,
        &run_record("validation"),
    )
    .expect_err("two inputs are not mapped to the loop");
    assert_eq!(
        failure,
        Failure::new(
            &texts::channel_bindings_unmapped(),
            Cause::Implementation,
            NextAction::OpenChannels
        )
    );
}

#[test]
fn the_default_limit_applies_when_the_loop_declares_no_policy_or_no_limit() {
    // `identificationPolicy?.maximumNormalizedError ?? 0.15`: no policy, and a policy without a limit, both give 0.15.
    let no_policy = loop_definition();
    assert!(
        (maximum_normalized_error(&no_policy).expect("no policy is no refusal") - 0.15).abs()
            <= 1e-12
    );
    let no_limit = loop_with_policy(json!({
        "openLoopPermitted": true,
        "sustainedHoldPermitted": true,
        "information": "frequency"
    }));
    assert!(
        (maximum_normalized_error(&no_limit).expect("a policy without a limit is no refusal")
            - 0.15)
            .abs()
            <= 1e-12
    );
}

#[test]
fn a_declared_limit_is_read_from_the_stored_policy() {
    let declared = loop_with_policy(json!({
        "maximumNormalizedError": 0.2,
        "openLoopPermitted": true,
        "sustainedHoldPermitted": false,
        "information": "basic"
    }));
    assert!(
        (maximum_normalized_error(&declared).expect("0.2 is above zero and below one") - 0.2).abs()
            <= 1e-12
    );
}

#[test]
fn a_stored_limit_outside_zero_and_one_or_not_a_number_is_refused_not_defaulted() {
    for stored in [
        json!(0.0),
        json!(1.0),
        json!(-0.1),
        json!(null),
        json!("0.2"),
    ] {
        let policy = loop_with_policy(json!({
            "maximumNormalizedError": stored,
            "openLoopPermitted": true,
            "sustainedHoldPermitted": true,
            "information": "basic"
        }));
        assert_eq!(
            maximum_normalized_error(&policy),
            Err(policy_refusal()),
            "the stored limit {stored} is refused"
        );
    }
}

#[test]
fn a_stored_policy_with_another_invalid_or_missing_field_is_refused() {
    // `validIdentificationPolicy`: the order is 1 to 8, and open-loop permission is a stated boolean.
    let order = loop_with_policy(json!({
        "maximumNormalizedError": 0.2,
        "openLoopPermitted": true,
        "sustainedHoldPermitted": true,
        "information": "basic",
        "maximumOrder": 9
    }));
    assert_eq!(maximum_normalized_error(&order), Err(policy_refusal()));
    let unstated = loop_with_policy(json!({
        "maximumNormalizedError": 0.2,
        "sustainedHoldPermitted": true,
        "information": "basic"
    }));
    assert_eq!(maximum_normalized_error(&unstated), Err(policy_refusal()));
}
