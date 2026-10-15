//! What an identification gives the page and the command line (specification 35, A8.4 and B3.8; Identify page specification,
//! "Identified model and plot"): the signals an identification used, the readable result of a stored Identify section with
//! its response plot, the evidence the study gives the stages, and the candidate a study keeps, as Tune answers it (A8.6).
//!
//! The result is read from the stored section, never from a fit run again (A8.6): the model type and the key numbers come
//! from its presentation ([`identified_model_type_text`], [`identified_model_key_numbers`]), and the plot from the traces of
//! the experiment and the predicted response the presentation keeps.

use crate::experiment::ControlExperimentKind;
use crate::failure::Failure;
use crate::identification::connected_run::several_channels;
use crate::identification::experiment_trace::ExperimentTrace;
use crate::identification::model_comparison::ComparisonPresentation;
use crate::identification::presentation::{
    identified_model_key_numbers, identified_model_type_text,
};
use crate::identification::recording_import::display_engineering_unit;
use crate::loop_definition::{channel_signal, LoopDefinition};
use crate::messages::{
    CandidateOutcome, IdentifyPlot, IdentifyResult, PlotRole, PlotTrace, TuneMethod, TuneResult,
};
use crate::model::{GeneralModel, ModelChannel, ModelValidation};
use crate::stages::{stage_availability, StageEvidence};
use crate::study::current_model::{current_model, CurrentModel};
use crate::study::general_model_persistence::supplied_model_words;
use crate::study::identify::{IdentifySection, IdentifySignals};
use crate::study::inputs::InputsSection;
use crate::study::tune::model::tuning_model;
use crate::study::tune::prediction::CandidateResponse;
use crate::study::tune::record::candidate_settings;
use crate::study::tune::requirements::study_requirements;
use crate::study::tune::result::{tune_result, ResultInputs};
use crate::study::tune::{controller_action, TuneSection};
use crate::study::Study;
use crate::tuning::objective::AUTOMATIC_OBJECTIVE;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::mapped_parameters;
use crate::tuning::texts::MANUAL_DESCRIPTION;

/// What the text adds to the model type of a provisional model (B3.5).
const PROVISIONAL_LABEL: &str = " · Provisional";

/// The signals an identification of `kind` uses on `loop_definition` (B3.2): the manipulated variable as the plant input,
/// the measurement as the measured output, and the setpoint, by symbol, only for a setpoint step (closed-loop
/// identification, B3.3).
#[must_use]
pub fn identify_signals(
    loop_definition: &LoopDefinition,
    kind: ControlExperimentKind,
) -> IdentifySignals {
    let signals = &loop_definition.signals;
    let (inputs, outputs): (Vec<String>, Vec<String>) = match loop_definition.channels.as_ref() {
        Some(channels) if several_channels(loop_definition) => (
            channels
                .inputs
                .iter()
                .map(|input| channel_symbol(loop_definition, &input.signal_id))
                .collect(),
            channels
                .outputs
                .iter()
                .map(|output| channel_symbol(loop_definition, &output.signal_id))
                .collect(),
        ),
        _ => (
            vec![signals.manipulated_variable.symbol.clone()],
            vec![signals.process_variable.symbol.clone()],
        ),
    };
    IdentifySignals {
        inputs,
        outputs,
        setpoint: signals
            .setpoint
            .as_ref()
            .filter(|_| kind == ControlExperimentKind::SetpointStep)
            .map(|setpoint| setpoint.symbol.clone()),
    }
}

/// The symbol of the loop signal a channel is bound to. A binding that names no declared signal keeps its identifier.
fn channel_symbol(loop_definition: &LoopDefinition, signal_id: &str) -> String {
    channel_signal(loop_definition, signal_id)
        .map_or_else(|| signal_id.to_owned(), |signal| signal.symbol.to_owned())
}

