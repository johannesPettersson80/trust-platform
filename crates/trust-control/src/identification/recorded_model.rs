//! The model fitted from recordings (`recordedModelIdentification.ts`; specification 35, B3.4, B3.9, B5.2 and N6.3): the
//! one entry point that turns one or two recordings of an experiment into a model. Its attempts run in the order the
//! `TypeScript` ran them, and every attempt that does not yield the model keeps its reason.
//!
//! The `TypeScript` ran each attempt in a worker and awaited it. Here each attempt is a plain function call; the bounded job
//! that cancels it is the orchestration's (the next layer). A refusal is the answer, not a failed attempt: no other fit may
//! replace it. A fit that is not refused is checked for the sign of its gain against the controller action (B3.9, N6.3).
//!
//! Not ported here: the multichannel fit (`multichannelIdentification.ts`), which is `multichannel_fit` and is reached by its own
//! entry, not by this one, since a loop with several channels has no single model channel to return here; and the state-model
//! fallback (`recordedStateIdentification.ts`, V7). Where the `TypeScript` would run the fallback, this returns
//! [`RecordedFitError::StateModelRequired`] with the failed attempts' sentences in the order they ran.

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::{LowOrderModelKind, SampleQuality};

use crate::experiment::plan::ControlExperimentKind;
use crate::experiment::{baseline_end_seconds, experiment_start_seconds};
use crate::identification::automatic::{
    AutomaticIdentificationResult, FitComparison, GeneralIdentificationFit,
};
use crate::identification::connected_record::{RuntimeVerificationResult, ValidationValue};
use crate::identification::data::{DataSource, IdentificationSample};
use crate::identification::evidence::{
    model_fit_evidence, pulse_fit_evidence, ModelFitEvidence, ModelFitPoint,
};
use crate::identification::excitation::identify_plant_from_excitation;
use crate::identification::experiment_trace::experiment_from_run;
use crate::identification::independent_validation::{
    recording_fingerprint, ValidationPredictionSeries,
};
use crate::identification::model_comparison::{comparison_presentation, ComparisonPresentation};
use crate::identification::plant::{identify_plant, IdentifiedModel, IdentifyPlantOptions};
use crate::identification::policy::{
    decode_identification_policy, IdentificationPolicy, PolicyProcess,
};
use crate::identification::refusal::Refusal;
use crate::identification::review::{
    prepare_identification_review, AcceptedIdentificationModel, IdentificationModelSelection,
    IdentificationReview, IdentificationReviewInput,
};
use crate::identification::selected_model_analysis::{
    provisional_identification_model, ChannelSet, OperatingValues,
};
use crate::identification::selected_recording::role_model_id;
use crate::identification::texts::{
    gain_sign_contradiction, zero_gain_refusal, MISSING_NONZERO_STEP,
};
use crate::identification::workflow::DEFAULT_WORKFLOW_MAXIMUM_NORMALIZED_ERROR;
use crate::loop_definition::{Action, LoopDefinition, LoopSignal};
use crate::model::{GeneralModel, ModelChannel, OperatingRange};
use crate::recording::channels::ChannelQuality;

/// Why a recorded fit gives no model.
#[derive(Debug, Clone, PartialEq)]
pub enum RecordedFitError {
    /// The fit refused: the sentence is the answer to show, and no other fit replaces it.
    Refused(Refusal),
    /// A path that moves with a later stage, named and not ported (for example the multichannel fit, V7).
    Parked {
        /// The path that is not ported, with its `TypeScript` file.
        path: &'static str,
    },
    /// No attempt gave the model, and the state-model fallback (V7, not ported) is what the `TypeScript` would try next.
    /// `attempts` are the failed attempts' sentences in the order they ran.
    StateModelRequired {
        /// The failed attempts, each a sentence `label: reason.`.
        attempts: Vec<String>,
    },
}

/// The kind of attempt a recording's experiment allows first (`provisional` in the `TypeScript`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provisional {
    /// A step, pulse or output step: the step-response fit.
    Step,
    /// A relay, PRBS, multisine, setpoint or load step, an observation or a project's scenario: the excitation fit (B3.5).
    Excitation,
}

