//! Tests of the draft record rules (specification 43, §6 and §8 repair 1). Port of the record rules
//! that `editors/vscode/src/test/suite/hmi-agent-draft-retention.test.ts` and
//! `hmi-agent-drafts.test.ts` exercise through the store (their store-level assertions are in
//! `drafts_tests.rs`), plus closed-form and boundary tests of each limit.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use super::super::error::{EXPIRED_DRAFT, INVALID_ARGUMENT, PROJECT_UNAVAILABLE};
use super::*;

/// A fresh folder under the system temporary folder, removed when the test ends.
struct Folder(PathBuf);

impl Folder {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "trust-hmi-records-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create the test folder");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A draft identifier used by the fixtures.
const ID_A: &str = "0191e1f0-7b2c-4d3e-8f4a-5b6c7d8e9f01";
/// A second draft identifier used by the fixtures.
const ID_B: &str = "0191e1f0-7b2c-4d3e-8f4a-5b6c7d8e9f02";
/// A draft identifier with no record file in any fixture.
const ID_C: &str = "0191e1f0-7b2c-4d3e-8f4a-5b6c7d8e9f03";

/// A stored record in the current format, for a draft with the given pages.
fn stored(id: &str, state: DraftState, files: &[(&str, &str)]) -> StoredDraft {
    let files: BTreeMap<String, String> = files
        .iter()
        .map(|(file, text)| ((*file).to_string(), (*text).to_string()))
        .collect();
    StoredDraft {
        format: FORMAT,
        draft: Draft {
            id: id.to_string(),
            workspace_id: "workspace".to_string(),
            revision: "1".to_string(),
            state,
            content_hashes: Some(content_hashes(&files)),
            files,
            ..Draft::default()
        },
        discarded: false,
        finished_at: None,
        creation: None,
        requests: BTreeMap::new(),
        applications: BTreeMap::new(),
    }
}

/// A record-list entry with the given size and modification time.
fn entry(id: &str, size: u64, modified: i64, stored: Result<StoredDraft, String>) -> RecordEntry {
    RecordEntry {
        id: id.to_string(),
        size,
        modified,
        stored,
    }
}

/// A finished (applied) record, finished at `finished_at`.
fn applied_at(id: &str, finished_at: i64, size: u64) -> RecordEntry {
    let mut record = stored(id, DraftState::Applied, &[]);
    record.finished_at = Some(finished_at);
    entry(id, size, 0, Ok(record))
}

/// A finished (discarded) record, finished at `finished_at`.
fn discarded_at(id: &str, finished_at: i64, size: u64) -> RecordEntry {
    let mut record = stored(id, DraftState::Draft, &[]);
    record.discarded = true;
    record.finished_at = Some(finished_at);
    entry(id, size, 0, Ok(record))
}

/// An open draft record, which is never removed by eviction.
fn open_draft(id: &str, size: u64) -> RecordEntry {
    entry(id, size, 0, Ok(stored(id, DraftState::Draft, &[])))
}

/// Limits that bound only the finished-record count, with retention that never expires.
fn count_limits(max_finished: usize) -> DraftLimits {
    DraftLimits {
        max_finished,
        retention_ms: i64::MAX,
        ..DraftLimits::PRODUCT
    }
}

/// The identifiers of an eviction plan, in order.
fn plan_ids(plan: &[&RecordEntry]) -> Vec<String> {
    plan.iter().map(|entry| entry.id.clone()).collect()
}

