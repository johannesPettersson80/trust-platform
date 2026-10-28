//! The inputs a study records (specification 35, §0.1a `controlStudio/inputs`, A8.1, A8.4, B4.2 and B4.3): the acquisition
//! settings of Identify, the requirements of Tune and Validate with the verification scenarios declared beside them, the tuning
//! method, a fixed structure, the pinned settings, and the settings the manual method takes. The page's editors write them as they
//! change, the study keeps them, so reopening shows them (A8.6), and a run (`controlStudio/run`, B8) reads them.
//!
//! Each input has one source. The response of a formula method is the requirements' declared response (B4.3), read by
//! [`InputsSection::declared_response`]; the tuning method is the `method` input, and the requirements keep no method of their own
//! ([`InputsSection::without_requirement_method`]).
//!
//! What a change invalidates is decided here (A8.4). A change of the requirements the study holds, or of the scenario list it
//! declares, invalidates the validations, because a validation is current only for the test it ran ([`HeldTest`]). A change of any
//! other input invalidates nothing here: a candidate is replaced only by Tune, and the model stays current until its signals change
//! (A8.2). Tune designs to the requirements the study holds ([`held_requirements`]), and Validate judges the candidate against them,
//! so a validation made for other requirements is no longer current ([`tune_as_held`]).

#![allow(clippy::module_name_repetitions)] // The section keeps the name of its stage's inputs.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::acquisition_settings::{
    check_acquisition_settings, unreadable_duration_failure, AcquisitionSettings,
};
use crate::loop_definition::LoopDefinition;
use crate::messages::{TuneChoice, TuneMethod};
use crate::recording::opened_channels::{check_selection, RecordingSignalSelection};
use crate::stages::StageEvidence;
use crate::study::current_model::current_model;
use crate::study::tune::state::StateDesignInput;
use crate::study::tune::{refused_as, TuneSection};
use crate::study::validate::{stage_evidence as validate_stage_evidence, ValidateSection};
use crate::study::{Study, StudySection};
use crate::tuning::automatic_criteria::validate_criteria;
use crate::tuning::requirements::{RequirementRefusal, TuningRequirements};
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::structure::Structure;
use crate::validation::engineer_scenario_effects::requirements_failure;
use crate::validation::engineer_verification_campaign::declared_scenarios;
use crate::validation::scenario_contract::{
    latest_scenario_check, valid_engineer_scenarios, EngineerScenario,
};
use crate::validation::texts;

/// The inputs a study records, kept under the key `inputs` (A8.1). Every input is optional: an absent one is not yet given,
/// so the stage uses its own default (Automatic for the method, the loop's declared limits for the acquisition).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputsSection {
    /// The acquisition settings of Identify: the method, the duration, the amplitude and the ranges (B3.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisition: Option<AcquisitionSettings>,
    /// The requirements of Tune and Validate (B4.3), as a whole record. A value the engineer gave is kept as given; a value
    /// that Automatic generated is listed in the record's `automaticCriteria`, and Tune fills it again from the model. The record
    /// holds the declared response (`tuningObjective`), and no tuning method (the `method` input holds it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements: Option<TuningRequirements>,
    /// The tuning method (B4.2); absent for Automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<TuneMethod>,
    /// The structure the engineer fixed (B4.4); absent leaves the structure to Automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<Structure>,
    /// The settings the engineer pinned for the design (B4.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<ControllerSettingsPins>,
    /// The settings the manual method takes: the record symbol of each setting and its value (B4.2, B4.5). A run that reaches
    /// Tune with the manual method and no settings pauses for them (B8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<BTreeMap<String, f64>>,
    /// The verification scenarios that the requirements editor declares, as a whole list (B4.3, B9.9): Validate runs these instead
    /// of the loop's own declared scenarios ([`held_scenarios`]). Absent when the engineer declared none, so the loop's scenarios
    /// stand; an empty list declares that there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenarios: Option<Vec<EngineerScenario>>,
    /// The plant input and measured output the engineer selected for an opened recording (B3.6). The study keeps them, so that
    /// reopening shows the selection; absent until a recording is opened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_selection: Option<RecordingSignalSelection>,
    /// The state-design choices of a state-feedback law (D-B11, C4): the family, the draft of the design form and the pinned
    /// coefficients, as the engineer left them. Tune reads them ([`StateDesignInput`]); absent until a state law is chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_design: Option<StateDesignInput>,
    /// The pinned settings of a switching controller (B4.2, C3): each of its three settings by symbol, as the engineer left them. Tune
    /// designs the other settings, applies these after the design, and refuses a pin outside its binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_off_pins: Option<BTreeMap<String, f64>>,
}

