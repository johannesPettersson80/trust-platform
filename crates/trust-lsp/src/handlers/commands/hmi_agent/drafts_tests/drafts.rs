use super::*;

// Port of "draft survives restart with stable identity and isolated returned objects"
// (hmi-agent-drafts.test.ts; spec 43 §6).
#[test]
fn a_draft_survives_restart_with_stable_identity_and_isolated_copies() {
    let root = TempDir::new("restart");
    let store = DraftStore::open(root.path()).expect("open the store");
    let created = create_draft(&store, "create-1", &[("hmi/pages/a.toml", "original")]);
    assert!(created.id.len() > 10, "opaque draft id");
    let mut copy = created.clone();
    copy.files.insert(
        "hmi/pages/a.toml".to_string(),
        "external mutation".to_string(),
    );
    let restored = DraftStore::open(root.path())
        .expect("reopen the store")
        .get(&created.id)
        .expect("the draft survives");
    assert_eq!(restored.files["hmi/pages/a.toml"], "original");
    assert_eq!(restored.revision, created.revision);
}

// Port of "quota exhaustion preserves all existing drafts" (hmi-agent-drafts.test.ts; spec 43 §4:
// five simultaneous drafts).
#[test]
fn quota_exhaustion_preserves_all_existing_drafts() {
    let root = TempDir::new("quota");
    let store = DraftStore::open(root.path()).expect("open the store");
    let mut ids = Vec::new();
    for index in 0..5 {
        ids.push(create_draft(&store, &format!("create-{index}"), &[]).id);
    }
    let refused = store
        .create("workspace", "create-6", &json!({"create": 6}), || {
            Ok(proposal(BTreeMap::new(), vec![]))
        })
        .expect_err("a sixth open draft");
    assert_eq!(refused.code, LIMIT_EXCEEDED);
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 5);
    for id in &ids {
        assert_eq!(&store.get(id).expect("the draft is kept").id, id);
    }
}

// Port of "edit persists idempotent outcome and refuses key reuse with another request"
// (hmi-agent-drafts.test.ts; spec 43 §4 and §6).
#[test]
fn an_edit_persists_its_outcome_and_refuses_key_reuse_with_another_request() {
    let root = TempDir::new("idempotent");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "before")]);
    let input = json!({"file": "page", "operations": []});
    let calls = Cell::new(0_u32);
    let second = store
        .record_edit(&first.id, &first.revision, "request", &input, |_| {
            calls.set(calls.get() + 1);
            Ok(EditResult {
                files: one_page("page", "after"),
                data: None,
            })
        })
        .expect("the edit");
    assert_eq!(second.draft.files["page"], "after");
    assert_ne!(second.draft.revision, first.revision);
    let replay = DraftStore::open(root.path())
        .expect("reopen the store")
        .record_edit(
            &first.id,
            &first.revision,
            "request",
            &input,
            must_not_run_edit,
        )
        .expect("the replay");
    assert_eq!(replay, second);
    assert_eq!(calls.get(), 1);
    let reused = store
        .record_edit(
            &first.id,
            &first.revision,
            "request",
            &json!({"file": "other"}),
            must_not_run_edit,
        )
        .expect_err("a key reused with other input");
    assert_eq!(reused.code, CONFLICT);
}

// Port of "only one concurrent edit of a revision commits" (hmi-agent-drafts.test.ts).
#[test]
fn only_one_concurrent_edit_of_a_revision_commits() {
    let root = TempDir::new("concurrent");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[]);
    let outcomes = race_two_edits(&store, &store, &first);
    assert_eq!(count_ok(&outcomes), 1);
    assert_eq!(count_code(&outcomes, CONFLICT), 1);
}

