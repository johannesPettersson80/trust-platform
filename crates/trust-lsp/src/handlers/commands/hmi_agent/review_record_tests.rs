//! Tests of the visual review record (specification 43, §10.1). No TypeScript test drives `reviewRecord`
//! directly, so these pin its rules: an accepted review holds no unresolved issue, the third
//! unsuccessful review asks for the customer's decision, and the stored shape keeps the TypeScript names.

use serde_json::json;

use super::super::error::INVALID_ARGUMENT;
use super::*;

fn input(reviewer: &str, verdict: Verdict, issues: Vec<ReviewIssue>) -> VisualReviewInput {
    VisualReviewInput {
        reviewer: reviewer.to_string(),
        verdict,
        issues,
    }
}

fn issue() -> ReviewIssue {
    ReviewIssue {
        file: "hmi/pages/overview.toml".to_string(),
        objects: vec!["tank".to_string()],
        message: "label clipped".to_string(),
    }
}

fn history(attempts: u32, unsuccessful: u32) -> ReviewHistory {
    ReviewHistory {
        attempts,
        unsuccessful,
    }
}

// An accepted review without issues is recorded for its revision as the first review.
#[test]
fn an_accepted_review_without_issues_is_recorded_for_its_revision() {
    let record = review_record(
        &input("Ana", Verdict::Accepted, vec![]),
        "3",
        ReviewHistory::default(),
    )
    .expect("an accepted review");
    assert_eq!(
        record,
        VisualReview {
            revision: "3".to_string(),
            reviewer: "Ana".to_string(),
            verdict: Verdict::Accepted,
            issues: vec![],
            attempt: 1,
            customer_decision_required: false,
        }
    );
}

// An accepted review may not keep an unresolved issue (specification 43, §10.1).
#[test]
fn an_accepted_review_with_an_unresolved_issue_is_refused() {
    let error = review_record(
        &input("Ana", Verdict::Accepted, vec![issue()]),
        "3",
        ReviewHistory::default(),
    )
    .expect_err("an accepted review with an issue");
    assert_eq!(error.code, INVALID_ARGUMENT);
    assert_eq!(
        error.message,
        "accepted visual reviews cannot contain unresolved issues."
    );
}

// The reviewer is required. The refusal keeps the TypeScript text, which names the issue rule and not
// the reviewer; the wording is the one recorded in `visualReview.ts`.
#[test]
fn a_blank_reviewer_is_refused() {
    for reviewer in ["", "   "] {
        let error = review_record(
            &input(reviewer, Verdict::NeedsRepair, vec![issue()]),
            "3",
            ReviewHistory::default(),
        )
        .expect_err("a blank reviewer");
        assert_eq!(error.code, INVALID_ARGUMENT);
        assert_eq!(error.message, "the visual review must name its reviewer.");
    }
}

// A review that needs repair keeps its issues. The first and second such reviews ask nothing of the
// customer.
#[test]
fn an_unsuccessful_review_before_the_third_asks_nothing_of_the_customer() {
    let first = review_record(
        &input("Ana", Verdict::NeedsRepair, vec![issue()]),
        "1",
        history(0, 0),
    )
    .expect("a review");
    assert_eq!(first.attempt, 1);
    assert!(!first.customer_decision_required);
    assert_eq!(first.issues, vec![issue()]);
    let second = review_record(
        &input("Ana", Verdict::NeedsRepair, vec![issue()]),
        "2",
        history(1, 1),
    )
    .expect("a review");
    assert_eq!(second.attempt, 2);
    assert!(!second.customer_decision_required);
}

