//! The draft store (specification 43, §4 and §6): the lifecycle of a proposal, `draft -> validated ->
//! reviewed -> applied` or `discarded`, the revision of its pages, the receipts that make a repeated
//! request return its original outcome, and the bounded storage its records live in.
//!
//! Port of `editors/vscode/src/hmiBuilder/agent/drafts.ts` (`DraftStore`). The store holds proposal file
//! texts and receipt data as they were given; it parses no page and runs no edit. The caller performs
//! each edit, validation, preview and source write. The store records the outcome in one atomic record
//! write, under the mutation lock.
//!
//! The lock is an operating-system file lock on `.mutation-lock` in the storage folder. A refusal changes
//! nothing. Where the TypeScript store differs from specification 43, the difference is noted at the
//! method concerned and in the port report.

use std::collections::BTreeMap;
use std::fs::{self, TryLockError};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::documents::DocumentSnapshot;
use super::draft_records::{
    content_hashes, create_storage_dir, describe_open, eviction_plan, finished_record, is_false,
    is_finished, mebibytes, new_draft_id, present_value, private_options, read_record, record_ids,
    record_size, remove_record, remove_temporaries, replace_record, request_fingerprint, scan,
    storage_error, with_edit_receipt, ApplicationReceipt, ApplicationStatus, CreationReceipt,
    DraftLimits, Finish, NewEditReceipt, RecordEntry, StoredDraft, FORMAT,
};
use super::error::{AgentError, AgentResult, EXPIRED_DRAFT, PROJECT_UNAVAILABLE};
use super::review_record::{review_record, ReviewHistory, VisualReview, VisualReviewInput};

mod checks;

pub(super) use self::checks::require_retained_pages;
use self::checks::{
    active, check_key, check_proposal_size, find_creation, next_revision, no_uncertain_application,
    receipt_draft, replay_application, replay_creation, system_millis,
};

/// The mutation lock file of the storage folder.
const LOCK_FILE: &str = ".mutation-lock";
/// How long a mutation waits for another owner of the lock before it refuses (`deadline` in `drafts.ts`).
const LOCK_WAIT: Duration = Duration::from_secs(5);
/// The pause between two attempts to take the lock.
const LOCK_POLL: Duration = Duration::from_millis(10);
/// The most files one proposal holds (specification 43, §4).
const MAX_FILES: usize = 32;
/// The longest creation, request or application key, in UTF-16 units (`key.length > 200`).
const MAX_KEY_UNITS: usize = 200;

/// The clock of the store, in milliseconds since the epoch. Tests supply their own.
type Clock = Box<dyn Fn() -> i64 + Send + Sync>;

/// The lifecycle state of a draft (specification 43, §6). Any edit returns a draft to `Draft`. The
/// store records validation with the review, as the TypeScript store does, so `Validated` is never
/// written by this store; it is kept so that the lifecycle names are complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum DraftState {
    /// Created or edited; no review of its revision is recorded.
    #[default]
    Draft,
    /// Validated; part of the lifecycle, not written by this store.
    Validated,
    /// Reviewed at its current revision; it can be applied.
    Reviewed,
    /// Applied to the source. Only its receipts are kept.
    Applied,
}

impl DraftState {
    /// The state's name as the messages and the tool results spell it.
    #[must_use]
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Validated => "validated",
            Self::Reviewed => "reviewed",
            Self::Applied => "applied",
        }
    }
}

