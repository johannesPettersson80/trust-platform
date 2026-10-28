//! Stage status (specification 35, §4.2): each stage's status follows the evidence the study
//! holds. Every stage stays reachable; the operations themselves enforce what they need.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::loop_definition::NO_CONTROLLER_LAW;

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
    /// Apply the validated candidate to the controller (B6): Restore previous settings and Keep in project follow it.
    Apply,
}

/// The stages in their order.
pub const STAGE_ORDER: [Stage; 6] = [
    Stage::Monitor,
    Stage::Test,
    Stage::Identify,
    Stage::Tune,
    Stage::Validate,
    Stage::Apply,
];

/// The six words of §4.2, and Not applicable for a stage the loop's structure does not use (C12).
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
    /// The loop has no controller law, so there is nothing for the stage to do (C12).
    NotApplicable,
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
            Self::NotApplicable => "Not applicable",
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
    /// Tune, Validate and Apply apply to the loop. False for an open-loop experiment or an observation loop (C12), whose
    /// project owns the controller: those three stages are Not applicable.
    pub controller_stages_apply: bool,
}

impl Default for StageEvidence {
    /// A bound loop with a controller law and no evidence yet.
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
            controller_stages_apply: true,
        }
    }
}

/// The reason Tune cannot start yet: it designs on a model, and the study holds none (B9.4).
pub const IDENTIFY_FIRST: &str = "Identify a plant model first.";

/// The reason Validate cannot start yet: it runs a candidate, and the study holds none (B9.4).
pub const TUNE_FIRST: &str = "Tune a controller first.";

/// One stage's status, with the words the panel shows. A stage that is Not applicable also names the reason, which the page
/// shows in place of its actions (C12), and so does a stage that an Apply's checkpoint holds (B6.4, B6.7) and a stage that cannot
/// start yet for want of what it works on (B9.4). `available` says whether the stage's own action can start now (B9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct StageAvailability {
    /// The stage.
    pub stage: Stage,
    /// Its status.
    pub status: StageStatus,
    /// The status in words.
    pub label: &'static str,
    /// Whether the stage's own action can start now (B9.4). The page enables the action exactly then, and holds no copy of the
    /// rule. A bound loop can always Monitor, Test and Identify. Tune needs a model, a declared analysis or manual controller
    /// parameters; Validate needs a candidate; Apply needs a candidate whose current validation passed ([`apply_available`]), and in
    /// an answer that states the Validate page's offers, Apply offered (B7, `application_offers::offered_stages`). A stage that is
    /// Not applicable never starts (C12), and while an Apply's checkpoint waits for its Restore only Monitor starts
    /// ([`blocked_stage_availability`]).
    pub available: bool,
    /// Why the stage does not apply, when its status is Not applicable (C12), or why its action waits, while an Apply's checkpoint
    /// waits for its Restore (B6.4, B6.7) and while the study lacks what the stage works on: a model for Tune, a candidate for
    /// Validate (B9.4). The page shows it on the stage's page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
}

/// The status of every stage, in [`STAGE_ORDER`].
#[must_use]
pub fn stage_availability(evidence: &StageEvidence) -> Vec<StageAvailability> {
    STAGE_ORDER
        .iter()
        .map(|&stage| {
            let status = stage_status(stage, evidence);
            let available = stage_available(stage, evidence);
            StageAvailability {
                stage,
                status,
                label: status.label(),
                available,
                reason: stage_reason(stage, status, available),
            }
        })
        .collect()
}