/// The result of the identification a section stores (B3.8): its model type, equation and key numbers, whether it is
/// provisional, the readable text, the response plot, the recording the model was fitted from, the revision of the model, the
/// requirements it is given, and `stages`. The text is the model type, " · Provisional" for a provisional model, then the
/// equation and the key numbers, each on its own line. The revision is the one Validate's model check names for this model
/// (B5.2). The requirements are the record [`identify_answer`] works out, and `None` where Automatic cannot form one. A section
/// with no presentation, a state-space model (B10.2), is answered as a supplied model is.
#[must_use]
pub fn identify_result(
    section: &IdentifySection,
    evidence: StageEvidence,
    requirements: Option<TuningRequirements>,
) -> IdentifyResult {
    let Some(comparison) = section.presentation.as_ref() else {
        return supplied_result(&section.selected_model, evidence, requirements);
    };
    let shown = &comparison.presentation;
    let model_type = identified_model_type_text(shown);
    let key_numbers = identified_model_key_numbers(shown);
    let label = if section.provisional {
        PROVISIONAL_LABEL
    } else {
        ""
    };
    let text = format!("{model_type}{label}\n{}\n{key_numbers}", shown.equation);
    IdentifyResult {
        model_type,
        equation: shown.equation.clone(),
        key_numbers,
        provisional: section.provisional,
        text,
        plot: response_plot(section, comparison),
        recording_id: Some(section.estimation_recording_id.clone()),
        model_revision: section.selected_model.identity().revision.clone(),
        requirements,
        stage_evidence: evidence,
        stages: stage_availability(&evidence),
    }
}

/// The Identify answer of the study's current model `current` (B3.8, B3.7, A8.6): for a fit, [`identify_result`] of its section;
/// for a supplied model, [`supplied_result`]. Both carry the requirements Tune designs to for the model now (B4.3), which come
/// from the study's Tune section and its recorded inputs.
///
/// # Errors
/// The study's Tune section or its inputs cannot be read.
pub fn identify_answer(
    study: &Study,
    loop_definition: &LoopDefinition,
    current: &CurrentModel,
    evidence: StageEvidence,
) -> Result<IdentifyResult, Failure> {
    let previous = study.section::<TuneSection>()?;
    let inputs = study.section::<InputsSection>()?;
    let requirements =
        study_requirements(loop_definition, current, previous.as_ref(), inputs.as_ref());
    Ok(match &current.identify {
        Some(section) => identify_result(section, evidence, requirements),
        None => supplied_result(&current.model, evidence, requirements),
    })
}

/// The Identify answer of a supplied model (B3.7, B9.4): its type and equation in plain words, and its channels as its key numbers.
/// It was not fitted, so it has no recording, no response plot and no fit error; the page shows it as it shows a fit, with the
/// stage status after it.
#[must_use]
pub fn supplied_result(
    model: &GeneralModel,
    evidence: StageEvidence,
    requirements: Option<TuningRequirements>,
) -> IdentifyResult {
    let words = supplied_model_words(model);
    let identity = model.identity();
    let provisional = identity.validation != ModelValidation::Validated;
    let key_numbers = format!(
        "Input {}; output {}",
        channels_text(&identity.inputs),
        channels_text(&identity.outputs)
    );
    let label = if provisional { PROVISIONAL_LABEL } else { "" };
    let text = format!(
        "{}{label}\n{}\n{key_numbers}",
        words.model_type, words.equation
    );
    IdentifyResult {
        model_type: words.model_type,
        equation: words.equation,
        key_numbers,
        provisional,
        text,
        plot: IdentifyPlot {
            time_seconds: Vec::new(),
            traces: Vec::new(),
        },
        recording_id: None,
        model_revision: identity.revision.clone(),
        requirements,
        stage_evidence: evidence,
        stages: stage_availability(&evidence),
    }
}

