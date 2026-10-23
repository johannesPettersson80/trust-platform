//! The independent model check of Validate (specification 35, B5.1 (a), B5.2 and B5.5; numerics N3.2 to N3.5). When the study's
//! model is provisional, a live campaign records the opposite excitation (`RunPurpose::RecordValidation`), the frozen model is
//! checked against that recording, and a pass qualifies the model on it. Every rule is in `trust-control`
//! (`model_validation_session`, `independent_validation` and the amplitude of `identification::policy`). This module makes the
//! runtime calls in their order, keeps the recording, and saves the Identify result that a pass qualifies.

use std::path::Path;
use std::sync::Arc;

use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::automatic::{CampaignStatus, IdentificationCampaignRecord};
use trust_control::identification::connected_run::{
    live_write_refusal, recording_not_kept, stored_fit_window, IDENTIFY_OPERATION,
};
use trust_control::identification::independent_validation::{
    recording_fingerprint, IndependentRow, RequirementStatus,
};
use trust_control::identification::recorded_limits::{
    evaluate_recorded_limits, RecordedLimitsEvaluation,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::model::general::ModelValidation;
use trust_control::model::GeneralModel;
use trust_control::runtime::RuntimeConnection;
use trust_control::study::current_model::CurrentModel;
use trust_control::study::general_model_persistence::SuppliedModelSection;
use trust_control::study::identify::IdentifySection;
use trust_control::validation::model_validation_session::{
    check_independent_model, check_supplied_model, estimation_record, maximum_normalized_error,
    model_validation_channels,
};
use trust_control::validation::texts;

use crate::identify_adapters::{RuntimeSampler, StudyRetention};
use crate::operation::OperationContext;
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::runtime_client::RuntimeClient;
use crate::storage::{self, StoredIdentify};
use crate::workflow::identify::run::connected_record;
use crate::workflow::identify::{
    run_identification, IdentifyPorts, IdentifyRequest, IdentifyResult, RunEvaluator, RunPurpose,
};

/// What a model check works on: the project folder, the loop, the study's current model, the study's stored Identify result with the
/// recording its model was fitted on (B10.2) when the model is a fit, and the settling time the check's requirements set (B3.4 step 2).
pub(crate) struct ModelCheckRequest<'a> {
    /// The project folder, where the study and its recording files are written.
    pub(crate) project_root: &'a Path,
    /// The loop the model was identified on, configured as its Identify campaign ran it (`configured_acquisition`, B3.4).
    pub(crate) loop_definition: &'a LoopDefinition,
    /// The study's current model, provisional (B5.2): a fit, or a supplied model.
    pub(crate) current: &'a CurrentModel,
    /// The study's stored Identify result with the recording file its model was fitted on. `None` for a supplied model, which was not
    /// fitted from a recording and has no estimation recording to check against (B5.2).
    pub(crate) stored: Option<&'a StoredIdentify>,
    /// The settling time the check's requirements set, in s; `None` when they set none (B3.4 step 2, A4.2).
    pub(crate) expected_settling_seconds: Option<f64>,
}

/// The outcome of the independent model check of an Identify result (B5.1 (a), B5.2, B5.5).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModelCheck {
    /// The model is already qualified: its Identify result names a validation recording, so no check runs.
    NotNeeded,
    /// The check passed, so the model is qualified on the check's recording. The Identify result now names that recording as its
    /// validation recording (B10.3).
    Passed {
        /// The identifier of the check's recording file in the study.
        recording_id: String,
        /// The largest normalized error of the frozen model's prediction over the whole response (B5.2, N3.2); `None` when
        /// no channel measured one.
        normalized_error: Option<f64>,
    },
    /// The check did not pass, so Validate stops with this failure (B5.5): the model must be identified again.
    Failed(Failure),
}

