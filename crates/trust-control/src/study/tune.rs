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
//! the margins, [`structure_choice`] says which structure a request designs for, and [`result`] writes the answer. This module
//! holds the section, what a run reads and gives ([`TuneInputs`], [`TuneOutcome`]), the refusals that come before any operation
//! starts ([`check_before_start`], B4.7), the manual preview ([`preview`], B4.5) and the relay evidence of the study's record
//! ([`relay_evidence_for`], N5.R).

#![allow(clippy::module_name_repetitions)] // The section keeps the name of its stage.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::LowOrderModelKind;

use crate::experiment::ControlExperimentKind;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::identify_result::study_evidence;
use crate::identification::plant::IdentifiedModel;
use crate::identification::recorded_model::check_gain_sign;
use crate::identification::refusal::Refusal;
use crate::loop_definition::{Action, ControllerProject, LoopDefinition};
use crate::messages::{TuneMethod, TuneRequest, TuneResult, TuneWords};
use crate::model::general::js_text;
use crate::model::GeneralModel;
use crate::recording::RecordingFile;
use crate::stages::{stage_availability, StageEvidence};
use crate::study::current_model::CurrentModel;
use crate::study::identify::{IdentifyMethod, IdentifySection};
use crate::study::StudySection;
use crate::tuning::automatic_criteria::validate_criteria;
use crate::tuning::candidate::{validate_parameters, ControlPidCandidate, LawCandidate};
use crate::tuning::design_to_requirements::structure_word;
use crate::tuning::group_candidate::GroupCandidate;
use crate::tuning::objective::TuningMethod;
use crate::tuning::on_off_candidate::OnOffCandidate;
use crate::tuning::plant::{check_normalized_error, check_plant, from_identified};
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::recommendation::Attempt;
use crate::tuning::relay_rule::{self, RelayCycleEvidence};
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::search::SearchCandidate;
use crate::tuning::settings_pins::{
    family_setting_symbols, ControllerSettingsPins, PinnedSettings,
};
use crate::tuning::state_controller::StateControllerDesign;
use crate::tuning::structure::Structure;
use crate::tuning::texts;

pub mod comparison;
pub mod design;
pub mod manual;
pub mod model;
pub mod on_off;
pub mod prediction;
pub mod record;
pub mod relay;
pub mod requirements;
pub mod result;
pub mod search_record;
pub mod state;
pub mod structure_choice;

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
    /// The candidate of a `Control.PID`, unapplied (B4.1, C1). Absent for a program's own law, which keeps `project_candidate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate: Option<ControlPidCandidate>,
    /// The candidate of a program's own law, unapplied (B4.1, C2). Absent for a `Control.PID`: exactly one of the two is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_candidate: Option<ProjectCandidate>,
    /// The design of a state-feedback law, unapplied (B4.1, C4). Absent for every other law: exactly one candidate field is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_candidate: Option<StateControllerDesign>,
    /// The best trial of a finite search over a program's law, unapplied (B4.1, C5). Absent for every other law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_candidate: Option<SearchCandidate>,
    /// The settings of a switching controller, unapplied (B4.1, C3). Absent for every other law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_off_candidate: Option<OnOffCandidate>,
    /// The design of a PID group, unapplied (B4.1, C7): each member's PI, the decoupler and the values Apply writes. Absent for every
    /// other law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_candidate: Option<GroupCandidate>,
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

    /// The candidate the section holds, whichever law it is for. `check` admits exactly one of the two candidates, so this is `None`
    /// only for a section that the study never keeps.
    #[must_use]
    pub fn law_candidate(&self) -> Option<LawCandidate> {
        match (
            &self.candidate,
            &self.project_candidate,
            &self.state_candidate,
            &self.search_candidate,
            &self.on_off_candidate,
            &self.group_candidate,
        ) {
            (Some(candidate), None, None, None, None, None) => Some(LawCandidate::Pid(*candidate)),
            (None, Some(candidate), None, None, None, None) => {
                Some(LawCandidate::Project(candidate.clone()))
            }
            (None, None, Some(design), None, None, None) => {
                Some(LawCandidate::State(Box::new(design.clone())))
            }
            (None, None, None, Some(search), None, None) => {
                Some(LawCandidate::Search(Box::new(search.clone())))
            }
            (None, None, None, None, Some(settings), None) => {
                Some(LawCandidate::OnOff(Box::new(settings.clone())))
            }
            (None, None, None, None, None, Some(group)) => {
                Some(LawCandidate::Group(Box::new(group.clone())))
            }
            _ => None,
        }
    }

    /// The structure of the study's controller candidate, when the candidate is a `Control.PID`'s or a program's own law's (B4.2): the
    /// family a request's pins belong to when it names no structure ([`resolve_pins`]).
    #[must_use]
    pub fn candidate_structure(&self) -> Option<Structure> {
        self.candidate
            .as_ref()
            .map(|candidate| candidate.structure)
            .or_else(|| {
                self.project_candidate
                    .as_ref()
                    .map(|candidate| candidate.structure)
            })
    }
}

