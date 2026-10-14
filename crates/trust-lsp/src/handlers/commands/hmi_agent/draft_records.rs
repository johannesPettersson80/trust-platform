//! The persisted draft records (specification 43, §6, and the repairs of 7 October 2026, §8 item 1):
//! the record format and its receipts, the file of each record, the bounds that decide which finished
//! records leave storage, and the reading and atomic writing of record files.
//!
//! Port of `editors/vscode/src/hmiBuilder/agent/draftRecords.ts`, and of the file operations of
//! `drafts.ts` (`file`, `read`, `records`, `scan`, `size`, `removeTemporaries`). The lifecycle of a
//! draft is in `drafts`; this module knows records, not transitions.
//!
//! On-disk layout: one `<draft id>.json` record per draft in the storage folder. A record is written
//! to a temporary `<draft id>.json.<random>.tmp` file that is renamed over it. The mutation lock
//! `.mutation-lock` lives beside the records. Unix modes: the folder 0700, the files 0600.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::documents::sha256_hex;
use super::drafts::{Draft, DraftState};
use super::error::{AgentError, AgentResult, EXPIRED_DRAFT, INVALID_ARGUMENT, PROJECT_UNAVAILABLE};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

/// The record format this engine writes and reads. Records of any other format are not read.
pub(super) const FORMAT: u64 = 2;

/// The storage bounds of one workspace (specification 43, §4, §6 and §8 repair 1). The product values
/// are [`DraftLimits::PRODUCT`]; tests may use smaller ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DraftLimits {
    /// Open drafts (neither applied nor discarded). Five at most; they are never removed automatically.
    pub(super) max_open: usize,
    /// Finished records (applied or discarded) kept as retry receipts; the oldest leave first: 64.
    pub(super) max_finished: usize,
    /// The bytes of all records together: 50 MiB (specification 43, §4).
    pub(super) max_bytes: u64,
    /// The edit receipts kept per draft; the oldest leave first: 64 (specification 43, §8 repair 1).
    pub(super) max_receipts: usize,
    /// How long a finished record is kept as a retry receipt, in milliseconds: seven days
    /// (specification 43, §8 repair 1).
    pub(super) retention_ms: i64,
}

impl DraftLimits {
    /// The product bounds (specification 43, §4, §6 and §8 repair 1).
    pub(super) const PRODUCT: Self = Self {
        max_open: 5,
        max_finished: 64,
        max_bytes: 50 * 1024 * 1024,
        max_receipts: 64,
        retention_ms: 7 * 24 * 60 * 60 * 1_000,
    };
}

/// The receipt of one edit: its committed revision and content hashes, and the data the edit returned.
/// It never holds page bodies (specification 43, §8 repair 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EditReceipt {
    /// The fingerprint of the request (see [`request_fingerprint`]).
    pub(super) fingerprint: String,
    /// The revision the edit committed.
    pub(super) revision: String,
    /// The content hashes of the pages at that revision.
    #[serde(default)]
    pub(super) content_hashes: BTreeMap<String, String>,
    /// The data the edit returned; a retry returns it unchanged.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub(super) data: Option<Value>,
    /// The order of commitment, for removing the oldest receipt first.
    pub(super) sequence: u64,
}

/// An edit receipt before its order of commitment is known (see [`with_edit_receipt`]).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct NewEditReceipt {
    /// The fingerprint of the request.
    pub(super) fingerprint: String,
    /// The revision the edit committed.
    pub(super) revision: String,
    /// The content hashes of the pages at that revision.
    pub(super) content_hashes: BTreeMap<String, String>,
    /// The data the edit returned.
    pub(super) data: Option<Value>,
}

/// The receipt of a creation: its key and the fingerprint of its request. While the original outcome can
/// still be returned, it also holds the content hashes and the data (specification 43, §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreationReceipt {
    /// The caller's creation key.
    pub(super) key: String,
    /// The fingerprint of the creation request.
    pub(super) fingerprint: String,
    /// The content hashes of the created pages, while the outcome can still be returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) content_hashes: Option<BTreeMap<String, String>>,
    /// The data of the created proposal, while the outcome can still be returned.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub(super) data: Option<Value>,
}

/// Whether an application is still being prepared, or the source commit succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum ApplicationStatus {
    /// The application started; its source outcome is not known yet.
    Prepared,
    /// The source commit succeeded; its outcome is replayed to every retry.
    Committed,
}

/// The receipt of an application, kept under its key (specification 43, §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ApplicationReceipt {
    /// The fingerprint of the application request.
    pub(super) fingerprint: String,
    /// Prepared or committed.
    pub(super) status: ApplicationStatus,
    /// The outcome of a committed application.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_value"
    )]
    pub(super) outcome: Option<Value>,
}