// Port of "failed edit leaves prior draft and review intact; successful edit invalidates review"
// (hmi-agent-drafts.test.ts; spec 43 §6: any edit invalidates the review).
#[test]
fn a_failed_edit_leaves_the_prior_draft_and_review_intact() {
    let root = TempDir::new("failed-edit");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "before")]);
    let reviewed = store
        .review(&first.id, &first.revision, vec![], BTreeMap::new(), None)
        .expect("the review");
    assert_eq!(reviewed.state, DraftState::Reviewed);
    let refused = store
        .record_edit(
            &reviewed.id,
            &reviewed.revision,
            "bad",
            &json!({"bad": true}),
            |_| Err(AgentError::invalid("invalid operation")),
        )
        .expect_err("a failed edit");
    assert!(refused.message.contains("invalid operation"));
    assert_eq!(store.get(&first.id).expect("get"), reviewed);
    let edited = store
        .record_edit(
            &reviewed.id,
            &reviewed.revision,
            "good",
            &json!({"good": true}),
            |_| {
                Ok(EditResult {
                    files: one_page("page", "after"),
                    data: None,
                })
            },
        )
        .expect("the edit")
        .draft;
    assert_eq!(edited.state, DraftState::Draft);
    assert!(edited.findings.is_empty());
}

// Port of "discard removes only the draft, and an unknown id cannot escape storage"
// (hmi-agent-drafts.test.ts).
#[test]
fn a_discard_removes_only_the_draft_and_an_unknown_id_cannot_escape_storage() {
    let root = TempDir::new("discard");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[]);
    store.discard(&first.id, None).expect("discard");
    assert_eq!(
        store.get(&first.id).expect_err("discarded").code,
        EXPIRED_DRAFT
    );
    assert_eq!(
        store.get("../outside").expect_err("traversal").code,
        INVALID_ARGUMENT
    );
}

// Port of "separate store instances cannot both edit the same persisted revision"
// (hmi-agent-drafts.test.ts).
#[test]
fn separate_store_instances_cannot_both_edit_the_same_revision() {
    let root = TempDir::new("two-instances");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[]);
    let other = DraftStore::open(root.path()).expect("a second store");
    let outcomes = race_two_edits(&store, &other, &first);
    assert_eq!(count_ok(&outcomes), 1);
}

// Port of "application receipt survives restart and prevents repeated source application"
// (hmi-agent-drafts.test.ts; spec 43 §6).
#[test]
fn an_application_receipt_survives_restart_and_prevents_repeated_source_application() {
    let root = TempDir::new("apply-receipt");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[]);
    let commits = Cell::new(0_u32);
    let commit = |_: &Draft| -> Result<Value, CommitFailure> {
        commits.set(commits.get() + 1);
        Ok(json!({"applied": true, "sequence": commits.get()}))
    };
    let outcome = store
        .apply(
            &first.id,
            &first.revision,
            "apply-1",
            &apply_input("1"),
            |_| Ok(()),
            commit,
        )
        .expect("apply");
    let replay = DraftStore::open(root.path())
        .expect("reopen the store")
        .apply(
            &first.id,
            &first.revision,
            "apply-1",
            &apply_input("1"),
            |_| Ok(()),
            commit,
        )
        .expect("the replay");
    assert_eq!(replay, outcome);
    assert_eq!(commits.get(), 1);
    assert_eq!(
        store.get(&first.id).expect("get").state,
        DraftState::Applied
    );
    let reused = store
        .apply(
            &first.id,
            &first.revision,
            "apply-1",
            &json!({"revision": "1", "save": true, "allowIncomplete": false}),
            |_| Ok(()),
            commit,
        )
        .expect_err("a key reused with other input");
    assert_eq!(reused.code, CONFLICT);
    assert_eq!(commits.get(), 1, "the reused key does not commit again");
}