/// A draft: the proposal the agent composes and reviews (`Draft` in `drafts.ts`). `files` holds the
/// proposal's page texts; `base` holds the snapshots the pages were prepared against; `content_hashes`
/// is the SHA-256 of each text. `superseded` marks a replayed receipt whose revision later edits replaced:
/// it carries no page bodies.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Draft {
    /// The opaque draft identifier.
    pub(super) id: String,
    /// The workspace the draft belongs to.
    pub(super) workspace_id: String,
    /// The revision, counted from `1`; every edit adds one.
    pub(super) revision: String,
    /// The lifecycle state.
    pub(super) state: DraftState,
    /// The design intent of a composed proposal, as the composition module built it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub(super) intent: Option<Value>,
    /// The visual review of the current revision; an edit clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) visual_review: Option<VisualReview>,
    /// The number of visual reviews of the draft, the last one's attempt number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) repair_attempts: Option<u32>,
    /// The visual reviews that needed repair; an edit keeps this count.
    #[serde(default)]
    pub(super) unsuccessful_reviews: u32,
    /// The SHA-256 of each page text, by file name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) content_hashes: Option<BTreeMap<String, String>>,
    /// The project images the review examined, by file name and revision; an edit clears them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) reviewed_assets: Option<BTreeMap<String, String>>,
    /// The proposal's page texts, by project-relative file name.
    pub(super) files: BTreeMap<String, String>,
    /// The snapshots of the pages when the proposal was prepared.
    #[serde(default)]
    pub(super) base: Vec<DocumentSnapshot>,
    /// The findings the last review recorded.
    #[serde(default)]
    pub(super) findings: Vec<Value>,
    /// The program revision the proposal was composed against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) program_revision: Option<String>,
    /// Set only on a replayed receipt of a revision that later edits superseded.
    #[serde(default, skip_serializing_if = "is_false")]
    pub(super) superseded: bool,
}

/// What a creation request builds: the pages and their base, and the data its request returns. It is
/// built only for a new request (`createOnce`).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PreparedProposal {
    /// The snapshots of the pages the proposal was prepared against.
    pub(super) base: Vec<DocumentSnapshot>,
    /// The proposal's page texts.
    pub(super) files: BTreeMap<String, String>,
    /// The program revision the proposal was composed against.
    pub(super) program_revision: Option<String>,
    /// The design intent, as the composition module built it.
    pub(super) intent: Option<Value>,
    /// The data the creation request returns.
    pub(super) data: Option<Value>,
}

/// A created proposal and the data its creation request returns.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Created {
    /// The proposal, at revision `1`, or its replayed receipt.
    pub(super) draft: Draft,
    /// The data the creation request returned.
    pub(super) data: Option<Value>,
}

/// The page texts an edit commits, and the data the edit returns.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct EditResult {
    /// The proposal's page texts after the edit.
    pub(super) files: BTreeMap<String, String>,
    /// The data the edit returns.
    pub(super) data: Option<Value>,
}

/// The outcome of an edit request: the draft it committed, or its replayed receipt, and the data.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct EditOutcome {
    /// The draft at the committed revision, or the receipt's draft.
    pub(super) draft: Draft,
    /// The data the edit returned.
    pub(super) data: Option<Value>,
}

/// A draft as discovery lists it: its identity, revision, state and page names. It never holds page
/// bodies, base snapshots or finished proposals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DraftSummary {
    /// The draft identifier.
    pub(super) draft_id: String,
    /// The current revision.
    pub(super) revision: String,
    /// The lifecycle state.
    pub(super) state: DraftState,
    /// The names of the pages the proposal changes, sorted.
    pub(super) files: Vec<String>,
}

/// What a prepared application answers: the draft to apply, or the outcome its key already committed (AT-09).
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)] // One answer per application, matched at once: boxing the draft would buy nothing.
pub(super) enum PreparedApplication {
    /// The preflight passed and the application is recorded as prepared; the caller runs the commit.
    Ready(Draft),
    /// The key committed earlier; its original outcome is returned without another edit.
    Committed(Value),
}

/// Why a source commit did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CommitFailure {
    /// The source was certainly not changed. The application receipt is removed, so the same key can be
    /// retried (`NoSourceChangeError` in `applicationState.ts`).
    NoSourceChange(AgentError),
    /// The source may have changed. The receipt stays prepared and blocks replays until it is inspected.
    Uncertain(String),
}

/// The draft and revision a discard expects (`discard` in `drafts.ts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiscardExpectation {
    /// The workspace the caller works in.
    pub(super) workspace_id: String,
    /// The revision the caller discards.
    pub(super) revision: String,
}

/// A record that cannot be read. It is kept for inspection and counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UnreadableRecord {
    /// The draft identifier of the record.
    pub(super) id: String,
    /// Why it cannot be read, as its typed refusal.
    pub(super) reason: String,
}

