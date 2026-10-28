//! The qualitative visual review of a draft (specification 43, §10.1, "Review accepts a qualitative
//! `visualReview`"): who reviewed a proposal, the verdict, and the object- or file-specific issues.
//! The record is evidence only. It grants no deployment or write authority and derives no beauty
//! score.
//!
//! Port of the record logic of `editors/vscode/src/hmiBuilder/agent/visualReview.ts` (`reviewRecord`
//! and the types it stores). The verdict is stored in the draft, so its rules are logic and live here;
//! the presentation of a review stays in the extension (checklist H4.7).

use serde::{Deserialize, Serialize};

use super::error::{AgentError, AgentResult};

/// The number of unsuccessful reviews after which the customer must decide: the third one
/// (specification 43, §10.1).
const CUSTOMER_DECISION_AT: u32 = 3;

/// The verdict of a visual review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Verdict {
    /// The proposal is acceptable. An accepted review holds no unresolved issue.
    Accepted,
    /// The proposal needs repair before it can be accepted.
    NeedsRepair,
}

/// An object- or file-specific issue of a review (`issues` in `visualReview.ts`). A stored issue tolerates
/// extra fields, so a record written by the TypeScript store still reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReviewIssue {
    /// The page the issue concerns. The caller checks that the draft holds this page.
    pub(super) file: String,
    /// The objects of that page the issue concerns. The caller checks that they exist.
    pub(super) objects: Vec<String>,
    /// What the reviewer saw.
    pub(super) message: String,
}

/// A review as the reviewer submits it: the fields `reviewRecord` takes from its caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct VisualReviewInput {
    /// Who reviewed the proposal. It must not be blank.
    pub(super) reviewer: String,
    /// The verdict.
    pub(super) verdict: Verdict,
    /// The issues that remain.
    pub(super) issues: Vec<ReviewIssue>,
}

/// A review of one revision of a draft, as the store keeps it (`VisualReview` in `visualReview.ts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VisualReview {
    /// The draft revision the review examined.
    pub(super) revision: String,
    /// Who reviewed the proposal.
    pub(super) reviewer: String,
    /// The verdict.
    pub(super) verdict: Verdict,
    /// The issues that remain.
    pub(super) issues: Vec<ReviewIssue>,
    /// The number of reviews of the draft, this one included.
    pub(super) attempt: u32,
    /// Whether the customer must decide, because this review is the third unsuccessful one.
    pub(super) customer_decision_required: bool,
}

/// The review history of a draft before a review: how many reviews were made, and how many of them
/// needed repair. An edit clears the verdict but keeps this history (specification 43, §10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct ReviewHistory {
    /// The reviews made so far.
    pub(super) attempts: u32,
    /// The reviews that needed repair so far.
    pub(super) unsuccessful: u32,
}

impl ReviewHistory {
    /// The history once a review with `verdict` is recorded.
    #[must_use]
    pub(super) fn after(self, verdict: Verdict) -> Self {
        Self {
            attempts: self.attempts.saturating_add(1),
            unsuccessful: self
                .unsuccessful
                .saturating_add(u32::from(verdict == Verdict::NeedsRepair)),
        }
    }
}

/// Records a review of `revision` made after `history` (`reviewRecord` in `visualReview.ts`).
///
/// The customer decision is requested by the third unsuccessful review, counted from the reviews that
/// needed repair; accepted reviews do not count (specification 43, §10.1).
///
/// # Errors
///
/// `INVALID_ARGUMENT` when the reviewer is blank, or when an accepted review holds an unresolved issue.
pub(super) fn review_record(
    input: &VisualReviewInput,
    revision: &str,
    history: ReviewHistory,
) -> AgentResult<VisualReview> {
    if input.reviewer.trim().is_empty() {
        return Err(AgentError::invalid(
            "the visual review must name its reviewer.",
        ));
    }
    if input.verdict == Verdict::Accepted && !input.issues.is_empty() {
        return Err(AgentError::invalid(
            "accepted visual reviews cannot contain unresolved issues.",
        ));
    }
    let after = history.after(input.verdict);
    Ok(VisualReview {
        revision: revision.to_string(),
        reviewer: input.reviewer.clone(),
        verdict: input.verdict,
        issues: input.issues.clone(),
        attempt: after.attempts,
        customer_decision_required: input.verdict == Verdict::NeedsRepair
            && after.unsuccessful >= CUSTOMER_DECISION_AT,
    })
}

#[cfg(test)]
#[path = "review_record_tests.rs"]
mod tests;