/// One draft record as stored (specification 43, §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StoredDraft {
    /// The record format: [`FORMAT`].
    pub(super) format: u64,
    /// The draft: its identity, revision, state, pages, base and findings.
    pub(super) draft: Draft,
    /// Whether the draft was discarded.
    #[serde(default)]
    pub(super) discarded: bool,
    /// When the draft was applied or discarded, in milliseconds since the epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) finished_at: Option<i64>,
    /// The creation receipt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) creation: Option<CreationReceipt>,
    /// The edit receipts, by idempotency key.
    #[serde(default)]
    pub(super) requests: BTreeMap<String, EditReceipt>,
    /// The application receipts, by idempotency key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) applications: BTreeMap<String, ApplicationReceipt>,
}

/// One record file as found in storage. An unreadable record is kept with its reason: it is retained
/// for inspection and counted, but it blocks neither discovery nor creation.
#[derive(Debug, Clone)]
pub(super) struct RecordEntry {
    /// The draft identifier, which is also the record's file name.
    pub(super) id: String,
    /// The size of the record file in bytes.
    pub(super) size: u64,
    /// The modification time of the record file, in milliseconds since the epoch.
    pub(super) modified: i64,
    /// The readable record, or the refusal that makes it unreadable.
    pub(super) stored: Result<StoredDraft, String>,
}

/// How a draft finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Finish {
    /// Applied to the source; the record keeps its application receipts.
    Applied,
    /// Discarded; the record keeps only its identity and creation key.
    Discarded,
}

/// The SHA-256 of each proposal file's text, as lowercase hexadecimal, by file name (`contentHashes`).
#[must_use]
pub(super) fn content_hashes(files: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    files
        .iter()
        .map(|(file, body)| (file.clone(), sha256_hex(body.as_bytes())))
        .collect()
}

/// The fingerprint of a request: the SHA-256 of its canonical JSON, so the order of its fields does not
/// matter (`requestFingerprint` in `service.ts`).
#[must_use]
pub(super) fn request_fingerprint(input: &Value) -> String {
    sha256_hex(canonical_json(input).as_bytes())
}

/// The canonical text of a JSON value: compact, with every object's keys sorted by their UTF-8 bytes,
/// recursively. Array order is kept.
#[must_use]
pub(super) fn canonical_json(value: &Value) -> String {
    let mut text = String::new();
    write_canonical(value, &mut text);
    text
}

/// Appends the canonical text of `value` to `out`.
fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(fields) => {
            let mut sorted: Vec<(&String, &Value)> = fields.iter().collect();
            sorted.sort_unstable_by_key(|(key, _)| *key);
            out.push('{');
            for (index, (key, field)) in sorted.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(field, out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

/// Whether `id` has the shape of a draft identifier: lowercase hexadecimal in the groups 8-4-4-4-12.
/// Draft identifiers are opaque; the file name of a record is always one of them.
#[must_use]
pub(super) fn is_draft_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8_usize, 4, 4, 4, 12])
            .all(|(part, length)| {
                part.len() == length
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
}

/// A new draft identifier: a random version-4 UUID in lowercase hexadecimal (`randomUUID` in
/// `drafts.ts`). It never comes from request text.
///
/// # Errors
///
/// `PROJECT_UNAVAILABLE` when the operating system has no random source.
pub(super) fn new_draft_id() -> AgentResult<String> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| {
        AgentError::new(
            PROJECT_UNAVAILABLE,
            format!("no random source for a draft identifier: {error}"),
        )
    })?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut id = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            id.push('-');
        }
        id.push(char::from(DIGITS[usize::from(*byte >> 4)]));
        id.push(char::from(DIGITS[usize::from(*byte & 0x0f)]));
    }
    Ok(id)
}

/// Whether a stored draft is finished: applied, or discarded (`isFinished` in `draftRecords.ts`).
#[must_use]
pub(super) fn is_finished(stored: &StoredDraft) -> bool {
    stored.discarded || stored.draft.state == DraftState::Applied
}

