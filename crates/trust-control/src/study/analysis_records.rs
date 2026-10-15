//! The analysis records of a study (specification 35, B10.2): the declared analysis, the Routh–Hurwitz reading, the general
//! feedback analysis and the measured frequency response, each kept with the loop revision it was calculated on and, where a
//! model applies, the model's revision. A section whose revisions are no longer current is not answered (A8.4), and a section
//! that refers to a recording the study does not list is refused (B10.4). The sections are registered under their names in
//! [`StudySection::KEY`], as the Tune section is.

#![allow(clippy::module_name_repetitions)] // The records are the study's own concept.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::analysis::declared::{
    check_saved_declared, DeclaredRootLocusResult, SavedDeclaredAnalysis,
};
use crate::analysis::general_feedback::{
    check_saved_general_feedback, FeedbackResult, SavedGeneralFeedback,
};
use crate::analysis::measured_frequency::{MeasuredFrequencyResult, MeasuredStatus};
use crate::analysis::measured_paths::{MeasuredPathsResult, PairingStatus};
use crate::analysis::routh::DeclaredRouthResult;
use crate::analysis::routh_session::{check_saved_routh, SavedRouthAnalysis};
use crate::failure::{Cause, Failure, NextAction};
use crate::model::GeneralModel;
use crate::study::{is_recording_id, Study, StudySection};

/// The declared analysis of the loop (`declaredAnalysis`, B10.2): its definition and pole paths, with the loop revision and the
/// revision of the declared plant it was calculated on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredAnalysisSection {
    /// The loop revision the analysis was calculated on.
    pub loop_revision: String,
    /// The revision of the declared plant the analysis was calculated on.
    pub model_revision: String,
    /// The definition and its pole paths.
    pub analysis: SavedDeclaredAnalysis,
}

/// The Routh–Hurwitz reading of the loop (`routh`, B10.2): its definition, its selected value and its result, with the revisions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouthSection {
    /// The loop revision the reading was calculated on.
    pub loop_revision: String,
    /// The revision of the declared plant the reading was calculated on.
    pub model_revision: String,
    /// The definition, the selected value and the result.
    pub analysis: SavedRouthAnalysis,
}

/// The general feedback analysis of the loop (`generalFeedback`, B10.2): its definition, its result with the margin and Nyquist
/// evidence, and the revisions it was calculated on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneralFeedbackSection {
    /// The loop revision the analysis was calculated on.
    pub loop_revision: String,
    /// The revision of the plant model the analysis was calculated on; the analysis is current only for the study's model.
    pub model_revision: String,
    /// The definition and the result.
    pub analysis: SavedGeneralFeedback,
}

/// The measured frequency response (`periodicFrequency`, B10.2): the recording it was measured on, its settings and its tones,
/// with the loop revision it was calculated on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeriodicFrequencySection {
    /// The loop revision the response was calculated on.
    pub loop_revision: String,
    /// The recording, its settings and the calculated tones.
    pub result: MeasuredFrequencyResult,
}

impl StudySection for DeclaredAnalysisSection {
    const KEY: &'static str = "declaredAnalysis";

    fn check(&self) -> Result<(), Failure> {
        check_revisions(&self.loop_revision, &self.model_revision)?;
        check_saved_declared(&self.analysis)?;
        if self.model_revision != self.analysis.definition.plant.revision {
            return Err(refused(
                "names a model revision that is not its declared plant's",
            ));
        }
        Ok(())
    }

    fn recording_ids(&self) -> Vec<&str> {
        Vec::new()
    }
}

impl StudySection for RouthSection {
    const KEY: &'static str = "routh";

    fn check(&self) -> Result<(), Failure> {
        check_revisions(&self.loop_revision, &self.model_revision)?;
        check_saved_routh(&self.analysis)?;
        if self.model_revision != self.analysis.definition.plant.revision {
            return Err(refused(
                "names a model revision that is not its declared plant's",
            ));
        }
        Ok(())
    }

    fn recording_ids(&self) -> Vec<&str> {
        Vec::new()
    }
}

impl StudySection for GeneralFeedbackSection {
    const KEY: &'static str = "generalFeedback";

    fn check(&self) -> Result<(), Failure> {
        check_revisions(&self.loop_revision, &self.model_revision)?;
        let result = &self.analysis.result;
        if usize::try_from(result.grid.points).ok() != Some(result.open_loop.len()) {
            return Err(refused(
                "holds a number of open-loop points that its grid does not give",
            ));
        }
        let frequencies: Vec<f64> = result
            .open_loop
            .iter()
            .map(|point| point.frequency_rad_per_second)
            .collect();
        check_saved_general_feedback(&self.analysis, &frequencies).map_err(Failure::from)?;
        if self.model_revision != self.analysis.definition.plant.identity().revision {
            return Err(refused("names a model revision that is not its plant's"));
        }
        Ok(())
    }