impl InputsSection {
    /// Whether the study chose the manual method and has not yet recorded its settings (B8): a run that reaches Tune then
    /// names the manual settings as the input the engineer must give.
    #[must_use]
    pub fn needs_manual_settings(&self) -> bool {
        self.method == Some(TuneMethod::Manual) && self.settings.is_none()
    }

    /// The settings the manual method takes, when the study chose that method (B4.2): the entered settings belong to the manual
    /// method alone, so after another method is chosen they are not given to Tune, which refuses settings with any other method.
    #[must_use]
    pub fn manual_settings(&self) -> Option<&BTreeMap<String, f64>> {
        if self.method == Some(TuneMethod::Manual) {
            self.settings.as_ref()
        } else {
            None
        }
    }

    /// The response of a formula method that the study declares (N5.4): the requirements' `tuningObjective`, unless Automatic
    /// generated it (B4.3). `None` leaves the response to the policy, which fills it (B4.3). Tune and the automatic run both read
    /// the response here, so a declared response reaches both the same way.
    #[must_use]
    pub fn declared_response(&self) -> Option<TuneChoice> {
        let requirements = self.requirements.as_ref()?;
        let generated = requirements
            .automatic_criteria
            .as_ref()
            .is_some_and(|criteria| {
                criteria
                    .fields
                    .iter()
                    .any(|field| field == "tuningObjective")
            });
        if generated {
            return None;
        }
        requirements.tuning_objective.map(TuneChoice::of_objective)
    }

    /// The section with the requirements' tuning method removed. The method is the `method` input (B4.2), and a requirement record
    /// that carried a copy of it would be a second source for the same choice (A8.1). Every other input is kept as given.
    #[must_use]
    pub fn without_requirement_method(mut self) -> Self {
        if let Some(requirements) = self.requirements.as_mut() {
            requirements.tuning_method = None;
        }
        self
    }
}

impl StudySection for InputsSection {
    const KEY: &'static str = "inputs";

    /// The requirements hold their rules (B4.3) and their generated-criteria record is valid; the acquisition settings hold the
    /// checks of Identify (B3.2); the declared scenarios hold the checks of the campaign. A section that breaks one is never kept
    /// and never read (B10.4).
    fn check(&self) -> Result<(), Failure> {
        if let Some(requirements) = &self.requirements {
            requirements
                .check()
                .map_err(RequirementRefusal::into_failure)?;
            if let Some(criteria) = &requirements.automatic_criteria {
                validate_criteria(criteria)
                    .map_err(refused_as(Cause::Settings, NextAction::OpenRequirements))?;
            }
        }
        if let Some(settings) = &self.acquisition {
            // A time that no duration reads is refused under its field; the other settings are checked as their JSON is, so the
            // refusal names the first rule of Identify they break, and a refused duration or range names its field (B3.2).
            if settings.duration_seconds.is_nan() {
                return Err(unreadable_duration_failure());
            }
            let value = serde_json::to_value(settings).unwrap_or_default();
            check_acquisition_settings(&value)?;
        }
        if self
            .scenarios
            .as_deref()
            .is_some_and(|scenarios| !valid_engineer_scenarios(scenarios))
        {
            return Err(requirements_failure(&texts::scenarios_invalid()).with_field("scenarios"));
        }
        if let Some(selection) = &self.recording_selection {
            check_selection(selection)?;
        }
        if self
            .on_off_pins
            .as_ref()
            .is_some_and(|pins| pins.values().any(|value| !value.is_finite()))
        {
            return Err(Failure::new(
                "A pinned switching setting is not a finite number.",
                Cause::Settings,
                NextAction::None,
            ));
        }
        if self
            .state_design
            .as_ref()
            .and_then(|design| design.pins.as_ref())
            .is_some_and(|pins| pins.values().any(|value| !value.is_finite()))
        {
            return Err(Failure::new(
                "A pinned state coefficient is not a finite number.",
                Cause::Settings,
                NextAction::None,
            ));
        }
        Ok(())
    }

    fn recording_ids(&self) -> Vec<&str> {
        Vec::new()
    }
}