// Port of "creation replay survives restart without rebuilding or reviving a discarded proposal"
// (hmi-agent-drafts.test.ts; spec 43 §6).
#[test]
fn a_creation_replay_survives_restart_without_rebuilding_or_reviving_a_discarded_proposal() {
    let root = TempDir::new("create-replay");
    let store = DraftStore::open(root.path()).expect("open the store");
    let builds = Cell::new(0_u32);
    let build = || -> AgentResult<PreparedProposal> {
        builds.set(builds.get() + 1);
        Ok(PreparedProposal {
            base: vec![],
            files: one_page("page", "proposal"),
            program_revision: None,
            intent: None,
            data: Some(json!({"model": "original"})),
        })
    };
    let input = json!({"request": "same"});
    let first = store
        .create("workspace", "create-1", &input, build)
        .expect("create");
    let replay = DraftStore::open(root.path())
        .expect("reopen the store")
        .create("workspace", "create-1", &input, build)
        .expect("the replay");
    assert_eq!(replay, first);
    assert_eq!(builds.get(), 1);
    let reused = store
        .create(
            "workspace",
            "create-1",
            &json!({"request": "different"}),
            build,
        )
        .expect_err("a key reused with other input");
    assert_eq!(reused.code, CONFLICT);
    store.discard(&first.draft.id, None).expect("discard");
    let expired = DraftStore::open(root.path())
        .expect("reopen the store")
        .create("workspace", "create-1", &input, build)
        .expect_err("a discarded proposal");
    assert_eq!(expired.code, EXPIRED_DRAFT);
    assert_eq!(
        builds.get(),
        1,
        "discarded proposals cannot be recreated by replaying old requests"
    );
}

// A creation key that belongs to another workspace is refused (`createOnce` in `drafts.ts`).
#[test]
fn a_creation_key_of_another_workspace_is_refused() {
    let root = TempDir::new("other-workspace");
    let store = DraftStore::open(root.path()).expect("open the store");
    let input = json!({"same": true});
    store
        .create("workspace", "shared-key", &input, || {
            Ok(proposal(one_page("page", "mine"), vec![]))
        })
        .expect("create");
    let refused = store
        .create("other-workspace", "shared-key", &input, || {
            Ok(proposal(one_page("page", "theirs"), vec![]))
        })
        .expect_err("another workspace's creation");
    assert_eq!(refused.code, PROJECT_UNAVAILABLE);
    assert_eq!(refused.message, "creation belongs to another workspace.");
}

// Port of "an applied draft cannot be moved back into reviewed state" (hmi-agent-drafts.test.ts; spec 43
// §6). The review of an applied draft is a conflict at the store; the service refuses its pages first.
#[test]
fn an_applied_draft_cannot_be_moved_back_into_reviewed_state() {
    let root = TempDir::new("applied-review");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[]);
    apply_ok(&store, &first, "apply");
    assert_eq!(
        store
            .review(&first.id, &first.revision, vec![], BTreeMap::new(), None)
            .expect_err("a review of an applied draft")
            .code,
        CONFLICT
    );
    assert_eq!(
        store.get(&first.id).expect("get").state,
        DraftState::Applied
    );
}

// Port of "discard checks the expected revision inside its mutation lock" (hmi-agent-drafts.test.ts).
#[test]
fn a_discard_checks_the_expected_revision_inside_the_lock() {
    let root = TempDir::new("discard-revision");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "before")]);
    let edited = store
        .record_edit(
            &first.id,
            &first.revision,
            "edit",
            &json!({"edit": 1}),
            |_| {
                Ok(EditResult {
                    files: one_page("page", "after"),
                    data: None,
                })
            },
        )
        .expect("the edit")
        .draft;
    let stale = DiscardExpectation {
        workspace_id: "workspace".to_string(),
        revision: first.revision.clone(),
    };
    assert_eq!(
        store
            .discard(&first.id, Some(&stale))
            .expect_err("a stale revision")
            .code,
        CONFLICT
    );
    assert_eq!(store.get(&first.id).expect("get").files["page"], "after");
    let current = DiscardExpectation {
        workspace_id: "workspace".to_string(),
        revision: edited.revision.clone(),
    };
    store.discard(&first.id, Some(&current)).expect("discard");
    store
        .discard(&first.id, Some(&current))
        .expect("a repeated discard");
    assert_eq!(
        store.get(&first.id).expect_err("discarded").code,
        EXPIRED_DRAFT
    );
}