/// What a cleanup removed, and which records it kept for inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CleanupReport {
    /// The identifiers of the finished records that were removed.
    pub(super) removed: Vec<String>,
    /// The records that cannot be read, kept for inspection.
    pub(super) unreadable: Vec<UnreadableRecord>,
}

/// The draft store of one workspace. It is shared by reference; every mutation takes the lock.
pub(super) struct DraftStore {
    /// The storage folder that holds the records.
    directory: PathBuf,
    /// The storage bounds.
    limits: DraftLimits,
    /// The clock that stamps finished records and decides retention.
    clock: Clock,
}

impl DraftStore {
    /// Opens the draft store in the storage folder `storage`, with the product bounds. The folder is
    /// created when it does not exist.
    ///
    /// # Errors
    ///
    /// `PROJECT_UNAVAILABLE` when the storage folder cannot be created.
    pub(super) fn open(storage: &Path) -> AgentResult<Self> {
        Self::open_with(storage, DraftLimits::PRODUCT, Box::new(system_millis))
    }

    /// Opens the store with explicit bounds and clock; tests use smaller bounds and a fixed clock.
    fn open_with(storage: &Path, limits: DraftLimits, clock: Clock) -> AgentResult<Self> {
        create_storage_dir(storage)?;
        Ok(Self {
            directory: storage.to_path_buf(),
            limits,
            clock,
        })
    }

    /// Creates the proposal of a creation request, or replays the original outcome of the same request.
    /// `input` is the request's JSON data; a replay is the same data in any field order. `build` produces
    /// the pages. It runs only for a new request, never for a replay, and it must not call this store.
    /// Replaying a discarded or applied proposal, or reusing the key with other input, is refused
    /// (`createOnce` in `drafts.ts`; specification 43, §6).
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a missing or long key; `CONFLICT` for a reused key with other input;
    /// `EXPIRED_DRAFT` for a replay of a discarded or applied proposal; `PROJECT_UNAVAILABLE` for another
    /// workspace's creation or a storage failure; `LIMIT_EXCEEDED` for the quota, for a proposal over its
    /// file bounds, or for a write that would pass the byte bound. The refusals of `build` are returned
    /// unchanged and nothing is stored.
    pub(super) fn create(
        &self,
        workspace_id: &str,
        key: &str,
        input: &Value,
        build: impl FnOnce() -> AgentResult<PreparedProposal>,
    ) -> AgentResult<Created> {
        self.exclusive(|| {
            check_key(key, "creation key and fingerprint are required.")?;
            let fingerprint = request_fingerprint(input);
            let entries = scan(&self.directory)?;
            if let Some((prior, receipt)) = find_creation(&entries, key) {
                return replay_creation(prior, receipt, workspace_id, &fingerprint);
            }
            self.room_for_draft(&entries)?;
            let PreparedProposal {
                base,
                files,
                program_revision,
                intent,
                data,
            } = build()?;
            let draft = Draft {
                id: new_draft_id()?,
                workspace_id: workspace_id.to_string(),
                revision: "1".to_string(),
                state: DraftState::Draft,
                intent,
                content_hashes: Some(content_hashes(&files)),
                files,
                base,
                program_revision,
                ..Draft::default()
            };
            // The receipt keeps the original data and hashes, never a second copy of the pages.
            let creation = CreationReceipt {
                key: key.to_string(),
                fingerprint,
                content_hashes: draft.content_hashes.clone(),
                data: data.clone(),
            };
            self.write(
                &StoredDraft {
                    format: FORMAT,
                    draft: draft.clone(),
                    discarded: false,
                    finished_at: None,
                    creation: Some(creation),
                    requests: BTreeMap::new(),
                    applications: BTreeMap::new(),
                },
                false,
            )?;
            Ok(Created { draft, data })
        })
    }

    /// The draft `id`, whole. An applied draft answers with its receipts only (no pages); a discarded
    /// draft is refused (`get` in `drafts.ts`).
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a malformed identifier or a corrupt record; `EXPIRED_DRAFT` when the draft
    /// is unavailable or discarded; `PROJECT_UNAVAILABLE` for a storage failure.
    pub(super) fn get(&self, id: &str) -> AgentResult<Draft> {
        let stored = read_record(&self.directory, id)?;
        active(&stored)?;
        Ok(stored.draft)
    }

