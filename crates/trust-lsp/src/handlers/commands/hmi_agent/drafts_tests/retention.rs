use super::*;

// Port of "a hundred edits keep one copy of the pages and a bounded receipt list"
// (hmi-agent-draft-retention.test.ts; spec 43 §8 repair 1: receipts never copy pages).
#[test]
fn a_hundred_edits_keep_one_copy_of_the_pages_and_a_bounded_receipt_list() {
    let root = TempDir::new("retention-edits");
    let store = DraftStore::open(root.path()).expect("open the store");
    let file = "hmi/pages/edited.toml";
    let page = |edit: u32| -> String {
        let mut text = format!("edit {edit}\n");
        while text.len() < 20 * 1024 {
            text.push('#');
        }
        text
    };
    let base: Vec<DocumentSnapshot> = (0..10)
        .map(|index| {
            snapshot(
                &format!("hmi/pages/base-{index}.toml"),
                &"b".repeat(20 * 1024),
            )
        })
        .collect();
    let created = store
        .create(
            "workspace",
            "create-edits",
            &json!({"create": "edits"}),
            || {
                Ok(PreparedProposal {
                    base: base.clone(),
                    files: one_page(file, &page(0)),
                    program_revision: None,
                    intent: None,
                    data: None,
                })
            },
        )
        .expect("create")
        .draft;
    let created_size = fs::metadata(record_file(root.path(), &created.id))
        .expect("the record")
        .len();
    let mut draft = created;
    for edit in 1..=100_u32 {
        let text = page(edit);
        draft = store
            .record_edit(
                &draft.id,
                &draft.revision,
                &format!("edit-{edit}"),
                &json!({"edit": edit}),
                |_| {
                    Ok(EditResult {
                        files: one_page(file, &text),
                        data: None,
                    })
                },
            )
            .expect("the edit")
            .draft;
    }
    let size = fs::metadata(record_file(root.path(), &draft.id))
        .expect("the record")
        .len();
    assert!(
        size * 2 < created_size * 3,
        "the record grew from {created_size} to {size} bytes over 100 edits"
    );
    let record_json = read_json(&record_file(root.path(), &draft.id));
    assert!(record_json["requests"].as_object().expect("receipts").len() <= 64);
    // Edit k commits revision k + 1. The newest retry returns the current draft whole.
    let newest = store
        .record_edit(
            &draft.id,
            "100",
            "edit-100",
            &json!({"edit": 100}),
            must_not_run_edit,
        )
        .expect("the newest retry");
    assert_eq!(newest.draft, draft);
    // A superseded but retained receipt names its revision and hashes, without page bodies.
    let earlier = store
        .record_edit(
            &draft.id,
            "90",
            "edit-90",
            &json!({"edit": 90}),
            must_not_run_edit,
        )
        .expect("a superseded retry");
    assert_eq!(earlier.draft.revision, "91");
    assert!(earlier.draft.superseded);
    assert!(earlier.draft.files.is_empty());
    assert_eq!(
        earlier.draft.content_hashes,
        Some(BTreeMap::from([(
            file.to_string(),
            sha256_hex(page(90).as_bytes())
        )]))
    );
    // A receipt past the bound is answered as a changed revision, never applied a second time.
    let refused = store
        .record_edit(
            &draft.id,
            "1",
            "edit-1",
            &json!({"edit": 1}),
            must_not_run_edit,
        )
        .expect_err("a receipt past the bound");
    assert_eq!(refused.code, CONFLICT);
    assert_eq!(refused.message, "draft revision changed.");
    assert_eq!(store.get(&draft.id).expect("get").files[file], page(100));
}

