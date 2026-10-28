//! The checks and receipt replays of the draft store (specification 43, §4 to §8).

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::super::documents::MAX_FILE_BYTES;
use super::super::draft_records::{
    corrupt_record, ApplicationReceipt, ApplicationStatus, CreationReceipt, RecordEntry,
    StoredDraft,
};
use super::super::error::{AgentError, AgentResult, EXPIRED_DRAFT, PROJECT_UNAVAILABLE};
use super::{Created, Draft, DraftState, MAX_FILES, MAX_KEY_UNITS};

/// Refuses an operation that needs an applied draft's pages: an applied draft keeps only its receipts
/// (specification 43, §8 repair 1; `requireRetainedPages` in `drafts.ts`).
///
/// # Errors
///
/// `EXPIRED_DRAFT` when the draft is applied.
pub(in super::super) fn require_retained_pages(draft: &Draft) -> AgentResult<()> {
    if draft.state == DraftState::Applied {
        return Err(AgentError::new(
            EXPIRED_DRAFT,
            "this proposal was applied and its private pages are no longer kept; inspect the source page or compose a new proposal.",
        ));
    }
    Ok(())
}

/// Refuses a discarded record (`active` in `drafts.ts`).
pub(super) fn active(stored: &StoredDraft) -> AgentResult<()> {
    if stored.discarded {
        return Err(AgentError::new(
            EXPIRED_DRAFT,
            "this proposal was discarded.",
        ));
    }
    Ok(())
}

/// Refuses a draft that has an application whose source outcome needs inspection: no edit, review or
/// other key may bypass it (`noUncertainApplication` in `drafts.ts`).
pub(super) fn no_uncertain_application(stored: &StoredDraft) -> AgentResult<()> {
    if stored
        .applications
        .values()
        .any(|application| application.status == ApplicationStatus::Prepared)
    {
        return Err(AgentError::conflict(
            "prior application outcome needs inspection; edit, review and another key cannot bypass it.",
        ));
    }
    Ok(())
}

/// Checks a request key: present, and at most 200 UTF-16 units long (`createOnce` in `drafts.ts`).
///
/// # Errors
///
/// `INVALID_ARGUMENT` with `refusal` when the key is empty or too long.
pub(super) fn check_key(key: &str, refusal: &'static str) -> AgentResult<()> {
    if key.is_empty() || key.encode_utf16().count() > MAX_KEY_UNITS {
        return Err(AgentError::invalid(refusal));
    }
    Ok(())
}

/// Checks the size of a proposal: at most 32 files, each at most 1 MiB (specification 43, §4).
///
/// # Errors
///
/// `LIMIT_EXCEEDED` when a bound is passed.
pub(super) fn check_proposal_size(files: &BTreeMap<String, String>) -> AgentResult<()> {
    let oversized = files
        .values()
        .any(|text| u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES);
    if files.len() > MAX_FILES || oversized {
        return Err(AgentError::limit(
            "at most 32 files, each no larger than 1 MiB.",
        ));
    }
    Ok(())
}

/// The record of a creation with `key`, when one exists (`createOnce` in `drafts.ts`).
pub(super) fn find_creation<'entries>(
    entries: &'entries [RecordEntry],
    key: &str,
) -> Option<(&'entries StoredDraft, &'entries CreationReceipt)> {
    entries
        .iter()
        .filter_map(|entry| entry.stored.as_ref().ok())
        .find_map(|stored| {
            stored
                .creation
                .as_ref()
                .filter(|receipt| receipt.key == key)
                .map(|receipt| (stored, receipt))
        })
}

/// The outcome of a repeated creation request: the original proposal while it is kept, or the reason it
/// is no longer available (`createOnce` in `drafts.ts`).
///
/// # Errors
///
/// `PROJECT_UNAVAILABLE` for another workspace; `CONFLICT` for other input; `EXPIRED_DRAFT` for a discarded
/// or applied proposal, or a receipt without its hashes.
pub(super) fn replay_creation(
    prior: &StoredDraft,
    receipt: &CreationReceipt,
    workspace_id: &str,
    fingerprint: &str,
) -> AgentResult<Created> {
    if prior.draft.workspace_id != workspace_id {
        return Err(AgentError::new(
            PROJECT_UNAVAILABLE,
            "creation belongs to another workspace.",
        ));
    }
    if receipt.fingerprint != fingerprint {
        return Err(AgentError::conflict(
            "creation key reused with different input.",
        ));
    }
    active(prior)?;
    if prior.draft.state == DraftState::Applied {
        return Err(AgentError::new(
            EXPIRED_DRAFT,
            "this proposal was already applied; its private pages are no longer kept.",
        ));
    }
    let Some(hashes) = receipt.content_hashes.as_ref() else {
        return Err(AgentError::new(
            EXPIRED_DRAFT,
            "original creation receipt is unavailable.",
        ));
    };
    Ok(Created {
        draft: receipt_draft(prior, "1", hashes),
        data: receipt.data.clone(),
    })
}

/// The draft a replayed receipt returns: the current draft whole while `revision` is still current, or,
/// for a revision that later edits superseded, its identity, revision and content hashes without page
/// bodies, marked `superseded` (`receiptDraft` in `drafts.ts`; specification 43, §8 repair 1).
pub(super) fn receipt_draft(
    stored: &StoredDraft,
    revision: &str,
    hashes: &BTreeMap<String, String>,
) -> Draft {
    if revision == stored.draft.revision {
        return stored.draft.clone();
    }
    Draft {
        id: stored.draft.id.clone(),
        workspace_id: stored.draft.workspace_id.clone(),
        revision: revision.to_string(),
        state: DraftState::Draft,
        intent: stored.draft.intent.clone(),
        content_hashes: Some(hashes.clone()),
        program_revision: stored.draft.program_revision.clone(),
        superseded: true,
        ..Draft::default()
    }
}

/// The outcome of a repeated application request: the committed outcome, or a refusal when the request
/// was made with other input, or its outcome is not known (`apply` in `drafts.ts`).
///
/// # Errors
///
/// `CONFLICT` for other input or a prepared application.
pub(super) fn replay_application(
    previous: &ApplicationReceipt,
    fingerprint: &str,
) -> AgentResult<Value> {
    if previous.fingerprint != fingerprint {
        return Err(AgentError::conflict(
            "application key reused with different input.",
        ));
    }
    if previous.status != ApplicationStatus::Committed {
        return Err(AgentError::conflict(
            "prior application outcome needs inspection; it will not be replayed.",
        ));
    }
    Ok(previous.outcome.clone().unwrap_or(Value::Null))
}

/// The revision after `revision`: its number plus one, as the TypeScript store wrote it.
///
/// # Errors
///
/// `INVALID_ARGUMENT` (corrupt record) when the revision is not a number.
pub(super) fn next_revision(revision: &str) -> AgentResult<String> {
    let number: u64 = revision.parse().map_err(|_| corrupt_record())?;
    number
        .checked_add(1)
        .map(|next| next.to_string())
        .ok_or_else(corrupt_record)
}

/// The current time in milliseconds since the epoch; zero when the clock is before the epoch.
pub(super) fn system_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}