/// The candidate fields of a Tune section: the one that the candidate's law fills, and the others absent (C1, C2, C4, C5).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CandidateFields {
    /// The `Control.PID` record.
    pub(crate) candidate: Option<ControlPidCandidate>,
    /// The candidate of a program's own law.
    pub(crate) project_candidate: Option<ProjectCandidate>,
    /// The design of a state-feedback law.
    pub(crate) state_candidate: Option<StateControllerDesign>,
    /// The best trial of a finite search.
    pub(crate) search_candidate: Option<SearchCandidate>,
    /// The settings of a switching controller.
    pub(crate) on_off_candidate: Option<OnOffCandidate>,
    /// The design of a PID group.
    pub(crate) group_candidate: Option<GroupCandidate>,
}

impl CandidateFields {
    /// The fields that hold `candidate`, with the other fields absent.
    #[must_use]
    pub(crate) fn of(candidate: LawCandidate) -> Self {
        match candidate {
            LawCandidate::Pid(candidate) => Self {
                candidate: Some(candidate),
                ..Self::default()
            },
            LawCandidate::Project(candidate) => Self {
                project_candidate: Some(candidate),
                ..Self::default()
            },
            LawCandidate::State(design) => Self {
                state_candidate: Some(*design),
                ..Self::default()
            },
            LawCandidate::Search(search) => Self {
                search_candidate: Some(*search),
                ..Self::default()
            },
            LawCandidate::OnOff(settings) => Self {
                on_off_candidate: Some(*settings),
                ..Self::default()
            },
            LawCandidate::Group(group) => Self {
                group_candidate: Some(*group),
                ..Self::default()
            },
        }
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
        let held = [
            self.candidate.is_some(),
            self.project_candidate.is_some(),
            self.state_candidate.is_some(),
            self.search_candidate.is_some(),
            self.on_off_candidate.is_some(),
            self.group_candidate.is_some(),
        ];
        if held.iter().filter(|present| **present).count() != 1 {
            return Err(invalid(
                "it holds neither a candidate nor a program candidate, or both (C1, C2)",
                Cause::Settings,
            ));
        }
        if let Some(candidate) = &self.candidate {
            validate_parameters(&candidate.parameters)
                .map_err(refused_as(Cause::Settings, NextAction::None))?;
            if candidate.applied {
                return Err(invalid(
                    "its candidate is marked applied, but Tune never writes to the controller (B4.1)",
                    Cause::Settings,
                ));
            }
        }
        if let Some(candidate) = &self.project_candidate {
            if candidate.applied {
                return Err(invalid(
                    "its program candidate is marked applied, but Tune never writes to the controller (B4.1)",
                    Cause::Settings,
                ));
            }
            if !candidate.is_finite() {
                return Err(invalid(
                    "its program candidate holds a value that is not finite",
                    Cause::Settings,
                ));
            }
        }
        if let Some(design) = &self.state_candidate
            && design.candidate.applied
        {
            return Err(invalid(
                    "its state candidate is marked applied, but Tune never writes to the controller (B4.1)",
                    Cause::Settings,
                ));
        }
        if let Some(search) = &self.search_candidate {
            search
                .check()
                .map_err(|reason| invalid(&reason, Cause::Settings))?;
        }
        if let Some(group) = &self.group_candidate {
            group
                .check_stored()
                .map_err(|reason| invalid(reason, Cause::Settings))?;
        }
        if let Some(settings) = &self.on_off_candidate {
            if settings.applied {
                return Err(invalid(
                    "its switching candidate is marked applied, but Tune never writes to the controller (B4.1)",
                    Cause::Settings,
                ));
            }
            if !settings.is_finite() {
                return Err(invalid(
                    "its switching candidate holds a value that is not finite",
                    Cause::Settings,
                ));
            }
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
    /// The pins the design takes (B4.2), with the family they are set for: the request's pins, resolved by [`resolve_pins`] or
    /// given as the run recorded them. `None` when the request names none.
    pub pins: Option<&'a ControllerSettingsPins>,
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

/// The refusal of a named method on a search law, which only Automatic tunes through its finite search (C5, B4.7).
const SEARCH_LAW_NAMED_METHOD: &str =
    "A search law is tuned by its finite search: choose Automatic.";

/// The refusals that come before any operation starts, in the order of B4.7's table. Each failure carries the next action
/// that fixes it:
/// - a loop without a controller law (C12): the failure of `require_controller_law`, which needs no action, as its stage is
///   Not applicable;
/// - no model that is current for the loop's signals, or a model the tuning rules cannot read: Identify;
/// - a gain that is zero or whose sign contradicts the controller action (N6.3): the loop setup;
/// - a loop that uses a feature the Rust engine does not run yet, or has no controller law without being a C12 loop: the
///   failure of `rust_engine_support` (no controller: the loop setup);
/// - settings that the manual method needs and the other methods refuse, pins for another loop revision or family, and
///   entered settings outside their bounds or without a structure (B4.2, B4.5): the settings themselves;
/// - a relay rule with no qualified relay record of the study (N5.R, N2.9): choose another method;
/// - a named method that does not apply to the model (N5.2 to N5.7): choose another method.
///
/// Automatic is not refused for its method: it tries the structures and reports each attempt (B4.4).
///
/// # Errors
/// The first of the refusals above that holds.
pub fn check_before_start(inputs: &TuneInputs<'_>) -> Result<(), Failure> {
    let loop_definition = inputs.loop_definition;
    // A search law is tuned by its own finite search, which Automatic starts (C5); no named method designs it.
    if inputs.request.method.is_some()
        && loop_definition
            .controller
            .project
            .as_ref()
            .is_some_and(ControllerProject::is_search_law)
    {
        return Err(Failure::new(
            SEARCH_LAW_NAMED_METHOD,
            Cause::Settings,
            NextAction::Tune,
        ));
    }
    // A switching controller has its own settings and its own checks (C3): no named method designs it, and its checks run in `on_off`.
    if loop_definition.controller.on_off.is_some() {
        return on_off::check_on_off(inputs);
    }
    // A PID group has no named method and its own checks (C7); its design runs in `design::pid_group`.
    if loop_definition.controller.group.is_some() {
        return design::pid_group::check(inputs);
    }
    // A state law is designed on the model's state-space form, which a low-order fit does not give, and its controller is the
    // state-feedback law itself, so the test for a loop without a controller law does not apply: its own checks run instead.
    if inputs.request.method == Some(TuneMethod::StateSpace) {
        current_for(inputs.model, loop_definition)?;
        return state::check_state(inputs);
    }
    loop_definition.require_controller_law()?;
    let current = current_for(inputs.model, loop_definition)?;
    let fitted = model::tuning_model(current)
        .map_err(refused_as(Cause::ModelOrData, NextAction::Identify))?;
    let integrating = fitted.kind == LowOrderModelKind::Integrating;
    check_gain_sign(loop_definition, fitted.gain, integrating)
        .map_err(refused_as(Cause::ModelOrData, NextAction::OpenLoopSetup))?;
    loop_definition.rust_engine_support()?;
    check_settings(inputs.request)?;
    check_pins(loop_definition, inputs)?;
    match inputs.request.method {
        Some(TuneMethod::Simc) => check_method(loop_definition, &fitted, TuningMethod::Simc),
        Some(TuneMethod::Amigo) => check_method(loop_definition, &fitted, TuningMethod::Amigo),
        Some(TuneMethod::Relay) => match relay::refusal_of(inputs.relay) {
            Some(failure) => Err(failure),
            None => Ok(()),
        },
        Some(TuneMethod::Manual) => check_manual(loop_definition, inputs.request),
        // A state law's own checks ran above. With no method named, nothing is left to check.
        None | Some(TuneMethod::StateSpace) => Ok(()),
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
    let mut outcome = design::run(inputs, held, running)?;
    let words = TuneWords::of_result(inputs.loop_definition, &outcome.result);
    outcome.result.words = Some(words);
    Ok(outcome)
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
    let words = TuneWords::of_result(inputs.loop_definition, &result);
    result.words = Some(words);
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

/// Whether the study's Identify section keeps a Feedback relay record: a live relay identification whose request holds the relay plan
/// (N5.R). The section alone says so, so a caller reads the estimation recording only when it does.
#[must_use]
pub fn holds_relay_record(identify: &IdentifySection) -> bool {
    matches!(
        identify.method,
        IdentifyMethod::Live(ControlExperimentKind::Relay)
    ) && identify.record_request.as_ref().is_some_and(|record| {
        matches!(record.request.experiment.kind, ControlExperimentKind::Relay)
            && record.request.experiment.relay_feedback.is_some()
    })
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
    if !holds_relay_record(identify) || recording.loop_id != loop_definition.id {
        return None;
    }
    let plan = identify
        .record_request
        .as_ref()?
        .request
        .experiment
        .relay_feedback
        .as_ref()?;
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

/// The pins of a request with the family they are set for (B4.2): the structure the request names, else the family of the study's
/// candidate, else the family of the study's recorded pins. The page names the pins without a family, so this is where the family is
/// decided. When none of the three applies, the pins have no family to belong to, and the request is refused with the sentence that
/// asks the page to choose a controller configuration (`family` field).
///
/// # Errors
/// The request names pins, and no structure, candidate family or recorded pins family applies.
pub fn resolve_pins(
    pins: &PinnedSettings,
    structure: Option<Structure>,
    candidate: Option<Structure>,
    recorded: Option<&ControllerSettingsPins>,
) -> Result<ControllerSettingsPins, Failure> {
    structure
        .or(candidate)
        .or(recorded.map(|recorded| recorded.family))
        .map(|family| pins.with_family(family))
        .ok_or_else(|| {
            Failure::new(texts::PINS_NEED_FAMILY, Cause::Settings, NextAction::None)
                .with_field("family")
        })
}

/// The settings a manual preview enters (B4.5): each symbol with its number. A preview needs them, so a missing value, or one that is
/// not a number, is refused with the preview's sentence.
///
/// # Errors
/// The settings are missing, or one of them is not a number.
pub fn preview_settings(settings: Option<&Value>) -> Result<BTreeMap<String, f64>, Failure> {
    // The page sends each entry as the engineer typed it: text that reads as a number is that number, and an entry that no number reads
    // is refused with the settings' sentence (B4.5).
    settings
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .map(|(symbol, value)| (symbol.clone(), crate::json::number_or_text(value)))
                .collect::<BTreeMap<String, f64>>()
        })
        .filter(|entries| entries.values().all(|value| !value.is_nan()))
        .ok_or_else(|| {
            Failure::new(
                texts::PREVIEW_NEEDS_SETTINGS,
                Cause::Settings,
                NextAction::None,
            )
        })
}

/// The pinned settings belong to this loop revision, and a request that fixes a structure fixes the pinned family (B4.2): the
/// pins are refused for another revision, and for a structure other than their family.
fn check_pins(loop_definition: &LoopDefinition, inputs: &TuneInputs<'_>) -> Result<(), Failure> {
    let Some(pins) = inputs.pins else {
        return Ok(());
    };
    if pins.loop_revision != loop_definition.revision {
        return Err(Failure::new(
            texts::PINS_LOOP_REVISION,
            Cause::Settings,
            NextAction::None,
        ));
    }
    if inputs
        .request
        .structure
        .is_some_and(|structure| structure != pins.family)
    {
        return Err(Failure::new(
            texts::PINS_FAMILY,
            Cause::Settings,
            NextAction::None,
        ));
    }
    let symbols = family_setting_symbols(loop_definition, pins.family);
    match pins.values.keys().find(|symbol| !symbols.contains(*symbol)) {
        Some(symbol) => Err(Failure::new(
            &texts::not_a_family_setting(symbol, structure_word(pins.family)),
            Cause::Settings,
            NextAction::None,
        )),
        None => Ok(()),
    }
}

/// The entered settings name the loop's settings and lie within their bounds (B4.5): the first that does not is the refusal,
/// with its own sentence. The presence of the settings is checked by [`check_settings`]. Manual settings take the structure the
/// request fixes: without one they are refused, as a preview is (B4.2).
fn check_manual(loop_definition: &LoopDefinition, request: &TuneRequest) -> Result<(), Failure> {
    let Some(settings) = &request.settings else {
        return Ok(());
    };
    // The refusal names its setting's field, under which the page shows it (B4.5).
    if let Some((symbol, sentence)) = manual::first_outside_bounds(loop_definition, settings) {
        return Err(Failure::new(&sentence, Cause::Settings, NextAction::None).with_field(&symbol));
    }
    // The structure the request fixes takes only its own settings (the page asks for these alone).
    let Some(family) = request.structure else {
        return Err(Failure::new(
            texts::MANUAL_NEEDS_CONTROLLER,
            Cause::Settings,
            NextAction::None,
        ));
    };
    let symbols = family_setting_symbols(loop_definition, family);
    match settings.keys().find(|symbol| !symbols.contains(*symbol)) {
        Some(symbol) => Err(Failure::new(
            &texts::not_a_family_setting(symbol, structure_word(family)),
            Cause::Settings,
            NextAction::None,
        )),
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
#[path = "tune_request_tests.rs"]
mod request_tests;

#[cfg(test)]
#[path = "tune_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "tune_tests.rs"]
mod tests;