/// The outcome of one provisional attempt that did not fail: a refusal, or the fitted model with its fit.
enum Attempt<E> {
    /// The fit refused; the sentence is the answer.
    Refused(String),
    /// The fitted result.
    Fitted(AutomaticIdentificationResult<E>),
}

/// Fits the model of one recording or of two independent recordings (`fitRecordedModel`). With one recording the simple
/// fits run first (a step or an excitation); with two, the independent-data fit runs (B5.2). The sign of a fitted gain is
/// checked against the loop's controller action before the model is returned.
///
/// # Errors
/// The recordings are not one or two; the loop has several channels (`multichannel_fit` fits those); a refusal of the fit; or no attempt gave the
/// model, when the state-model fallback (V7) would be needed.
pub fn fit_recorded_model<E: Clone>(
    loop_definition: &LoopDefinition,
    runs: &[RuntimeVerificationResult<E>],
) -> Result<AutomaticIdentificationResult<E>, RecordedFitError> {
    if runs.is_empty() || runs.len() > 2 {
        return Err(refused(
            "One estimation recording or two independent recordings are required.",
        ));
    }
    if has_several_channels(loop_definition) {
        return Err(RecordedFitError::Parked {
            path: "the multichannel fit (multichannelIdentification.ts, V7)",
        });
    }
    let mut failed: Vec<String> = Vec::new();
    if let [only] = runs {
        let (kind, label) = provisional_of(only);
        let attempt = match kind {
            Provisional::Step => step_fit(loop_definition, only),
            Provisional::Excitation => excitation_fit(loop_definition, only),
        };
        match attempt {
            Ok(Attempt::Refused(sentence)) => return Err(refused(&sentence)),
            Ok(Attempt::Fitted(result)) => {
                return checked_action(loop_definition, result).map_err(RecordedFitError::Refused);
            }
            Err(reason) => failed.push(sentence(&format!("{label}: {reason}"))),
        }
    } else {
        match two_recording_fit(loop_definition, runs) {
            Ok(accepted) => {
                let presentation =
                    presentation_of(loop_definition, &runs[1], &accepted.model, &accepted.fit)
                        .map_err(|reason| RecordedFitError::Refused(Refusal::new(reason)))?;
                let result = AutomaticIdentificationResult::Accepted {
                    accepted: Box::new(accepted),
                    runs: runs.to_vec(),
                    presentation: Box::new(presentation),
                };
                return checked_action(loop_definition, result).map_err(RecordedFitError::Refused);
            }
            Err(reason) => failed.push(sentence(&format!("Two-recording fit: {reason}"))),
        }
    }
    Err(RecordedFitError::StateModelRequired { attempts: failed })
}

/// Checks a fitted gain against the loop's controller action (`checkGainSign`, B3.9, N6.3): a zero gain is refused (N5.7),
/// and a gain whose sign contradicts the action is refused with the sentence that names the direction.
///
/// # Errors
/// The gain is zero, or its sign contradicts the controller action; the refusal is the sentence to show.
pub fn check_gain_sign(
    loop_definition: &LoopDefinition,
    gain: f64,
    integrating: bool,
) -> Result<(), Refusal> {
    let suffix = if integrating { "/s" } else { "" };
    let gain_unit = format!(
        "{}/{}{suffix}",
        loop_definition.signals.process_variable.unit,
        loop_definition.signals.manipulated_variable.unit
    );
    if gain == 0.0 {
        return Err(Refusal::new(zero_gain_refusal(&gain_unit)));
    }
    let action = match loop_definition.controller.action {
        Action::Direct => ControllerAction::Direct,
        Action::Reverse => ControllerAction::Reverse,
    };
    match gain_sign_contradiction(gain, &gain_unit, action) {
        Some(message) => Err(Refusal::new(message)),
        None => Ok(()),
    }
}

