use super::*;

// Port of "a definite pre-commit cancellation leaves the same application key retryable"
// (hmi-agent-apply.test.ts): a commit certain to have changed nothing removes its receipt.
#[test]
fn a_definite_pre_commit_cancellation_leaves_the_application_key_retryable() {
    let root = TempDir::new("retryable");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("hmi/pages/new.toml", "new page")]);
    let cancelled = store
        .apply(
            &first.id,
            &first.revision,
            "retry",
            &apply_input("1"),
            |_| Ok(()),
            |_| {
                Err(CommitFailure::NoSourceChange(AgentError::new(
                    CANCELLED,
                    "draft application cancelled before commit.",
                )))
            },
        )
        .expect_err("the cancelled application");
    assert_eq!(cancelled.code, CANCELLED);
    let applied = DraftStore::open(root.path())
        .expect("reopen the store")
        .apply(
            &first.id,
            &first.revision,
            "retry",
            &apply_input("1"),
            |_| Ok(()),
            |_| Ok(json!({"applied": true})),
        )
        .expect("the retry");
    assert_eq!(applied, json!({"applied": true}));
}

// An applied draft answers only apply retries; operations that need its pages refuse with EXPIRED_DRAFT
// (specification 43 §8 repair 1), and an apply retry with its key returns the original outcome.
#[test]
fn an_applied_draft_answers_only_apply_retries() {
    let root = TempDir::new("applied-only");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "body")]);
    let outcome = apply_ok(&store, &first, "apply");
    assert_eq!(
        store
            .record_edit(
                &first.id,
                &first.revision,
                "edit",
                &json!({"edit": 1}),
                must_not_run_edit
            )
            .expect_err("an edit needs the pages")
            .code,
        EXPIRED_DRAFT
    );
    assert_eq!(
        store
            .apply(
                &first.id,
                &first.revision,
                "other",
                &apply_input("1"),
                |_| Ok(()),
                must_not_run_commit
            )
            .expect_err("a new application needs the pages")
            .code,
        EXPIRED_DRAFT
    );
    assert_eq!(
        store
            .apply(
                &first.id,
                &first.revision,
                "apply",
                &apply_input("1"),
                must_not_run_preflight,
                must_not_run_commit
            )
            .expect("the apply retry"),
        outcome
    );
}

// Discarding an applied draft keeps its application receipt, so an apply retry still returns its outcome
// (specification 43 §8 repair 1). The TypeScript store dropped the receipts here.
#[test]
fn discarding_an_applied_draft_keeps_its_application_receipt() {
    let root = TempDir::new("applied-discard");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "body")]);
    let outcome = apply_ok(&store, &first, "apply");
    store
        .discard(
            &first.id,
            Some(&DiscardExpectation {
                workspace_id: "workspace".to_string(),
                revision: first.revision.clone(),
            }),
        )
        .expect("discarding an applied draft changes nothing");
    assert_eq!(
        store
            .apply(
                &first.id,
                &first.revision,
                "apply",
                &apply_input("1"),
                must_not_run_preflight,
                must_not_run_commit
            )
            .expect("the apply retry"),
        outcome
    );
    assert_eq!(
        store.get(&first.id).expect("get").state,
        DraftState::Applied
    );
}

// Specification 43 §8 repair 1: `requireRetainedPages` refuses an applied draft and accepts any other.
#[test]
fn require_retained_pages_refuses_only_an_applied_draft() {
    let mut draft = Draft {
        state: DraftState::Reviewed,
        ..Draft::default()
    };
    assert!(require_retained_pages(&draft).is_ok());
    draft.state = DraftState::Applied;
    let refused = require_retained_pages(&draft).expect_err("an applied draft");
    assert_eq!(refused.code, EXPIRED_DRAFT);
    assert!(
        refused.message.starts_with("this proposal was applied"),
        "{}",
        refused.message
    );
}

// An apply with another revision than the draft's is a conflict and commits nothing.
#[test]
fn an_apply_at_another_revision_is_a_conflict() {
    let root = TempDir::new("apply-revision");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "body")]);
    let refused = store
        .apply(
            &first.id,
            "9",
            "apply",
            &apply_input("9"),
            |_| Ok(()),
            must_not_run_commit,
        )
        .expect_err("another revision");
    assert_eq!(refused.code, CONFLICT);
    assert_eq!(refused.message, "draft cannot be applied at this revision.");
}