// Port of "restart discovery lists only recoverable proposals in the selected workspace"
// (hmi-agent-drafts.test.ts; spec 43 §6: the listing excludes finished proposals and page bodies).
#[test]
fn restart_discovery_lists_only_recoverable_proposals_in_the_selected_workspace() {
    let root = TempDir::new("discovery");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(
        &store,
        "create-first",
        &[("hmi/pages/keep.toml", "private proposal body")],
    );
    let gone = create_draft(&store, "create-gone", &[("page", "discard me")]);
    let applied = create_draft(&store, "create-applied", &[("page", "applied")]);
    store
        .create(
            "other-workspace",
            "create-other",
            &json!({"other": true}),
            || Ok(proposal(one_page("page", "other"), vec![])),
        )
        .expect("another workspace's draft");
    store.discard(&gone.id, None).expect("discard");
    apply_ok(&store, &applied, "apply");
    let proposals = DraftStore::open(root.path())
        .expect("reopen the store")
        .list("workspace")
        .expect("list");
    assert_eq!(
        proposals,
        vec![DraftSummary {
            draft_id: first.id.clone(),
            revision: first.revision.clone(),
            state: DraftState::Draft,
            files: vec!["hmi/pages/keep.toml".to_string()],
        }]
    );
    let listed = serde_json::to_string(&proposals).expect("JSON");
    assert!(!listed.contains("private proposal body"));
}

// Port of "file hashes survive restart, change with edits and detect damaged proposal contents"
// (hmi-agent-drafts.test.ts; spec 43 §6: reads reject a mismatched record).
#[test]
fn file_hashes_survive_restart_change_with_edits_and_detect_damage() {
    let root = TempDir::new("hashes");
    let store = DraftStore::open(root.path()).expect("open the store");
    let file = "hmi/pages/hashed.toml";
    let first = create_draft(&store, "create-1", &[(file, "before")]);
    assert_eq!(
        first.content_hashes,
        Some(BTreeMap::from([(
            file.to_string(),
            "6db7d803e74f1ffa7d8f5adc0bf95b3e15bf4c8373fffadf546227cc6c6742cb".to_string()
        )])),
        "drafts need verifiable content hashes"
    );
    let edited = store
        .record_edit(
            &first.id,
            &first.revision,
            "edit",
            &json!({"edit": "after"}),
            |_| {
                Ok(EditResult {
                    files: one_page(file, "after"),
                    data: None,
                })
            },
        )
        .expect("the edit")
        .draft;
    assert_eq!(
        edited.content_hashes,
        Some(BTreeMap::from([(
            file.to_string(),
            "f39592393ef0859cb196a52693d2cea00fb2df784b3c04ae54aa7cadb8e562f8".to_string()
        )]))
    );
    let restarted = DraftStore::open(root.path())
        .expect("reopen the store")
        .get(&first.id)
        .expect("get");
    assert_eq!(restarted.content_hashes, edited.content_hashes);
    let path = record_file(root.path(), &first.id);
    let mut record = read_json(&path);
    record["draft"]["files"][file] = json!("damaged body");
    fs::write(&path, serde_json::to_vec(&record).expect("JSON")).expect("damage the record");
    let damaged = DraftStore::open(root.path())
        .expect("reopen the store")
        .get(&first.id)
        .expect_err("a damaged proposal");
    assert_eq!(damaged.code, INVALID_ARGUMENT);
    assert!(damaged.message.contains("corrupt"), "{}", damaged.message);
}

