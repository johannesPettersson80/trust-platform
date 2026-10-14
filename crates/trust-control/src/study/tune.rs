//! The Tune section of a study (specification 35, A8.1, A8.4, B4 and B10.2; the Tune page specification): the candidate that
//! Tune designed, the method and the requirements it was designed with, and the revision of the model it was designed on.
//!
//! The candidate is stored unapplied: Tune never writes to the running controller (B4.1). The section names the model it was
//! designed on, so a new or replaced model leaves it behind: [`TuneSection::is_current_for`] says whether the candidate still
//! belongs to a model (A8.4), and [`stage_evidence`] reports the Tune stage from it. The requirements keep the values that
//! Tune generated apart from the engineer's (B4.3), so a later Tune recalculates only the generated ones.
//!
//! The design is in the submodules, each with one responsibility: [`model`] reads the study's model as the tuning rules take
//! it, [`requirements`] fills the requirement record, [`design`] chooses the structure and the gains (with the pole
//! placement of [`crate::tuning::placement`]), [`relay`] and [`manual`] give the relay rule's and the entered settings'
//! candidates, [`comparison`] evaluates the constrained comparison (N5.10), [`prediction`] predicts the response and reads
//! the margins, and [`result`] writes the answer. This module holds the section, what a run reads and gives ([`TuneInputs`],
//! [`TuneOutcome`]), the refusals that come before any operation starts ([`check_before_start`], B4.7), the manual preview
//! ([`preview`], B4.5) and the relay evidence of the study's record ([`relay_evidence_for`], N5.R).

#![allow(clippy::module_name_repetitions)] // The section keeps the name of its stage.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::LowOrderModelKind;

use crate::experiment::ControlExperimentKind;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::identify_result::study_evidence;
use crate::identification::plant::IdentifiedModel;
use crate::identification::recorded_model::check_gain_sign;
use crate::identification::refusal::Refusal;
use crate::loop_definition::{Action, LoopDefinition};
use crate::messages::{TuneMethod, TuneRequest, TuneResult};
use crate::model::general::js_text;
use crate::model::GeneralModel;
use crate::recording::RecordingFile;
use crate::stages::{stage_availability, StageEvidence};
use crate::study::current_model::CurrentModel;
use crate::study::identify::{IdentifyMethod, IdentifySection};
use crate::study::StudySection;
use crate::tuning::automatic_criteria::validate_criteria;
use crate::tuning::candidate::{validate_parameters, ControlPidCandidate};
use crate::tuning::objective::TuningMethod;
use crate::tuning::plant::{check_normalized_error, check_plant, from_identified};
use crate::tuning::recommendation::Attempt;
use crate::tuning::relay_rule::{self, RelayCycleEvidence};
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::texts;

pub mod comparison;
pub mod design;
pub mod manual;
pub mod model;
pub mod prediction;
pub mod record;
pub mod relay;
pub mod requirements;
pub mod result;

/// The result of one Tune, kept in the study under the key `tune`: the candidate, how it was designed, the requirements it was
/// designed to, and the model it was designed on. The candidate is unapplied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TuneSection {
    /// The method the engineer chose; absent for Automatic (B4.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<TuneMethod>,
    /// The revision of the model the candidate was designed on (A8.4): the candidate is current only while this is the
    /// study's model.
    pub model_revision: String,
    /// The requirements the candidate was designed to. The values that Tune generated are listed in their
    /// `automaticCriteria` (B4.3).
    pub requirements: TuningRequirements,
    /// The candidate, unapplied (B4.1).
    pub candidate: ControlPidCandidate,
    /// Why the candidate was chosen (B4.4).
    pub reason: String,
    /// The closed-loop time constant `λ` in seconds that the formula used (N5.4: shown and saved with the candidate);
    /// absent for AMIGO and for a design without `λ`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_loop_time_seconds: Option<f64>,
    /// Every structure Automatic attempted, with its status and reason (B4.4; Part C, "Candidates and results": the
    /// rejected candidates); empty for a named method.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempts: Vec<Attempt>,
    /// The settings the engineer pinned for the design (B4.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<ControllerSettingsPins>,
}

impl TuneSection {
    /// Whether the candidate was designed on `model` (A8.4): the model's revision is the one the section names. A new or
    /// replaced model has another revision, so a candidate designed on the old one is no longer current.
    #[must_use]
    pub fn is_current_for(&self, model: &GeneralModel) -> bool {
        self.model_revision == model.identity().revision
    }
}

impl StudySection for TuneSection {
    const KEY: &'static str = "tune";