/// The record that remains of a finished draft (`finishedRecord` in `draftRecords.ts`). It keeps the
/// identity and the creation key, so an old retry cannot revive or rebuild the draft. An applied draft
/// also keeps its application receipts, so an apply retry still returns the original outcome. It holds
/// no page bodies, base snapshots or edit receipts.
#[must_use]
pub(super) fn finished_record(stored: &StoredDraft, now: i64, finish: Finish) -> StoredDraft {
    let applied = finish == Finish::Applied;
    let draft = &stored.draft;
    StoredDraft {
        format: FORMAT,
        draft: Draft {
            id: draft.id.clone(),
            workspace_id: draft.workspace_id.clone(),
            revision: draft.revision.clone(),
            state: if applied {
                DraftState::Applied
            } else {
                draft.state
            },
            content_hashes: Some(BTreeMap::new()),
            ..Draft::default()
        },
        discarded: !applied,
        finished_at: Some(now),
        creation: stored.creation.as_ref().map(|creation| CreationReceipt {
            key: creation.key.clone(),
            fingerprint: creation.fingerprint.clone(),
            content_hashes: None,
            data: None,
        }),
        requests: BTreeMap::new(),
        applications: if applied {
            stored.applications.clone()
        } else {
            BTreeMap::new()
        },
    }
}

/// The edit receipts with `receipt` added under `key`, keeping only the newest `max_receipts` of all
/// (specification 43, §8 repair 1; `withEditReceipt` in `draftRecords.ts`).
#[must_use]
pub(super) fn with_edit_receipt(
    requests: &BTreeMap<String, EditReceipt>,
    key: &str,
    receipt: NewEditReceipt,
    max_receipts: usize,
) -> BTreeMap<String, EditReceipt> {
    let mut entries: Vec<(&String, &EditReceipt)> = requests.iter().collect();
    entries.sort_by_key(|(_, entry)| entry.sequence);
    let keep = entries.len().saturating_sub(max_receipts.saturating_sub(1));
    let sequence = entries
        .last()
        .map_or(0, |(_, last)| last.sequence.saturating_add(1));
    let mut next: BTreeMap<String, EditReceipt> = entries
        .iter()
        .skip(keep)
        .map(|(name, entry)| ((*name).clone(), (*entry).clone()))
        .collect();
    next.insert(
        key.to_string(),
        EditReceipt {
            fingerprint: receipt.fingerprint,
            revision: receipt.revision,
            content_hashes: receipt.content_hashes,
            data: receipt.data,
            sequence,
        },
    );
    next
}

/// The finished records to remove, oldest first: those past retention, those beyond the count bound,
/// then as many more as `need_bytes` requires. Open records, unreadable records and `keep` never appear
/// (`evictionPlan` in `draftRecords.ts`).
#[must_use]
pub(super) fn eviction_plan<'entries>(
    entries: &'entries [RecordEntry],
    limits: &DraftLimits,
    now: i64,
    need_bytes: u64,
    keep: Option<&str>,
) -> Vec<&'entries RecordEntry> {
    let mut finished: Vec<&RecordEntry> = entries
        .iter()
        .filter(|entry| {
            keep != Some(entry.id.as_str()) && entry.stored.as_ref().is_ok_and(is_finished)
        })
        .collect();
    finished.sort_by(|first, second| {
        finished_time(first)
            .cmp(&finished_time(second))
            .then_with(|| first.id.cmp(&second.id))
    });
    let total = finished.len();
    let mut freed: u64 = 0;
    let mut plan = Vec::new();
    for (index, entry) in finished.into_iter().enumerate() {
        let expired = now.saturating_sub(finished_time(entry)) >= limits.retention_ms;
        let surplus = total - index > limits.max_finished;
        if expired || surplus || freed < need_bytes {
            freed = freed.saturating_add(entry.size);
            plan.push(entry);
        }
    }
    plan
}

/// The open drafts as a person can act on them: identity, pages, state and revision
/// (`describeOpen` in `draftRecords.ts`).
#[must_use]
pub(super) fn describe_open(open: &[&StoredDraft]) -> String {
    open.iter()
        .map(|stored| {
            let draft = &stored.draft;
            let pages: Vec<&str> = draft.files.keys().map(String::as_str).collect();
            let pages = if pages.is_empty() {
                "no pages".to_string()
            } else {
                pages.join(", ")
            };
            format!(
                "{} ({pages}; {}; revision {})",
                draft.id,
                draft.state.label(),
                draft.revision
            )
        })
        .collect::<Vec<String>>()
        .join("; ")
}

/// A byte count in mebibytes as the messages print it, rounded to two places and without trailing
/// zeros: `50 MiB`, `1.5 MiB`, `0.15 MiB` (`mebibytes` in `drafts.ts`).
#[must_use]
pub(super) fn mebibytes(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    let hundredths = bytes.saturating_mul(100).saturating_add(MIB / 2) / MIB;
    let whole = hundredths / 100;
    let fraction = hundredths % 100;
    if fraction == 0 {
        format!("{whole} MiB")
    } else if fraction.is_multiple_of(10) {
        format!("{whole}.{} MiB", fraction / 10)
    } else {
        format!("{whole}.{fraction:02} MiB")
    }
}