// Port of "truncated storage reports a typed corrupt-record error" (hmi-agent-drafts.test.ts).
#[test]
fn truncated_storage_reports_a_typed_corrupt_record_error() {
    let root = TempDir::new("truncated");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[]);
    fs::write(record_file(root.path(), &first.id), r#"{"format":1,"#).expect("truncate");
    let error = store.get(&first.id).expect_err("a truncated record");
    assert_eq!(error.code, INVALID_ARGUMENT);
    assert!(error.message.contains("corrupt"), "{}", error.message);
}

// Port of "read-only apply preflight does not consume a receipt and sees the locked current review"
// (hmi-agent-drafts.test.ts; spec 43 §6).
#[test]
fn a_refused_preflight_consumes_no_receipt_and_sees_the_locked_review() {
    let root = TempDir::new("preflight");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "body")]);
    store
        .review(
            &first.id,
            &first.revision,
            vec![],
            one_page("asset", "current"),
            None,
        )
        .expect("the review");
    let blocked = Cell::new(true);
    let commits = Cell::new(0_u32);
    let attempt = |owner: &DraftStore| {
        owner.apply(
            &first.id,
            &first.revision,
            "apply",
            &json!({"same": true}),
            |draft| {
                assert_eq!(draft.reviewed_assets, Some(one_page("asset", "current")));
                if blocked.get() {
                    return Err(AgentError::conflict("review needs correction"));
                }
                Ok(())
            },
            |_| {
                commits.set(commits.get() + 1);
                Ok(json!({"applied": true, "count": commits.get()}))
            },
        )
    };
    let refused = attempt(&store).expect_err("the preflight refusal");
    assert_eq!(refused.code, CONFLICT);
    assert_eq!(refused.message, "review needs correction");
    assert_eq!(commits.get(), 0);
    blocked.set(false);
    assert_eq!(
        attempt(&store).expect("the apply"),
        json!({"applied": true, "count": 1})
    );
    assert_eq!(
        attempt(&store).expect("the retry"),
        json!({"applied": true, "count": 1})
    );
    assert_eq!(commits.get(), 1);
}

// Port of "uncertain source application cannot be bypassed with another key or draft edit"
// (hmi-agent-drafts.test.ts; spec 43 §6).
#[test]
fn an_uncertain_source_application_cannot_be_bypassed() {
    let root = TempDir::new("uncertain");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "body")]);
    let failed = store
        .apply(
            &first.id,
            &first.revision,
            "first",
            &apply_input("1"),
            |_| Ok(()),
            |_| Err(CommitFailure::Uncertain("editor disconnected".to_string())),
        )
        .expect_err("an uncertain commit");
    assert_eq!(failed.code, CONFLICT);
    assert_eq!(
        failed.message,
        "source application outcome needs inspection: editor disconnected"
    );
    let called = Cell::new(false);
    let bypass = DraftStore::open(root.path())
        .expect("reopen the store")
        .apply(
            &first.id,
            &first.revision,
            "second",
            &apply_input("1"),
            |_| Ok(()),
            |_| {
                called.set(true);
                Ok(json!({"applied": true}))
            },
        )
        .expect_err("another key");
    assert_eq!(bypass.code, CONFLICT);
    assert!(bypass.message.contains("inspection"));
    assert!(!called.get(), "the source is not written again");
    let edit = store
        .record_edit(
            &first.id,
            &first.revision,
            "edit",
            &json!({"edit": 1}),
            |_| {
                Ok(EditResult {
                    files: one_page("page", "other"),
                    data: None,
                })
            },
        )
        .expect_err("an edit");
    assert_eq!(edit.code, CONFLICT);
    assert!(edit.message.contains("inspection"));
    let review = store
        .review(&first.id, &first.revision, vec![], BTreeMap::new(), None)
        .expect_err("a review");
    assert_eq!(review.code, CONFLICT);
    assert!(review.message.contains("inspection"));
}