// Closed-form: SHA-256 of the empty text and of "abc", as lowercase hexadecimal (`contentHashes`).
#[test]
fn content_hashes_are_sha256_of_each_proposal_text() {
    let files = BTreeMap::from([
        ("hmi/pages/empty.toml".to_string(), String::new()),
        ("hmi/pages/abc.toml".to_string(), "abc".to_string()),
    ]);
    assert_eq!(
        content_hashes(&files),
        BTreeMap::from([
            (
                "hmi/pages/abc.toml".to_string(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string()
            ),
            (
                "hmi/pages/empty.toml".to_string(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()
            ),
        ])
    );
}

// Canonical JSON sorts object keys recursively and keeps array order (`requestFingerprint`).
#[test]
fn canonical_json_sorts_keys_recursively_and_keeps_array_order() {
    let value = json!({"b": 1, "a": {"y": 2, "x": [3, {"d": 4, "c": 5}]}});
    assert_eq!(
        canonical_json(&value),
        r#"{"a":{"x":[3,{"c":5,"d":4}],"y":2},"b":1}"#
    );
    assert_eq!(canonical_json(&json!("a\"b")), r#""a\"b""#);
    assert_eq!(canonical_json(&json!([2, 1])), "[2,1]");
}

// Closed-form fingerprint: SHA-256 of the canonical text `{"a":1,"b":"x"}`.
#[test]
fn request_fingerprint_is_the_sha256_of_the_canonical_text() {
    assert_eq!(
        request_fingerprint(&json!({"b": "x", "a": 1})),
        "ecf9e98ec0641e23113ff3ce8bdc78d0ddd249886517fd4a7f68cc83d4e65667"
    );
}

// Replaying the same JSON data in another field order is the same request; other data is another one.
#[test]
fn request_fingerprint_ignores_field_order_but_not_content_or_array_order() {
    let first = json!({"file": "hmi/pages/a.toml", "operations": [{"op": "move", "x": 8}]});
    let reordered = json!({"operations": [{"x": 8, "op": "move"}], "file": "hmi/pages/a.toml"});
    assert_eq!(request_fingerprint(&first), request_fingerprint(&reordered));
    let moved = json!({"file": "hmi/pages/a.toml", "operations": [{"op": "move", "x": 9}]});
    assert_ne!(request_fingerprint(&first), request_fingerprint(&moved));
    assert_ne!(
        request_fingerprint(&json!([1, 2])),
        request_fingerprint(&json!([2, 1]))
    );
}

// A draft identifier has the shape of a lowercase version-4 UUID; anything else is refused.
#[test]
fn draft_identifiers_are_lowercase_uuid_shaped_and_nothing_else() {
    assert!(is_draft_id(ID_A));
    assert!(!is_draft_id("../outside"));
    assert!(
        !is_draft_id("0191E1F0-7B2C-4D3E-8F4A-5B6C7D8E9F01"),
        "uppercase"
    );
    assert!(
        !is_draft_id("0191e1f0-7b2c-4d3e-8f4a-5b6c7d8e9f0"),
        "35 characters"
    );
    assert!(
        !is_draft_id("0191e1f0-7b2c-4d3e-8f4a-5b6c7d8e9f011"),
        "37 characters"
    );
    assert!(
        !is_draft_id("0191e1f0-7b2c-4d3e-8f4a-5b6c7d8e9fzz"),
        "not hexadecimal"
    );
    assert!(
        !is_draft_id("0191e1f07b2c-4d3e-8f4a-5b6c7d8e9f01"),
        "a missing dash"
    );
}

// Generated identifiers are unique, have the identifier shape, and are version 4 with the RFC variant.
#[test]
fn generated_draft_identifiers_are_unique_version_four_uuids() {
    let ids: Vec<String> = (0..200)
        .map(|_| new_draft_id().expect("a random id"))
        .collect();
    let unique: std::collections::BTreeSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), 200);
    for id in &ids {
        assert!(is_draft_id(id), "{id}");
        assert_eq!(id.as_bytes()[14], b'4', "version nibble in {id}");
        assert!(
            matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'),
            "variant in {id}"
        );
    }
}

// Each edit receipt keeps the newest `max_receipts` of all, and each new receipt is the newest.
#[test]
fn edit_receipts_keep_only_the_newest_bound() {
    let receipt = |number: u64| NewEditReceipt {
        fingerprint: format!("f{number}"),
        revision: number.to_string(),
        content_hashes: BTreeMap::new(),
        data: None,
    };
    let mut requests = BTreeMap::new();
    for number in 1..=4 {
        requests = with_edit_receipt(&requests, &format!("k{number}"), receipt(number), 3);
    }
    let keys: Vec<&str> = requests.keys().map(String::as_str).collect();
    assert_eq!(keys, ["k2", "k3", "k4"]);
    assert_eq!(requests["k2"].sequence, 1);
    assert_eq!(requests["k4"].sequence, 3);
    // Inside the bound nothing is removed: three receipts fit a bound of three.
    let mut three = BTreeMap::new();
    for number in 1..=3 {
        three = with_edit_receipt(&three, &format!("k{number}"), receipt(number), 3);
    }
    assert_eq!(three.len(), 3);
}

// Surplus finished records leave oldest first; exactly the bound many stay (`evictionPlan`, count bound).
#[test]
fn eviction_removes_the_oldest_finished_records_beyond_the_count_bound() {
    let entries = vec![
        applied_at("c", 300, 10),
        discarded_at("a", 100, 10),
        applied_at("b", 200, 10),
    ];
    let limits = count_limits(2);
    assert_eq!(
        plan_ids(&eviction_plan(&entries, &limits, 1_000, 0, None)),
        ["a"]
    );
    let at_bound = count_limits(3);
    assert!(eviction_plan(&entries, &at_bound, 1_000, 0, None).is_empty());
}

// A finished record expires when its age reaches the retention period, not one millisecond before it.
#[test]
fn eviction_expires_a_finished_record_at_exactly_the_retention_period() {
    let entries = vec![applied_at("old", 900, 10), applied_at("young", 901, 10)];
    let limits = DraftLimits {
        retention_ms: 100,
        ..DraftLimits::PRODUCT
    };
    assert_eq!(
        plan_ids(&eviction_plan(&entries, &limits, 1_000, 0, None)),
        ["old"]
    );
}

// Records leave until the freed bytes reach the requested amount, oldest first (the byte bound).
#[test]
fn eviction_frees_bytes_oldest_first_until_the_need_is_met() {
    let entries = vec![
        applied_at("c", 3, 10),
        applied_at("a", 1, 10),
        applied_at("b", 2, 10),
    ];
    let limits = count_limits(64);
    assert_eq!(
        plan_ids(&eviction_plan(&entries, &limits, 10, 15, None)),
        ["a", "b"]
    );
    assert_eq!(
        plan_ids(&eviction_plan(&entries, &limits, 10, 10, None)),
        ["a"]
    );
    assert!(eviction_plan(&entries, &limits, 10, 0, None).is_empty());
}

// Open drafts, unreadable records and the record being written never leave in an eviction.
#[test]
fn eviction_never_removes_open_unreadable_or_kept_records() {
    let entries = vec![
        open_draft("open", 1_000),
        entry(
            "unreadable",
            1_000,
            0,
            Err("INVALID_ARGUMENT: corrupt".to_string()),
        ),
        applied_at("kept", 1, 10),
        applied_at("other", 2, 10),
    ];
    let limits = count_limits(0);
    assert_eq!(
        plan_ids(&eviction_plan(
            &entries,
            &limits,
            10,
            1_000_000,
            Some("kept")
        )),
        ["other"]
    );
}

// Equal finishing times are ordered by identifier, so the plan is deterministic.
#[test]
fn eviction_breaks_ties_by_identifier() {
    let entries = vec![discarded_at("b", 5, 10), discarded_at("a", 5, 10)];
    let limits = count_limits(1);
    assert_eq!(
        plan_ids(&eviction_plan(&entries, &limits, 10, 0, None)),
        ["a"]
    );
}

// A record without a finishing time is aged by its file's modification time (`finishedTime`).
#[test]
fn a_finished_record_without_a_finishing_time_uses_its_file_time() {
    let mut record = stored(ID_A, DraftState::Applied, &[]);
    record.finished_at = None;
    let entries = vec![entry(ID_A, 10, 1_000, Ok(record))];
    let limits = DraftLimits {
        retention_ms: 500,
        ..DraftLimits::PRODUCT
    };
    assert_eq!(
        plan_ids(&eviction_plan(&entries, &limits, 1_500, 0, None)),
        [ID_A]
    );
    assert!(eviction_plan(&entries, &limits, 1_499, 0, None).is_empty());
}

// The record that remains of an applied draft keeps its identity, key and application receipts only.
#[test]
fn an_applied_draft_keeps_its_identity_key_and_application_receipts() {
    let mut record = stored(ID_A, DraftState::Reviewed, &[("hmi/pages/a.toml", "body")]);
    record.creation = Some(CreationReceipt {
        key: "create-1".to_string(),
        fingerprint: "f1".to_string(),
        content_hashes: Some(BTreeMap::new()),
        data: Some(json!({"model": 1})),
    });
    record.applications.insert(
        "apply-1".to_string(),
        ApplicationReceipt {
            fingerprint: "f2".to_string(),
            status: ApplicationStatus::Committed,
            outcome: Some(json!({"applied": true})),
        },
    );
    let kept = finished_record(&record, 42, Finish::Applied);
    assert_eq!(kept.format, FORMAT);
    assert_eq!(kept.finished_at, Some(42));
    assert!(!kept.discarded);
    assert_eq!(kept.draft.id, ID_A);
    assert_eq!(kept.draft.state, DraftState::Applied);
    assert_eq!(kept.draft.revision, "1");
    assert!(kept.draft.files.is_empty());
    assert!(kept.draft.base.is_empty());
    assert_eq!(kept.draft.content_hashes, Some(BTreeMap::new()));
    assert!(kept.requests.is_empty());
    assert_eq!(
        kept.creation,
        Some(CreationReceipt {
            key: "create-1".to_string(),
            fingerprint: "f1".to_string(),
            content_hashes: None,
            data: None,
        })
    );
    assert_eq!(kept.applications, record.applications);
}

// A discarded draft keeps its identity, its state and its creation key only; no application receipts.
#[test]
fn a_discarded_draft_keeps_its_identity_and_creation_key_only() {
    let mut record = stored(ID_B, DraftState::Reviewed, &[("hmi/pages/b.toml", "body")]);
    record.creation = Some(CreationReceipt {
        key: "create-2".to_string(),
        fingerprint: "f3".to_string(),
        content_hashes: Some(BTreeMap::new()),
        data: None,
    });
    let kept = finished_record(&record, 7, Finish::Discarded);
    assert!(kept.discarded);
    assert_eq!(
        kept.draft.state,
        DraftState::Reviewed,
        "the state before discard is kept"
    );
    assert!(kept.draft.files.is_empty());
    assert!(kept.applications.is_empty());
    assert_eq!(
        kept.creation.as_ref().map(|receipt| receipt.key.as_str()),
        Some("create-2")
    );
    assert!(is_finished(&kept));
}

// The message lists each open draft with its pages (sorted), state and revision; none means "no pages".
#[test]
fn open_drafts_are_described_with_pages_state_and_revision() {
    let first = stored(
        ID_A,
        DraftState::Reviewed,
        &[("hmi/pages/b.toml", "x"), ("hmi/pages/a.toml", "y")],
    );
    let second = stored(ID_B, DraftState::Draft, &[]);
    assert_eq!(
        describe_open(&[&first]),
        format!("{ID_A} (hmi/pages/a.toml, hmi/pages/b.toml; reviewed; revision 1)")
    );
    assert_eq!(
        describe_open(&[&first, &second]),
        format!(
            "{ID_A} (hmi/pages/a.toml, hmi/pages/b.toml; reviewed; revision 1); {ID_B} (no pages; draft; revision 1)"
        )
    );
    assert_eq!(describe_open(&[]), "");
}

// Byte counts print as two decimals without trailing zeros, rounded half up (`mebibytes`).
#[test]
fn byte_counts_print_in_mebibytes_as_the_messages_do() {
    assert_eq!(mebibytes(0), "0 MiB");
    assert_eq!(mebibytes(1), "0 MiB");
    assert_eq!(mebibytes(153_600), "0.15 MiB", "150 KiB");
    assert_eq!(mebibytes(1_101_005), "1.05 MiB");
    assert_eq!(mebibytes(1_153_434), "1.1 MiB");
    assert_eq!(mebibytes(1_572_864), "1.5 MiB");
    assert_eq!(mebibytes(52_428_800), "50 MiB", "the product bound");
}

// A JSON `null` receipt datum is a present value; an absent one is not.
#[test]
fn a_null_receipt_datum_is_kept_and_an_absent_one_is_not() {
    let null: EditReceipt = serde_json::from_value(json!({
        "fingerprint": "f", "revision": "2", "data": null, "sequence": 0
    }))
    .expect("an edit receipt");
    assert_eq!(null.data, Some(serde_json::Value::Null));
    let absent: EditReceipt = serde_json::from_value(json!({
        "fingerprint": "f", "revision": "2", "sequence": 0
    }))
    .expect("an edit receipt");
    assert_eq!(absent.data, None);
}

// A record of the current format reads back with its hashes recomputed from the proposal texts.
#[test]
fn a_stored_record_reads_back_with_hashes_recomputed_from_its_texts() {
    let folder = Folder::new("read-valid");
    let record = stored(ID_A, DraftState::Draft, &[("hmi/pages/a.toml", "body")]);
    fs::write(
        folder.path().join(format!("{ID_A}.json")),
        serde_json::to_vec(&record).expect("json"),
    )
    .expect("write the record");
    let read = read_record(folder.path(), ID_A).expect("a valid record");
    assert_eq!(read.draft.files["hmi/pages/a.toml"], "body");
    assert_eq!(read.draft.content_hashes, record.draft.content_hashes);
    assert_eq!(read.draft.revision, "1");
}

// Each refusal of a record: damaged text, a record under another name, malformed JSON, an earlier
// format, an unknown format and a missing record. Each keeps its typed code.
#[test]
fn a_record_that_cannot_be_trusted_is_refused_with_a_typed_error() {
    let folder = Folder::new("read-refusals");
    let record = stored(ID_A, DraftState::Draft, &[("hmi/pages/a.toml", "body")]);
    let mut damaged: Value = serde_json::to_value(&record).expect("json");
    damaged["draft"]["files"]["hmi/pages/a.toml"] = json!("damaged body");
    fs::write(
        folder.path().join(format!("{ID_A}.json")),
        damaged.to_string(),
    )
    .expect("write");
    let error = read_record(folder.path(), ID_A).expect_err("damaged text");
    assert_eq!(error.code, INVALID_ARGUMENT);
    assert!(error.message.contains("corrupt"), "{}", error.message);

    fs::write(
        folder.path().join(format!("{ID_B}.json")),
        serde_json::to_vec(&record).expect("json"),
    )
    .expect("write");
    let renamed = read_record(folder.path(), ID_B).expect_err("a record under another name");
    assert_eq!(renamed.code, INVALID_ARGUMENT);
    assert!(renamed.message.contains("corrupt"));

    fs::write(
        folder.path().join(format!("{ID_A}.json")),
        r#"{"format":2,"#,
    )
    .expect("write");
    let truncated = read_record(folder.path(), ID_A).expect_err("truncated JSON");
    assert_eq!(truncated.code, INVALID_ARGUMENT);
    assert!(truncated.message.contains("corrupt"));

    let mut earlier: Value = serde_json::to_value(&record).expect("json");
    earlier["format"] = json!(1);
    fs::write(
        folder.path().join(format!("{ID_A}.json")),
        earlier.to_string(),
    )
    .expect("write");
    let legacy = read_record(folder.path(), ID_A).expect_err("an earlier format");
    assert_eq!(legacy.code, INVALID_ARGUMENT);
    assert!(
        legacy.message.contains("earlier format"),
        "{}",
        legacy.message
    );
    assert!(legacy.message.contains("clean up"), "{}", legacy.message);

    let mut unknown: Value = serde_json::to_value(&record).expect("json");
    unknown["format"] = json!(3);
    fs::write(
        folder.path().join(format!("{ID_A}.json")),
        unknown.to_string(),
    )
    .expect("write");
    assert_eq!(
        read_record(folder.path(), ID_A).expect_err("format 3").code,
        INVALID_ARGUMENT
    );

    let missing = read_record(folder.path(), ID_C).expect_err("no record");
    assert_eq!(missing.code, EXPIRED_DRAFT);
    assert_eq!(missing.message, "draft is unavailable.");

    let malformed = read_record(folder.path(), "../outside").expect_err("a path");
    assert_eq!(malformed.code, INVALID_ARGUMENT);
    assert_eq!(malformed.message, "invalid draft id.");
}

// Discovery keeps readable and unreadable records in identifier order, skips files whose names are not
// draft records, and leaves a temporary file to the cleanup step.
#[test]
fn scan_lists_readable_and_unreadable_records_and_ignores_other_files() {
    let folder = Folder::new("scan");
    let record = stored(ID_A, DraftState::Draft, &[("hmi/pages/a.toml", "body")]);
    fs::write(
        folder.path().join(format!("{ID_A}.json")),
        serde_json::to_vec(&record).expect("json"),
    )
    .expect("write");
    fs::write(
        folder.path().join(format!("{ID_B}.json")),
        r#"{"format":2,"#,
    )
    .expect("write");
    fs::write(folder.path().join("notes.json"), "{}").expect("write");
    fs::write(folder.path().join(".mutation-lock"), "").expect("write");
    fs::write(folder.path().join(format!("{ID_A}.json.x.tmp")), "partial").expect("write");
    let entries = scan(folder.path()).expect("scan");
    let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, [ID_A, ID_B]);
    assert!(entries[0].stored.is_ok());
    let reason = entries[1].stored.as_ref().expect_err("unreadable");
    assert!(reason.starts_with("INVALID_ARGUMENT: "), "{reason}");
    assert_eq!(entries[1].size, 12);
}