// Port of "many apply and discard cycles stay within the product bounds and new drafts are still
// accepted" (hmi-agent-draft-retention.test.ts; spec 43 §8 repair 1: 69 readable records at most).
#[test]
fn many_apply_and_discard_cycles_stay_within_the_product_bounds() {
    let root = TempDir::new("cycles");
    let store = DraftStore::open_with(root.path(), DraftLimits::PRODUCT, ticking_clock())
        .expect("open the store");
    let limits = DraftLimits::PRODUCT;
    let mut ids = Vec::new();
    for cycle in 0..80_u32 {
        let created = store
            .create(
                "workspace",
                &format!("create-{cycle}"),
                &json!({"cycle": cycle}),
                || {
                    Ok(proposal(
                        one_page("hmi/pages/proposal.toml", &format!("cycle {cycle}")),
                        vec![snapshot("hmi/pages/base.toml", "base page")],
                    ))
                },
            )
            .expect("create")
            .draft;
        let edited = store
            .record_edit(
                &created.id,
                &created.revision,
                &format!("edit-{cycle}"),
                &json!({"cycle": cycle}),
                |_| {
                    Ok(EditResult {
                        files: one_page("hmi/pages/proposal.toml", &format!("edited {cycle}")),
                        data: None,
                    })
                },
            )
            .expect("the edit")
            .draft;
        if cycle % 2 == 1 {
            store
                .discard(
                    &edited.id,
                    Some(&DiscardExpectation {
                        workspace_id: "workspace".to_string(),
                        revision: edited.revision.clone(),
                    }),
                )
                .expect("discard");
        } else {
            store
                .apply(
                    &edited.id,
                    &edited.revision,
                    &format!("apply-{cycle}"),
                    &apply_input(&edited.revision),
                    |_| Ok(()),
                    |_| Ok(json!({"applied": true, "cycle": cycle})),
                )
                .expect("apply");
        }
        ids.push(created.id.clone());
        assert!(
            record_count(root.path()) <= limits.max_open + limits.max_finished,
            "{} records after cycle {cycle}",
            record_count(root.path())
        );
        assert!(storage_bytes(root.path()) <= limits.max_bytes);
    }
    let accepted = create_draft(
        &store,
        "create-accepted",
        &[("hmi/pages/next.toml", "next")],
    );
    assert_eq!(
        store.get(&accepted.id).expect("get").files["hmi/pages/next.toml"],
        "next"
    );
    let listed: Vec<String> = store
        .list("workspace")
        .expect("list")
        .into_iter()
        .map(|summary| summary.draft_id)
        .collect();
    assert_eq!(listed, vec![accepted.id.clone()]);
    assert!(
        record_count(root.path()) <= limits.max_finished + 1,
        "finished records beyond the bound left"
    );
    assert_eq!(
        store
            .get(&ids[0])
            .expect_err("the oldest finished record")
            .code,
        EXPIRED_DRAFT,
        "the oldest finished record left first"
    );
    let replayed = store
        .apply(
            &ids[78],
            "2",
            "apply-78",
            &apply_input("2"),
            must_not_run_preflight,
            must_not_run_commit,
        )
        .expect("a recent apply retry");
    assert_eq!(replayed, json!({"applied": true, "cycle": 78}));
    let applied = read_json(&record_file(root.path(), &ids[78]));
    assert_eq!(applied["draft"]["files"], json!({}));
    assert_eq!(applied["draft"]["base"], json!([]));
    assert_eq!(
        applied["requests"],
        json!({}),
        "an applied draft keeps receipts only"
    );
}