/// The status of every stage, in [`STAGE_ORDER`], while `block` holds the loop: the reason of an Apply's checkpoint that waits for its
/// Restore (B6.4, B6.7, A8.6; `commissioning::checkpoint_block`), or `None` when nothing holds it. A held stage whose action writes or
/// changes the study (Test, Identify, Tune, Validate and Apply) cannot start and names the block's reason; its status still follows the
/// evidence. Monitor, which only observes, and a stage that is Not applicable, which names its own reason (C12), keep theirs.
#[must_use]
pub fn blocked_stage_availability(
    evidence: &StageEvidence,
    block: Option<&'static str>,
) -> Vec<StageAvailability> {
    let mut stages = stage_availability(evidence);
    if let Some(reason) = block {
        for item in stages.iter_mut().filter(|item| {
            item.stage != Stage::Monitor && item.status != StageStatus::NotApplicable
        }) {
            item.available = false;
            item.reason = Some(reason);
        }
    }
    stages
}

/// Why the stage names a reason (see [`StageAvailability::reason`]): a stage that is Not applicable says so (C12); a stage that cannot
/// start yet says what it waits for, Tune a model and Validate a candidate (B9.4); a stage that can start names none.
fn stage_reason(stage: Stage, status: StageStatus, available: bool) -> Option<&'static str> {
    if status == StageStatus::NotApplicable {
        return Some(NO_CONTROLLER_LAW);
    }
    match stage {
        Stage::Tune if !available => Some(IDENTIFY_FIRST),
        Stage::Validate if !available => Some(TUNE_FIRST),
        _ => None,
    }
}

/// Whether the stage's own action can start now (see [`StageAvailability::available`]).
fn stage_available(stage: Stage, evidence: &StageEvidence) -> bool {
    match stage {
        Stage::Monitor | Stage::Test | Stage::Identify => evidence.loop_bound,
        Stage::Tune | Stage::Validate | Stage::Apply if !evidence.controller_stages_apply => false,
        Stage::Tune => has_tune_basis(evidence),
        Stage::Validate => evidence.has_candidate,
        Stage::Apply => apply_available(evidence),
    }
}

/// What Tune designs from: a model, a declared analysis or manual controller parameters.
fn has_tune_basis(evidence: &StageEvidence) -> bool {
    evidence.has_model || evidence.has_declared_analysis || evidence.has_manual_parameters
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
        Stage::Tune | Stage::Validate | Stage::Apply if !evidence.controller_stages_apply => {
            StageStatus::NotApplicable
        }
        Stage::Tune => tune_status(evidence),
        Stage::Validate => validate_status(evidence),
        Stage::Apply => apply_stage_status(evidence),
    }
}