/// Reads a JSON value that may be `null`. A plain `Option<Value>` field would read `null` as absent.
pub(super) fn present_value<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    Value::deserialize(deserializer).map(Some)
}

/// Whether a flag is false; used to leave a false flag out of a stored record.
#[allow(clippy::trivially_copy_pass_by_ref)]
#[must_use]
pub(super) fn is_false(value: &bool) -> bool {
    !*value
}

/// The refusal of a record that cannot be read: its JSON is malformed, its shape or format is unknown,
/// or its proposal texts do not match their stored hashes (specification 43, §6).
#[must_use]
pub(super) fn corrupt_record() -> AgentError {
    AgentError::invalid(
        "unsupported or corrupt draft record; retain it for inspection and recover from the source project.",
    )
}

/// The refusal of a file operation on draft storage. The message names the file, not the path.
#[must_use]
pub(super) fn storage_error(action: &str, name: &str, error: &io::Error) -> AgentError {
    AgentError::new(
        PROJECT_UNAVAILABLE,
        format!("draft storage cannot {action} {name}: {error}"),
    )
}

/// Reads and verifies one record: the JSON parses, the format is [`FORMAT`], the record names its own
/// identifier, and the stored content hashes match the proposal texts (`read` in `drafts.ts`). The
/// hashes are recomputed from the texts, so a record never exposes damaged proposal text.
///
/// # Errors
///
/// `EXPIRED_DRAFT` when the record does not exist. `INVALID_ARGUMENT` for a malformed identifier, a
/// record of an earlier format (which is not read, but reported as draft storage to clean up), or a
/// corrupt record. `PROJECT_UNAVAILABLE` when the file cannot be read.
pub(super) fn read_record(dir: &Path, id: &str) -> AgentResult<StoredDraft> {
    let path = record_path(dir, id)?;
    let raw = match fs::read(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(AgentError::new(EXPIRED_DRAFT, "draft is unavailable."));
        }
        Err(error) => return Err(storage_error("read", &format!("{id}.json"), &error)),
    };
    let value: Value = serde_json::from_slice(&raw).map_err(|_| corrupt_record())?;
    match value.get("format").and_then(Value::as_u64) {
        Some(FORMAT) => {}
        Some(1) => {
            return Err(AgentError::invalid(format!(
                "draft record {id}.json has an earlier format and is not read; it is draft storage to clean up."
            )));
        }
        _ => return Err(corrupt_record()),
    }
    let mut stored: StoredDraft = serde_json::from_value(value).map_err(|_| corrupt_record())?;
    if stored.draft.id != id {
        return Err(corrupt_record());
    }
    let hashes = content_hashes(&stored.draft.files);
    if stored
        .draft
        .content_hashes
        .as_ref()
        .is_some_and(|saved| *saved != hashes)
    {
        return Err(corrupt_record());
    }
    stored.draft.content_hashes = Some(hashes);
    Ok(stored)
}

/// Every record of `dir` with its size, age and readability, in identifier order. A record that vanishes
/// while it is read is skipped; an unreadable record is kept with its reason (`scan` in `drafts.ts`).
///
/// # Errors
///
/// `PROJECT_UNAVAILABLE` when the folder or a record file cannot be inspected.
pub(super) fn scan(dir: &Path) -> AgentResult<Vec<RecordEntry>> {
    let mut entries = Vec::new();
    for id in record_ids(dir)? {
        let name = format!("{id}.json");
        let metadata = match fs::metadata(dir.join(&name)) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(storage_error("read", &name, &error)),
        };
        let stored = match read_record(dir, &id) {
            Ok(stored) => Ok(stored),
            Err(error) if error.code == EXPIRED_DRAFT => continue,
            Err(error) if error.code == INVALID_ARGUMENT => Err(error.to_string()),
            Err(error) => return Err(error),
        };
        entries.push(RecordEntry {
            id,
            size: metadata.len(),
            modified: modified_millis(&metadata),
            stored,
        });
    }
    Ok(entries)
}