// Spec 43 §6 and §8 repair 1: a record of an earlier format is never read, so the store can neither use
// nor remove it. It counts toward the byte bound, and creation names it as draft storage to clean up.
// This replaces the TypeScript test "finished records left by the earlier format leave oldest first".
#[test]
fn records_of_an_earlier_format_are_never_read_and_are_reported_for_cleanup() {
    let root = TempDir::new("legacy");
    let body = "x".repeat(20 * 1024);
    let legacy: Vec<String> = (0..6).map(|_| legacy_record(root.path(), &body)).collect();
    let limits = DraftLimits {
        max_bytes: 300 * 1024,
        ..DraftLimits::PRODUCT
    };
    let store =
        DraftStore::open_with(root.path(), limits, ticking_clock()).expect("open the store");
    assert!(
        storage_bytes(root.path()) > 300 * 1024,
        "fixture: the earlier records alone exceed the bound"
    );
    let refused = store
        .create("workspace", "create-new", &json!({"new": true}), || {
            Ok(proposal(one_page("hmi/pages/new.toml", "new"), vec![]))
        })
        .expect_err("creation names the earlier records");
    assert_eq!(refused.code, LIMIT_EXCEEDED);
    for id in &legacy {
        assert!(refused.message.contains(id), "{}", refused.message);
    }
    assert!(
        legacy
            .iter()
            .all(|id| record_file(root.path(), id).exists()),
        "nothing of an earlier format is removed"
    );
    let report = store.cleanup().expect("cleanup");
    assert!(report.removed.is_empty());
    assert_eq!(report.unreadable.len(), 6);
    assert!(report
        .unreadable
        .iter()
        .all(|record| record.reason.contains("earlier format")));
    assert_eq!(
        store
            .get(&legacy[0])
            .expect_err("an earlier record is not read")
            .code,
        INVALID_ARGUMENT
    );
}

/// Writes a record of the earlier format 1 with its pages, receipts and application, and returns its id.
fn legacy_record(root: &Path, body: &str) -> String {
    let id = new_draft_id().expect("a draft identifier");
    let draft = json!({
        "id": id.clone(),
        "workspaceId": "workspace",
        "revision": "2",
        "state": "applied",
        "files": {"hmi/pages/old.toml": body},
        "base": [{"file": "hmi/pages/old.toml", "exists": true, "text": body, "revision": "disk", "version": null, "dirty": false}],
        "findings": []
    });
    let outcome = draft.clone();
    let record = json!({
        "format": 1,
        "draft": draft,
        "requests": {"edit": {"fingerprint": "f", "outcome": outcome}},
        "applications": {"apply": {"fingerprint": "f", "status": "committed", "outcome": {"applied": true}}}
    });
    fs::write(record_file(root, &id), record.to_string()).expect("write the earlier record");
    id
}

// Port of "open drafts are never removed: a full bound names them, and a discard always makes room"
// (hmi-agent-draft-retention.test.ts; spec 43 §8 repair 1).
#[test]
fn open_drafts_are_never_removed_and_a_full_bound_names_them() {
    let root = TempDir::new("open-bound");
    let limits = DraftLimits {
        max_bytes: 150 * 1024,
        ..DraftLimits::PRODUCT
    };
    let store =
        DraftStore::open_with(root.path(), limits, ticking_clock()).expect("open the store");
    let base = vec![snapshot("hmi/pages/base.toml", &"z".repeat(60 * 1024))];
    let reviewed = create_with_base(
        &store,
        "create-reviewed",
        &base,
        &[("hmi/pages/reviewed.toml", "under review")],
    );
    store
        .review(
            &reviewed.id,
            &reviewed.revision,
            vec![],
            BTreeMap::new(),
            None,
        )
        .expect("the review");
    let other = create_with_base(
        &store,
        "create-other",
        &base,
        &[("hmi/pages/other.toml", "other")],
    );
    let refused = store
        .create("workspace", "create-third", &json!({"third": true}), || {
            Ok(proposal(
                one_page("hmi/pages/third.toml", "third"),
                base.clone(),
            ))
        })
        .expect_err("the byte bound");
    assert_eq!(refused.code, LIMIT_EXCEEDED);
    assert!(
        refused
            .message
            .starts_with("draft storage would exceed 0.15 MiB"),
        "{}",
        refused.message
    );
    assert!(
        refused.message.contains(&reviewed.id),
        "{}",
        refused.message
    );
    assert!(refused.message.contains(&other.id), "{}", refused.message);
    assert_eq!(
        store.get(&reviewed.id).expect("get").state,
        DraftState::Reviewed,
        "the reviewed draft is untouched"
    );
    assert_eq!(
        store.get(&other.id).expect("get").files["hmi/pages/other.toml"],
        "other"
    );
    store
        .discard(
            &other.id,
            Some(&DiscardExpectation {
                workspace_id: "workspace".to_string(),
                revision: other.revision.clone(),
            }),
        )
        .expect("discard");
    let third = create_with_base(
        &store,
        "create-third",
        &base,
        &[("hmi/pages/third.toml", "third")],
    );
    let mut listed: Vec<String> = store
        .list("workspace")
        .expect("list")
        .into_iter()
        .map(|summary| summary.draft_id)
        .collect();
    listed.sort();
    let mut expected = vec![reviewed.id.clone(), third.id.clone()];
    expected.sort();
    assert_eq!(listed, expected);
}