    fn recording_ids(&self) -> Vec<&str> {
        Vec::new()
    }
}

impl StudySection for PeriodicFrequencySection {
    const KEY: &'static str = "periodicFrequency";

    fn check(&self) -> Result<(), Failure> {
        let result = &self.result;
        if self.loop_revision.trim().is_empty()
            || result.recording_revision.trim().is_empty()
            || !is_recording_id(&result.recording_id)
        {
            return Err(refused("does not name its recording and revision"));
        }
        if !(result.sample_time_seconds.is_finite() && result.sample_time_seconds > 0.0)
            || result.segment_count < 2
            || result.startup_samples < 1
            || !(0.0..=1.0).contains(&result.minimum_coherence)
        {
            return Err(refused("holds settings outside their limits (N2.8)"));
        }
        let points_match = match result.status {
            MeasuredStatus::Qualified => result.points.len() == result.frequencies.len(),
            MeasuredStatus::Rejected => result.points.is_empty() && result.reason.is_some(),
        };
        if !points_match {
            return Err(refused("holds tones that do not match its status"));
        }
        Ok(())
    }

    fn recording_ids(&self) -> Vec<&str> {
        vec![self.result.recording_id.as_str()]
    }
}

/// The measured MIMO paths of a recording (`measuredPaths`, C7; N4.5): the answer with its recording and trials, and the loop revision
/// it was calculated on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeasuredPathsSection {
    /// The loop revision the paths were calculated on.
    pub loop_revision: String,
    /// The recording, its trials, the matrices and the pairing.
    pub result: MeasuredPathsResult,
}

impl StudySection for MeasuredPathsSection {
    const KEY: &'static str = "measuredPaths";

    fn check(&self) -> Result<(), Failure> {
        let result = &self.result;
        if self.loop_revision.trim().is_empty()
            || result.recording_revision.trim().is_empty()
            || !is_recording_id(&result.recording_id)
        {
            return Err(refused("does not name its recording and revision"));
        }
        let inputs = result.inputs.len();
        let outputs = result.outputs.len();
        let shaped = inputs >= 2
            && inputs == outputs
            && result.trials.len() == inputs
            && is_matrix(&result.gain_matrix, outputs, inputs)
            && is_matrix(&result.relative_gain_array, outputs, inputs)
            && is_matrix(&result.decoupler, inputs, outputs);
        let paired = match result.pairing.status {
            PairingStatus::Named => {
                result.pairing.pairs.len() == outputs && result.pairing.reason.is_none()
            }
            PairingStatus::Refused => {
                result.pairing.pairs.is_empty() && result.pairing.reason.is_some()
            }
        };
        if !shaped || !paired {
            return Err(refused(
                "holds channels, trials, matrices or a pairing that do not match one another (C7)",
            ));
        }
        Ok(())
    }

    fn recording_ids(&self) -> Vec<&str> {
        vec![self.result.recording_id.as_str()]
    }
}

/// Whether `matrix` has `rows` rows of `columns` entries each.
fn is_matrix(matrix: &[Vec<f64>], rows: usize, columns: usize) -> bool {
    matrix.len() == rows && matrix.iter().all(|row| row.len() == columns)
}

/// A result that the study keeps (B10.2), as the analysis tool calculated it.
#[derive(Debug, Clone, PartialEq)]
pub enum KeptAnalysis {
    /// A declared analysis with its pole paths (`rootLocus`).
    RootLocus(Box<SavedDeclaredAnalysis>),
    /// A Routh–Hurwitz reading (`routh`).
    Routh(Box<SavedRouthAnalysis>),
    /// A general feedback analysis (`feedback`).
    Feedback(Box<SavedGeneralFeedback>),
    /// A measured frequency response (`measuredFrequency`).
    MeasuredFrequency(Box<MeasuredFrequencyResult>),
    /// The measured MIMO paths of a recording (`measuredPaths`, C7).
    MeasuredPaths(Box<MeasuredPathsResult>),
}

