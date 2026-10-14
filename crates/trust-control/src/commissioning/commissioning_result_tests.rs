#![allow(clippy::float_cmp)] // Targets and gains are compared exactly: they are the values the run wrote and reported.

use super::*;

use serde_json::json;

use crate::failure::{Cause, NextAction};
use crate::loop_definition::Range;
use crate::validation::commissioning_assessment::CommissioningCriteria;

use super::super::parameter_status::RecordState;

fn channel(id: &str, unit: &str) -> CommissioningChannel {
    CommissioningChannel {
        id: id.to_owned(),
        unit: unit.to_owned(),
    }
}

/// The three channels every run records: setpoint, measurement and command (`primary` in the `TypeScript` test).
fn primary() -> Vec<CommissioningChannel> {
    vec![
        channel("P1.SetpointC", "°C"),
        channel("P1.TemperatureC", "°C"),
        channel("P1.AppliedPowerPct", "%"),
    ]
}

fn criteria() -> CommissioningCriteria {
    CommissioningCriteria {
        target: 38.0,
        duration_seconds: 60.0,
        settling_time_seconds: 30.0,
        maximum_error: 0.2,
        task_period_ms: 100.0,
        process_range: Range {
            minimum: 0.0,
            maximum: 80.0,
        },
        output_limits: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
        switching: None,
    }
}

/// A result that records the given channels and failed on its response.
fn failed_result(channels: Vec<CommissioningChannel>) -> CommissioningResult {
    CommissioningResult {
        status: CommissioningStatus::Failed,
        reason: "The response left the declared band.".to_owned(),
        recording: None,
        transfers: Vec::new(),
        recovery_confirmed: true,
        maximum_error: None,
        target: 38.0,
        parameters: None,
        model_revision: "sha256:model".to_owned(),
        channels,
        criteria: criteria(),
        gain_recovery: None,
        failure: None,
        found_mode: None,
    }
}

// The TypeScript test `control-studio-commissioning-result.test.ts`: "keeps a commissioning result that recorded the loop's
// additional signals". Only its channel rule is ported here; the rest of the stored result's decoder is not.
#[test]
fn a_result_keeps_the_channels_it_recorded() {
    assert!(
        valid_channels(&primary()),
        "Setpoint, measurement and command alone are accepted"
    );
    let mut with_extra = primary();
    with_extra.push(channel("P1.DisturbanceKw", "kW"));
    with_extra.push(channel("P1.IntegralPct", "%"));
    assert!(
        valid_channels(&with_extra),
        "A recording of the loop's declared disturbance and diagnostics has more than three distinct channels"
    );
    let mut duplicated = primary();
    duplicated.push(primary()[1].clone());
    assert!(
        !valid_channels(&duplicated),
        "A channel listed twice is rejected"
    );
}

#[test]
fn a_result_needs_at_least_three_channels_and_no_blank_identifier() {
    assert!(
        !valid_channels(&primary()[..2]),
        "two channels are not enough"
    );
    let mut blank = primary();
    blank[2].id = "  ".to_owned();
    assert!(!valid_channels(&blank), "a blank identifier is refused");
}

fn checkpoint() -> CheckpointView {
    CheckpointView {
        stage: JournalStage::Completed,
        model_revision: "model:1".to_owned(),
        origin_program: "program:1".to_owned(),
        origin_binding: "binding:1".to_owned(),
        parameter_operation_id: "parameter:1".to_owned(),
        candidate: BTreeMap::from([("Main.Kp".to_owned(), 2.0)]),
        transfer: Some(TransferOwner {
            id: "transfer:1".to_owned(),
            program: "program:1".to_owned(),
            binding: "transfer:1".to_owned(),
        }),
    }
}