    /// The requirements hold their rules (B4.3), the record of the generated values decodes, the candidate's record holds its
    /// rules (C1), the candidate is unapplied (B4.1), and the model revision is named. A section that breaks one is never
    /// kept and never read (B10.4).
    fn check(&self) -> Result<(), Failure> {
        self.requirements
            .validate()
            .map_err(refused_as(Cause::Settings, NextAction::OpenRequirements))?;
        if let Some(criteria) = &self.requirements.automatic_criteria {
            validate_criteria(criteria)
                .map_err(refused_as(Cause::Settings, NextAction::OpenRequirements))?;
        }
        validate_parameters(&self.candidate.parameters)
            .map_err(refused_as(Cause::Settings, NextAction::None))?;
        if self.candidate.applied {
            return Err(invalid(
                "its candidate is marked applied, but Tune never writes to the controller (B4.1)",
                Cause::Settings,
            ));
        }
        if !js_text(&self.model_revision) {
            return Err(invalid("it names no model revision", Cause::ModelOrData));
        }
        if self
            .closed_loop_time_seconds
            .is_some_and(|lambda| !(lambda.is_finite() && lambda > 0.0))
        {
            return Err(invalid(
                "its closed-loop time constant is not a positive number of seconds",
                Cause::Settings,
            ));
        }
        Ok(())
    }

    fn recording_ids(&self) -> Vec<&str> {
        Vec::new()
    }
}

/// What [`tune`] and [`check_before_start`] read: the loop, the study's Identify section, the previous Tune section, the
/// request, and the relay cycle of the study's record when the method is the relay rule.
#[derive(Debug, Clone, Copy)]
pub struct TuneInputs<'a> {
    /// The loop that is tuned.
    pub loop_definition: &'a LoopDefinition,
    /// The study's current model (B3.7, A8.4), or `None` when the study has none. [`check_before_start`] refuses a missing model
    /// and one that is not current for the loop (B4.7: next action Identify).
    pub model: Option<&'a CurrentModel>,
    /// The study's Tune section from an earlier run, when it has one: the values it supplied and the ones it generated
    /// carry over (B4.3).
    pub previous: Option<&'a TuneSection>,
    /// What the engineer asked for.
    pub request: &'a TuneRequest,
    /// The relay cycle of the study's estimation recording, when the method is the relay rule and the engine read it
    /// ([`relay_evidence_for`]). The relay rule is refused without a qualified one (N5.R).
    pub relay: Option<&'a RelayCycleEvidence>,
}

/// What [`tune`] gives: the section to store, unapplied, and the result to answer.
#[derive(Debug, Clone, PartialEq)]
pub struct TuneOutcome {
    /// The section to keep in the study.
    pub section: TuneSection,
    /// The result to answer.
    pub result: TuneResult,
}

/// The evidence of a study that sets the stages' status (A8.4): [`study_evidence`] decides whether a model exists (the study's
/// current model is current for the loop), and the study has a candidate only when the Tune section was designed on that model.
/// A new or replaced model leaves the candidate behind.
#[must_use]
pub fn stage_evidence(
    loop_definition: &LoopDefinition,
    model: Option<&CurrentModel>,
    previous: Option<&TuneSection>,
    has_recording: bool,
) -> StageEvidence {
    let evidence = study_evidence(loop_definition, model, has_recording);
    let has_candidate = match (model, previous) {
        (Some(current), Some(designed)) => {
            evidence.has_model && designed.is_current_for(&current.model)
        }
        _ => false,
    };
    StageEvidence {
        has_candidate,
        ..evidence
    }
}

/// The refusals that come before any operation starts, in the order of B4.7's table. Each failure carries the next action
/// that fixes it:
/// - a loop without a controller law (C12): the failure of `require_controller_law`, which needs no action, as its stage is
///   Not applicable;
/// - no model that is current for the loop's signals, or a model the tuning rules cannot read: Identify;
/// - a gain that is zero or whose sign contradicts the controller action (N6.3): the loop setup;
/// - a loop that uses a feature the Rust engine does not run yet, or has no controller law without being a C12 loop: the
///   failure of `rust_engine_support` (no controller: the loop setup);
/// - settings that the manual method needs and the other methods refuse, pins for another loop revision or family, and
///   entered settings outside their bounds (B4.2, B4.5): the settings themselves;
/// - a relay rule with no qualified relay record of the study (N5.R, N2.9): choose another method;
/// - a named method that does not apply to the model (N5.2 to N5.7): choose another method.
///
/// Automatic is not refused for its method: it tries the structures and reports each attempt (B4.4).
///
/// # Errors
/// The first of the refusals above that holds.
pub fn check_before_start(inputs: &TuneInputs<'_>) -> Result<(), Failure> {
    let loop_definition = inputs.loop_definition;
    loop_definition.require_controller_law()?;
    let current = current_for(inputs.model, loop_definition)?;
    let fitted = model::tuning_model(current)
        .map_err(refused_as(Cause::ModelOrData, NextAction::Identify))?;
    let integrating = fitted.kind == LowOrderModelKind::Integrating;
    check_gain_sign(loop_definition, fitted.gain, integrating)
        .map_err(refused_as(Cause::ModelOrData, NextAction::OpenLoopSetup))?;
    loop_definition.rust_engine_support()?;
    check_settings(inputs.request)?;
    check_pins(loop_definition, inputs.request)?;
    match inputs.request.method {
        None => Ok(()),
        Some(TuneMethod::Simc) => check_method(loop_definition, &fitted, TuningMethod::Simc),
        Some(TuneMethod::Amigo) => check_method(loop_definition, &fitted, TuningMethod::Amigo),
        Some(TuneMethod::Relay) => match relay::refusal_of(inputs.relay) {
            Some(failure) => Err(failure),
            None => Ok(()),
        },
        Some(TuneMethod::Manual) => check_manual(loop_definition, inputs.request),
    }
}