// A folder that does not exist yet has no records (discovery never fails on a missing folder).
#[test]
fn a_missing_folder_has_no_records() {
    let folder = Folder::new("missing-folder");
    let absent = folder.path().join("not-created");
    assert!(scan(&absent).expect("scan").is_empty());
}

// Replacing a record writes its bytes and leaves no temporary file; the record is private (0600 on Unix).
#[test]
fn replacing_a_record_is_atomic_and_leaves_no_temporary_file() {
    let folder = Folder::new("replace");
    replace_record(folder.path(), ID_A, b"first").expect("write");
    replace_record(folder.path(), ID_A, b"second").expect("replace");
    assert_eq!(
        fs::read(folder.path().join(format!("{ID_A}.json"))).expect("read"),
        b"second"
    );
    let names: Vec<String> = fs::read_dir(folder.path())
        .expect("list")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("name")
        })
        .collect();
    assert_eq!(names, [format!("{ID_A}.json")], "no temporary file is left");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(folder.path().join(format!("{ID_A}.json")))
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

// Removing temporary files touches only names ending in `.tmp`, and a record that is already gone is
// removed without error.
#[test]
fn remove_temporaries_removes_only_temporary_files() {
    let folder = Folder::new("temporaries");
    fs::write(folder.path().join(format!("{ID_A}.json.x.tmp")), "partial").expect("write");
    fs::write(folder.path().join(format!("{ID_A}.json")), "{}").expect("write");
    remove_temporaries(folder.path()).expect("remove");
    assert!(!folder.path().join(format!("{ID_A}.json.x.tmp")).exists());
    assert!(folder.path().join(format!("{ID_A}.json")).exists());
    remove_record(folder.path(), ID_A).expect("remove the record");
    remove_record(folder.path(), ID_A).expect("a record that is gone is removed");
    assert_eq!(
        remove_record(folder.path(), "../outside")
            .expect_err("a path")
            .code,
        INVALID_ARGUMENT
    );
}

// The storage folder is created with its parents, private to the owner on Unix (mode 0700).
#[test]
fn the_storage_folder_is_created_private() {
    let folder = Folder::new("storage-mode");
    let storage = folder.path().join("nested").join("private");
    create_storage_dir(&storage).expect("create");
    assert!(storage.is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(storage)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }
}

// Storage that cannot be read is reported as draft storage, never as a missing draft.
#[test]
fn an_unreadable_storage_folder_is_a_storage_error() {
    let folder = Folder::new("unreadable");
    let file = folder.path().join("not-a-folder");
    fs::write(&file, "x").expect("write");
    let error = scan(&file).expect_err("a file is not a folder");
    assert_eq!(error.code, PROJECT_UNAVAILABLE);
    assert!(
        error.message.starts_with("draft storage cannot list"),
        "{}",
        error.message
    );
}