// Port of "cancellation during proposal construction prevents its private commit"
// (hmi-agent-drafts.test.ts): a construction that is cancelled stores nothing.
#[test]
fn cancellation_during_proposal_construction_prevents_its_private_commit() {
    let root = TempDir::new("cancel-create");
    let store = DraftStore::open(root.path()).expect("open the store");
    let refused = store
        .create(
            "workspace",
            "cancel-create",
            &json!({"cancel": true}),
            || {
                Err(AgentError::new(
                    CANCELLED,
                    "proposal construction cancelled.",
                ))
            },
        )
        .expect_err("a cancelled construction");
    assert_eq!(refused.code, CANCELLED);
    assert!(store.list("workspace").expect("list").is_empty());
}

// Port of "cancellation during a draft edit preserves its previous revision"
// (hmi-agent-drafts.test.ts).
#[test]
fn cancellation_during_a_draft_edit_preserves_its_previous_revision() {
    let root = TempDir::new("cancel-edit");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "before")]);
    let refused = store
        .record_edit(
            &first.id,
            &first.revision,
            "cancel-edit",
            &json!({"cancel": true}),
            |_| Err(AgentError::new(CANCELLED, "edit cancelled before commit.")),
        )
        .expect_err("a cancelled edit");
    assert_eq!(refused.code, CANCELLED);
    assert_eq!(store.get(&first.id).expect("get"), first);
}

// Port of "edit detail receipts survive later edits and restart without recomputing metadata"
// (hmi-agent-drafts.test.ts; spec 43 §8 repair 1: the receipt keeps its data, revision and hashes).
#[test]
fn edit_detail_receipts_survive_later_edits_and_restart() {
    let root = TempDir::new("edit-detail");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "before")]);
    let calls = Cell::new(0_u32);
    let update = |_: &Draft| -> AgentResult<EditResult> {
        calls.set(calls.get() + 1);
        Ok(EditResult {
            files: one_page("page", "after"),
            data: Some(json!({"changes": [{"id": "unit", "properties": ["at"]}]})),
        })
    };
    let first_edit = store
        .record_edit(
            &first.id,
            &first.revision,
            "edit-detail",
            &json!({"edit": "detail"}),
            update,
        )
        .expect("the detailed edit");
    store
        .record_edit(
            &first.id,
            &first_edit.draft.revision,
            "later",
            &json!({"later": true}),
            |_| {
                Ok(EditResult {
                    files: one_page("page", "later"),
                    data: None,
                })
            },
        )
        .expect("a later edit");
    let replay = DraftStore::open(root.path())
        .expect("reopen the store")
        .record_edit(
            &first.id,
            &first.revision,
            "edit-detail",
            &json!({"edit": "detail"}),
            update,
        )
        .expect("the replay");
    assert_eq!(replay.data, first_edit.data);
    assert_eq!(calls.get(), 1);
    assert_eq!(replay.draft.revision, first_edit.draft.revision);
    assert_eq!(replay.draft.content_hashes, first_edit.draft.content_hashes);
    assert!(replay.draft.superseded, "a superseded revision is marked");
    assert!(
        replay.draft.files.is_empty(),
        "its page bodies are not kept"
    );
    assert_eq!(store.get(&first.id).expect("get").files["page"], "later");
}