/// Designs the candidate (B4.1 to B4.7) and gives the section to store, unapplied, and the result to answer. Every rule is
/// the rules' own: this only delegates to `design::run`. Callers run [`check_before_start`] first.
///
/// # Errors
/// The failure of the design: the requirements cannot be formed, no structure meets them, or the method does not apply
/// (B4.7).
pub fn tune(inputs: &TuneInputs<'_>) -> Result<TuneOutcome, Failure> {
    tune_held(inputs, None, None)
}

/// [`tune`] with the requirements the study holds (B4.3) and the settings of the running controller (B4.5): `held` is the
/// requirements the engineer recorded in the study's inputs, which the design takes as given, and `None` keeps the earlier Tune
/// section's record as [`tune`] does. `running` is the record's values as the running controller holds them, which the engine
/// reads through the request's connection (B5.3); `None` predicts the candidate alone.
///
/// # Errors
/// The failure of the design, as [`tune`] names it (B4.7).
pub fn tune_held(
    inputs: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
    running: Option<&BTreeMap<String, f64>>,
) -> Result<TuneOutcome, Failure> {
    design::run(inputs, held, running)
}

/// The answer of a manual preview (B4.5): the result that Tune with the manual method would finish with, on the study as it
/// is, with the running controller's settings when `running` is given. Nothing is kept: the stages are the study's own, with its
/// earlier Tune section, and not as if the candidate were kept.
///
/// # Errors
/// The refusals of [`check_before_start`] and of the design (B4.7).
pub fn preview(
    inputs: &TuneInputs<'_>,
    running: Option<&BTreeMap<String, f64>>,
) -> Result<TuneResult, Failure> {
    let mut result = design::run(inputs, None, running)?.result;
    let evidence = stage_evidence(
        inputs.loop_definition,
        inputs.model,
        inputs.previous,
        inputs
            .model
            .is_some_and(|current| current.identify.is_some()),
    );
    result.stages = stage_availability(&evidence);
    result.stage_evidence = evidence;
    Ok(result)
}

/// The relay cycle of the study's estimation recording (N5.R, N2.9): `None` unless the Identify section is a live Feedback
/// relay whose request holds the relay plan, and the recording is the loop's. Otherwise the evidence: qualified, or not, with
/// the reason the record gives.
#[must_use]
pub fn relay_evidence_for(
    identify: &IdentifySection,
    loop_definition: &LoopDefinition,
    recording: &RecordingFile,
) -> Option<RelayCycleEvidence> {
    if !matches!(
        identify.method,
        IdentifyMethod::Live(ControlExperimentKind::Relay)
    ) || recording.loop_id != loop_definition.id
    {
        return None;
    }
    let experiment = &identify.record_request.as_ref()?.request.experiment;
    if !matches!(experiment.kind, ControlExperimentKind::Relay) {
        return None;
    }
    let plan = experiment.relay_feedback.as_ref()?;
    Some(relay_rule::evidence(
        plan.high_output,
        plan.low_output,
        &relay_rule::samples_of(&recording.run),
    ))
}

/// The study's current model, when it is current for the loop's signals (A8.4, B3.7).
fn current_for<'a>(
    model: Option<&'a CurrentModel>,
    loop_definition: &LoopDefinition,
) -> Result<&'a CurrentModel, Failure> {
    model
        .filter(|current| current.is_current_for(loop_definition))
        .ok_or_else(no_model)
}

/// The failure of a study with no model for the loop's current signals (B4.7: no model, next action Identify).
fn no_model() -> Failure {
    Failure::new(
        "The study has no model for the loop's current signals. Identify the plant first, so Tune has a model to design on.",
        Cause::ModelOrData,
        NextAction::Identify,
    )
}

