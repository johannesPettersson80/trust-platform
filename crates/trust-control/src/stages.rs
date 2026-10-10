//! Stage status (specification 35, §4.2): each stage's status follows the evidence the study
//! holds. Every stage stays reachable; the operations themselves enforce what they need.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A stage of Control Studio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Stage {
    /// The live loop.
    Monitor,
    /// A manual test of the loop.
    Test,
    /// Identify the plant.
    Identify,
    /// Tune the controller.
    Tune,
    /// Validate the controller.
    Validate,
}

/// The stages in their order.
pub const STAGE_ORDER: [Stage; 5] = [
    Stage::Monitor,
    Stage::Test,
    Stage::Identify,
    Stage::Tune,
    Stage::Validate,
];

/// The six words of §4.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StageStatus {
    /// It can run.
    Ready,
    /// It needs evidence it does not have.
    NeedsData,
    /// It is running.
    InProgress,
    /// It has its result.
    Complete,
    /// It has a result with a warning.
    Warning,
    /// Its last run failed.
    Failed,
}

impl StageStatus {
    /// The status as the panel writes it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::NeedsData => "Needs data",
            Self::InProgress => "In progress",
            Self::Complete => "Complete",
            Self::Warning => "Warning",
            Self::Failed => "Failed",
        }
    }
}

/// The evidence a study holds that decides the stages' status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct StageEvidence {
    /// A loop is bound, so Monitor and Test can be used.
    pub loop_bound: bool,
    /// A plant model exists.
    pub has_model: bool,
    /// The project declares its own analysis.
    pub has_declared_analysis: bool,
    /// Manual controller parameters are entered.
    pub has_manual_parameters: bool,
    /// A controller candidate exists.
    pub has_candidate: bool,
    /// A recording exists.
    pub has_recording: bool,
    /// The last identification failed.
    pub identification_failed: bool,
    /// Validation has a result.
    pub validation_has_result: bool,
    /// That result passed.
    pub validation_passed: bool,
    /// That result is inconclusive: a requirement lacks the evidence to pass or to fail (B5.4).
    pub validation_inconclusive: bool,
}

impl Default for StageEvidence {
    /// A bound loop with no evidence yet.
    fn default() -> Self {
        Self {
            loop_bound: true,
            has_model: false,
            has_declared_analysis: false,
            has_manual_parameters: false,
            has_candidate: false,
            has_recording: false,
            identification_failed: false,
            validation_has_result: false,
            validation_passed: false,
            validation_inconclusive: false,
        }
    }
}

/// One stage's status, with the words the panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct StageAvailability {
    /// The stage.
    pub stage: Stage,
    /// Its status.
    pub status: StageStatus,
    /// The status in words.
    pub label: &'static str,
}

/// The status of every stage, in [`STAGE_ORDER`].
#[must_use]
pub fn stage_availability(evidence: &StageEvidence) -> Vec<StageAvailability> {
    STAGE_ORDER
        .iter()
        .map(|&stage| {
            let status = stage_status(stage, evidence);
            StageAvailability {
                stage,
                status,
                label: status.label(),
            }
        })
        .collect()
}

fn stage_status(stage: Stage, evidence: &StageEvidence) -> StageStatus {
    match stage {
        Stage::Monitor | Stage::Test => {
            if evidence.loop_bound {
                StageStatus::Ready
            } else {
                StageStatus::NeedsData
            }
        }
        Stage::Identify => identify_status(evidence),
        Stage::Tune => tune_status(evidence),
        Stage::Validate => validate_status(evidence),
    }
}

fn identify_status(evidence: &StageEvidence) -> StageStatus {
    if evidence.identification_failed {
        StageStatus::Failed
    } else if evidence.has_model {
        StageStatus::Complete
    } else if evidence.has_recording {
        StageStatus::Ready
    } else {
        StageStatus::NeedsData
    }
}

fn tune_status(evidence: &StageEvidence) -> StageStatus {
    let has_basis =
        evidence.has_model || evidence.has_declared_analysis || evidence.has_manual_parameters;
    match (has_basis, evidence.has_candidate) {
        (false, _) => StageStatus::NeedsData,
        (true, true) => StageStatus::Complete,
        (true, false) => StageStatus::Ready,
    }
}

