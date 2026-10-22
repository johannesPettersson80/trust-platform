//! The Apply section of a study (specification 35, A8.1, A8.4, A8.5, B6.4 and B6.5): the receipts of each Apply, each Restore
//! previous settings and each Keep in project. An Apply's receipt names its checkpoint (A8.5), and a Restore or a Keep names the
//! checkpoint it acts on, so the section decides from its own receipts when Restore and Keep are offered. It authorizes nothing:
//! Apply is authorized by the Validate result alone (`stages::apply_available`, B5.4). An invalidated receipt stays in the study
//! as history and never authorizes an Apply (A8.4).

#![allow(clippy::module_name_repetitions)] // `ApplySection` is the section of the Apply stage, as `ValidateSection` is of Validate.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::commissioning::commissioning_result::{CommissioningResult, CommissioningStatus};
use crate::commissioning::found_mode::FoundMode;
use crate::failure::{Cause, Failure, NextAction};

use super::StudySection;

pub mod texts;

/// The receipt of one Apply (A8.1): the checkpoint it journaled, the validation it was reviewed on, whether its settings are
/// still applied, and its result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationReceipt {
    /// The identifier of the checkpoint the Apply journaled before its first write (A8.5).
    pub checkpoint: String,
    /// The validation identity the Apply was reviewed on (B6.1, A8.4).
    pub validation_identity: String,
    /// Whether the checkpoint ended `completed`: the Apply's settings are applied, and Restore can return them (B6.4).
    pub settings_applied: bool,
    /// The Apply's result (B6.3, B6.7).
    pub result: CommissioningResult,
}

/// The receipt of one Restore previous settings (B6.4): the checkpoint it acted on, whether the runtime confirmed the restored
/// settings, whether the checkpoint was retired because the runtime was replaced, the sentence that says so, and the mode the loop
/// was left in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestorationReceipt {
    /// The checkpoint the Restore acted on.
    pub checkpoint: String,
    /// Whether the runtime confirmed the previous settings back (B6.4: Restored is shown only after that).
    pub restored: bool,
    /// Whether the checkpoint was retired as history because the runtime was restarted or its program reloaded (B6.4).
    pub retired: bool,
    /// The sentence the engineer reads (A11.1).
    pub reason: String,
    /// The mode the loop was left in, when it is known (A6.2, B6.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<FoundMode>,
}

/// The receipt of one Keep in project (B6.5): the checkpoint of the Apply whose values it kept, and the fields it kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeepReceipt {
    /// The checkpoint of the Apply whose running values were kept.
    pub checkpoint: String,
    /// The record that was kept, for example `Main.TIC101_Parameters`.
    pub record: String,
    /// The fields the Keep wrote.
    pub kept: Vec<String>,
    /// The fields that the equipment has no setting for, which were not kept.
    pub unknown: Vec<String>,
}

/// The Apply section of a study, kept under the key `apply` (A8.1): each Apply, Restore and Keep, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplySection {
    /// Each Apply's receipt, oldest first.
    #[serde(default)]
    pub applications: Vec<ApplicationReceipt>,
    /// Each Restore's receipt, oldest first.
    #[serde(default)]
    pub restorations: Vec<RestorationReceipt>,
    /// Each Keep's receipt, oldest first.
    #[serde(default)]
    pub keeps: Vec<KeepReceipt>,
}

impl ApplySection {
    /// The newest Apply, when its settings are still applied: its checkpoint ended `completed`, and no Restore has confirmed them
    /// or retired the checkpoint (B6.4). An earlier Apply is history, because a later Apply replaces its checkpoint (A8.5).
    fn applied(&self) -> Option<&ApplicationReceipt> {
        let application = self.applications.last()?;
        let closed = self.restorations.iter().any(|restoration| {
            restoration.checkpoint == application.checkpoint
                && (restoration.restored || restoration.retired)
        });
        (application.settings_applied && !closed).then_some(application)
    }

    /// The identity of the validation that the newest Apply applied (A8.1, A8.4), whatever became of its settings since: they may stand,
    /// or a Restore may have returned the previous settings or retired its checkpoint. Apply is not offered again for that validation
    /// (B7, `application_offers::newest_applied_validation`). `None` before the first Apply's receipt.
    #[must_use]
    pub fn newest_validation(&self) -> Option<&str> {
        self.applications
            .last()
            .map(|application| application.validation_identity.as_str())
    }

