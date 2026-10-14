//! The inputs a study records (specification 35, §0.1a `controlStudio/inputs`, A8.1, A8.4, B4.2 and B4.3): the acquisition
//! settings of Identify, the requirements of Tune and Validate, the tuning method with its response and a fixed structure, the
//! pinned settings, and the settings the manual method takes. The page's editors write them as they change, the study keeps
//! them, so reopening shows them (A8.6), and a run (`controlStudio/run`, B8) reads them.
//!
//! What a change invalidates is decided here (A8.4). Changed requirements invalidate the validations, because a validation is
//! current only for the requirements it validated; every other input invalidates nothing until its own stage runs again. Tune
//! designs to the requirements the study holds ([`held_requirements`]), and Validate judges the candidate against them, so a
//! validation made for other requirements is no longer current ([`tune_as_held`]).

#![allow(clippy::module_name_repetitions)] // The section keeps the name of its stage's inputs.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::acquisition_settings::{
    decode_acquisition_settings, AcquisitionSettings,
};
use crate::loop_definition::LoopDefinition;
use crate::messages::{TuneChoice, TuneMethod};
use crate::stages::StageEvidence;
use crate::study::identify::IdentifySection;
use crate::study::tune::{refused_as, TuneSection};
use crate::study::validate::{stage_evidence as validate_stage_evidence, ValidateSection};
use crate::study::{Study, StudySection};
use crate::tuning::automatic_criteria::validate_criteria;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::structure::Structure;

/// The inputs a study records, kept under the key `inputs` (A8.1). Every input is optional: an absent one is not yet given,
/// so the stage uses its own default (Automatic for the method, the loop's declared limits for the acquisition).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputsSection {
    /// The acquisition settings of Identify: the method, the duration, the amplitude and the ranges (B3.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisition: Option<AcquisitionSettings>,
    /// The requirements of Tune and Validate (B4.3), as a whole record. A value the engineer gave is kept as given; a value
    /// that Automatic generated is listed in the record's `automaticCriteria`, and Tune fills it again from the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements: Option<TuningRequirements>,
    /// The tuning method (B4.2); absent for Automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<TuneMethod>,
    /// The response of a formula method (N5.4); absent leaves it to the method's default (B4.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<TuneChoice>,
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
}

impl StudySection for InputsSection {
    const KEY: &'static str = "inputs";

    /// The requirements hold their rules (B4.3) and their generated-criteria record is valid; the acquisition settings hold the
    /// checks of Identify (B3.2). A section that breaks one is never kept and never read (B10.4).
    fn check(&self) -> Result<(), Failure> {
        if let Some(requirements) = &self.requirements {
            requirements
                .validate()
                .map_err(refused_as(Cause::Settings, NextAction::OpenRequirements))?;
            if let Some(criteria) = &requirements.automatic_criteria {
                validate_criteria(criteria)
                    .map_err(refused_as(Cause::Settings, NextAction::OpenRequirements))?;
            }
        }
        if let Some(settings) = &self.acquisition {
            let decodable = serde_json::to_value(settings)
                .is_ok_and(|value| decode_acquisition_settings(&value).is_some());
            if !decodable {
                return Err(invalid(
                    "the acquisition settings break a rule of Identify (B3.2)",
                ));
            }
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

/// What a change of the requirements the study holds invalidates (A8.4): the validations, when the held requirements differ.
/// A candidate stays with the model and the requirements it was designed to, and every other input waits for its own stage,
/// so none of them is reported here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Invalidation {
    /// Whether the study's validations are no longer current for the requirements it holds (A8.4).
    pub validation: bool,
}

/// What a change from `held_before` to `held_after` invalidates (A8.4): the validations, when the requirements the study holds
/// are not the same. Nothing changes when they are equal, and a change of any other input invalidates nothing here.
#[must_use]
pub fn invalidation(
    held_before: Option<&TuningRequirements>,
    held_after: Option<&TuningRequirements>,
) -> Invalidation {
    Invalidation {
        validation: held_before != held_after,
    }
}

/// The evidence that sets the stages of a study (A8.4, A8.6): Identify and Tune read their sections, and a Validate result
/// counts only while it is current for the study's model, its candidate and the requirements the study holds.
///
/// # Errors
/// A stored section of the study cannot be read (B10.4).
pub fn stage_evidence_of(
    loop_definition: &LoopDefinition,
    study: &Study,
) -> Result<StageEvidence, Failure> {
    let identify = study.section::<IdentifySection>()?;
    let tune = study.section::<TuneSection>()?;
    let inputs = study.section::<InputsSection>()?;
    let validate = study.section::<ValidateSection>()?;
    let held = tune
        .as_ref()
        .map(|tune| tune_as_held(tune, inputs.as_ref()));
    Ok(validate_stage_evidence(
        loop_definition,
        identify.as_ref(),
        held.as_ref(),
        validate.as_ref(),
        !study.recordings.is_empty(),
    ))
}

/// A stored inputs section that breaks a rule of its own (B10.4).
fn invalid(reason: &str) -> Failure {
    Failure::new(
        &format!("The inputs of the study are not valid: {reason}."),
        Cause::Settings,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "inputs_tests.rs"]
mod tests;