    /// The open drafts of `workspace_id`: never discarded or applied, and never with their bodies
    /// (`list` in `drafts.ts`). It survives restarts and lists nothing from another workspace.
    ///
    /// # Errors
    ///
    /// `PROJECT_UNAVAILABLE` when the storage folder cannot be read.
    pub(super) fn list(&self, workspace_id: &str) -> AgentResult<Vec<DraftSummary>> {
        self.exclusive(|| {
            let entries = scan(&self.directory)?;
            Ok(entries
                .iter()
                .filter_map(|entry| entry.stored.as_ref().ok())
                .filter(|stored| !is_finished(stored) && stored.draft.workspace_id == workspace_id)
                .map(|stored| DraftSummary {
                    draft_id: stored.draft.id.clone(),
                    revision: stored.draft.revision.clone(),
                    state: stored.draft.state,
                    files: stored.draft.files.keys().cloned().collect(),
                })
                .collect())
        })
    }

    /// Commits one edit of the draft `id` made at `revision`, or replays the original receipt of the
    /// request `key`. `input` is the request's JSON data (its fingerprint). `update` computes the edited
    /// pages from the draft; it runs only for a new request, and it must not call this store. Any edit
    /// clears the findings, the review of the assets and the visual review, and it keeps the repair
    /// history (`editDetailed` in `drafts.ts`; specification 43, §6 and §8 repair 1).
    ///
    /// A retry of an edit that later edits superseded returns its receipt with `superseded` set and no
    /// page bodies. A receipt beyond the newest 64 is gone: its retry is a changed revision, which is
    /// `CONFLICT`, so it is never applied again.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a missing or long key; `CONFLICT` for a reused key with other input, a
    /// draft with an uncertain application, or a changed revision; `EXPIRED_DRAFT` for a discarded or
    /// applied draft, and for a draft whose pages are no longer kept; `LIMIT_EXCEEDED` for an oversized
    /// proposal or a write that would pass the byte bound. The refusals of `update` are returned unchanged
    /// and nothing is stored.
    pub(super) fn record_edit(
        &self,
        id: &str,
        revision: &str,
        key: &str,
        input: &Value,
        update: impl FnOnce(&Draft) -> AgentResult<EditResult>,
    ) -> AgentResult<EditOutcome> {
        self.exclusive(|| {
            check_key(key, "request key and fingerprint are required.")?;
            let fingerprint = request_fingerprint(input);
            let mut stored = read_record(&self.directory, id)?;
            active(&stored)?;
            if let Some(previous) = stored.requests.get(key) {
                if previous.fingerprint != fingerprint {
                    return Err(AgentError::conflict(
                        "request key already used with different input.",
                    ));
                }
                return Ok(EditOutcome {
                    draft: receipt_draft(&stored, &previous.revision, &previous.content_hashes),
                    data: previous.data.clone(),
                });
            }
            no_uncertain_application(&stored)?;
            require_retained_pages(&stored.draft)?;
            if stored.draft.revision != revision {
                return Err(AgentError::conflict("draft revision changed."));
            }
            let edit = update(&stored.draft)?;
            let hashes = content_hashes(&edit.files);
            let next = next_revision(&stored.draft.revision)?;
            stored.requests = with_edit_receipt(
                &stored.requests,
                key,
                NewEditReceipt {
                    fingerprint,
                    revision: next.clone(),
                    content_hashes: hashes.clone(),
                    data: edit.data.clone(),
                },
                self.limits.max_receipts,
            );
            stored.draft.files = edit.files;
            stored.draft.content_hashes = Some(hashes);
            stored.draft.revision = next;
            stored.draft.state = DraftState::Draft;
            stored.draft.findings.clear();
            stored.draft.reviewed_assets = None;
            stored.draft.visual_review = None;
            self.write(&stored, false)?;
            Ok(EditOutcome {
                draft: stored.draft.clone(),
                data: edit.data,
            })
        })
    }