    /// Whether Restore previous settings is offered (B6.4): the newest Apply's settings are applied. A Restore that the runtime has
    /// not confirmed leaves it offered, so the engineer restores again (B6.7).
    #[must_use]
    pub fn restore_available(&self) -> bool {
        self.applied().is_some()
    }

    /// The checkpoint that Keep in project keeps, when it is offered (B6.5): the newest Apply's, whose settings are applied and whose
    /// result passed or failed a requirement, that is, was not cancelled. Keep is not offered after a Restore, a Restore that the runtime
    /// has not confirmed blocks it as well (B7), and once Keep has kept these values, it is not offered again for them.
    #[must_use]
    pub fn keep_checkpoint(&self) -> Option<&str> {
        let application = self.applied()?;
        let named = |checkpoint: &str| checkpoint == application.checkpoint;
        let restored = self
            .restorations
            .iter()
            .any(|restoration| named(&restoration.checkpoint));
        let kept = self.keeps.iter().any(|keep| named(&keep.checkpoint));
        (application.result.status != CommissioningStatus::Cancelled && !restored && !kept)
            .then_some(application.checkpoint.as_str())
    }

    /// Whether Keep in project is offered (B6.5); see [`ApplySection::keep_checkpoint`].
    #[must_use]
    pub fn keep_available(&self) -> bool {
        self.keep_checkpoint().is_some()
    }

    /// Whether an Apply of the section made the checkpoint `checkpoint`: a Restore or a Keep receipt may name only such a checkpoint
    /// (A8.5). An Apply that was interrupted before its receipt was kept made a checkpoint that no receipt names, and the loop's journal
    /// is then the checkpoint's only record.
    #[must_use]
    pub fn names_an_apply(&self, checkpoint: &str) -> bool {
        self.applications
            .iter()
            .any(|application| application.checkpoint == checkpoint)
    }
}

impl StudySection for ApplySection {
    const KEY: &'static str = "apply";

    /// Every Apply names its own checkpoint, once; every Restore and Keep names the checkpoint of an Apply (A8.5); a Restore is
    /// confirmed or retired, never both (B6.4). A section that breaks one of these rules is never kept and never read (B10.4).
    fn check(&self) -> Result<(), Failure> {
        for (index, application) in self.applications.iter().enumerate() {
            if application.checkpoint.is_empty() {
                return Err(invalid("an Apply names no checkpoint"));
            }
            if self.applications[..index]
                .iter()
                .any(|earlier| earlier.checkpoint == application.checkpoint)
            {
                return Err(invalid("two Applies name the same checkpoint"));
            }
        }
        for restoration in &self.restorations {
            if restoration.restored && restoration.retired {
                return Err(invalid("a Restore is both confirmed and retired"));
            }
            if !self.names_an_apply(&restoration.checkpoint) {
                return Err(invalid("a Restore names the checkpoint of no Apply"));
            }
        }
        if self
            .keeps
            .iter()
            .any(|keep| !self.names_an_apply(&keep.checkpoint))
        {
            return Err(invalid("a Keep names the checkpoint of no Apply"));
        }
        Ok(())
    }

    /// The recordings that the Applies' results hold: each one is kept in the study (A8.3).
    fn recording_ids(&self) -> Vec<&str> {
        self.applications
            .iter()
            .filter_map(|application| {
                application
                    .result
                    .recording
                    .as_ref()
                    .map(|run| run.recording_id.as_str())
            })
            .collect()
    }
}

/// The refusal of a Restore that the study does not offer (B6.4): no Apply's settings are applied, or its checkpoint is closed.
#[must_use]
pub fn restore_unavailable() -> Failure {
    Failure::new(
        texts::RESTORE_UNAVAILABLE,
        Cause::Settings,
        NextAction::None,
    )
}

/// The refusal of a Keep in project that the study does not offer (B6.5).
#[must_use]
pub fn keep_unavailable() -> Failure {
    Failure::new(texts::KEEP_UNAVAILABLE, Cause::Settings, NextAction::None)
}

fn invalid(reason: &str) -> Failure {
    Failure::new(
        &texts::broken_section(reason),
        Cause::ModelOrData,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