/// Apply is ready exactly when Apply is available (B5.4): the candidate has a current validation that passed. The words of
/// "applied" come from the study's Apply section, which the status of this stage does not read.
fn apply_stage_status(evidence: &StageEvidence) -> StageStatus {
    if apply_available(evidence) {
        StageStatus::Ready
    } else {
        StageStatus::NeedsData
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
    match (has_tune_basis(evidence), evidence.has_candidate) {
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

    #[test]
    fn apply_follows_validate_and_is_ready_only_when_apply_is_available() {
        assert_eq!(STAGE_ORDER.last(), Some(&Stage::Apply));
        assert_eq!(
            STAGE_ORDER
                .iter()
                .position(|stage| *stage == Stage::Validate),
            Some(4),
            "Apply comes right after Validate"
        );
        assert_eq!(
            status(StageEvidence::default(), Stage::Apply),
            StageStatus::NeedsData
        );
        let candidate = StageEvidence {
            has_model: true,
            has_candidate: true,
            ..StageEvidence::default()
        };
        assert_eq!(
            status(candidate, Stage::Apply),
            StageStatus::NeedsData,
            "a candidate without a passed validation is not applied (B5.4)"
        );
        let passed = StageEvidence {
            validation_has_result: true,
            validation_passed: true,
            ..candidate
        };
        assert_eq!(status(passed, Stage::Apply), StageStatus::Ready);
    }

    #[test]
    fn a_loop_without_a_controller_law_makes_tune_validate_and_apply_not_applicable() {
        // C12: an open-loop experiment or observation loop has a model and a recording, and still nothing to tune.
        let observed = StageEvidence {
            has_model: true,
            has_recording: true,
            controller_stages_apply: false,
            ..StageEvidence::default()
        };
        let availability = stage_availability(&observed);
        for stage in [Stage::Tune, Stage::Validate, Stage::Apply] {
            let item = availability
                .iter()
                .find(|item| item.stage == stage)
                .expect("every stage");
            assert_eq!(item.status, StageStatus::NotApplicable, "{stage:?}");
            assert_eq!(item.label, "Not applicable", "{stage:?}");
            assert_eq!(item.reason, Some(NO_CONTROLLER_LAW), "{stage:?}");
        }
        assert_eq!(
            status(observed, Stage::Identify),
            StageStatus::Complete,
            "Identify still runs on the loop"
        );
        assert_eq!(status(observed, Stage::Monitor), StageStatus::Ready);
        assert!(
            availability
                .iter()
                .filter(|item| item.reason.is_none())
                .all(|item| item.status != StageStatus::NotApplicable),
            "only the stages that need a law name the reason"
        );
        assert!(!apply_available(&observed), "no candidate, so no Apply");
    }

    #[test]
    fn not_applicable_is_a_status_word_of_the_wire_and_a_reason_only_when_it_applies() {
        assert_eq!(
            serde_json::to_value(StageStatus::NotApplicable).expect("status"),
            serde_json::json!("not-applicable")
        );
        let ready = stage_availability(&StageEvidence::default());
        let wire = serde_json::to_value(ready[0]).expect("stage");
        assert!(wire.get("reason").is_none(), "{wire}");
        let lawless = stage_availability(&StageEvidence {
            controller_stages_apply: false,
            ..StageEvidence::default()
        });
        let tune = serde_json::to_value(lawless[3]).expect("stage");
        assert_eq!(tune["status"], "not-applicable", "{tune}");
        assert_eq!(tune["label"], "Not applicable", "{tune}");
        assert_eq!(tune["reason"], NO_CONTROLLER_LAW, "{tune}");
    }

    fn available(evidence: StageEvidence, stage: Stage) -> bool {
        stage_availability(&evidence)
            .into_iter()
            .find(|item| item.stage == stage)
            .map(|item| item.available)
            .expect("every stage")
    }

    #[test]
    fn a_stage_is_available_when_its_own_action_can_start() {
        // B9.4: the page enables Tune, Validate and Apply from this answer and keeps no copy of the rule.
        let none = StageEvidence::default();
        assert!(available(none, Stage::Identify));
        assert!(!available(none, Stage::Tune));
        assert!(!available(none, Stage::Validate));
        assert!(!available(none, Stage::Apply));
        for basis in [
            StageEvidence {
                has_model: true,
                ..none
            },
            StageEvidence {
                has_declared_analysis: true,
                ..none
            },
            StageEvidence {
                has_manual_parameters: true,
                ..none
            },
        ] {
            assert!(available(basis, Stage::Tune), "{basis:?}");
            assert!(
                !available(basis, Stage::Validate),
                "a basis is not a candidate: {basis:?}"
            );
        }
        let candidate = StageEvidence {
            has_model: true,
            has_candidate: true,
            ..none
        };
        assert!(available(candidate, Stage::Validate));
        assert!(
            !available(candidate, Stage::Apply),
            "a candidate without a passed validation is not applied (B5.4)"
        );
        let passed = StageEvidence {
            validation_has_result: true,
            validation_passed: true,
            ..candidate
        };
        assert!(available(passed, Stage::Apply));
    }

    #[test]
    fn a_candidate_alone_does_not_make_tune_available() {
        // Tune needs a model, a declared analysis or manual parameters, as its status says (NeedsData).
        let candidate_only = StageEvidence {
            has_candidate: true,
            ..StageEvidence::default()
        };
        assert!(!available(candidate_only, Stage::Tune));
        assert_eq!(
            status(candidate_only, Stage::Tune),
            StageStatus::NeedsData,
            "the status and the availability agree"
        );
        assert!(available(candidate_only, Stage::Validate));
    }

    #[test]
    fn a_stage_that_does_not_apply_or_has_no_bound_loop_never_starts() {
        // C12: a loop without a controller law leaves Tune, Validate and Apply Not applicable, whatever evidence the study holds.
        let lawless = StageEvidence {
            has_model: true,
            has_candidate: true,
            has_recording: true,
            validation_has_result: true,
            validation_passed: true,
            controller_stages_apply: false,
            ..StageEvidence::default()
        };
        for stage in [Stage::Tune, Stage::Validate, Stage::Apply] {
            assert!(!available(lawless, stage), "{stage:?}");
        }
        assert!(available(lawless, Stage::Identify));
        let unbound = StageEvidence {
            loop_bound: false,
            ..StageEvidence::default()
        };
        for stage in [Stage::Monitor, Stage::Test, Stage::Identify] {
            assert!(!available(unbound, stage), "{stage:?}");
        }
    }

    #[test]
    fn a_stage_that_cannot_run_yet_states_why() {
        // B9.4: the page shows the engine's reason on the stage's page and writes none, so a stage that waits for what it works on names
        // it: Tune a model (or a declared analysis, or manual parameters), Validate a candidate. The words are the page's former ones.
        let reason = |evidence: StageEvidence, stage: Stage| {
            stage_availability(&evidence)
                .into_iter()
                .find(|item| item.stage == stage)
                .and_then(|item| item.reason)
        };
        let none = StageEvidence::default();
        assert_eq!(
            reason(none, Stage::Tune),
            Some("Identify a plant model first.")
        );
        assert_eq!(
            reason(none, Stage::Validate),
            Some("Tune a controller first.")
        );
        // Once Tune can start its reason is gone, and Validate keeps waiting for a candidate.
        let model = StageEvidence {
            has_model: true,
            ..none
        };
        assert_eq!(reason(model, Stage::Tune), None);
        assert_eq!(
            reason(model, Stage::Validate),
            Some("Tune a controller first.")
        );
        // A candidate that is validated and passed leaves no stage waiting; Identify, Monitor, Test and Apply name none either way.
        let passed = StageEvidence {
            has_model: true,
            has_candidate: true,
            validation_has_result: true,
            validation_passed: true,
            ..none
        };
        for stage in STAGE_ORDER {
            assert_eq!(reason(passed, stage), None, "{stage:?}");
        }
        for stage in [Stage::Monitor, Stage::Test, Stage::Identify, Stage::Apply] {
            assert_eq!(reason(none, stage), None, "{stage:?}");
        }
        // A stage that does not apply says so rather than what it waits for (C12), and a checkpoint's block replaces either (B6.4).
        let lawless = StageEvidence {
            controller_stages_apply: false,
            ..none
        };
        assert_eq!(reason(lawless, Stage::Tune), Some(NO_CONTROLLER_LAW));
        let blocked = blocked_stage_availability(&none, Some("A Restore waits."));
        assert_eq!(blocked[3].reason, Some("A Restore waits."));
        assert_eq!(blocked[4].reason, Some("A Restore waits."));
    }

    #[test]
    fn availability_is_a_field_of_the_wire() {
        let model = StageEvidence {
            has_model: true,
            ..StageEvidence::default()
        };
        let stages = stage_availability(&model);
        let tune = serde_json::to_value(stages[3]).expect("stage");
        assert_eq!(tune["stage"], "Tune", "{tune}");
        assert_eq!(tune["available"], serde_json::json!(true), "{tune}");
        let validate = serde_json::to_value(stages[4]).expect("stage");
        assert_eq!(validate["stage"], "Validate", "{validate}");
        assert_eq!(
            validate["available"],
            serde_json::json!(false),
            "{validate}"
        );
    }
}