/// The attempt a recording's experiment allows first, with its label. A step, pulse or output step is a step-response fit.
/// Every other recording is fitted from its recorded plant input as an excitation, an observation or a project's scenario
/// included (B3.5, C12).
fn provisional_of<E>(run: &RuntimeVerificationResult<E>) -> (Provisional, &'static str) {
    match run.request.experiment.kind {
        ControlExperimentKind::MvStep
        | ControlExperimentKind::MvPulse
        | ControlExperimentKind::OutputStep => (Provisional::Step, "Step-response fit"),
        ControlExperimentKind::Relay
        | ControlExperimentKind::Prbs
        | ControlExperimentKind::Multisine
        | ControlExperimentKind::SetpointStep
        | ControlExperimentKind::LoadStep
        | ControlExperimentKind::Observation
        | ControlExperimentKind::ProjectSequence => (Provisional::Excitation, "Excitation fit"),
    }
}

/// What a single-input, single-output fit shows (B3.8): its presentation with the traces of the run it was read from, over the
/// loop's plant input and measured output. The failure is the sentence to keep.
fn presentation_of<E>(
    loop_definition: &LoopDefinition,
    run: &RuntimeVerificationResult<E>,
    low_order: &IdentifiedModel,
    fit: &ModelFitEvidence,
) -> Result<ComparisonPresentation, String> {
    let channels = channels_of(loop_definition);
    let (Some(input), Some(output)) = (channels.inputs.first(), channels.outputs.first()) else {
        return Err(String::from(
            "The identified loop has no plant input or measured output.",
        ));
    };
    let experiment =
        experiment_from_run(loop_definition, &run.run, None).map_err(|refusal| refusal.0)?;
    Ok(comparison_presentation(
        experiment, input, output, low_order, fit,
    ))
}

/// The step-response attempt on one recording (the provisional step worker): an identification of the recording from its
/// fit window, a refusal as the answer, or the model with its fit.
fn step_fit<E: Clone>(
    loop_definition: &LoopDefinition,
    run: &RuntimeVerificationResult<E>,
) -> Result<Attempt<E>, String> {
    let samples = identification_samples(loop_definition, run);
    let pulse = run.request.experiment.kind == ControlExperimentKind::MvPulse;
    let options = IdentifyPlantOptions {
        declared_process: declared_process(loop_definition),
        closed_loop: false,
        pulse,
    };
    let identified = identify_plant(
        &samples,
        &[],
        DataSource::Simulation,
        None,
        maximum_normalized_error(loop_definition),
        &options,
    )
    .map_err(|refusal| refusal.0)?;
    let refusal = identified.refusal.clone().or_else(|| {
        if identified.recommended.kind == LowOrderModelKind::Integrating {
            identified.integrator_refusal.clone()
        } else {
            None
        }
    });
    if let Some(refusal) = refusal {
        return Ok(Attempt::Refused(refusal));
    }
    let model_channels = channels_of(loop_definition);
    let model = provisional_identification_model(
        &identified.recommended,
        &samples,
        &model_channels,
        &recording_fingerprint(&run.run),
        None,
    )
    .map_err(|refusal| refusal.0)?;
    let fit = if pulse {
        pulse_fit_evidence(&identified.recommended, &samples)
    } else {
        model_fit_evidence(&identified.recommended, &samples)
    }
    .map_err(|refusal| refusal.0)?;
    let presentation = presentation_of(loop_definition, run, &identified.recommended, &fit)?;
    // N3.2: the reported error is the fit's own, over its response and normalised by the output span of its samples; a flat
    // output is refused by the single-run fit before this point.
    let normalized_error = identified.recommended.decisive_error();
    let fit_record = single_record_fit(
        loop_definition,
        run,
        &model,
        normalized_error,
        fit.points,
        "Step-response estimation fit; independent validation is pending.",
    );
    Ok(Attempt::Fitted(AutomaticIdentificationResult::Model {
        model: Box::new(model),
        runs: vec![run.clone()],
        fit: Some(fit_record),
        presentation: Box::new(presentation),
    }))
}