/// The completed receipt of the checkpoint: a passed, recovery-confirmed run whose Auto transfer is output-image confirmed.
fn receipt() -> CommissioningResult {
    let mut result = failed_result(primary());
    result.status = CommissioningStatus::Passed;
    result.model_revision = "model:1".to_owned();
    result.parameters = Some(ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("parameter:1".to_owned()),
        program_revision: "program:1".to_owned(),
        binding_revision: "binding:1".to_owned(),
        active_revision: "active:1".to_owned(),
        values: BTreeMap::from([("Main.Kp".to_owned(), 2.0)]),
        status: RecordState::Applied,
        previous_revision: None,
        previous_values: None,
        reason: None,
    });
    result.transfers = vec![TransferStatus {
        program_revision: "program:1".to_owned(),
        binding_revision: "transfer:1".to_owned(),
        values: BTreeMap::new(),
        runtime_time_ns: None,
        status: TransferState::Applied,
        reason: None,
        supported_tracking: None,
        transaction: Some(TransferTransaction {
            id: "transfer:1".to_owned(),
            program_revision: "program:1".to_owned(),
            binding_revision: "transfer:1".to_owned(),
            status: TransferState::Applied,
            reason: None,
            observed_values: None,
            confirmation: Some(TransferConfirmation::OutputImage),
            initial_values: None,
            applied_values: None,
            restored: None,
        }),
    }];
    result
}

// The TypeScript test `control-studio-commissioning-checkpoint.test.ts`: "accepts runtime recordings retained in rawSamples while
// monitor samples are empty". Its decoder assertions belong to the journal, which is transport; only the completion rule is here.
#[test]
fn only_the_receipt_of_this_checkpoint_completes_it() {
    let checkpoint = checkpoint();
    assert!(completed_matches_checkpoint(Some(&receipt()), &checkpoint));

    let recovered = CheckpointView {
        stage: JournalStage::Recovered,
        ..checkpoint.clone()
    };
    assert!(
        !completed_matches_checkpoint(Some(&receipt()), &recovered),
        "a checkpoint that is not completed has no receipt"
    );

    let mut older = receipt();
    if let Some(transaction) = older.transfers[0].transaction.as_mut() {
        transaction.id = "older-transfer".to_owned();
    }
    assert!(
        !completed_matches_checkpoint(Some(&older), &checkpoint),
        "an earlier operation with the same model and gains is not this checkpoint's receipt"
    );

    let mut other_model = receipt();
    other_model.model_revision = "model:other".to_owned();
    assert!(!completed_matches_checkpoint(
        Some(&other_model),
        &checkpoint
    ));
}

#[test]
fn a_receipt_must_be_recovery_confirmed_output_image_and_present() {
    let checkpoint = checkpoint();
    assert!(
        !completed_matches_checkpoint(None, &checkpoint),
        "no receipt, no completion"
    );

    let mut unconfirmed = receipt();
    unconfirmed.recovery_confirmed = false;
    assert!(!completed_matches_checkpoint(
        Some(&unconfirmed),
        &checkpoint
    ));

    let mut post_st = receipt();
    if let Some(transaction) = post_st.transfers[0].transaction.as_mut() {
        transaction.confirmation = Some(TransferConfirmation::PostSt);
    }
    assert!(
        !completed_matches_checkpoint(Some(&post_st), &checkpoint),
        "a transfer confirmed only by the post-execution check is not applied (E6.7)"
    );
}

#[test]
fn a_transfer_value_is_a_bare_number_or_a_bool_in_the_json() {
    assert_eq!(
        serde_json::to_value(TransferValue::Number(1.5)).expect("json"),
        json!(1.5)
    );
    assert_eq!(
        serde_json::to_value(TransferValue::Boolean(true)).expect("json"),
        json!(true)
    );
    let decoded: TransferValue = serde_json::from_value(json!(0.25)).expect("a number decodes");
    assert_eq!(decoded, TransferValue::Number(0.25));
    assert!(
        serde_json::from_value::<TransferValue>(json!("1")).is_err(),
        "a text is not a transfer value"
    );
}

#[test]
fn a_transfer_status_keeps_the_runtime_names_and_decodes_its_own_json() {
    let status = receipt().transfers.remove(0);
    let value = serde_json::to_value(&status).expect("json");
    assert_eq!(
        value["binding_revision"], "transfer:1",
        "the status keeps the runtime's snake_case names"
    );
    assert_eq!(value["transaction"]["confirmation"], "output-image");
    assert!(
        value["transaction"].get("initial_values").is_none(),
        "absent optional values are not written"
    );
    let decoded: TransferStatus = crate::json::decode_value(value).expect("the status decodes");
    assert_eq!(decoded, status);
}

