//! The apply input of `hmi_apply_draft` (AT-09) and the visual review it may carry.

use serde::Deserialize;

use super::super::error::{AgentError, AgentResult};
use super::super::review_record::{ReviewIssue, Verdict, VisualReviewInput};
use super::{text_items, text_length, unique_items, Bounded, MAX_ISSUES, MAX_ISSUE_OBJECTS};

/// The input of `hmi_apply_draft` (AT-09): review, apply or discard.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(in super::super) struct ApplyInput {
    /// The draft.
    pub(in super::super) draft_id: String,
    /// The revision the action names.
    pub(in super::super) revision: String,
    /// What to do.
    pub(in super::super) action: ApplyAction,
    /// Whether applying also saves the files.
    pub(in super::super) save: Option<bool>,
    /// The visual review, recorded with a review.
    pub(in super::super) visual_review: Option<VisualReviewRequest>,
    /// Whether an incomplete design may be saved, labelled incomplete.
    pub(in super::super) allow_incomplete: Option<bool>,
    /// Whether the review includes an interactive navigation snapshot. The extension builds that snapshot.
    #[allow(dead_code)] // Read by the extension's exploration, not by the record.
    pub(in super::super) explore: Option<bool>,
}

/// The action of an apply request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(in super::super) enum ApplyAction {
    /// Review the draft.
    Review,
    /// Apply the reviewed draft.
    Apply,
    /// Discard the draft.
    Discard,
}

/// A visual review as the reviewer submits it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(in super::super) struct VisualReviewRequest {
    /// Who reviewed, 1 to 200 characters.
    pub(in super::super) reviewer: String,
    /// The verdict.
    pub(in super::super) verdict: Verdict,
    /// The issues that remain, at most 100.
    pub(in super::super) issues: Vec<IssueRequest>,
}

/// One issue of a visual review.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(in super::super) struct IssueRequest {
    /// The page the issue concerns.
    pub(in super::super) file: String,
    /// The objects it concerns, at most 500 unique IDs.
    pub(in super::super) objects: Vec<String>,
    /// What the reviewer saw, 1 to 2000 characters.
    pub(in super::super) message: String,
}

impl Bounded for ApplyInput {
    fn bounds(&self) -> AgentResult<()> {
        text_length("input.draftId", &self.draft_id, 1, usize::MAX)?;
        text_length("input.revision", &self.revision, 1, usize::MAX)?;
        if let Some(review) = &self.visual_review {
            text_length("input.visualReview.reviewer", &review.reviewer, 1, 200)?;
            if review.issues.len() > MAX_ISSUES {
                return Err(AgentError::limit(format!(
                    "input.visualReview.issues may hold at most {MAX_ISSUES} issues."
                )));
            }
            for (index, issue) in review.issues.iter().enumerate() {
                let field = format!("input.visualReview.issues[{index}]");
                text_length(&format!("{field}.file"), &issue.file, 0, 200)?;
                text_length(&format!("{field}.message"), &issue.message, 1, 2000)?;
                if issue.objects.len() > MAX_ISSUE_OBJECTS {
                    return Err(AgentError::limit(format!(
                        "{field}.objects may hold at most {MAX_ISSUE_OBJECTS} IDs."
                    )));
                }
                unique_items(&format!("{field}.objects"), &issue.objects)?;
                text_items(&format!("{field}.objects"), &issue.objects)?;
            }
        }
        Ok(())
    }
}

impl ApplyInput {
    /// The visual review in the form the review record takes (`reviewRecord`).
    #[must_use]
    pub(in super::super) fn review_input(&self) -> Option<VisualReviewInput> {
        self.visual_review.as_ref().map(|review| VisualReviewInput {
            reviewer: review.reviewer.clone(),
            verdict: review.verdict,
            issues: review
                .issues
                .iter()
                .map(|issue| ReviewIssue {
                    file: issue.file.clone(),
                    objects: issue.objects.clone(),
                    message: issue.message.clone(),
                })
                .collect(),
        })
    }
}