/// Replaces the record `id` with `bytes`: the bytes go to a new temporary file, which is renamed over
/// the record. A reader therefore sees the old record or the new one, never part of either (`write` in
/// `drafts.ts`). The temporary file is removed when the write fails.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for a malformed identifier; `PROJECT_UNAVAILABLE` when the record cannot be
/// written or renamed.
pub(super) fn replace_record(dir: &Path, id: &str, bytes: &[u8]) -> AgentResult<()> {
    let target = record_path(dir, id)?;
    let temporary = dir.join(format!("{id}.json.{}.tmp", new_draft_id()?));
    let written = write_new_file(&temporary, bytes).and_then(|()| fs::rename(&temporary, target));
    if let Err(error) = written {
        let _ = fs::remove_file(temporary);
        return Err(storage_error("write", &format!("{id}.json"), &error));
    }
    Ok(())
}

/// Removes the record `id`. A record that is already gone counts as removed.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for a malformed identifier; `PROJECT_UNAVAILABLE` when the file cannot be removed.
pub(super) fn remove_record(dir: &Path, id: &str) -> AgentResult<()> {
    let name = format!("{id}.json");
    ignore_missing(fs::remove_file(record_path(dir, id)?))
        .map_err(|error| storage_error("remove", &name, &error))
}

/// Removes the temporary files of interrupted writes. Call it only under the mutation lock: no write is
/// in progress there, so every temporary file belongs to a write that crashed (`removeTemporaries` in
/// `drafts.ts`).
///
/// # Errors
///
/// `PROJECT_UNAVAILABLE` when a temporary file cannot be removed.
pub(super) fn remove_temporaries(dir: &Path) -> AgentResult<()> {
    for name in file_names(dir)? {
        if name.ends_with(".tmp") {
            ignore_missing(fs::remove_file(dir.join(&name)))
                .map_err(|error| storage_error("remove", &name, &error))?;
        }
    }
    Ok(())
}

/// Creates the storage folder and its missing parents, private to the owner on Unix (mode 0700).
///
/// # Errors
///
/// `PROJECT_UNAVAILABLE` when the folder cannot be created.
pub(super) fn create_storage_dir(dir: &Path) -> AgentResult<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(dir)
        .map_err(|error| storage_error("create", "the storage folder", &error))
}

/// The path of the record `id`, after the identifier is checked (`file` in `drafts.ts`).
///
/// # Errors
///
/// `INVALID_ARGUMENT` when `id` is not a draft identifier, so no file name can come from request text.
fn record_path(dir: &Path, id: &str) -> AgentResult<PathBuf> {
    if !is_draft_id(id) {
        return Err(AgentError::invalid("invalid draft id."));
    }
    Ok(dir.join(format!("{id}.json")))
}

/// The identifiers of the record files in `dir`, sorted. A record name is 36 characters from `0-9`,
/// `a-f` and `-`, then `.json`; the identifier itself is checked when the record is read.
pub(super) fn record_ids(dir: &Path) -> AgentResult<Vec<String>> {
    let mut ids: Vec<String> = file_names(dir)?
        .into_iter()
        .filter_map(|name| name.strip_suffix(".json").map(str::to_owned))
        .filter(|id| {
            id.len() == 36
                && id.bytes().all(|byte| {
                    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) || byte == b'-'
                })
        })
        .collect();
    ids.sort();
    Ok(ids)
}

/// The size of the record `id`; a record that is already gone holds no bytes.
///
/// # Errors
///
/// `PROJECT_UNAVAILABLE` when the record cannot be inspected.
pub(super) fn record_size(dir: &Path, id: &str) -> AgentResult<u64> {
    let name = format!("{id}.json");
    match fs::metadata(dir.join(&name)) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(storage_error("read", &name, &error)),
    }
}

/// The names of the files in `dir`; a folder that does not exist has none.
fn file_names(dir: &Path) -> AgentResult<Vec<String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(storage_error("list", "the storage folder", &error)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| storage_error("list", "the storage folder", &error))?;
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
        }
    }
    Ok(names)
}

/// The modification time of a file in milliseconds since the epoch; zero when the platform has none.
fn modified_millis(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

/// The finishing time of a finished record, or the file's modification time when the record has none.
fn finished_time(entry: &RecordEntry) -> i64 {
    entry
        .stored
        .as_ref()
        .ok()
        .and_then(|stored| stored.finished_at)
        .unwrap_or(entry.modified)
}

/// Writes a new file that must not exist yet, readable and writable by its owner only on Unix, and
/// flushes it to disk before returning.
fn write_new_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = private_options().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Options for a new private file (mode 0600 on Unix).
pub(super) fn private_options() -> OpenOptions {
    #[cfg(unix)]
    {
        let mut options = OpenOptions::new();
        options.mode(0o600);
        options
    }
    #[cfg(not(unix))]
    {
        OpenOptions::new()
    }
}

/// Treats an absent file as removed.
pub(super) fn ignore_missing(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
#[path = "draft_records_tests.rs"]
mod tests;
