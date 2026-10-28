//! The record a campaign journals before each dispatch (`IdentificationCampaignJournal`; specification 35, A8.5). The host keeps
//! it beside the loop revision and the mode checkpoint, so that a reopened host can reattach to a running step (B3.4).

#![allow(clippy::module_name_repetitions)] // The journal's names state what it records.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::baseline::BaselineSummary;
use crate::experiment::{ControlExperimentPlan, RecordingRecoveryOwner};
use crate::failure::{Cause, Failure, NextAction};

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;

/// The version of the journal's record (`schemaVersion: 1`).
pub const JOURNAL_SCHEMA_VERSION: u32 = 1;

/// The sentence of a resume that finds no running step to reattach to (`resumeCampaign`).
pub const NO_RUNNING_EXPERIMENT_TEXT: &str = "The interrupted identification has no running experiment to resume. Identify again to start a new campaign.";

/// The phase a campaign is in when it journals its record (`phase` of the journal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CampaignPhase {
    /// The campaign waits for the output to settle, before its step.
    Baseline,
    /// The campaign has dispatched its step and holds it.
    Step,
}

/// The record a campaign keeps before each dispatch (A8.5): its schema version, its loop revision, its phase, the identity of
/// its recording, and for a step its plan, its baseline and where the step starts. `channels_remaining` counts the plant inputs
/// that still follow this one: a resumed campaign steps them in the same recording (C7, C8). `baseline_end_seconds` is where the
/// steady baseline ends, which the fit of the recording starts its window from, and `stepped_plans` holds the plans of the inputs
/// stepped before this one, so that the record of a resumed campaign keeps every experiment of the recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationCampaignJournal {
    /// Always [`JOURNAL_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The loop revision the campaign runs against.
    pub loop_revision: String,
    /// The phase the campaign is in.
    pub phase: CampaignPhase,
    /// The recording the campaign follows: its identity, with no credential and no data.
    pub recording: RecordingRecoveryOwner,
    /// The reviewed plan of the step, once the step is dispatched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<ControlExperimentPlan>,
    /// The steady baseline the step is judged against, once it is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<BaselineSummary>,
    /// Where the step starts, in process time of the recording, once the step is dispatched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_start_seconds: Option<f64>,
    /// Where the steady baseline ends, in process time of the recording, once the step is dispatched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_end_seconds: Option<f64>,
    /// The plans of the plant inputs stepped before this one, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stepped_plans: Vec<ControlExperimentPlan>,
    /// The plant inputs that still follow this one.
    pub channels_remaining: u64,
}

impl IdentificationCampaignJournal {
    /// The record of a campaign in its baseline phase, before any step is dispatched.
    #[must_use]
    pub fn baseline_phase(
        loop_revision: &str,
        recording: RecordingRecoveryOwner,
        channels_remaining: u64,
    ) -> Self {
        Self {
            schema_version: JOURNAL_SCHEMA_VERSION,
            loop_revision: loop_revision.to_string(),
            phase: CampaignPhase::Baseline,
            recording,
            plan: None,
            baseline: None,
            step_start_seconds: None,
            baseline_end_seconds: None,
            stepped_plans: Vec::new(),
            channels_remaining,
        }
    }

    /// The record of a campaign whose step is dispatched: the reviewed plan, the baseline, and where the step starts. Its progress
    /// (the end of the baseline and the plans stepped before) is added by [`Self::with_progress`].
    #[must_use]
    pub fn step_phase(
        loop_revision: &str,
        recording: RecordingRecoveryOwner,
        plan: ControlExperimentPlan,
        baseline: BaselineSummary,
        step_start_seconds: f64,
        channels_remaining: u64,
    ) -> Self {
        Self {
            schema_version: JOURNAL_SCHEMA_VERSION,
            loop_revision: loop_revision.to_string(),
            phase: CampaignPhase::Step,
            recording,
            plan: Some(plan),
            baseline: Some(baseline),
            step_start_seconds: Some(step_start_seconds),
            baseline_end_seconds: None,
            stepped_plans: Vec::new(),
            channels_remaining,
        }
    }

    /// The record with the end of the steady baseline and the plans of the inputs stepped before this step.
    #[must_use]
    pub fn with_progress(
        mut self,
        baseline_end_seconds: f64,
        stepped_plans: Vec<ControlExperimentPlan>,
    ) -> Self {
        self.baseline_end_seconds = Some(baseline_end_seconds);
        self.stepped_plans = stepped_plans;
        self
    }

    /// The plan and the baseline of a dispatched step that a resumed campaign reattaches to. The record must be a step of
    /// the same schema version and loop revision, and must hold both the plan and the baseline (`resumeCampaign`).
    ///
    /// # Errors
    /// The record is not a dispatched step of this loop revision: there is no running experiment to resume.
    pub fn resumable_step(
        &self,
        loop_revision: &str,
    ) -> Result<(&ControlExperimentPlan, &BaselineSummary), Failure> {
        match (&self.plan, &self.baseline) {
            (Some(plan), Some(baseline))
                if self.schema_version == JOURNAL_SCHEMA_VERSION
                    && self.phase == CampaignPhase::Step
                    && self.loop_revision == loop_revision =>
            {
                Ok((plan, baseline))
            }
            _ => Err(Failure::new(
                NO_RUNNING_EXPERIMENT_TEXT,
                Cause::Unclassified,
                NextAction::Identify,
            )),
        }
    }
}