/// The excitation attempt on one recording (the provisional excitation worker): the simple candidates of the excitation
/// fit, with the operating point of the steady baseline, and the model with its fit.
fn excitation_fit<E: Clone>(
    loop_definition: &LoopDefinition,
    run: &RuntimeVerificationResult<E>,
) -> Result<Attempt<E>, String> {
    let samples = identification_samples(loop_definition, run);
    let baseline_end = baseline_end_seconds(
        run.request.baseline_start_seconds,
        run.request.baseline_end_seconds,
    );
    // A setpoint step is a closed-loop fit (B3.3), which the integrator rule does not apply to (N2.3a).
    let options = IdentifyPlantOptions {
        declared_process: declared_process(loop_definition),
        closed_loop: run.request.experiment.kind == ControlExperimentKind::SetpointStep,
        pulse: false,
    };
    let fit = match identify_plant_from_excitation(
        &samples,
        maximum_normalized_error(loop_definition),
        baseline_end,
        &options,
    ) {
        Ok(fit) => fit,
        // The missing nonzero step (N2.2, N2.6) is the answer of the fit, not a failed attempt: no other fit replaces it.
        Err(refusal) if refusal.0 == MISSING_NONZERO_STEP => {
            return Ok(Attempt::Refused(refusal.0));
        }
        Err(refusal) => return Err(refusal.0),
    };
    // The refusals of the recommended form are refusals of the attempt, not failed attempts (A10.3, N2.3a, B3.9).
    let refusal = fit.refusal.clone().or_else(|| {
        if fit.recommended.kind == LowOrderModelKind::Integrating {
            fit.integrator_refusal.clone()
        } else {
            None
        }
    });
    if let Some(refusal) = refusal {
        return Ok(Attempt::Refused(refusal));
    }
    let model_channels = channels_of(loop_definition);
    let model = provisional_identification_model(
        &fit.recommended,
        &samples,
        &model_channels,
        &recording_fingerprint(&run.run),
        Some(OperatingValues {
            input0: fit.input0,
            output0: fit.output0,
        }),
    )
    .map_err(|refusal| refusal.0)?;
    let fit_evidence =
        model_fit_evidence(&fit.recommended, &samples).map_err(|refusal| refusal.0)?;
    let presentation = presentation_of(loop_definition, run, &fit.recommended, &fit_evidence)?;
    let fit_record = single_record_fit(
        loop_definition,
        run,
        &model,
        fit.recommended.decisive_error(),
        fit.points,
        "Estimation fit to the recorded excitation; independent validation is pending.",
    );
    Ok(Attempt::Fitted(AutomaticIdentificationResult::Model {
        model: Box::new(model),
        runs: vec![run.clone()],
        fit: Some(fit_record),
        presentation: Box::new(presentation),
    }))
}

/// The independent-data fit of two recordings (the two-recording worker): the review of the estimation and validation
/// records, which must accept the model.
fn two_recording_fit<E>(
    loop_definition: &LoopDefinition,
    runs: &[RuntimeVerificationResult<E>],
) -> Result<AcceptedIdentificationModel, String> {
    accept_independent_samples(
        identification_samples(loop_definition, &runs[0]),
        identification_samples(loop_definition, &runs[1]),
        &runs[0].run.id,
        &runs[1].run.id,
        maximum_normalized_error(loop_definition),
    )
}

/// The acceptance of a model on two independent sample sets (`identificationWorker.ts`): the review of the estimation and
/// the validation, which must accept the model. The reason is the sentence the `TypeScript` threw.
///
/// # Errors
/// The review refuses the sets, or it asks for independent validation again.
pub fn accept_independent_samples(
    estimation: Vec<IdentificationSample>,
    validation: Vec<IdentificationSample>,
    estimation_recording_id: &str,
    validation_recording_id: &str,
    maximum_normalized_error: f64,
) -> Result<AcceptedIdentificationModel, String> {
    let input = IdentificationReviewInput {
        recording: None,
        step_response_settings: None,
        first_order_step_response_settings: None,
        maximum_normalized_error: Some(maximum_normalized_error),
        estimation,
        validation: Some(validation),
        estimation_recording_id: Some(estimation_recording_id.to_owned()),
        validation_recording_id: Some(validation_recording_id.to_owned()),
        excluded: Vec::new(),
        source: DataSource::Simulation,
        requested_kind: IdentificationModelSelection::Automatic,
        declared_process: None,
        closed_loop: None,
    };
    match prepare_identification_review(&input).map_err(|refusal| refusal.0)? {
        IdentificationReview::AcceptanceRequired(acceptance) => Ok(AcceptedIdentificationModel {
            model: acceptance.selected.clone(),
            fit: acceptance.fit.clone(),
            evidence: acceptance.result.source,
            review: acceptance,
        }),
        IdentificationReview::ValidationRequired(_) => {
            Err("Independent validation is required before model selection.".to_owned())
        }
    }
}

