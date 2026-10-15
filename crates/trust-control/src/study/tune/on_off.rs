//! The Tune of a switching controller (specification 35, C3 and B4): the settings designed on the study's current model, the answer that a
//! run gives and a reopened study gives from the section it keeps, and the section itself. The rules of the settings are the design's
//! ([`super::design::on_off`]); this module reads the study and gives the outcome. The candidate is unapplied (B4.1): Validate runs it on
//! the actual Structured Text, and Apply writes it.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::messages::{CandidateOutcome, TuneGainUnits, TuneResult};
use crate::stages::{stage_availability, StageEvidence};
use crate::study::tune::design::on_off::design;
use crate::study::tune::{
    check_settings, current_for, stage_evidence, TuneInputs, TuneOutcome, TuneSection,
};
use crate::tuning::objective::AUTOMATIC_OBJECTIVE;
use crate::tuning::on_off_candidate::OnOffCandidate;
use crate::tuning::requirements::TuningRequirements;

/// The title of the switching controller's settings in the answer.
const ON_OFF_TITLE: &str = "ON/OFF settings";
/// The description of the switching controller's settings in the answer (C3).
const ON_OFF_DESCRIPTION: &str = "The hysteresis comes from the measurement's noise and the settling band of the process requirement. The minimum ON and OFF times come from the switched equipment's own limits and are never shorter than them. Validate runs the settings on the actual ST.";
/// The reason of a candidate that Tune designed (C3).
const ON_OFF_READY: &str = "ON/OFF settings respect the declared process band and equipment dwell. Validate the actual ST before applying.";
/// Why the switching settings have no prediction: Validate runs them on the actual ST (C3).
const ON_OFF_UNPREDICTED: &str = "A switching controller is checked by its actual ST in Validate; Tune predicts no response for it.";
/// Why a switching controller shows no margins (C0, C3).
const ON_OFF_MARGINS: &str = "A switching controller shows no margins (C0).";
/// The refusal of a named method on a switching controller: its settings are the only ones it has (C3).
const NAMED_METHOD: &str =
    "The bound switching ST controller supports ON/OFF settings only: choose Automatic.";
/// The refusal of pinned settings on a switching controller, which the Rust engine does not take yet (open item of stage W4-C3).
const PINS_NOT_TAKEN: &str = "A switching controller takes its pinned settings as its settings map (onOffPins), not as the Control.PID pin record.";
/// The refusal of a run without requirements: the study holds none, and Tune takes the band from them (B4.3).
const NEEDS_REQUIREMENTS: &str = "Tune needs the process requirements: declare them before Tune.";
/// The defect of a run that reaches its design without the study's current model, which the checks before the start refuse first.
const NO_MODEL: &str = "Tune reached its switching design without the study's current model.";

/// The checks before a switching controller's design (C3, B4.7): a named method is refused, the study's model must be current for the
/// loop, the loop must be one the engine runs, the request's settings must be valid, and pinned settings are refused (see
/// [`PINS_NOT_TAKEN`]).
pub(crate) fn check_on_off(inputs: &TuneInputs<'_>) -> Result<(), Failure> {
    let loop_definition = inputs.loop_definition;
    if inputs.request.method.is_some() {
        return Err(Failure::new(
            NAMED_METHOD,
            Cause::Settings,
            NextAction::Tune,
        ));
    }
    current_for(inputs.model, loop_definition)?;
    loop_definition.rust_engine_support()?;
    check_settings(inputs.request)?;
    if inputs.request.pins.is_some() {
        return Err(Failure::new(
            PINS_NOT_TAKEN,
            Cause::Settings,
            NextAction::None,
        ));
    }
    Ok(())
}

/// Designs the switching settings on the study's current model and gives the section to store, unapplied, and the result to answer
/// (C3, B4.1 to B4.6). The checks before the start run first.
///
/// # Errors
/// The checks before the start; the study holds no requirements; or the design's own refusal (see [`design`]).
pub fn run(
    inputs: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
    pins: Option<&BTreeMap<String, f64>>,
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
                NEEDS_REQUIREMENTS,
                Cause::Settings,
                NextAction::OpenRequirements,
            )
        })?;
    let candidate = design(loop_definition, requirements, pins)?;
    let section = TuneSection {
        method: None,
        model_revision: current.model.identity().revision.clone(),
        requirements: requirements.clone(),
        candidate: None,
        project_candidate: None,
        state_candidate: None,
        search_candidate: None,
        on_off_candidate: Some(candidate.clone()),
        group_candidate: None,
        reason: ON_OFF_READY.to_owned(),
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
    let result = answer(&section, &candidate, evidence);
    Ok(TuneOutcome { section, result })
}

/// The Tune answer of a switching controller (C3, B4.5, A8.6): the settings it designed, with the requirements, the stages and the
/// reasons. Nothing is predicted and no margins are shown: Validate runs the settings on the actual ST.
#[must_use]
pub(crate) fn answer(
    section: &TuneSection,
    candidate: &OnOffCandidate,
    evidence: StageEvidence,
) -> TuneResult {
    TuneResult {
        feedforward: None,
        state: None,
        candidate: None,
        project_candidate: None,
        group: None,
        settings: candidate.values.clone(),
        method: None,
        method_title: ON_OFF_TITLE.to_owned(),
        method_description: ON_OFF_DESCRIPTION.to_owned(),
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
            reason: ON_OFF_UNPREDICTED.to_owned(),
        },
        response: None,
        prediction_unavailable: Some(ON_OFF_UNPREDICTED.to_owned()),
        settling_time_seconds: None,
        overshoot_percent: None,
        margins: None,
        delay_margin_seconds: None,
        margins_unavailable: Some(ON_OFF_MARGINS.to_owned()),
        comparison: Vec::new(),
        stages: stage_availability(&evidence),
        stage_evidence: evidence,
        schedule: None,
    }
}