    /// Records the review of the draft `id` at `revision`: its findings, the project images it examined,
    /// and, when `visual` is given, the visual review. The caller must have run validation and produced
    /// the previews first; a failure of either leaves the draft unreviewed. The state becomes `Reviewed`.
    /// Without a visual review the stored one is kept (`review` in `drafts.ts`).
    ///
    /// The third unsuccessful visual review requests the customer's decision: the store counts the
    /// unsuccessful reviews, not all reviews. The TypeScript store compared the attempt number instead;
    /// specification 43 §10.1 is followed here.
    ///
    /// # Errors
    ///
    /// `EXPIRED_DRAFT` for a discarded draft; `CONFLICT` for an applied draft, an uncertain application
    /// or a changed revision; `INVALID_ARGUMENT` for a blank reviewer or an accepted review with an issue;
    /// `PROJECT_UNAVAILABLE` or `LIMIT_EXCEEDED` for storage.
    pub(super) fn review(
        &self,
        id: &str,
        revision: &str,
        findings: Vec<Value>,
        reviewed_assets: BTreeMap<String, String>,
        visual: Option<&VisualReviewInput>,
    ) -> AgentResult<Draft> {
        self.exclusive(|| {
            let mut stored = read_record(&self.directory, id)?;
            active(&stored)?;
            if stored.draft.state == DraftState::Applied {
                return Err(AgentError::conflict(
                    "an applied proposal cannot become a new review.",
                ));
            }
            no_uncertain_application(&stored)?;
            if stored.draft.revision != revision {
                return Err(AgentError::conflict("draft revision changed."));
            }
            let history = ReviewHistory {
                attempts: stored.draft.repair_attempts.unwrap_or(0),
                unsuccessful: stored.draft.unsuccessful_reviews,
            };
            if let Some(input) = visual {
                let record = review_record(input, revision, history)?;
                stored.draft.unsuccessful_reviews = history.after(input.verdict).unsuccessful;
                stored.draft.repair_attempts = Some(record.attempt);
                stored.draft.visual_review = Some(record);
            }
            stored.draft.state = DraftState::Reviewed;
            stored.draft.findings = findings;
            stored.draft.reviewed_assets = Some(reviewed_assets);
            self.write(&stored, false)?;
            Ok(stored.draft.clone())
        })
    }

    /// The first half of an application (AT-09): under the mutation lock, the read-only preflight runs and the application is
    /// recorded as prepared before the editor transaction. A committed key answers its outcome. A refused preflight leaves no
    /// receipt (`apply` in `drafts.ts`; specification 43, §6 and §8 repair 1).
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a missing or long key; `CONFLICT` for a reused key with other input, a prior application that needs
    /// inspection, or a draft at another revision; `EXPIRED_DRAFT` for a discarded or applied draft; the refusal of `preflight`;
    /// `PROJECT_UNAVAILABLE` for storage.
    pub(super) fn prepare_application(
        &self,
        id: &str,
        revision: &str,
        key: &str,
        input: &Value,
        preflight: impl FnOnce(&Draft) -> AgentResult<()>,
    ) -> AgentResult<PreparedApplication> {
        self.exclusive(|| {
            check_key(key, "application key and fingerprint are required.")?;
            let fingerprint = request_fingerprint(input);
            let mut stored = read_record(&self.directory, id)?;
            active(&stored)?;
            if let Some(previous) = stored.applications.get(key) {
                return replay_application(previous, &fingerprint)
                    .map(PreparedApplication::Committed);
            }
            no_uncertain_application(&stored)?;
            require_retained_pages(&stored.draft)?;
            if stored.draft.revision != revision {
                return Err(AgentError::conflict(
                    "draft cannot be applied at this revision.",
                ));
            }
            preflight(&stored.draft)?;
            stored.applications.insert(
                key.to_string(),
                ApplicationReceipt {
                    fingerprint,
                    status: ApplicationStatus::Prepared,
                    outcome: None,
                },
            );
            self.write(&stored, true)?;
            Ok(PreparedApplication::Ready(stored.draft.clone()))
        })
    }