/// Runs the independent model check of the study's Identify result when its model is provisional (B5.1 (a), B5.2), and qualifies
/// the model when the check passes.
///
/// The refusals that need no runtime come first, in the order of `check_independent_model`: the loop's policy, the estimation
/// recording and the channels. Then the target is checked for a physical runtime (B3.4), the live campaign records the opposite
/// excitation and keeps its recording (`RunPurpose::RecordValidation`), and `trust-control` compares the frozen model with it. A
/// check that does not pass is `ModelCheck::Failed`. A pass names the kept recording as the validation recording of the Identify
/// result, with the model validated on it, and saves the result through `storage`.
///
/// # Errors
/// The loop's identification policy does not hold; the estimation recording cannot be rebuilt from the study; the model's
/// channels are not one input and one output; the target is physical, which needs an approval this path does not carry; the
/// campaign fails (see `run_identification`); the check cannot be made; or the recording cannot be kept or the result saved.
pub(crate) fn run_model_check(
    context: &OperationContext,
    connection: &RuntimeConnection,
    pending: &Arc<PendingReleases>,
    storage_folder: &Path,
    request: &ModelCheckRequest<'_>,
    IdentifyPorts { sampler, retention }: IdentifyPorts<'_, RuntimeSampler<'_>, StudyRetention<'_>>,
) -> Result<ModelCheck, Failure> {
    if !check_is_needed(request.current) {
        return Ok(ModelCheck::NotNeeded);
    }
    let loop_definition = request.loop_definition;
    let model = &request.current.model;
    let campaigns = estimation_campaigns(loop_definition, request.stored)?;
    maximum_normalized_error(loop_definition)?;
    if let Some(campaigns) = &campaigns
        && estimation_record(model, campaigns).is_none()
    {
        return Err(estimation_missing());
    }
    model_validation_channels(loop_definition, model)?;
    let source = RuntimeClient::connect(connection)?.status()?.source();
    if let Some(refusal) = live_write_refusal(source, IDENTIFY_OPERATION) {
        return Err(refusal);
    }
    let evaluate: RunEvaluator<'_, RecordedLimitsEvaluation> = &evaluate_recorded_limits;
    let run = IdentifyRequest {
        loop_definition,
        expected_settling_seconds: request.expected_settling_seconds,
        evaluate,
    };
    let outcome = run_identification(
        context,
        connection,
        pending,
        storage_folder,
        &run,
        RunPurpose::RecordValidation,
        IdentifyPorts {
            sampler: &mut *sampler,
            retention: &mut *retention,
        },
    )?;
    let IdentifyResult::Recorded(record) = outcome.result else {
        return Err(fitted_instead_of_recorded());
    };
    let check = match &campaigns {
        Some(campaigns) => check_independent_model(loop_definition, model, campaigns, &*record)?,
        None => check_supplied_model(loop_definition, model, &*record)?,
    };
    if let Some(failure) = check_failure(check.status, &check.rows) {
        return Ok(ModelCheck::Failed(failure));
    }
    let normalized_error = check
        .rows
        .iter()
        .filter_map(|row| row.normalized_error)
        .reduce(f64::max);
    let file_id = kept_file_id(retention, &outcome.recording_id)?;
    let fingerprint = recording_fingerprint(&check.record.run);
    if let Some(stored) = request.stored {
        let qualified = qualified_section(&stored.section, &file_id, fingerprint, normalized_error);
        // The current model checked again with its evidence is not a new model, so the manual identification stays (B3.7).
        storage::save_identify_keeping_manual(
            request.project_root,
            retention.study(),
            &qualified,
            &[],
        )?;
    } else {
        let qualified = qualified_supplied(model, fingerprint);
        storage::save_supplied_model_keeping_manual(
            request.project_root,
            retention.study(),
            &qualified,
        )?;
    }
    Ok(ModelCheck::Passed {
        recording_id: file_id,
        normalized_error,
    })
}

/// Whether the check runs: only a provisional model is checked (B5.1 (a)). A fit whose Identify result names its validation recording,
/// and a supplied model that carries evidence of independent validation, are already qualified.
#[must_use]
fn check_is_needed(current: &CurrentModel) -> bool {
    current.provisional()
}

