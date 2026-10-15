//! The Tune of a state-feedback law (specification 35, C4; numerical annex N5.14): the law's state-space design on the study's
//! current model, with the state-design choices the study keeps (D-B11). The rules are the state controller's own
//! (`tuning::state_controller`); this module reads the study, designs, and gives the section to keep and the result to answer.
//! The candidate is unapplied (B4.1): Validate runs it on the actual Structured Text, and Apply writes it.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{check_pins, check_settings, stage_evidence, TuneInputs, TuneOutcome, TuneSection};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::messages::{CandidateOutcome, TuneGainUnits, TuneMethod, TuneResult};
use crate::model::GeneralModel;
use crate::stages::{stage_availability, StageEvidence};
use crate::tuning::candidate_margins::{state_feedback_margins, CandidateMargins};
use crate::tuning::objective::AUTOMATIC_OBJECTIVE;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::robustness::delay_margin;
use crate::tuning::state_controller::answer::StateAnswer;
use crate::tuning::state_controller::design::{
    design_state_controller, pin_state_design, StateControllerDesign,
};
use crate::tuning::state_controller::preferences::{
    field_key, form_fields, parse_state_design_draft, FormField, StateDesignDraft,
    StateDesignPreferences,
};
use crate::tuning::state_controller::{
    decode_state_controller_binding, texts, StateControllerFamily,
};

/// The refusal of a run that reaches the state design without the study's current model: the checks before the start refuse it.
const NO_MODEL: &str = "Tune reached its state design without the study's current model.";
/// The refusal of a loop channel that names an output which the model does not have.
const UNKNOWN_OUTPUT: &str = "A loop channel names an output that the model does not have.";
/// The margins of a sampled model, which the state margins are not read on (N7.9).
const MARGINS_CONTINUOUS: &str =
    "The margins of a state law are given on a continuous model only (N7.9).";
/// The margins of a law with several inputs, which the state margins are not read on (N7.9).
const MARGINS_ONE_INPUT: &str = "The margins of a state law are given for one input only (N7.9).";

/// The state-design choices a study keeps for a state-feedback law (D-B11, A8.1): the family the engineer chose, the draft of the
/// design form as the engineer left it, and the coefficients the engineer pinned. Tune reads them. Only the parsed draft reaches a
/// solver (`parse_state_design_draft`); the draft is editing state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateDesignInput {
    /// The family of the law that the engineer chose; absent takes the family the loop declares.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<StateControllerFamily>,
    /// The text of each field of the design form: the desired poles, the weights, the covariances and the horizon (C4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<StateDesignDraft>,
    /// The coefficients the engineer pinned, by symbol (B4.2, C4): each replaces its designed value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<BTreeMap<String, f64>>,
}

/// The checks of a state-feedback law before its design (C4): the loop declares the law, the engine runs the loop, and the request
/// holds no PID settings or pins. The design's own rules (the model's form, the weights, the horizon and the pins) run in [`run`].
pub(crate) fn check_state(inputs: &TuneInputs<'_>) -> Result<(), Failure> {
    let loop_definition = inputs.loop_definition;
    if loop_definition.controller.state_feedback.is_none() {
        return Err(Failure::new(
            texts::NO_STATE_LAW,
            Cause::ControllerCapability,
            NextAction::OpenLoopSetup,
        ));
    }
    loop_definition.rust_engine_support()?;
    check_settings(inputs.request)?;
    check_pins(loop_definition, inputs.request)
}