    /// The second half of an application (AT-09): records the outcome the extension reported after its editor transaction.
    /// Committed finishes the draft; a certain no-change removes the receipt so the key can be retried; an uncertain outcome keeps
    /// the receipt, which blocks replays until it is inspected.
    ///
    /// # Errors
    ///
    /// `CONFLICT` when no application of the key awaits an outcome, the key is reused with other input, the outcome is uncertain, or
    /// a committed receipt could not be saved; the refusal of a commit that is certain not to have changed the source, unchanged;
    /// `PROJECT_UNAVAILABLE` for storage.
    pub(super) fn record_application(
        &self,
        id: &str,
        key: &str,
        input: &Value,
        outcome: Result<Value, CommitFailure>,
    ) -> AgentResult<Value> {
        self.exclusive(|| {
            let fingerprint = request_fingerprint(input);
            let mut stored = read_record(&self.directory, id)?;
            let Some(prepared) = stored.applications.get(key) else {
                return Err(AgentError::conflict(
                    "no application of this key awaits an outcome.",
                ));
            };
            if prepared.status != ApplicationStatus::Prepared {
                return Err(AgentError::conflict(
                    "no application of this key awaits an outcome.",
                ));
            }
            if prepared.fingerprint != fingerprint {
                return Err(AgentError::conflict(
                    "application key reused with different input.",
                ));
            }
            match outcome {
                Ok(outcome) => {
                    stored.applications.insert(
                        key.to_string(),
                        ApplicationReceipt {
                            fingerprint,
                            status: ApplicationStatus::Committed,
                            outcome: Some(outcome.clone()),
                        },
                    );
                    let finished = finished_record(&stored, (self.clock)(), Finish::Applied);
                    self.write(&finished, true).map_err(|_| {
                        AgentError::conflict(
                            "source application completed but its receipt could not be saved; inspect the source before continuing.",
                        )
                    })?;
                    Ok(outcome)
                }
                Err(CommitFailure::NoSourceChange(error)) => {
                    stored.applications.remove(key);
                    self.write(&stored, true)?;
                    Err(error)
                }
                Err(CommitFailure::Uncertain(message)) => Err(AgentError::conflict(format!(
                    "source application outcome needs inspection: {message}"
                ))),
            }
        })
    }

    /// Applies the draft in one call, as the store's tests do; the production path splits it around the editor transaction through
    /// `prepare_application` and `record_application`.
    #[cfg(test)]
    pub(super) fn apply(
        &self,
        id: &str,
        revision: &str,
        key: &str,
        input: &Value,
        preflight: impl FnOnce(&Draft) -> AgentResult<()>,
        commit: impl FnOnce(&Draft) -> Result<Value, CommitFailure>,
    ) -> AgentResult<Value> {
        match self.prepare_application(id, revision, key, input, preflight)? {
            PreparedApplication::Committed(outcome) => Ok(outcome),
            PreparedApplication::Ready(draft) => {
                let outcome = commit(&draft);
                self.record_application(id, key, input, outcome)
            }
        }
    }

    /// Discards the draft `id`, keeping only its identity and creation key, so an old request cannot
    /// revive it. With `expected`, the workspace and the revision must match. A draft that is already
    /// discarded is left as it is. An applied draft is already finished and keeps its application receipts,
    /// so discarding it is a no-op; the TypeScript store dropped those receipts (specification 43, §8
    /// repair 1). Without `expected`, a draft that no longer exists is not an error (`discard` in
    /// `drafts.ts`).
    ///
    /// # Errors
    ///
    /// `PROJECT_UNAVAILABLE` for another workspace or a storage failure; `CONFLICT` for a changed revision;
    /// `EXPIRED_DRAFT` for a missing draft when `expected` is given; `INVALID_ARGUMENT` for a malformed or
    /// corrupt record.
    pub(super) fn discard(
        &self,
        id: &str,
        expected: Option<&DiscardExpectation>,
    ) -> AgentResult<()> {
        self.exclusive(|| {
            let stored = match read_record(&self.directory, id) {
                Ok(stored) => stored,
                Err(error) if expected.is_none() && error.code == EXPIRED_DRAFT => return Ok(()),
                Err(error) => return Err(error),
            };
            if let Some(expected) = expected {
                if stored.draft.workspace_id != expected.workspace_id {
                    return Err(AgentError::new(
                        PROJECT_UNAVAILABLE,
                        "draft belongs to another workspace.",
                    ));
                }
                if stored.draft.revision != expected.revision {
                    return Err(AgentError::conflict("draft changed before discard."));
                }
            }
            if stored.discarded || stored.draft.state == DraftState::Applied {
                return Ok(());
            }
            self.write(
                &finished_record(&stored, (self.clock)(), Finish::Discarded),
                true,
            )
        })
    }