#[test]
fn a_result_serializes_with_the_typescript_names_and_omits_its_absent_parts() {
    let value = serde_json::to_value(failed_result(primary())).expect("json");
    assert_eq!(value["status"], "failed");
    assert_eq!(value["recoveryConfirmed"], true);
    assert_eq!(value["modelRevision"], "sha256:model");
    assert_eq!(value["criteria"]["settlingTimeSeconds"], 30.0);
    assert!(
        value.get("maximumError").is_none(),
        "an unassessed result has no maximum error"
    );
    assert!(value.get("failure").is_none());
    assert!(value.get("foundMode").is_none());
}

#[test]
fn a_failure_is_written_with_its_typed_next_action() {
    let mut result = failed_result(primary());
    result.failure = Some(Failure::new(
        "The response left the declared band.",
        Cause::Settings,
        NextAction::Restore,
    ));
    let value = serde_json::to_value(&result).expect("json");
    assert_eq!(value["failure"]["nextAction"], "restore");
    assert_eq!(value["failure"]["cause"], "settings");
}

#[test]
fn the_run_statuses_are_written_in_lower_case() {
    assert_eq!(
        serde_json::to_value(CommissioningStatus::Passed).expect("json"),
        json!("passed")
    );
    assert_eq!(
        serde_json::to_value(CommissioningStatus::Cancelled).expect("json"),
        json!("cancelled")
    );
}

#[test]
fn a_result_decodes_its_own_json_with_its_failure() {
    let mut result = failed_result(primary());
    result.failure = Some(Failure::new(
        "The response left the declared band.",
        Cause::Settings,
        NextAction::Restore,
    ));
    let value = serde_json::to_value(&result).expect("json");
    let decoded: CommissioningResult =
        crate::json::decode_value(value).expect("the stored result decodes");
    assert_eq!(decoded, result);
}

#[test]
fn a_result_with_transfers_keeps_its_fractions_and_its_booleans() {
    let mut result = receipt();
    let mut transaction = result.transfers[0]
        .transaction
        .clone()
        .expect("a transaction");
    transaction.observed_values = Some(BTreeMap::from([
        ("Main.Auto".to_owned(), TransferValue::Boolean(false)),
        ("Main.Kp".to_owned(), TransferValue::Number(36.5)),
    ]));
    transaction.initial_values = Some(BTreeMap::from([(
        "LevelSP".to_owned(),
        TransferValue::Number(0.1),
    )]));
    result.transfers[0].transaction = Some(transaction);
    result.transfers[0].values =
        BTreeMap::from([("Main.Kp".to_owned(), TransferValue::Number(2.25))]);
    let value = serde_json::to_value(&result).expect("json");
    assert_eq!(value["transfers"][0]["values"]["Main.Kp"], 2.25);
    let decoded: CommissioningResult =
        crate::json::decode_value(value).expect("the stored transfers decode");
    assert_eq!(decoded, result);
}

#[test]
fn every_cause_and_next_action_decodes_from_the_name_it_serializes_to() {
    for cause in CAUSES {
        let mut result = failed_result(primary());
        result.failure = Some(Failure::new("A refusal.", cause, NextAction::None));
        let value = serde_json::to_value(&result).expect("json");
        let decoded: CommissioningResult =
            crate::json::decode_value(value).expect("the stored result decodes");
        assert_eq!(decoded.failure.map(|failure| failure.cause), Some(cause));
    }
    for action in NEXT_ACTIONS {
        let mut result = failed_result(primary());
        result.failure = Some(Failure::new("A refusal.", Cause::Runtime, action));
        let value = serde_json::to_value(&result).expect("json");
        let decoded: CommissioningResult =
            crate::json::decode_value(value).expect("the stored result decodes");
        assert_eq!(
            decoded.failure.map(|failure| failure.next_action),
            Some(action)
        );
    }
}

#[test]
fn a_failure_with_a_name_no_variant_writes_is_refused() {
    let mut result = failed_result(primary());
    result.failure = Some(Failure::new("A refusal.", Cause::Runtime, NextAction::None));
    let mut value = serde_json::to_value(&result).expect("json");
    value["failure"]["cause"] = json!("sideways");
    assert!(crate::json::decode_value::<CommissioningResult>(value).is_err());
}