// The third unsuccessful review requests the customer's decision: the boundary is exactly three.
#[test]
fn the_third_unsuccessful_review_requests_the_customer_decision() {
    let third = review_record(
        &input("Ana", Verdict::NeedsRepair, vec![issue()]),
        "3",
        history(2, 2),
    )
    .expect("a review");
    assert_eq!(third.attempt, 3);
    assert!(third.customer_decision_required);
    let fourth = review_record(
        &input("Ana", Verdict::NeedsRepair, vec![issue()]),
        "4",
        history(3, 3),
    )
    .expect("a review");
    assert_eq!(fourth.attempt, 4);
    assert!(
        fourth.customer_decision_required,
        "the decision stays requested"
    );
}

// An accepted review never requests the customer's decision, however many reviews came before it.
#[test]
fn an_accepted_review_never_requests_the_customer_decision() {
    let record = review_record(&input("Ana", Verdict::Accepted, vec![]), "5", history(3, 2))
        .expect("an accepted review");
    assert_eq!(record.attempt, 4);
    assert!(!record.customer_decision_required);
}

// Specification 43 §10.1 counts unsuccessful reviews: an accepted review that came first is not one of
// them, so the customer's decision waits for the third unsuccessful review, not for the third review.
#[test]
fn an_earlier_accepted_review_is_not_counted_as_unsuccessful() {
    let first = review_record(
        &input("Ana", Verdict::NeedsRepair, vec![issue()]),
        "2",
        history(1, 0),
    )
    .expect("a review");
    assert_eq!(first.attempt, 2);
    assert!(!first.customer_decision_required);
    let second = review_record(
        &input("Ana", Verdict::NeedsRepair, vec![issue()]),
        "3",
        history(2, 1),
    )
    .expect("a review");
    assert_eq!(second.attempt, 3);
    assert!(
        !second.customer_decision_required,
        "only the second unsuccessful review"
    );
    let third = review_record(
        &input("Ana", Verdict::NeedsRepair, vec![issue()]),
        "4",
        history(3, 2),
    )
    .expect("a review");
    assert_eq!(third.attempt, 4);
    assert!(third.customer_decision_required);
}

// The history counts each review and each unsuccessful one, and saturates instead of overflowing.
#[test]
fn review_history_counts_reviews_and_unsuccessful_reviews() {
    assert_eq!(
        ReviewHistory::default().after(Verdict::NeedsRepair),
        history(1, 1)
    );
    assert_eq!(
        ReviewHistory::default().after(Verdict::Accepted),
        history(1, 0)
    );
    assert_eq!(
        history(u32::MAX, u32::MAX).after(Verdict::NeedsRepair),
        history(u32::MAX, u32::MAX)
    );
}

// The stored record and the submitted review use the TypeScript field names and verdict spellings.
#[test]
fn the_review_record_keeps_the_stored_names_and_verdict_spellings() {
    let record = review_record(
        &input("Ana", Verdict::NeedsRepair, vec![issue()]),
        "1",
        ReviewHistory::default(),
    )
    .expect("a review");
    let value = serde_json::to_value(&record).expect("json");
    assert_eq!(value["verdict"], json!("needs-repair"));
    assert_eq!(value["customerDecisionRequired"], json!(false));
    assert_eq!(value["attempt"], json!(1));
    assert_eq!(value["issues"][0]["objects"], json!(["tank"]));
}

// A submitted review decodes from the wire names; a field that is not part of the review is refused.
#[test]
fn a_submitted_review_decodes_from_wire_names_and_refuses_unknown_fields() {
    let decoded: VisualReviewInput = serde_json::from_value(json!({
        "reviewer": "Ana",
        "verdict": "needs-repair",
        "issues": [{"file": "hmi/pages/overview.toml", "objects": ["tank"], "message": "label clipped"}]
    }))
    .expect("a review");
    assert_eq!(decoded.verdict, Verdict::NeedsRepair);
    assert_eq!(decoded.issues, vec![issue()]);
    let unknown = serde_json::from_value::<VisualReviewInput>(json!({
        "reviewer": "Ana", "verdict": "accepted", "issues": [], "score": 9
    }));
    assert!(unknown.is_err(), "a beauty score is not part of a review");
}