/// The channels in words: each identifier with its unit, in the model's order.
fn channels_text(channels: &[ModelChannel]) -> String {
    channels
        .iter()
        .map(|channel| format!("{} ({})", channel.id, channel.unit))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The identification a study holds, as `controlStudio/open` answers it (A8.6): the Identify answer of the study's current model
/// (B3.7), whatever its source, when that model is current for the loop (A8.4). A study with no model, or one that is not current,
/// holds none, as it does after Reset.
///
/// # Errors
/// The study's sections cannot be read.
pub fn study_identification(
    study: &Study,
    loop_definition: &LoopDefinition,
    evidence: StageEvidence,
) -> Result<Option<IdentifyResult>, Failure> {
    let Some(current) = current_model(study)? else {
        return Ok(None);
    };
    if !current.is_current_for(loop_definition) {
        return Ok(None);
    }
    identify_answer(study, loop_definition, &current, evidence).map(Some)
}

/// The text of a kept candidate's answer where the study keeps no prediction: the response and the margins are not predicted on
/// open, and Tune runs again to show them (B4.5, A8.6).
const NOT_KEPT_PREDICTION: &str =
    "The study keeps the candidate but not its prediction, so the response and the margins are not shown until Tune runs again.";

/// The candidate a study keeps, answered as `controlStudio/tune` answers it (B4.5, A8.6): the builder of the Tune answer
/// ([`tune_result`]) on the study's Tune section and its current model. The study keeps the design, its candidate, method, reason,
/// attempts, `λ` and requirements, but no prediction. So the answer holds no response, settling time or overshoot, its outcome is
/// unpredicted, and it gives no margins and no comparison: nothing is predicted on open. Two values are rebuilt from what the
/// study keeps and are not the answer's own words: a relay candidate's description is its reason, because the relay cycle's
/// numbers are not kept, and the objective of an Automatic candidate is its declared response, or More robust, because the study
/// keeps the `λ` of a SIMC candidate and not the response it was chosen with. It is `None` without a current model or a Tune
/// section, or when the model cannot be read.
#[must_use]
pub fn kept_candidate(
    loop_definition: &LoopDefinition,
    current: Option<&CurrentModel>,
    previous: Option<&TuneSection>,
    evidence: StageEvidence,
) -> Option<TuneResult> {
    let current = current?;
    let section = previous?;
    let model = tuning_model(current).ok()?;
    let signals = &loop_definition.signals;
    let process_unit = display_engineering_unit(&signals.process_variable.unit);
    let manipulated_unit = display_engineering_unit(&signals.manipulated_variable.unit);
    let requirements = &section.requirements;
    let objective = match section.method.and_then(TuneMethod::formula) {
        Some(_) => requirements.formula_objective(),
        None => requirements.tuning_objective.unwrap_or(AUTOMATIC_OBJECTIVE),
    };
    let description = match section.method {
        Some(TuneMethod::Manual) => Some(MANUAL_DESCRIPTION.to_owned()),
        Some(TuneMethod::Relay) => Some(section.reason.clone()),
        _ => None,
    };
    let candidate = section.law_candidate()?;
    Some(tune_result(ResultInputs {
        model: &model,
        method: section.method,
        description,
        action: controller_action(loop_definition),
        process_unit: &process_unit,
        manipulated_unit: &manipulated_unit,
        settings: candidate_settings(&candidate, &mapped_parameters(loop_definition)),
        candidate,
        objective,
        closed_loop_time_seconds: section.closed_loop_time_seconds,
        attempts: section.attempts.clone(),
        reason: section.reason.clone(),
        requirements: requirements.clone(),
        outcome: CandidateOutcome::Unpredicted {
            reason: NOT_KEPT_PREDICTION.to_owned(),
        },
        prediction: CandidateResponse {
            response: None,
            unavailable: Some(NOT_KEPT_PREDICTION.to_owned()),
            settling_time_seconds: None,
            overshoot_percent: None,
        },
        margins: (None, Some(NOT_KEPT_PREDICTION.to_owned())),
        comparison: Vec::new(),
        stage_evidence: evidence,
    }))
}

/// The candidate that `controlStudio/open` holds for a study (B4.5, A8.6): [`kept_candidate`] of the study's sections, given only
/// when the stage evidence says the study has a current candidate (A8.4). The caller passes the evidence `stage_evidence_of`
/// gives, so the rule for a current candidate is decided in one place.
///
/// # Errors
/// The study's sections cannot be read.
pub fn study_candidate(
    study: &Study,
    loop_definition: &LoopDefinition,
    evidence: StageEvidence,
) -> Result<Option<TuneResult>, Failure> {
    if !evidence.has_candidate {
        return Ok(None);
    }
    let current = current_model(study)?;
    let previous = study.section::<TuneSection>()?;
    Ok(kept_candidate(
        loop_definition,
        current.as_ref(),
        previous.as_ref(),
        evidence,
    ))
}

/// The evidence of a study of `loop_definition` that sets the stages' status: the loop is bound, a model exists when the study's
/// current model is current for the loop (A8.4, B3.7), `has_recording` says whether the study holds a recording, and the loop's
/// kind decides whether Tune, Validate and Apply apply (C12).
#[must_use]
pub fn study_evidence(
    loop_definition: &LoopDefinition,
    current: Option<&CurrentModel>,
    has_recording: bool,
) -> StageEvidence {
    StageEvidence {
        loop_bound: true,
        has_model: current.is_some_and(|current| current.is_current_for(loop_definition)),
        has_recording,
        controller_stages_apply: !loop_definition.is_observation_or_open_loop(),
        ..StageEvidence::default()
    }
}

/// One trace before it is placed on the plot's time base: its role, symbol and unit, and its values at its own times.
struct Series<'a> {
    role: PlotRole,
    symbol: &'a str,
    unit: &'a str,
    points: Vec<(f64, Option<f64>)>,
}

