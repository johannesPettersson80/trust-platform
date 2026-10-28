//! Tests of the draft store (specification 43, §4, §6 and §8 repair 1). Port of
//! `editors/vscode/src/test/suite/hmi-agent-drafts.test.ts`, `hmi-agent-draft-retention.test.ts`, and
//! the store-level assertions of `hmi-agent-apply.test.ts`. Each test names its TypeScript test.
//!
//! Adapted: the TypeScript tests cancel with an `AbortSignal`; here the caller's cancellation is
//! the closure's refusal (`CANCELLED`), which the store then never commits. Not ported: the cancelled
//! review and discard tests, because the store takes no cancellation signal; the caller checks it
//! before it calls the store, and the store commits in one atomic write.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::documents::sha256_hex;
use super::super::error::{
    CANCELLED, CONFLICT, EXPIRED_DRAFT, INVALID_ARGUMENT, LIMIT_EXCEEDED, PROJECT_UNAVAILABLE,
};
use super::super::review_record::{ReviewIssue, Verdict};
use super::*;

#[path = "drafts_tests/apply.rs"]
mod apply;
#[path = "drafts_tests/drafts.rs"]
mod drafts;
#[path = "drafts_tests/retention.rs"]
mod retention;

/// `2026-10-07T00:00:00Z` in milliseconds (`Date.UTC(2026, 9, 7)` in the TypeScript tests).
const T0: i64 = 1_791_331_200_000;

/// A fresh folder under the system temporary folder, removed when the test ends.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "trust-hmi-drafts-{label}-{}-{}",
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

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A clock held by the test: it reads the value the test stores.
fn clock_at(start: i64) -> (Arc<AtomicI64>, Clock) {
    let shared = Arc::new(AtomicI64::new(start));
    let reader = Arc::clone(&shared);
    (shared, Box::new(move || reader.load(Ordering::SeqCst)))
}

/// A clock that moves one millisecond per reading, so finishing times strictly increase.
fn ticking_clock() -> Clock {
    let ticks = AtomicI64::new(0);
    Box::new(move || ticks.fetch_add(1, Ordering::SeqCst) + 1)
}

/// The record file of a draft.
fn record_file(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.json"))
}

/// A record file as JSON.
fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read the record")).expect("record JSON")
}

/// The number of record files (`.json`) in the storage folder.
fn record_count(root: &Path) -> usize {
    fs::read_dir(root)
        .expect("list the storage folder")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".json"))
        .count()
}

/// The bytes of all record files.
fn storage_bytes(root: &Path) -> u64 {
    fs::read_dir(root)
        .expect("list the storage folder")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".json"))
        .map(|entry| entry.metadata().map_or(0, |metadata| metadata.len()))
        .sum()
}

/// One page as a file map.
fn one_page(file: &str, text: &str) -> BTreeMap<String, String> {
    BTreeMap::from([(file.to_string(), text.to_string())])
}

/// Several pages as a file map.
fn pages(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(file, text)| ((*file).to_string(), (*text).to_string()))
        .collect()
}

/// A base snapshot of a page that exists on disk.
fn snapshot(file: &str, text: &str) -> DocumentSnapshot {
    DocumentSnapshot {
        file: file.to_string(),
        exists: true,
        text: text.to_string(),
        revision: format!("disk:{file}"),
        version: None,
        dirty: false,
    }
}

/// A prepared proposal with the given pages and base.
fn proposal(files: BTreeMap<String, String>, base: Vec<DocumentSnapshot>) -> PreparedProposal {
    PreparedProposal {
        base,
        files,
        program_revision: None,
        intent: None,
        data: None,
    }
}

/// Creates a proposal in the workspace `workspace` with the given pages and no base.
fn create_draft(store: &DraftStore, key: &str, files: &[(&str, &str)]) -> Draft {
    store
        .create("workspace", key, &json!({"create": key}), || {
            Ok(proposal(pages(files), vec![]))
        })
        .expect("create the proposal")
        .draft
}

/// Creates a proposal in the workspace `workspace` with the given pages and base.
fn create_with_base(
    store: &DraftStore,
    key: &str,
    base: &[DocumentSnapshot],
    files: &[(&str, &str)],
) -> Draft {
    store
        .create("workspace", key, &json!({"create": key}), || {
            Ok(proposal(pages(files), base.to_vec()))
        })
        .expect("create the proposal")
        .draft
}

/// The input of an application request at `revision`.
fn apply_input(revision: &str) -> Value {
    json!({"revision": revision, "save": false, "allowIncomplete": false})
}

/// Applies a draft at its revision, with a preflight that passes and a commit that succeeds.
fn apply_ok(store: &DraftStore, draft: &Draft, key: &str) -> Value {
    store
        .apply(
            &draft.id,
            &draft.revision,
            key,
            &apply_input(&draft.revision),
            |_| Ok(()),
            |_| Ok(json!({"applied": true})),
        )
        .expect("apply the draft")
}

/// The issue used by the visual review tests.
fn issue() -> ReviewIssue {
    ReviewIssue {
        file: "hmi/pages/overview.toml".to_string(),
        objects: vec!["tank".to_string()],
        message: "label clipped".to_string(),
    }
}

/// An edit that must never run: a replayed request does not run its update.
fn must_not_run_edit(_draft: &Draft) -> AgentResult<EditResult> {
    Err(AgentError::invalid(
        "the original request must not run again",
    ))
}

/// A preflight that must never run.
fn must_not_run_preflight(_draft: &Draft) -> AgentResult<()> {
    Err(AgentError::invalid(
        "the original request must not run again",
    ))
}

/// A source commit that must never run.
fn must_not_run_commit(_draft: &Draft) -> Result<Value, CommitFailure> {
    Err(CommitFailure::Uncertain(
        "the original request must not run again".to_string(),
    ))
}

/// The codes of the refusals among `outcomes`, counted by `code`.
fn count_code<T>(outcomes: &[AgentResult<T>], code: &str) -> usize {
    outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Err(error) if error.code == code))
        .count()
}

/// The successes among `outcomes`.
fn count_ok<T>(outcomes: &[AgentResult<T>]) -> usize {
    outcomes.iter().filter(|outcome| outcome.is_ok()).count()
}

/// Two edits of the same draft revision, raced on two threads through `first` and `second`.
fn race_two_edits(
    first: &DraftStore,
    second: &DraftStore,
    draft: &Draft,
) -> Vec<AgentResult<EditOutcome>> {
    thread::scope(|scope| {
        let handles: Vec<_> = [(first, "key1"), (second, "key2")]
            .into_iter()
            .enumerate()
            .map(|(index, (owner, key))| {
                let id = draft.id.clone();
                let revision = draft.revision.clone();
                scope.spawn(move || {
                    owner.record_edit(&id, &revision, key, &json!({"index": index}), move |_| {
                        Ok(EditResult {
                            files: one_page("page", &index.to_string()),
                            data: None,
                        })
                    })
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("the edit thread"))
            .collect()
    })
}