/// The estimation campaign the check compares a fitted model against: the recording the model was fitted on, rebuilt from the study
/// with the request it was taken with (B3.6). `check_independent_model` finds the run whose fingerprint the model names. `None` for a
/// supplied model, which has no estimation recording (B5.2): its check compares the frozen model with the recorded run alone.
///
/// # Errors
/// The fitted model's Identify result keeps no request for its estimation recording, so the recording cannot be rebuilt; or the
/// rebuilt record does not pass the limits check (see `connected_record`).
fn estimation_campaigns(
    loop_definition: &LoopDefinition,
    stored: Option<&StoredIdentify>,
) -> Result<Option<Vec<IdentificationCampaignRecord<RecordedLimitsEvaluation>>>, Failure> {
    let Some(stored) = stored else {
        return Ok(None);
    };
    let Some(taken) = &stored.section.record_request else {
        return Err(estimation_missing());
    };
    let evaluate: RunEvaluator<'_, RecordedLimitsEvaluation> = &evaluate_recorded_limits;
    let record = connected_record(
        loop_definition,
        &stored.estimation.run,
        None,
        &taken.request.experiment,
        stored_fit_window(&taken.request),
        evaluate,
    )?;
    Ok(Some(vec![IdentificationCampaignRecord {
        status: CampaignStatus::Completed,
        reason: None,
        runs: vec![record],
    }]))
}

/// The failure of a check that did not pass (B5.5): the reason of its first row that did not pass, which says what to change,
/// with the next action Identify. `None` when the check passed.
#[must_use]
fn check_failure(status: RequirementStatus, rows: &[IndependentRow]) -> Option<Failure> {
    if status == RequirementStatus::Pass {
        return None;
    }
    let reason = rows
        .iter()
        .find(|row| row.status != RequirementStatus::Pass)
        .or(rows.first())
        .map_or("", |row| row.reason.as_str());
    Some(Failure::new(
        reason,
        Cause::ModelOrData,
        NextAction::Identify,
    ))
}

/// The Identify result a passing check keeps (B5.2): the check's recording is its validation recording, so it is no longer
/// provisional; its model is validated on that recording, and the check's error over the whole response is the model's validation
/// error, which Tune reads for a qualified model (B4.2). The other parts of the result are unchanged.
#[must_use]
fn qualified_section(
    section: &IdentifySection,
    file_id: &str,
    fingerprint: String,
    normalized_error: Option<f64>,
) -> IdentifySection {
    let presentation = section.presentation.clone().map(|mut shown| {
        shown.presentation.validation.normalized_error = normalized_error;
        shown
    });
    IdentifySection {
        validation_recording_id: Some(file_id.to_string()),
        provisional: false,
        selected_model: qualified_model(&section.selected_model, fingerprint),
        presentation,
        ..section.clone()
    }
}

/// The supplied model a passing check qualifies (B5.2): the model of [`qualified_model`], with no saved analysis. An analysis is kept
/// only for the model it was made for (`SuppliedModelAnalysis`), and the qualified model is a different model, so the analysis is
/// dropped and the page computes it again.
#[must_use]
fn qualified_supplied(model: &GeneralModel, fingerprint: String) -> SuppliedModelSection {
    SuppliedModelSection::new(qualified_model(model, fingerprint), None)
}

/// The model a passing check qualifies (B5.2): validated, with the fingerprint of the check's recording among its evidence. Its
/// revision, channels, delay and coefficients are unchanged.
#[must_use]
fn qualified_model(model: &GeneralModel, fingerprint: String) -> GeneralModel {
    let mut qualified = model.clone();
    let identity = match &mut qualified {
        GeneralModel::TransferFunction { identity, .. }
        | GeneralModel::ContinuousStateSpace { identity, .. }
        | GeneralModel::DiscreteStateSpace { identity, .. } => identity,
    };
    identity.validation = ModelValidation::Validated;
    identity.evidence.push(fingerprint);
    qualified
}

/// The identifier of the recording file that the check's runtime recording became (B10.3).
///
/// # Errors
/// The recording was not kept in the study.
fn kept_file_id(retention: &StudyRetention<'_>, recording_id: &str) -> Result<String, Failure> {
    retention
        .kept_file_id(recording_id)
        .map(str::to_string)
        .ok_or_else(|| recording_not_kept(recording_id))
}

/// The refusal of a model with no estimation recording to check it against: the text of `check_independent_model`.
fn estimation_missing() -> Failure {
    Failure::new(
        &texts::estimation_recording_missing(),
        Cause::ModelOrData,
        NextAction::Identify,
    )
}

/// A failure of the engine's own wiring (`Cause::Implementation`): the check's campaign fitted a model, which it never does.
fn fitted_instead_of_recorded() -> Failure {
    Failure::new(
        "The model check fitted a model instead of recording its run.",
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "validate_model_check_tests.rs"]
mod tests;