/// Checks the result of an attempt that did not fail (`checkedAction`): an accepted model's gain sign and its refusals, or
/// a model's static or rate gain. A refusal the review carries is raised here.
fn checked_action<E>(
    loop_definition: &LoopDefinition,
    result: AutomaticIdentificationResult<E>,
) -> Result<AutomaticIdentificationResult<E>, Refusal> {
    match &result {
        AutomaticIdentificationResult::Accepted { accepted, .. } => {
            let integrating = accepted.model.kind == LowOrderModelKind::Integrating;
            check_gain_sign(loop_definition, accepted.model.gain, integrating)?;
            let fitted = &accepted.review.result;
            if let Some(refusal) = &fitted.refusal {
                return Err(Refusal::new(refusal.clone()));
            }
            let integrator_blocked = integrating && !declares_integrating(loop_definition);
            if let (Some(refusal), true) = (&fitted.integrator_refusal, integrator_blocked) {
                return Err(Refusal::new(refusal.clone()));
            }
        }
        AutomaticIdentificationResult::Model { model, .. } => {
            if let GeneralModel::TransferFunction { transfer, .. } = model.as_ref() {
                if let Some((gain, integrating)) =
                    transfer_gain(&transfer.numerator, &transfer.denominator)
                {
                    check_gain_sign(loop_definition, gain, integrating)?;
                }
            }
        }
        AutomaticIdentificationResult::StateModel { .. } => {}
    }
    Ok(result)
}

/// The static gain of a transfer function, or its rate gain when it integrates (`transferGain`). The polynomials are in
/// descending powers, so the constant term is the last coefficient. `None` for a model with neither.
pub(crate) fn transfer_gain(numerator: &[f64], denominator: &[f64]) -> Option<(f64, bool)> {
    let top = *numerator.last()?;
    let constant = *denominator.last()?;
    if constant != 0.0 {
        return Some((top / constant, false));
    }
    let rate = denominator
        .len()
        .checked_sub(2)
        .and_then(|index| denominator.get(index))
        .copied();
    match rate {
        Some(rate) if rate != 0.0 => Some((top / rate, true)),
        _ => None,
    }
}

/// The fit of a single recording as the page shows it: one comparison, and the estimation predictions on that recording.
fn single_record_fit<E>(
    loop_definition: &LoopDefinition,
    run: &RuntimeVerificationResult<E>,
    model: &GeneralModel,
    normalized_error: f64,
    points: Vec<ModelFitPoint>,
    reason: &str,
) -> GeneralIdentificationFit {
    let output = loop_definition.signals.process_variable.unit.clone();
    let estimation = ValidationPredictionSeries {
        channel_id: model
            .identity()
            .outputs
            .first()
            .map_or_else(String::new, |channel| channel.id.clone()),
        unit: output,
        recording_id: run.run.id.clone(),
        model_revision: model.identity().revision.clone(),
        points,
    };
    GeneralIdentificationFit {
        comparisons: vec![FitComparison {
            order: denominator_order(model),
            normalized_errors: Some(vec![normalized_error]),
            reason: reason.to_owned(),
            adequate: true,
        }],
        validation_predictions: Vec::new(),
        estimation_predictions: vec![estimation],
    }
}

/// The order of a transfer model: the degree of its denominator (`model.denominator.length − 1`).
fn denominator_order(model: &GeneralModel) -> usize {
    match model {
        GeneralModel::TransferFunction { transfer, .. } => {
            transfer.denominator.len().saturating_sub(1)
        }
        _ => 0,
    }
}