/// The requirements the study holds (B4.3, A8.4): the ones the engineer recorded in the inputs, or else the ones the last
/// Tune designed to. Tune designs to them, and Validate judges the candidate against them.
#[must_use]
pub fn held_requirements<'a>(
    inputs: Option<&'a InputsSection>,
    tune: Option<&'a TuneSection>,
) -> Option<&'a TuningRequirements> {
    inputs
        .and_then(|inputs| inputs.requirements.as_ref())
        .or_else(|| tune.map(|tune| &tune.requirements))
}

/// The verification scenarios the study holds (B4.3, A8.4): the list the engineer declared, or else the loop's own declared list.
/// A loop's list that the campaign refuses is no list a validation can have run with, so it is taken as none here; Validate
/// refuses that list itself (B5.5).
#[must_use]
pub fn held_scenarios(
    loop_definition: &LoopDefinition,
    inputs: Option<&InputsSection>,
) -> Vec<EngineerScenario> {
    match inputs.and_then(|inputs| inputs.scenarios.as_ref()) {
        Some(scenarios) => scenarios.clone(),
        None => declared_scenarios(loop_definition).unwrap_or_default(),
    }
}

/// The loop as Validate runs it (B4.3, A8.4): the loop with the scenario list the study declares in place of its own. The
/// campaign reads its scenarios from the loop, so Validate gives this loop to the campaign and to the validation it records.
///
/// # Errors
/// The declared list cannot be written into the loop (an implementation failure).
pub fn held_loop(
    loop_definition: &LoopDefinition,
    inputs: Option<&InputsSection>,
) -> Result<LoopDefinition, Failure> {
    let mut held = loop_definition.clone();
    if let Some(scenarios) = inputs.and_then(|inputs| inputs.scenarios.as_ref()) {
        held.verification_scenarios = Some(serde_json::to_value(scenarios).map_err(|_| {
            Failure::new(
                UNWRITABLE_SCENARIOS,
                Cause::Implementation,
                NextAction::None,
            )
        })?);
    }
    Ok(held)
}

/// The Tune section as Validate reads it: its requirements are the ones the study holds ([`held_requirements`]), and its
/// candidate and model revision are the design's own. A section without recorded requirements is returned unchanged.
#[must_use]
pub fn tune_as_held(tune: &TuneSection, inputs: Option<&InputsSection>) -> TuneSection {
    let mut held = tune.clone();
    if let Some(requirements) = held_requirements(inputs, None) {
        held.requirements = requirements.clone();
    }
    held
}

/// The test a study holds (A8.4): the requirements and the declared scenarios that Tune designs to and Validate judges against.
/// A validation is current only for the test it ran, so a change of either one leaves it behind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeldTest<'a> {
    /// The requirements the study holds ([`held_requirements`]).
    pub requirements: Option<&'a TuningRequirements>,
    /// The verification scenarios the study holds ([`held_scenarios`]).
    pub scenarios: &'a [EngineerScenario],
}

/// The sentence the Validate page shows when a change left the validations not current (A8.4, B9.9).
pub const INPUTS_CHANGED: &str = "Inputs changed. Tune and verify before applying.";

/// What a change invalidates (A8.4): the validations, when the test the study holds before the change differs from the test it
/// holds after it. A candidate stays with the model and the test it was designed to, and every other input waits for its own
/// stage, so none of them is reported here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Invalidation {
    /// Whether the study's validations are no longer current for the test it holds (A8.4).
    pub validation: bool,
    /// The sentence the page shows on the Validate page when the validations are no longer current, as the engine words it
    /// ([`INPUTS_CHANGED`]); absent when the change invalidated nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
}

impl Invalidation {
    /// What a change invalidates when it leaves the validations not current (`validation`) or current: the former states its sentence.
    #[must_use]
    pub fn of(validation: bool) -> Self {
        Self {
            validation,
            reason: validation.then_some(INPUTS_CHANGED),
        }
    }
}

/// What a change from the test `before` to the test `after` invalidates (A8.4): the validations, when the requirements or the
/// declared scenarios are not the same. Nothing changes when both are equal. A changed limit or method invalidates nothing here.
#[must_use]
pub fn invalidation(before: &HeldTest<'_>, after: &HeldTest<'_>) -> Invalidation {
    Invalidation::of(before != after)
}