/// The checks of a named method before it runs (B4.7: method not applicable, next action Choose another method): the model
/// has a form the method takes (N5.2, N5.3), its error is within the limit (N5.7), and the plant is within the method's
/// domain (N5.3, N5.5).
fn check_method(
    loop_definition: &LoopDefinition,
    fitted: &IdentifiedModel,
    method: TuningMethod,
) -> Result<(), Failure> {
    let not_applicable = refused_as(Cause::ModelOrData, NextAction::ChooseMethod);
    let plant = from_identified(fitted, method).map_err(&not_applicable)?;
    check_normalized_error(fitted, method).map_err(&not_applicable)?;
    let unit = gain_unit(loop_definition, plant.is_integrating());
    check_plant(&plant, method, controller_action(loop_definition), &unit).map_err(&not_applicable)
}

/// The settings belong to the manual method, which takes them (B4.2): the manual method is refused at once without them, and
/// any other method is refused with them.
fn check_settings(request: &TuneRequest) -> Result<(), Failure> {
    match (request.method, request.settings.is_some()) {
        (Some(TuneMethod::Manual), false) => Err(Failure::new(
            texts::MANUAL_NEEDS_SETTINGS,
            Cause::Settings,
            NextAction::None,
        )),
        (_, false) | (Some(TuneMethod::Manual), true) => Ok(()),
        (_, true) => Err(Failure::new(
            texts::SETTINGS_NEED_MANUAL,
            Cause::Settings,
            NextAction::None,
        )),
    }
}

/// The pinned settings belong to this loop revision, and a request that fixes a structure fixes the pinned family (B4.2): the
/// pins are refused for another revision, and for a structure other than their family.
fn check_pins(loop_definition: &LoopDefinition, request: &TuneRequest) -> Result<(), Failure> {
    let Some(pins) = &request.pins else {
        return Ok(());
    };
    if pins.loop_revision != loop_definition.revision {
        return Err(Failure::new(
            texts::PINS_LOOP_REVISION,
            Cause::Settings,
            NextAction::None,
        ));
    }
    if request
        .structure
        .is_some_and(|structure| structure != pins.family)
    {
        return Err(Failure::new(
            texts::PINS_FAMILY,
            Cause::Settings,
            NextAction::None,
        ));
    }
    Ok(())
}

/// The entered settings name the loop's settings and lie within their bounds (B4.5): the first that does not is the refusal,
/// with its own sentence. The presence of the settings is checked by [`check_settings`].
fn check_manual(loop_definition: &LoopDefinition, request: &TuneRequest) -> Result<(), Failure> {
    let Some(settings) = &request.settings else {
        return Ok(());
    };
    match manual::outside_bounds(loop_definition, settings) {
        Some(sentence) => Err(Failure::new(&sentence, Cause::Settings, NextAction::None)),
        None => Ok(()),
    }
}

/// The settings refusal of a design's pins or of the manual method, as the failure that ends the run: the rule's sentence,
/// the settings cause and no next action (B4.2, B4.5).
pub(crate) fn settings_refused() -> impl Fn(Refusal) -> Failure {
    refused_as(Cause::Settings, NextAction::None)
}

/// The controller's action as the tuning rules take it (N6.1).
#[must_use]
pub(crate) fn controller_action(loop_definition: &LoopDefinition) -> ControllerAction {
    match loop_definition.controller.action {
        Action::Direct => ControllerAction::Direct,
        Action::Reverse => ControllerAction::Reverse,
    }
}

/// The unit of the plant's gain in the loop's units, as the sign sentences print it (N6.3): the measurement's unit per the
/// plant input's unit, and per second for an integrating model, whose gain is a rate.
#[must_use]
pub(crate) fn gain_unit(loop_definition: &LoopDefinition, integrating: bool) -> String {
    let signals = &loop_definition.signals;
    let rate = if integrating { "/s" } else { "" };
    format!(
        "{}/{}{rate}",
        signals.process_variable.unit, signals.manipulated_variable.unit
    )
}

/// A rule's refusal as the failure that ends the request: the rule's sentence, its cause and the one next action.
pub(crate) fn refused_as(cause: Cause, next_action: NextAction) -> impl Fn(Refusal) -> Failure {
    move |Refusal(sentence)| Failure::new(&sentence, cause, next_action)
}

/// A stored Tune section that breaks a rule of its own (B10.4).
fn invalid(reason: &str, cause: Cause) -> Failure {
    Failure::new(
        &format!("The Tune result of the study is not valid: {reason}."),
        cause,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "tune_tests.rs"]
mod tests;