// Port of "the open-draft quota names every open draft instead of removing one"
// (hmi-agent-draft-retention.test.ts; spec 43 §4: five simultaneous drafts).
#[test]
fn the_open_draft_quota_names_every_open_draft_and_a_discard_makes_room() {
    let root = TempDir::new("quota-names");
    let store = DraftStore::open(root.path()).expect("open the store");
    let mut open_drafts = Vec::new();
    for index in 0..DraftLimits::PRODUCT.max_open {
        let file = format!("hmi/pages/p{index}.toml");
        open_drafts.push(create_draft(
            &store,
            &format!("create-{index}"),
            &[(file.as_str(), "page")],
        ));
    }
    let refused = store
        .create("workspace", "create-extra", &json!({"extra": true}), || {
            Ok(proposal(one_page("hmi/pages/extra.toml", "extra"), vec![]))
        })
        .expect_err("the open-draft quota");
    assert_eq!(refused.code, LIMIT_EXCEEDED);
    assert!(
        refused
            .to_string()
            .starts_with("LIMIT_EXCEEDED: 5 open proposals already exist"),
        "{refused}"
    );
    for open in &open_drafts {
        assert!(
            refused.message.contains(&open.id),
            "the refusal names {}",
            open.id
        );
        assert_eq!(store.get(&open.id).expect("get").id, open.id);
    }
    store
        .discard(&open_drafts[0].id, None)
        .expect("discard one open draft");
    create_draft(&store, "create-extra", &[("hmi/pages/extra.toml", "extra")]);
}

// Port of "finished records expire after the retention period and open drafts do not"
// (hmi-agent-draft-retention.test.ts; spec 43 §8 repair 1: seven days).
#[test]
fn finished_records_expire_after_the_retention_period_and_open_drafts_do_not() {
    let root = TempDir::new("expiry");
    let (clock, reader) = clock_at(T0);
    let store = DraftStore::open_with(root.path(), DraftLimits::PRODUCT, reader).expect("open");
    let applied = create_draft(
        &store,
        "create-applied",
        &[("hmi/pages/applied.toml", "applied")],
    );
    apply_ok(&store, &applied, "apply");
    let discarded = create_draft(
        &store,
        "create-discarded",
        &[("hmi/pages/discarded.toml", "discarded")],
    );
    store.discard(&discarded.id, None).expect("discard");
    let open = create_draft(&store, "create-open", &[("hmi/pages/open.toml", "open")]);
    clock.store(T0 + DraftLimits::PRODUCT.retention_ms, Ordering::SeqCst);
    create_draft(&store, "create-later", &[("hmi/pages/later.toml", "later")]);
    assert!(!record_file(root.path(), &applied.id).exists());
    assert!(!record_file(root.path(), &discarded.id).exists());
    assert_eq!(
        store.get(&applied.id).expect_err("expired").code,
        EXPIRED_DRAFT
    );
    assert_eq!(
        store.get(&open.id).expect("get").files["hmi/pages/open.toml"],
        "open"
    );
}