// Repair history: an edit clears the verdict and keeps the count of reviews (specification 43 §10.1).
#[test]
fn an_edit_clears_the_visual_verdict_and_keeps_the_repair_history() {
    let root = TempDir::new("verdict");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "body")]);
    let needs = VisualReviewInput {
        reviewer: "Ana".to_string(),
        verdict: Verdict::NeedsRepair,
        issues: vec![issue()],
    };
    let reviewed = store
        .review(
            &first.id,
            &first.revision,
            vec![],
            BTreeMap::new(),
            Some(&needs),
        )
        .expect("the review");
    assert_eq!(reviewed.repair_attempts, Some(1));
    assert_eq!(reviewed.unsuccessful_reviews, 1);
    assert_eq!(
        reviewed.visual_review.as_ref().map(|review| review.attempt),
        Some(1)
    );
    let edited = store
        .record_edit(
            &first.id,
            &reviewed.revision,
            "edit",
            &json!({"edit": 1}),
            |_| {
                Ok(EditResult {
                    files: one_page("page", "after"),
                    data: None,
                })
            },
        )
        .expect("the edit")
        .draft;
    assert!(edited.visual_review.is_none(), "the verdict is invalidated");
    assert_eq!(
        edited.repair_attempts,
        Some(1),
        "the repair history survives"
    );
    assert_eq!(edited.unsuccessful_reviews, 1);
    assert_eq!(edited.state, DraftState::Draft);
    let again = store
        .review(
            &first.id,
            &edited.revision,
            vec![],
            BTreeMap::new(),
            Some(&needs),
        )
        .expect("the second review");
    assert_eq!(again.repair_attempts, Some(2));
    assert_eq!(again.unsuccessful_reviews, 2);
    assert_eq!(
        again
            .visual_review
            .as_ref()
            .map(|review| review.customer_decision_required),
        Some(false)
    );
}

// Specification 43 §10.1 through the store: the third unsuccessful review requests the customer's decision.
#[test]
fn the_third_unsuccessful_review_requests_the_customer_decision_through_the_store() {
    let root = TempDir::new("third-review");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "body")]);
    let needs = VisualReviewInput {
        reviewer: "Ana".to_string(),
        verdict: Verdict::NeedsRepair,
        issues: vec![issue()],
    };
    let decisions: Vec<bool> = (0..3)
        .map(|_| {
            store
                .review(
                    &first.id,
                    &first.revision,
                    vec![],
                    BTreeMap::new(),
                    Some(&needs),
                )
                .expect("a review")
                .visual_review
                .expect("a visual review")
                .customer_decision_required
        })
        .collect();
    assert_eq!(decisions, [false, false, true]);
}

// A review without a visual review keeps the stored one (specification 43 §10.1).
#[test]
fn a_review_without_a_visual_review_keeps_the_stored_one() {
    let root = TempDir::new("keep-visual");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "body")]);
    let accepted = VisualReviewInput {
        reviewer: "Ana".to_string(),
        verdict: Verdict::Accepted,
        issues: vec![],
    };
    store
        .review(
            &first.id,
            &first.revision,
            vec![],
            BTreeMap::new(),
            Some(&accepted),
        )
        .expect("the review");
    let kept = store
        .review(&first.id, &first.revision, vec![], BTreeMap::new(), None)
        .expect("a review without a visual review");
    assert_eq!(
        kept.visual_review.map(|review| review.verdict),
        Some(Verdict::Accepted)
    );
    assert_eq!(kept.repair_attempts, Some(1));
}

// Discarded drafts refuse edits and reviews with EXPIRED_DRAFT (`active` in `drafts.ts`).
#[test]
fn a_discarded_draft_refuses_edits_and_reviews() {
    let root = TempDir::new("discarded-refusals");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[("page", "body")]);
    store.discard(&first.id, None).expect("discard");
    assert_eq!(
        store
            .record_edit(
                &first.id,
                &first.revision,
                "edit",
                &json!({"edit": 1}),
                must_not_run_edit
            )
            .expect_err("an edit of a discarded draft")
            .code,
        EXPIRED_DRAFT
    );
    assert_eq!(
        store
            .review(&first.id, &first.revision, vec![], BTreeMap::new(), None)
            .expect_err("a review of a discarded draft")
            .code,
        EXPIRED_DRAFT
    );
}