/// The kept results of a loop that the study answers as current (`controlStudio/open`, `analysis`). A result is absent when the
/// study keeps none, when its loop revision is not the loop's, or, for a general feedback analysis, when its model is not the
/// study's model (A8.4).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRecords {
    /// The declared analysis's pole paths, with their sentences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_locus: Option<DeclaredRootLocusResult>,
    /// The Routh–Hurwitz reading, with its sentences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routh: Option<DeclaredRouthResult>,
    /// The general feedback analysis, with its sentences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<FeedbackResult>,
    /// The measured frequency response, with its sentences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_frequency: Option<MeasuredFrequencyResult>,
    /// The measured MIMO paths, with their matrices and pairing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_paths: Option<MeasuredPathsResult>,
}

/// Keeps `analysis` in the study under its section (B10.2), with the loop revision `loop_revision` and the model revision the
/// result was calculated on. A result that refers to a recording the study does not list is refused, and nothing is kept.
///
/// # Errors
/// Returns the refusal of the section's rules, or of a recording that the study does not list.
pub fn keep(study: &mut Study, loop_revision: &str, analysis: KeptAnalysis) -> Result<(), Failure> {
    let loop_revision = loop_revision.to_string();
    match analysis {
        KeptAnalysis::RootLocus(analysis) => {
            let model_revision = analysis.definition.plant.revision.clone();
            keep_section(
                study,
                &DeclaredAnalysisSection {
                    loop_revision,
                    model_revision,
                    analysis: *analysis,
                },
            )
        }
        KeptAnalysis::Routh(analysis) => {
            let model_revision = analysis.definition.plant.revision.clone();
            keep_section(
                study,
                &RouthSection {
                    loop_revision,
                    model_revision,
                    analysis: *analysis,
                },
            )
        }
        KeptAnalysis::Feedback(analysis) => {
            let model_revision = analysis.definition.plant.identity().revision.clone();
            keep_section(
                study,
                &GeneralFeedbackSection {
                    loop_revision,
                    model_revision,
                    analysis: *analysis,
                },
            )
        }
        KeptAnalysis::MeasuredFrequency(result) => keep_section(
            study,
            &PeriodicFrequencySection {
                loop_revision,
                result: *result,
            },
        ),
        KeptAnalysis::MeasuredPaths(result) => keep_section(
            study,
            &MeasuredPathsSection {
                loop_revision,
                result: *result,
            },
        ),
    }
}

/// The records of `study` that are current for the loop revision `loop_revision` and, for a general feedback analysis, the
/// study's `model` (A8.4). A section that breaks its rules, or that refers to a recording the study does not list, is not read.
#[must_use]
pub fn current_records(
    study: &Study,
    loop_revision: &str,
    model: Option<&GeneralModel>,
) -> AnalysisRecords {
    let declared = read::<DeclaredAnalysisSection>(study)
        .filter(|section| section.loop_revision == loop_revision);
    let routh =
        read::<RouthSection>(study).filter(|section| section.loop_revision == loop_revision);
    let feedback = read::<GeneralFeedbackSection>(study).filter(|section| {
        section.loop_revision == loop_revision
            && model.is_some_and(|model| model.identity().revision == section.model_revision)
    });
    let periodic = read::<PeriodicFrequencySection>(study)
        .filter(|section| section.loop_revision == loop_revision);
    let paths = read::<MeasuredPathsSection>(study)
        .filter(|section| section.loop_revision == loop_revision);
    AnalysisRecords {
        root_locus: declared.map(|section| section.analysis.result),
        routh: routh.map(|section| section.analysis.result),
        feedback: feedback.map(|section| section.analysis.result),
        measured_frequency: periodic.map(|section| section.result),
        measured_paths: paths.map(|section| section.result),
    }
}

fn read<S: StudySection>(study: &Study) -> Option<S> {
    study.section::<S>().ok().flatten()
}

/// Keeps a section: its recordings must already be listed by the study, and its rules must hold.
fn keep_section<S: StudySection>(study: &mut Study, section: &S) -> Result<(), Failure> {
    for id in section.recording_ids() {
        if !study.recordings.iter().any(|listed| listed.as_str() == id) {
            return Err(refused(&format!(
                "refers to recording {id}, which the study does not list"
            )));
        }
    }
    study.set_section(section)
}

fn check_revisions(loop_revision: &str, model_revision: &str) -> Result<(), Failure> {
    if loop_revision.trim().is_empty() || model_revision.trim().is_empty() {
        return Err(refused("names no loop or model revision"));
    }
    Ok(())
}

fn refused(reason: &str) -> Failure {
    Failure::new(
        &format!("This analysis record cannot be used: it {reason}."),
        Cause::ModelOrData,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "analysis_records_tests.rs"]
mod tests;