// The retention period is exact: a finished record is kept one millisecond before it expires.
#[test]
fn a_finished_record_is_kept_until_the_retention_period_ends() {
    let root = TempDir::new("expiry-boundary");
    let (clock, reader) = clock_at(T0);
    let store = DraftStore::open_with(root.path(), DraftLimits::PRODUCT, reader).expect("open");
    let applied = create_draft(
        &store,
        "create-applied",
        &[("hmi/pages/applied.toml", "applied")],
    );
    apply_ok(&store, &applied, "apply");
    clock.store(T0 + DraftLimits::PRODUCT.retention_ms - 1, Ordering::SeqCst);
    create_draft(&store, "create-first", &[("hmi/pages/first.toml", "first")]);
    assert!(
        record_file(root.path(), &applied.id).exists(),
        "one millisecond early"
    );
    clock.store(T0 + DraftLimits::PRODUCT.retention_ms, Ordering::SeqCst);
    create_draft(
        &store,
        "create-second",
        &[("hmi/pages/second.toml", "second")],
    );
    assert!(
        !record_file(root.path(), &applied.id).exists(),
        "at the end of the period"
    );
}

// Port of "an unreadable record no longer blocks discovery or creation and is kept for inspection"
// (hmi-agent-draft-retention.test.ts; spec 43 §8 repair 1).
#[test]
fn an_unreadable_record_blocks_neither_discovery_nor_creation_and_is_kept() {
    let root = TempDir::new("unreadable");
    let store = DraftStore::open(root.path()).expect("open the store");
    let unreadable = new_draft_id().expect("an id");
    fs::write(record_file(root.path(), &unreadable), r#"{"format":1,"#).expect("write");
    let crashed = root.path().join(format!(
        "{}.json.{}.tmp",
        new_draft_id().expect("an id"),
        new_draft_id().expect("an id")
    ));
    fs::write(&crashed, "partial write").expect("write the crashed temporary file");
    let draft = create_draft(&store, "create-fresh", &[("hmi/pages/fresh.toml", "fresh")]);
    let listed: Vec<String> = store
        .list("workspace")
        .expect("list")
        .into_iter()
        .map(|summary| summary.draft_id)
        .collect();
    assert_eq!(listed, vec![draft.id.clone()]);
    assert!(
        record_file(root.path(), &unreadable).exists(),
        "retained for inspection"
    );
    let error = store.get(&unreadable).expect_err("an unreadable record");
    assert_eq!(error.code, INVALID_ARGUMENT);
    assert!(error.message.contains("corrupt"), "{}", error.message);
    assert!(
        !crashed.exists(),
        "a crashed write's temporary file is removed"
    );
}

// Cleanup removes the finished records past retention and the temporary files, keeps open drafts, and
// reports the unreadable records (specification 43 §6 and §8 repair 1).
#[test]
fn cleanup_removes_expired_finished_records_and_temporary_files_and_reports_unreadable_ones() {
    let root = TempDir::new("cleanup");
    let (clock, reader) = clock_at(T0);
    let store = DraftStore::open_with(root.path(), DraftLimits::PRODUCT, reader).expect("open");
    let applied = create_draft(
        &store,
        "create-applied",
        &[("hmi/pages/applied.toml", "applied")],
    );
    apply_ok(&store, &applied, "apply");
    let open = create_draft(&store, "create-open", &[("hmi/pages/open.toml", "open")]);
    let unreadable = new_draft_id().expect("an id");
    fs::write(record_file(root.path(), &unreadable), r#"{"format":2,"#).expect("write");
    fs::write(root.path().join("leftover.json.x.tmp"), "partial").expect("write");
    clock.store(T0 + DraftLimits::PRODUCT.retention_ms, Ordering::SeqCst);
    let report = store.cleanup().expect("cleanup");
    assert_eq!(report.removed, vec![applied.id.clone()]);
    assert_eq!(report.unreadable.len(), 1);
    assert_eq!(report.unreadable[0].id, unreadable);
    assert!(!root.path().join("leftover.json.x.tmp").exists());
    assert!(record_file(root.path(), &open.id).exists());
}