/// The samples an identification reads from a recording: from the start of its steady window, each with the plant input
/// and the measured output and the quality good only when both are good (`samples` in `recordedModelIdentification.ts`).
#[must_use]
pub fn identification_samples<E>(
    loop_definition: &LoopDefinition,
    run: &RuntimeVerificationResult<E>,
) -> Vec<IdentificationSample> {
    let input_symbol = loop_definition.signals.manipulated_variable.symbol.as_str();
    let output_symbol = loop_definition.signals.process_variable.symbol.as_str();
    let from = experiment_start_seconds(run.request.baseline_start_seconds) - 1e-9;
    run.run
        .samples
        .iter()
        .filter(|sample| sample.time_seconds >= from)
        .map(|sample| {
            let input_reading = sample.values.get(input_symbol);
            let output_reading = sample.values.get(output_symbol);
            let good = |reading: Option<&ValidationValue>| {
                reading.is_some_and(|reading| matches!(reading.quality, ChannelQuality::Good))
            };
            IdentificationSample {
                time_seconds: sample.time_seconds,
                input: input_reading
                    .and_then(|reading| reading.value)
                    .unwrap_or(f64::NAN),
                output: output_reading
                    .and_then(|reading| reading.value)
                    .unwrap_or(f64::NAN),
                quality: if good(input_reading) && good(output_reading) {
                    SampleQuality::Good
                } else {
                    SampleQuality::Bad
                },
                input_unit: Some(loop_definition.signals.manipulated_variable.unit.clone()),
                output_unit: Some(loop_definition.signals.process_variable.unit.clone()),
                clipped: None,
                saturated: None,
            }
        })
        .collect()
}

/// The model channels of a single-recording fit: the loop's plant input and measured output, with the identifiers the loop
/// gives them (`modelChannelId`), or their symbols (`role_model_id`).
fn channels_of(loop_definition: &LoopDefinition) -> ChannelSet {
    let input = &loop_definition.signals.manipulated_variable;
    let output = &loop_definition.signals.process_variable;
    ChannelSet {
        inputs: vec![model_channel(
            role_model_id(loop_definition, "inputs"),
            input,
        )],
        outputs: vec![model_channel(
            role_model_id(loop_definition, "outputs"),
            output,
        )],
    }
}

fn model_channel(id: String, signal: &LoopSignal) -> ModelChannel {
    ModelChannel {
        id,
        unit: signal.unit.clone(),
        operating_range: OperatingRange {
            minimum: signal.range.minimum,
            maximum: signal.range.maximum,
        },
    }
}

/// Whether the loop has more than one input or output channel (`loop.channels?.inputs.length > 1`). Such a loop is fitted by
/// the multichannel fit (`multichannel_fit::fit_multichannel_model`), not here.
pub(crate) fn has_several_channels(loop_definition: &LoopDefinition) -> bool {
    loop_definition
        .channels
        .as_ref()
        .is_some_and(|channels| channels.inputs.len() > 1 || channels.outputs.len() > 1)
}

/// The decoded identification policy of the loop, when it has a valid one.
pub(crate) fn policy_of(loop_definition: &LoopDefinition) -> Option<IdentificationPolicy> {
    loop_definition
        .identification_policy
        .as_ref()
        .and_then(decode_identification_policy)
}

/// The process the loop's policy declares, as the plant fit reads it (`identificationPolicy?.process`).
fn declared_process(loop_definition: &LoopDefinition) -> Option<String> {
    policy_of(loop_definition)?
        .process
        .map(|process| match process {
            PolicyProcess::Integrating => "integrating".to_owned(),
        })
}

/// Whether the loop's policy declares an integrating process.
fn declares_integrating(loop_definition: &LoopDefinition) -> bool {
    declared_process(loop_definition).as_deref() == Some("integrating")
}

/// The largest normalised error the loop's policy allows, or the workflow's default (`?? 0.15`).
pub(crate) fn maximum_normalized_error(loop_definition: &LoopDefinition) -> f64 {
    policy_of(loop_definition)
        .and_then(|policy| policy.maximum_normalized_error)
        .unwrap_or(DEFAULT_WORKFLOW_MAXIMUM_NORMALIZED_ERROR)
}

/// The sentence of a refusal, as an error of the recorded fit.
fn refused(sentence: &str) -> RecordedFitError {
    RecordedFitError::Refused(Refusal::new(sentence))
}

/// Ends a reason with one full stop, without doubling one it already has (`sentence`).
fn sentence(text: &str) -> String {
    format!("{}.", text.strip_suffix('.').unwrap_or(text))
}

#[cfg(test)]
#[path = "recorded_model_tests.rs"]
mod tests;