/// The response plot of a section (B3.8, B9.7): the applied plant inputs and the measured outputs of the experiment, then the
/// model's prediction, on the times of the recording. Without the experiment's traces the measured output is the measured
/// value of each fitted point, as the page drew it, named by the channel the fit read (`read_channels`, B3.6), and its prediction
/// carries that name too. Nothing is interpolated or resampled.
fn response_plot(section: &IdentifySection, comparison: &ComparisonPresentation) -> IdentifyPlot {
    let shown = &comparison.presentation;
    let fitted = &shown.predicted_response;
    let predicted_output = comparison
        .output_id
        .as_deref()
        .or_else(|| {
            section
                .selected_model
                .identity()
                .outputs
                .first()
                .map(|channel| channel.id.as_str())
        })
        .unwrap_or_default();
    // The measured trace of a fit without the experiment's traces is named by the recorded channel the fit read, or by the model's
    // output when the fit read none. Its prediction takes the same name: the model of an opened recording on a channel the loop does
    // not bind carries the loop's model id, which is not that channel's name, so the two traces of the plot would disagree.
    let fitted_name = section
        .read_channels()
        .outputs
        .first()
        .map_or(predicted_output, String::as_str);
    let mut series = Vec::new();
    if let Some(experiment) = &comparison.experiment {
        series.extend(
            experiment
                .inputs
                .iter()
                .map(|trace| traced(PlotRole::PlantInput, trace)),
        );
        series.extend(
            experiment
                .outputs
                .iter()
                .map(|trace| traced(PlotRole::Measurement, trace)),
        );
    } else if !fitted.is_empty() {
        series.push(Series {
            role: PlotRole::Measurement,
            symbol: fitted_name,
            unit: &shown.units.output,
            points: fitted
                .iter()
                .map(|point| (point.time_seconds, Some(point.measured)))
                .collect(),
        });
    }
    if !fitted.is_empty() {
        // A plot with the experiment's own traces keeps the model's output id for its prediction, as it did before.
        series.push(Series {
            role: PlotRole::Prediction,
            symbol: if comparison.experiment.is_some() {
                predicted_output
            } else {
                fitted_name
            },
            unit: &shown.units.output,
            points: fitted
                .iter()
                .map(|point| (point.time_seconds, Some(point.predicted)))
                .collect(),
        });
    }
    on_one_time_base(series)
}

/// The series of an experiment trace.
fn traced(role: PlotRole, trace: &ExperimentTrace) -> Series<'_> {
    Series {
        role,
        symbol: &trace.id,
        unit: &trace.unit,
        points: trace
            .points
            .iter()
            .map(|point| (point.time_seconds, point.value))
            .collect(),
    }
}

/// The series on one time base: every finite time any of them has, in increasing order, and each series' value at each of
/// those times, `None` where it has none (B10.3: a missing value stays missing).
fn on_one_time_base(series: Vec<Series<'_>>) -> IdentifyPlot {
    let mut time_seconds: Vec<f64> = series
        .iter()
        .flat_map(|trace| trace.points.iter().map(|(time, _)| *time))
        .filter(|time| time.is_finite())
        .collect();
    time_seconds.sort_by(f64::total_cmp);
    time_seconds.dedup_by(|later, earlier| later.total_cmp(earlier).is_eq());
    let traces = series
        .into_iter()
        .map(|trace| {
            let mut values = vec![None; time_seconds.len()];
            for (time, value) in trace.points {
                if let Ok(index) = time_seconds.binary_search_by(|probe| probe.total_cmp(&time)) {
                    values[index] = value.filter(|value| value.is_finite());
                }
            }
            PlotTrace {
                role: trace.role,
                symbol: trace.symbol.to_owned(),
                unit: trace.unit.to_owned(),
                values,
            }
        })
        .collect();
    IdentifyPlot {
        time_seconds,
        traces,
    }
}

#[cfg(test)]
#[path = "identify_result_tests.rs"]
pub(crate) mod tests;