    /// The storage maintenance that creation also runs: removes temporary files and the finished records
    /// past retention or beyond the count bound, and reports the records that cannot be read. It never
    /// removes an open draft or an unreadable record (specification 43, §6 and §8 repair 1).
    ///
    /// # Errors
    ///
    /// `PROJECT_UNAVAILABLE` when the storage folder or a record cannot be removed or read.
    pub(super) fn cleanup(&self) -> AgentResult<CleanupReport> {
        self.exclusive(|| {
            let entries = scan(&self.directory)?;
            let kept: Vec<String> = self
                .evict(&entries, 0, None)?
                .iter()
                .map(|entry| entry.id.clone())
                .collect();
            let removed = entries
                .iter()
                .filter(|entry| !kept.contains(&entry.id))
                .map(|entry| entry.id.clone())
                .collect();
            let unreadable = entries
                .iter()
                .filter_map(|entry| {
                    entry.stored.as_ref().err().map(|reason| UnreadableRecord {
                        id: entry.id.clone(),
                        reason: reason.clone(),
                    })
                })
                .collect();
            Ok(CleanupReport {
                removed,
                unreadable,
            })
        })
    }

    /// Runs `action` under the mutation lock, after the storage folder exists and the temporary files of
    /// any interrupted write are gone. Inside the lock no write is in progress.
    fn exclusive<T>(&self, action: impl FnOnce() -> AgentResult<T>) -> AgentResult<T> {
        create_storage_dir(&self.directory)?;
        let _lock = self.lock()?;
        remove_temporaries(&self.directory)?;
        action()
    }

