//! The pure rules of the independent model check (`modelValidationSession.ts`; specification 35, B5.2 and B5.5; numerics
//! N3.2, N3.4 and N3.5). The check runs the frozen model on a validation record that is distinct from its estimation
//! record. Four rules are kept here. The loop's identification policy gives the error limit: its declared limit, or the
//! workflow's default of 0.15 when it declares none. A stored policy that does not hold is refused, not defaulted. The
//! estimation record is the run whose fingerprint the model names as its evidence. The channels are the model's single
//! input and single output, mapped to the loop's manipulated variable and process variable; any other model is refused.
//! The check itself is `validate_frozen_model`.
//!
//! Left out (I/O and orchestration): the connected acquisition of the validation record, the analysis worker, the progress
//! reports, and the cancellation. The caller acquires the record and passes it in.

use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::automatic::IdentificationCampaignRecord;
use crate::identification::connected_record::RuntimeVerificationResult;
use crate::identification::independent_validation::{
    recording_fingerprint, validate_frozen_model, ChannelIdentity, IndependentModelValidation,
    ModelValidationChannels, RunEvaluation,
};
use crate::identification::policy::decode_identification_policy;
use crate::identification::workflow::DEFAULT_WORKFLOW_MAXIMUM_NORMALIZED_ERROR;
use crate::loop_definition::{LoopDefinition, LoopSignal};
use crate::model::GeneralModel;
use crate::validation::texts;

/// The key of the normalized error limit in a stored identification policy (`maximumNormalizedError`).
const LIMIT_KEY: &str = "maximumNormalizedError";

/// The estimation record of a model: the first run of any campaign whose fingerprint the model names among its evidence
/// (`campaigns.flatMap(c => c.runs).find(record => model.evidence.includes(recordingFingerprint(record.run)))`).
#[must_use]
pub fn estimation_record<'a, E>(
    model: &GeneralModel,
    campaigns: &'a [IdentificationCampaignRecord<E>],
) -> Option<&'a RuntimeVerificationResult<E>> {
    let evidence = &model.identity().evidence;
    campaigns
        .iter()
        .flat_map(|campaign| campaign.runs.iter())
        .find(|record| evidence.contains(&recording_fingerprint(&record.run)))
}

/// The channels of a single-input, single-output check: the model's input is the loop's manipulated variable and the
/// model's output is the loop's process variable, each with the signal's symbol and unit.
///
/// # Errors
/// The model does not have exactly one input and one output. The `TypeScript` throws the same sentence (`connectedModelChannels`
/// without channel bindings, `loopChannels.ts`): a model with several channels needs the channel bindings, which are not
/// ported yet (stage V7).
pub fn model_validation_channels(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<ModelValidationChannels, Failure> {
    let identity = model.identity();
    let ([input], [output]) = (identity.inputs.as_slice(), identity.outputs.as_slice()) else {
        return Err(Failure::new(
            &texts::channel_bindings_unmapped(),
            Cause::Implementation,
            NextAction::OpenChannels,
        ));
    };
    let signals = &loop_definition.signals;
    Ok(ModelValidationChannels {
        inputs: vec![channel(&input.id, &signals.manipulated_variable)],
        outputs: vec![channel(&output.id, &signals.process_variable)],
    })
}

/// The normalized error limit of the check (N3.2): the loop's identification policy's `maximumNormalizedError`, or the
/// workflow's default of 0.15 when the loop declares no policy or no limit (`identificationPolicy?.maximumNormalizedError ??
/// 0.15`). A stored policy that does not hold is refused, not defaulted. The predicate `validIdentificationPolicy` of
/// `identificationPolicy.ts` (line 17) requires a limit above zero and below one, and the loader rejects a loop whose policy
/// fails it (`loopDefinition.ts`, line 211). A limit that is null is declared, so it is refused as well.
///
/// # Errors
/// The loop's identification policy is stored and does not hold: its limit is not finite, is not above zero or not below one,
/// or is null, or another of its fields is invalid or missing.
pub fn maximum_normalized_error(loop_definition: &LoopDefinition) -> Result<f64, Failure> {
    let Some(stored) = loop_definition.identification_policy.as_ref() else {
        return Ok(DEFAULT_WORKFLOW_MAXIMUM_NORMALIZED_ERROR);
    };
    let refused = || {
        Failure::new(
            &texts::identification_policy_invalid(),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
    };
    let Some(policy) = decode_identification_policy(stored) else {
        return Err(refused());
    };
    if stored.get(LIMIT_KEY).is_some_and(Value::is_null) {
        return Err(refused());
    }
    Ok(policy
        .maximum_normalized_error
        .unwrap_or(DEFAULT_WORKFLOW_MAXIMUM_NORMALIZED_ERROR))
}

/// The independent model check of a frozen model on the record of its validation run (`checkIndependentModel`, its pure
/// part). The refusals come in the `TypeScript`'s order: the loop's policy (which the loader checks), then the estimation
/// record, then the channels. `validate_frozen_model` then makes the check, and a refused check is returned as an
/// inconclusive result, as that function does.
///
/// # Errors
/// The loop's identification policy does not hold (see [`maximum_normalized_error`]). No campaign run is the model's
/// estimation record, so the model has no evidence to be checked against: identify the plant again. The model does not
/// have a single input and a single output (see [`model_validation_channels`]).
pub fn check_independent_model<E: RunEvaluation + Clone>(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    campaigns: &[IdentificationCampaignRecord<E>],
    record: &RuntimeVerificationResult<E>,
) -> Result<IndependentModelValidation<E>, Failure> {
    let limit = maximum_normalized_error(loop_definition)?;
    let estimation = estimation_record(model, campaigns).ok_or_else(|| {
        Failure::new(
            &texts::estimation_recording_missing(),
            Cause::ModelOrData,
            NextAction::Identify,
        )
    })?;
    let channels = model_validation_channels(loop_definition, model)?;
    Ok(validate_frozen_model(
        model, estimation, record, &channels, limit,
    ))
}

/// A channel identity from the model's channel identifier and the loop's signal.
fn channel(id: &str, signal: &LoopSignal) -> ChannelIdentity {
    ChannelIdentity {
        id: id.to_owned(),
        symbol: signal.symbol.clone(),
        unit: signal.unit.clone(),
    }
}

#[cfg(test)]
#[path = "model_validation_session_tests.rs"]
mod tests;