fn validate_status(evidence: &StageEvidence) -> StageStatus {
    if !evidence.has_candidate {
        return if evidence.has_recording {
            StageStatus::Ready
        } else {
            StageStatus::NeedsData
        };
    }
    match (
        evidence.validation_has_result,
        evidence.validation_passed,
        evidence.validation_inconclusive,
    ) {
        (false, _, _) => StageStatus::Ready,
        (true, true, _) => StageStatus::Complete,
        (true, false, true) => StageStatus::NeedsData,
        (true, false, false) => StageStatus::Failed,
    }
}

/// Whether Apply is available (B5.4): the candidate has a current validation that passed. That result already holds the
/// model check and every requirement of the candidate (N8.5), so nothing else decides Apply.
#[must_use]
pub fn apply_available(evidence: &StageEvidence) -> bool {
    evidence.has_candidate && evidence.validation_has_result && evidence.validation_passed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(evidence: StageEvidence, stage: Stage) -> StageStatus {
        stage_availability(&evidence)
            .into_iter()
            .find(|item| item.stage == stage)
            .map(|item| item.status)
            .expect("every stage")
    }

    #[test]
    fn manual_parameters_or_a_declared_analysis_make_tune_ready_without_a_model() {
        let manual = StageEvidence {
            has_manual_parameters: true,
            ..StageEvidence::default()
        };
        assert_eq!(status(manual, Stage::Tune), StageStatus::Ready);
        assert_eq!(status(manual, Stage::Identify), StageStatus::NeedsData);
        assert_eq!(status(manual, Stage::Validate), StageStatus::NeedsData);
        let declared = StageEvidence {
            has_declared_analysis: true,
            ..StageEvidence::default()
        };
        assert_eq!(status(declared, Stage::Tune), StageStatus::Ready);
        assert_eq!(status(declared, Stage::Identify), StageStatus::NeedsData);
    }

    #[test]
    fn the_status_words_of_section_4_2_follow_the_evidence() {
        let none = StageEvidence::default();
        for stage in [Stage::Identify, Stage::Tune, Stage::Validate] {
            assert_eq!(status(none, stage), StageStatus::NeedsData);
        }
        let model = StageEvidence {
            has_model: true,
            ..none
        };
        assert_eq!(
            (status(model, Stage::Identify), status(model, Stage::Tune)),
            (StageStatus::Complete, StageStatus::Ready)
        );
        let candidate = StageEvidence {
            has_candidate: true,
            ..model
        };
        assert_eq!(status(candidate, Stage::Tune), StageStatus::Complete);
        assert_eq!(
            status(candidate, Stage::Validate),
            StageStatus::Ready,
            "a candidate exists, so Validate is ready to run"
        );
        let passed = StageEvidence {
            validation_has_result: true,
            validation_passed: true,
            ..candidate
        };
        assert_eq!(status(passed, Stage::Validate), StageStatus::Complete);
        let failed = StageEvidence {
            validation_passed: false,
            ..passed
        };
        assert_eq!(status(failed, Stage::Validate), StageStatus::Failed);
        let candidate_only = StageEvidence {
            has_candidate: true,
            ..none
        };
        assert_eq!(
            status(candidate_only, Stage::Tune),
            StageStatus::NeedsData,
            "a candidate without a model, analysis or settings needs data"
        );
        let unbound = StageEvidence {
            loop_bound: false,
            ..none
        };
        assert_eq!(
            (
                status(unbound, Stage::Monitor),
                status(unbound, Stage::Test)
            ),
            (StageStatus::NeedsData, StageStatus::NeedsData)
        );
        assert_eq!(StageStatus::NeedsData.label(), "Needs data");
    }

    #[test]
    fn an_inconclusive_validation_needs_data_and_apply_waits_for_a_passed_one() {
        let candidate = StageEvidence {
            has_model: true,
            has_candidate: true,
            ..StageEvidence::default()
        };
        let inconclusive = StageEvidence {
            validation_has_result: true,
            validation_inconclusive: true,
            ..candidate
        };
        assert_eq!(
            status(inconclusive, Stage::Validate),
            StageStatus::NeedsData,
            "missing evidence is not a failed run (B5.4)"
        );
        assert!(!apply_available(&inconclusive));
        let passed = StageEvidence {
            validation_has_result: true,
            validation_passed: true,
            ..candidate
        };
        assert_eq!(status(passed, Stage::Validate), StageStatus::Complete);
        assert!(apply_available(&passed));
        assert!(!apply_available(&candidate), "no validation, no Apply");
    }
}