    /// Takes the mutation lock: an operating-system file lock on `.mutation-lock`, held while the returned
    /// file is open. A crashed owner releases it, so no stale lock needs guessing (the TypeScript store
    /// checked a recorded process identifier for that). The lock file stays in place: removing it would
    /// let two owners lock different files.
    ///
    /// # Errors
    ///
    /// `CONFLICT` when another owner keeps the lock for the whole wait; `PROJECT_UNAVAILABLE` when the lock
    /// file cannot be opened or locked.
    fn lock(&self) -> AgentResult<fs::File> {
        let file = private_options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.directory.join(LOCK_FILE))
            .map_err(|error| storage_error("lock", LOCK_FILE, &error))?;
        let deadline = Instant::now() + LOCK_WAIT;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err(AgentError::conflict(
                            "draft storage is busy or its owner needs recovery.",
                        ));
                    }
                    thread::sleep(LOCK_POLL);
                }
                Err(TryLockError::Error(error)) => {
                    return Err(storage_error("lock", LOCK_FILE, &error));
                }
            }
        }
    }

    /// Makes room for one more open draft: finished records past retention or beyond the count bound
    /// leave first. Open drafts are never removed. When the open drafts are already at the quota, the
    /// refusal names each of them (`roomForDraft` in `drafts.ts`).
    ///
    /// # Errors
    ///
    /// `LIMIT_EXCEEDED` at the quota; `PROJECT_UNAVAILABLE` when a record cannot be removed.
    fn room_for_draft(&self, entries: &[RecordEntry]) -> AgentResult<()> {
        let remaining = self.evict(entries, 0, None)?;
        let open: Vec<&StoredDraft> = remaining
            .iter()
            .filter_map(|entry| entry.stored.as_ref().ok())
            .filter(|stored| !is_finished(stored))
            .collect();
        if open.len() >= self.limits.max_open {
            return Err(AgentError::limit(format!(
                "{} open proposals already exist: {}. Discard one that is no longer needed (hmi_apply_draft with action \"discard\"); open proposals are never removed automatically.",
                open.len(),
                describe_open(&open)
            )));
        }
        Ok(())
    }

    /// Removes the finished records that the eviction plan selects, and returns the entries that remain.
    ///
    /// # Errors
    ///
    /// `PROJECT_UNAVAILABLE` when a record cannot be removed.
    fn evict<'entries>(
        &self,
        entries: &'entries [RecordEntry],
        need_bytes: u64,
        keep: Option<&str>,
    ) -> AgentResult<Vec<&'entries RecordEntry>> {
        let plan = eviction_plan(entries, &self.limits, (self.clock)(), need_bytes, keep);
        let mut removed: Vec<&str> = Vec::with_capacity(plan.len());
        for entry in plan {
            remove_record(&self.directory, &entry.id)?;
            removed.push(entry.id.as_str());
        }
        Ok(entries
            .iter()
            .filter(|entry| !removed.contains(&entry.id.as_str()))
            .collect())
    }

    /// Stores one record atomically. A write that would pass the byte bound first removes finished records,
    /// oldest first. Only a write that still passes the bound with open drafts alone is refused, and the
    /// refusal names those drafts. A `settle` write (a discard or an application receipt) is never refused
    /// for space, so storage can always be cleaned up (`write` in `drafts.ts`).
    ///
    /// # Errors
    ///
    /// `LIMIT_EXCEEDED` for an oversized proposal or a refused write; `PROJECT_UNAVAILABLE` for storage.
    fn write(&self, stored: &StoredDraft, settle: bool) -> AgentResult<()> {
        check_proposal_size(&stored.draft.files)?;
        let data = serde_json::to_vec(stored).map_err(|error| {
            AgentError::new(
                PROJECT_UNAVAILABLE,
                format!("draft record cannot be encoded: {error}"),
            )
        })?;
        let id = stored.draft.id.as_str();
        let own = u64::try_from(data.len()).unwrap_or(u64::MAX);
        let mut bytes = own;
        for other_id in record_ids(&self.directory)? {
            if other_id != id {
                bytes = bytes.saturating_add(record_size(&self.directory, &other_id)?);
            }
        }
        if bytes > self.limits.max_bytes {
            let entries = scan(&self.directory)?;
            let remaining = self.evict(&entries, bytes - self.limits.max_bytes, Some(id))?;
            let others: Vec<&RecordEntry> = remaining
                .into_iter()
                .filter(|entry| entry.id != id)
                .collect();
            bytes = others
                .iter()
                .fold(own, |total, entry| total.saturating_add(entry.size));
            if !settle && bytes > self.limits.max_bytes {
                return Err(self.storage_full(&others));
            }
        }
        replace_record(&self.directory, id, &data)
    }

    /// The refusal of a write that open drafts alone keep above the byte bound. It names each open draft
    /// and the unreadable records, so the customer can discard one (`write` in `drafts.ts`).
    #[must_use]
    fn storage_full(&self, others: &[&RecordEntry]) -> AgentError {
        let open: Vec<&StoredDraft> = others
            .iter()
            .filter_map(|entry| entry.stored.as_ref().ok())
            .filter(|stored| !is_finished(stored))
            .collect();
        let unreadable: Vec<&str> = others
            .iter()
            .filter(|entry| entry.stored.is_err())
            .map(|entry| entry.id.as_str())
            .collect();
        let open_text = if open.is_empty() {
            "none".to_string()
        } else {
            describe_open(&open)
        };
        let mut message = format!(
            "draft storage would exceed {}; open proposals: {open_text}. ",
            mebibytes(self.limits.max_bytes)
        );
        if !unreadable.is_empty() {
            message.push_str(&format!(
                "Unreadable records kept for inspection: {}. ",
                unreadable.join(", ")
            ));
        }
        message.push_str("Discard a proposal that is no longer needed or submit a smaller one.");
        AgentError::limit(message)
    }
}

#[cfg(test)]
#[path = "drafts_tests.rs"]
mod tests;