// Creation refuses more than 32 files, and a page over 1 MiB; 32 files and a page of exactly 1 MiB are
// accepted (specification 43 §4: the boundaries of a proposal).
#[test]
fn a_proposal_is_limited_to_32_files_and_1_mebibyte_per_file() {
    let root = TempDir::new("proposal-size");
    let store = DraftStore::open(root.path()).expect("open the store");
    let many: BTreeMap<String, String> = (0..33)
        .map(|index| (format!("hmi/pages/p{index}.toml"), "x".to_string()))
        .collect();
    let refused = store
        .create("workspace", "create-many", &json!({"many": 33}), || {
            Ok(proposal(many.clone(), vec![]))
        })
        .expect_err("33 files");
    assert_eq!(refused.code, LIMIT_EXCEEDED);
    assert_eq!(
        refused.message,
        "at most 32 files, each no larger than 1 MiB."
    );
    let exact: BTreeMap<String, String> = (0..32)
        .map(|index| (format!("hmi/pages/p{index}.toml"), "x".to_string()))
        .collect();
    store
        .create(
            "workspace",
            "create-32",
            &json!({"thirty_two": true}),
            || Ok(proposal(exact.clone(), vec![])),
        )
        .expect("32 files");
    let large = one_page("hmi/pages/large.toml", &"a".repeat(1_048_577));
    let too_large = store
        .create("workspace", "create-large", &json!({"large": true}), || {
            Ok(proposal(large.clone(), vec![]))
        })
        .expect_err("1 MiB plus one byte");
    assert_eq!(too_large.code, LIMIT_EXCEEDED);
    let limit = one_page("hmi/pages/limit.toml", &"a".repeat(1_048_576));
    store
        .create("workspace", "create-limit", &json!({"limit": true}), || {
            Ok(proposal(limit.clone(), vec![]))
        })
        .expect("exactly 1 MiB");
}

// Request keys are one to 200 UTF-16 units long; a key of 200 units is accepted and 201 is refused
// (specification 43 §6 and `createOnce` in `drafts.ts`).
#[test]
fn a_request_key_is_limited_to_200_utf16_units() {
    let root = TempDir::new("key-length");
    let store = DraftStore::open(root.path()).expect("open the store");
    let at_limit = "😀".repeat(100);
    create_draft(&store, &at_limit, &[]);
    let over = "😀".repeat(101);
    let refused = store
        .create("workspace", &over, &json!({"over": true}), || {
            Ok(proposal(BTreeMap::new(), vec![]))
        })
        .expect_err("202 UTF-16 units");
    assert_eq!(refused.code, INVALID_ARGUMENT);
    let empty = store
        .create("workspace", "", &json!({"empty": true}), || {
            Ok(proposal(BTreeMap::new(), vec![]))
        })
        .expect_err("an empty key");
    assert_eq!(empty.code, INVALID_ARGUMENT);
    assert_eq!(empty.message, "creation key and fingerprint are required.");
}

// The mutation lock waits for another owner for five seconds, then refuses without changing anything
// (`deadline` in `drafts.ts`).
#[test]
fn a_mutation_refuses_when_another_owner_keeps_the_lock() {
    let root = TempDir::new("busy");
    let store = DraftStore::open(root.path()).expect("open the store");
    let first = create_draft(&store, "create-1", &[]);
    let owner = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.path().join(LOCK_FILE))
        .expect("the lock file");
    // A blocking take: a process another test spawns can hold a duplicate of the store's released descriptor for a moment
    // between its fork and its exec.
    owner.lock().expect("the other owner takes the lock");
    let started = Instant::now();
    let refused = store.discard(&first.id, None).expect_err("a busy lock");
    assert_eq!(refused.code, CONFLICT);
    assert_eq!(
        refused.message,
        "draft storage is busy or its owner needs recovery."
    );
    assert!(started.elapsed() >= LOCK_WAIT - Duration::from_millis(100));
    drop(owner);
    store
        .discard(&first.id, None)
        .expect("the lock is free again");
}

// Discovery and creation do not fail on a missing storage folder: the store creates it when it opens.
#[test]
fn the_store_creates_its_storage_folder_when_it_opens() {
    let root = TempDir::new("folder");
    let storage = root.path().join("private").join("drafts");
    let store = DraftStore::open(&storage).expect("open the store");
    assert!(storage.is_dir());
    assert!(store.list("workspace").expect("list").is_empty());
}