/// Designs the state-feedback law of the request on the study's current model (C4, N5.14) with the study's state-design choices
/// `state` (D-B11), and gives the section to store, unapplied, and the result to answer (B4.1 to B4.6). Without `state` the design
/// takes the declared family, the controller's defaults and no pins.
///
/// # Errors
/// The checks before the start; the requirements the study does not hold (Tune takes the response time from them); a draft that
/// the design form refuses; a pin that the law does not map or that is outside its bounds; or the design's own refusal.
pub fn run(
    inputs: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
    state: Option<&StateDesignInput>,
) -> Result<TuneOutcome, Failure> {
    super::check_before_start(inputs)?;
    let loop_definition = inputs.loop_definition;
    let current = inputs
        .model
        .ok_or_else(|| Failure::new(NO_MODEL, Cause::Implementation, NextAction::Identify))?;
    let requirements = held
        .or_else(|| inputs.previous.map(|previous| &previous.requirements))
        .ok_or_else(|| {
            Failure::new(
                texts::NEEDS_REQUIREMENTS,
                Cause::Settings,
                NextAction::OpenRequirements,
            )
        })?;
    let model = &current.model;
    let family = match state.and_then(|state| state.family) {
        Some(family) => family,
        None => declared_family(loop_definition)?,
    };
    let preferences = match state.and_then(|state| state.draft.as_ref()) {
        Some(draft) => Some(parse_draft(draft, family)?),
        None => None,
    };
    let controlled = controlled_outputs(loop_definition, model)?;
    let designed = design_state_controller(
        loop_definition,
        model,
        requirements.settling_time_seconds,
        Some(family),
        preferences.as_ref(),
        &controlled,
    )?;
    let designed = match state.and_then(|state| state.pins.as_ref()) {
        Some(pins) => pin_state_design(loop_definition, model, &designed, pins)?,
        None => designed,
    };
    let section = TuneSection {
        method: Some(TuneMethod::StateSpace),
        model_revision: model.identity().revision.clone(),
        requirements: requirements.clone(),
        candidate: None,
        project_candidate: None,
        state_candidate: Some(designed.clone()),
        search_candidate: None,
        on_off_candidate: None,
        group_candidate: None,
        reason: texts::STATE_READY.to_owned(),
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        pins: None,
    };
    let evidence = stage_evidence(
        loop_definition,
        Some(current),
        Some(&section),
        current.identify.is_some(),
    );
    let result = answer(loop_definition, model, &section, &designed, evidence);
    Ok(TuneOutcome { section, result })
}

/// The Tune answer of a state law (C4, B4.5, A8.6): the answer that a run gives, and that a reopened study gives from the section it
/// keeps, the design and the study's model. The margins are read from the model and the gains, so a reopened study has the margins
/// of the run. Nothing is predicted here: the outcome is unpredicted, and Validate runs the law on the actual Structured Text.
#[must_use]
pub fn answer(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    section: &TuneSection,
    design: &StateControllerDesign,
    evidence: StageEvidence,
) -> TuneResult {
    let (margins, margins_unavailable) = state_margins(loop_definition, model, design);
    TuneResult {
        group: None,
        feedforward: None,
        state: Some(StateAnswer::of(design)),
        candidate: None,
        project_candidate: None,
        settings: design.candidate.parameters.clone(),
        method: Some(TuneMethod::StateSpace),
        method_title: TuneMethod::StateSpace.title().to_owned(),
        method_description: texts::METHOD_DESCRIPTION.to_owned(),
        reason: section.reason.clone(),
        objective: AUTOMATIC_OBJECTIVE,
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        gain_units: TuneGainUnits {
            proportional: String::new(),
            integral: String::new(),
            derivative: String::new(),
        },
        requirements: section.requirements.clone(),
        outcome: CandidateOutcome::Unpredicted {
            reason: texts::STATE_UNPREDICTED.to_owned(),
        },
        response: None,
        prediction_unavailable: Some(texts::STATE_UNPREDICTED.to_owned()),
        settling_time_seconds: None,
        overshoot_percent: None,
        delay_margin_seconds: margins.as_ref().map(delay_margin),
        margins,
        margins_unavailable,
        comparison: Vec::new(),
        stages: stage_availability(&evidence),
        stage_evidence: evidence,
        schedule: None,
    }
}