/// The stage evidence after a change (A8.4): a validation the change invalidated is no longer current, so the evidence names no result.
/// The stages follow it, and the page hides the table of that result. The study keeps the result as history.
#[must_use]
pub fn evidence_after_invalidation(
    mut evidence: StageEvidence,
    invalidated: Invalidation,
) -> StageEvidence {
    if invalidated.validation {
        evidence.validation_has_result = false;
        evidence.validation_inconclusive = false;
        evidence.validation_passed = false;
    }
    evidence
}

/// The refusal of a declared check that ends after the validation duration (B4.3, B9.9): a change that gives the requirements or the
/// scenarios must leave every check inside the validation, so the latest check may not end after the duration it runs in. The refusal
/// names the duration it needs, under the duration field. A scenario event written after the duration is refused the same way, under
/// the scenarios' field (the page's event-time limit, B4.3). Stored sections are not checked again here, so a study kept before this rule
/// still reads.
///
/// # Errors
/// Returns the settings failure of the first scenario event after the duration, under `scenarios`, or of the latest check that ends after
/// the duration, under `durationSeconds`.
pub fn check_within_duration(
    requirements: &TuningRequirements,
    scenarios: &[EngineerScenario],
) -> Result<(), Failure> {
    let duration = requirements.duration_seconds;
    if let Some(scenario) = scenarios.iter().find(|scenario| {
        scenario
            .events
            .iter()
            .any(|event| event.at_ms > duration * 1000.0 + 1e-9)
    }) {
        let sentence = format!(
            "Scenario {} writes an event after the validation duration of {duration} s.",
            scenario.id
        );
        return Err(requirements_failure(&sentence).with_field("scenarios"));
    }
    let Some(latest) =
        latest_scenario_check(scenarios).filter(|latest| latest.end_seconds > duration + 1e-9)
    else {
        return Ok(());
    };
    let sentence = texts::scenario_check_after_run(
        &latest.scenario,
        &latest.requirement,
        latest.end_seconds,
        duration,
    );
    Err(requirements_failure(&sentence).with_field("durationSeconds"))
}

/// The refusal of a scenario event that is not written on a task sample (B4.3, B9.9): Validate writes an event at a sample of the loop's
/// task, so an event time is a whole number of the task's periods (`engineer_verification_campaign` refuses the same event when the
/// campaign is built). A change of the scenarios is checked here as it is made, under the scenarios' field. A task period that is not a
/// whole number of milliseconds is refused by the campaign with its own sentence, so no event is checked against it here, and stored
/// sections are not checked again, so a study kept before this rule still reads.
///
/// # Errors
/// Returns the settings failure of the first scenario event whose time is not a multiple of the task period, under `scenarios`.
pub fn check_events_on_grid(
    scenarios: &[EngineerScenario],
    task_period_ms: f64,
) -> Result<(), Failure> {
    if task_period_ms < 1.0 || task_period_ms.fract() != 0.0 {
        return Ok(());
    }
    for scenario in scenarios {
        if let Some(event) = scenario
            .events
            .iter()
            .find(|event| event.at_ms % task_period_ms != 0.0)
        {
            let sentence =
                texts::scenario_event_between_samples(&scenario.id, event.at_ms, task_period_ms);
            return Err(requirements_failure(&sentence).with_field("scenarios"));
        }
    }
    Ok(())
}

/// The evidence that sets the stages of a study (A8.4, A8.6, B3.7): Identify and Tune read the study's current model, and a Validate
/// result counts only while it is current for that model, its candidate, the requirements the study holds and the scenarios it
/// declares.
///
/// # Errors
/// A stored section of the study cannot be read (B10.4).
pub fn stage_evidence_of(
    loop_definition: &LoopDefinition,
    study: &Study,
) -> Result<StageEvidence, Failure> {
    let current = current_model(study)?;
    let tune = study.section::<TuneSection>()?;
    let inputs = study.section::<InputsSection>()?;
    let validate = study.section::<ValidateSection>()?;
    let held = tune
        .as_ref()
        .map(|tune| tune_as_held(tune, inputs.as_ref()));
    let scenarios = held_scenarios(loop_definition, inputs.as_ref());
    Ok(validate_stage_evidence(
        loop_definition,
        current.as_ref(),
        held.as_ref(),
        validate.as_ref(),
        !study.recordings.is_empty(),
        &scenarios,
    ))
}

/// The failure of declared scenarios that cannot be written into the loop, which no valid list does.
const UNWRITABLE_SCENARIOS: &str = "The declared scenarios cannot be written into the loop.";

#[cfg(test)]
#[path = "inputs_tests.rs"]
mod tests;