/// The design-form fields that a state law shows before its first Tune (D-B12): the fields of the family the engineer chose, or the
/// one the loop declares, under the solver policy of the stored draft. The horizon steps and the terminal weights appear only for
/// the finite horizon. `None` for a loop without a state law, so the answers that carry the fields leave them out for it.
#[must_use]
pub fn state_fields(
    loop_definition: &LoopDefinition,
    state: Option<&StateDesignInput>,
) -> Option<Vec<FormField>> {
    loop_definition.controller.state_feedback.as_ref()?;
    let family = match state.and_then(|input| input.family) {
        Some(family) => family,
        None => declared_family(loop_definition).ok()?,
    };
    let finite_horizon = state
        .and_then(|input| input.draft.as_ref())
        .is_some_and(|draft| draft.solver_mode.as_deref() == Some("finite"));
    Some(form_fields(family, finite_horizon))
}

/// The family the loop declares for its state-feedback law (C4).
fn declared_family(loop_definition: &LoopDefinition) -> Result<StateControllerFamily, Failure> {
    loop_definition
        .controller
        .state_feedback
        .as_ref()
        .and_then(|value| {
            decode_state_controller_binding(value, loop_definition.parameter_binding.as_ref())
        })
        .map(|binding| binding.family)
        .ok_or_else(|| {
            Failure::new(
                texts::NO_STATE_LAW,
                Cause::ControllerCapability,
                NextAction::OpenLoopSetup,
            )
        })
}

/// The stored draft of the design form, read for `family` (`parseStateDesignDraft`): a refusal names the field it concerns in
/// its sentence.
fn parse_draft(
    draft: &StateDesignDraft,
    family: StateControllerFamily,
) -> Result<StateDesignPreferences, Failure> {
    parse_state_design_draft(draft, family).map_err(|error| {
        let failure = Failure::new(&error.message, Cause::Settings, NextAction::None);
        // A refusal of one field names that field's key, so the page shows the sentence under it (D-B12).
        match error.field {
            Some(field) => failure.with_field(field_key(field)),
            None => failure,
        }
    })
}

/// The model outputs that the loop declares as controlled objectives, in channel order: the outputs with a setpoint (C4, D-B3).
/// A loop without channels declares none, so the design refuses it for its missing objectives.
fn controlled_outputs(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<Vec<usize>, Failure> {
    let outputs = &model.identity().outputs;
    let Some(channels) = &loop_definition.channels else {
        return Ok(Vec::new());
    };
    channels
        .outputs
        .iter()
        .filter(|output| output.setpoint_signal_id.is_some())
        .map(|output| {
            outputs
                .iter()
                .position(|channel| channel.id == output.model_channel_id)
                .ok_or_else(|| {
                    Failure::new(UNKNOWN_OUTPUT, Cause::ModelOrData, NextAction::OpenChannels)
                })
        })
        .collect()
}

/// The margins of a state law at its input (N7.9): for a continuous model with one input, the loop `L(z) = K·(zI − A)⁻¹·B` of the
/// model sampled at the task period. The other models give none, and the answer says why.
fn state_margins(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    design: &StateControllerDesign,
) -> (Option<CandidateMargins>, Option<String>) {
    let GeneralModel::ContinuousStateSpace {
        identity, system, ..
    } = model
    else {
        return (None, Some(MARGINS_CONTINUOUS.to_owned()));
    };
    let Some(gains) = gain_row(loop_definition, design).filter(|_| identity.inputs.len() == 1)
    else {
        return (None, Some(MARGINS_ONE_INPUT.to_owned()));
    };
    let period = loop_definition.controller.task_period_ms / 1000.0;
    match state_feedback_margins(system, period, identity.delay_seconds, &gains) {
        Ok(margins) => (Some(margins), None),
        Err(refusal) => (None, Some(refusal.0)),
    }
}

/// The gain row `K` of the law, from the designed values of its gain symbols in their order.
fn gain_row(loop_definition: &LoopDefinition, design: &StateControllerDesign) -> Option<Vec<f64>> {
    let binding = loop_definition
        .controller
        .state_feedback
        .as_ref()
        .and_then(|value| {
            decode_state_controller_binding(value, loop_definition.parameter_binding.as_ref())
        })?;
    let row = binding.gain_symbols.first()?;
    row.iter()
        .map(|symbol| design.candidate.parameters.get(symbol).copied())
        .collect()
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
