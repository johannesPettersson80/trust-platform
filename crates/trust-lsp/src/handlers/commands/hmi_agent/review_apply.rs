//! The review, application and discard of a draft (specification 43, AT-09) and the draft reads the
//! extension needs (`drafts` in `service.ts`).
//!
//! The review and the application each take two calls, because the extension renders the review and runs
//! the editor transaction between them:
//!
//! - `review` with phase `prepare` answers the validation, the object changes and the pages to render;
//!   with phase `record`, after the extension rendered and presented the proposal, it records the review
//!   (its findings, the project images it examined, the visual review) on the draft.
//! - `apply` with phase `prepare` runs the read-only preflight under the draft lock and records the
//!   application as prepared, then answers the files the editor transaction must write. With phase
//!   `receipt`, it records the outcome the extension reports after its transaction: committed, certainly
//!   unchanged, or uncertain (which keeps the receipt until it is inspected).
//!
//! Port of the review and apply paths of `service.ts`, `applyDraftFiles`' checks in `apply.ts` and the
//! image preflight of `assets.ts`. The rendering and the editor transaction are the extension's.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use serde_json::{json, Value};

use crate::state::ServerState;

use super::argument::Argument;
use super::boundary::Success;
use super::documents;
use super::drafts::{
    require_retained_pages, CommitFailure, DiscardExpectation, Draft, DraftState, DraftStore,
    PreparedApplication,
};
use super::error::{
    AgentError, AgentResult, CANCELLED, CONFLICT, EXPIRED_DRAFT, INVALID_ARGUMENT, LIMIT_EXCEEDED,
    PROJECT_UNAVAILABLE, RENDER_FAILED, UNSUPPORTED_CONTEXT, VALIDATION_FAILED,
};
use super::native;
use super::queries;
use super::review::describe_page_changes;
use super::schema::{self, is_page_file, ApplyAction, ApplyInput};
use super::service::{
    at_revision, encode, owned_draft, success, validate_draft, with_field, with_fields,
};

/// The most project images one proposal may review (`assets.ts`, specification 43, AT-07).
const MAX_IMAGES: usize = 32;
/// The largest image a review reads: 8 MiB (specification 43, AT-07).
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
/// The largest page a transaction writes: 1 MiB (`applyDraftFiles`).
const MAX_PAGE_BYTES: usize = 1024 * 1024;
/// The most files one transaction writes (`applyDraftFiles`).
const MAX_CHANGED_FILES: usize = 32;

/// `drafts`: the open drafts of this workspace (`list`), or one draft's pages and revision for the
/// extension to render or to apply (`read`). An applied draft keeps only its receipts, so it is refused
/// with `EXPIRED_DRAFT` for `read`. `cleanup` removes the finished records past retention, as the customer
/// asks, and reports the records that cannot be read (specification 43, §6 and §8 repair 1).
///
/// # Errors
///
/// `INVALID_ARGUMENT` for an unknown action or a missing `draftId`; the refusals of the store.
pub(super) fn drafts(argument: &Argument) -> AgentResult<Success> {
    let store = DraftStore::open(&argument.storage)?;
    match argument.raw.get("action").and_then(Value::as_str) {
        Some("list") => {
            let summaries = store.list(&argument.workspace_id)?;
            Ok(success(
                json!({ "drafts": encode(&summaries)? }),
                BTreeMap::new(),
            ))
        }
        Some("read") => {
            let id = argument
                .raw
                .get("draftId")
                .and_then(Value::as_str)
                .ok_or_else(|| AgentError::new(INVALID_ARGUMENT, "draftId is required."))?;
            let draft = owned_draft(&store, argument, id)?;
            require_retained_pages(&draft)?;
            let revisions = BTreeMap::from([(draft.id.clone(), draft.revision.clone())]);
            Ok(success(read_value(&draft)?, revisions))
        }
        Some("cleanup") => {
            let report = store.cleanup()?;
            Ok(success(encode(&report)?, BTreeMap::new()))
        }
        _ => Err(AgentError::new(
            INVALID_ARGUMENT,
            "action must be list, read or cleanup.",
        )),
    }
}

/// A draft's pages and state, as the extension reads them to render or apply it.
fn read_value(draft: &Draft) -> AgentResult<Value> {
    Ok(json!({
        "draftId": draft.id,
        "revision": draft.revision,
        "state": draft.state.label(),
        "files": draft.files,
        "intent": draft.intent.clone().unwrap_or(Value::Null),
        "findings": draft.findings,
        "visualReview": encode(&draft.visual_review)?,
        "programRevision": draft.program_revision,
    }))
}

/// `review`: the two phases of a review (AT-09, `action` review).
///
/// # Errors
///
/// `INVALID_ARGUMENT` for another action or phase; the refusals of each phase.
pub(super) fn review(
    state: &ServerState,
    argument: &Argument,
    raw: &Value,
) -> AgentResult<Success> {
    let input = decode_action(raw, ApplyAction::Review)?;
    match argument.raw.get("phase").and_then(Value::as_str) {
        Some("prepare") => review_prepare(state, argument, &input),
        Some("record") => review_record(state, argument, &input),
        _ => Err(AgentError::new(
            INVALID_ARGUMENT,
            "phase must be prepare or record.",
        )),
    }
}

/// `apply`: the two phases of an application (AT-09, `action` apply).
///
/// # Errors
///
/// `INVALID_ARGUMENT` for another action or phase or a status of the receipt; the refusals of each phase.
pub(super) fn apply(state: &ServerState, argument: &Argument, raw: &Value) -> AgentResult<Success> {
    let input = decode_action(raw, ApplyAction::Apply)?;
    match argument.raw.get("phase").and_then(Value::as_str) {
        Some("prepare") => apply_prepare(state, argument, &input),
        Some("receipt") => apply_receipt(argument, &input),
        _ => Err(AgentError::new(
            INVALID_ARGUMENT,
            "phase must be prepare or receipt.",
        )),
    }
}

/// `discard`: removes the draft's pages and keeps its identity (AT-09, `action` discard).
///
/// # Errors
///
/// `INVALID_ARGUMENT` for another action; the store's refusals for a changed or unknown draft.
pub(super) fn discard(argument: &Argument, raw: &Value) -> AgentResult<Success> {
    let input = decode_action(raw, ApplyAction::Discard)?;
    let store = DraftStore::open(&argument.storage)?;
    store.discard(
        &input.draft_id,
        Some(&DiscardExpectation {
            workspace_id: argument.workspace_id.clone(),
            revision: input.revision.clone(),
        }),
    )?;
    Ok(success(
        json!({ "draftId": input.draft_id, "discarded": true }),
        BTreeMap::new(),
    ))
}

/// Decodes an apply request and checks its action.
fn decode_action(raw: &Value, expected: ApplyAction) -> AgentResult<ApplyInput> {
    let input: ApplyInput = schema::read("hmi_apply_draft", raw.clone())?;
    if input.action != expected {
        return Err(AgentError::new(
            INVALID_ARGUMENT,
            format!("this command takes action {}.", action_name(expected)),
        ));
    }
    Ok(input)
}

/// The name of an action, as the tool spells it.
const fn action_name(action: ApplyAction) -> &'static str {
    match action {
        ApplyAction::Review => "review",
        ApplyAction::Apply => "apply",
        ApplyAction::Discard => "discard",
    }
}

/// The first phase of a review: the validation, the object changes and the pages the extension renders.
fn review_prepare(
    state: &ServerState,
    argument: &Argument,
    input: &ApplyInput,
) -> AgentResult<Success> {
    let store = DraftStore::open(&argument.storage)?;
    let draft = owned_draft(&store, argument, &input.draft_id)?;
    at_revision(&draft, &input.revision)?;
    require_retained_pages(&draft)?;
    let workspace = queries::load(state, argument, false)?;
    let result = validate_draft(state, argument, &workspace, &draft, None)?;
    let mut object_changes = Vec::with_capacity(draft.files.len());
    let mut pages = Vec::with_capacity(draft.files.len());
    for (file, text) in &draft.files {
        let base = draft.base.iter().find(|snapshot| &snapshot.file == file);
        let before = match base {
            Some(snapshot) if snapshot.exists => Some(native::page(state, &snapshot.text, file)?),
            _ => None,
        };
        let after = native::page(state, text, file)?;
        let changes = encode(&describe_page_changes(before.as_ref(), &after))?;
        object_changes.push(with_field(changes.clone(), "file", json!(file)));
        pages.push(json!({
            "file": file,
            "title": after.title,
            "changes": changes,
            "baseExists": base.is_some_and(|snapshot| snapshot.exists),
            "baseRevision": base.map(|snapshot| snapshot.revision.clone()),
        }));
    }
    let assets = reviewed_assets(state, argument, &draft)?;
    let data = with_fields(
        result,
        &[
            ("intent", draft.intent.clone().unwrap_or(Value::Null)),
            ("changes", json!(draft.files.keys().collect::<Vec<_>>())),
            ("objectChanges", Value::Array(object_changes)),
            ("pages", Value::Array(pages)),
            ("assetRevisions", json!(assets)),
            ("state", json!("proposal")),
            ("origin", json!("synthetic")),
        ],
    );
    let revisions = BTreeMap::from([(draft.id.clone(), draft.revision.clone())]);
    Ok(success(data, revisions))
}

/// The second phase of a review: the review is recorded once the extension rendered and presented it. The
/// findings are computed again here, not taken from the extension.
fn review_record(
    state: &ServerState,
    argument: &Argument,
    input: &ApplyInput,
) -> AgentResult<Success> {
    let store = DraftStore::open(&argument.storage)?;
    let draft = owned_draft(&store, argument, &input.draft_id)?;
    at_revision(&draft, &input.revision)?;
    require_retained_pages(&draft)?;
    if let Some(review) = &input.visual_review {
        for issue in &review.issues {
            let Some(text) = draft.files.get(&issue.file) else {
                return Err(AgentError::new(
                    INVALID_ARGUMENT,
                    "visual issue must name a file in this draft.",
                ));
            };
            let page = native::page(state, text, &issue.file)?;
            let known = issue
                .objects
                .iter()
                .all(|id| page.objects.iter().any(|object| &object.id == id));
            if !known {
                return Err(AgentError::new(
                    INVALID_ARGUMENT,
                    "visual issue names an unknown object.",
                ));
            }
        }
    }
    let workspace = queries::load(state, argument, false)?;
    let result = validate_draft(state, argument, &workspace, &draft, None)?;
    let assets = reviewed_assets(state, argument, &draft)?;
    let findings: Vec<Value> = result
        .get("findings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let reviewed = store.review(
        &input.draft_id,
        &input.revision,
        findings,
        assets.clone(),
        input.review_input().as_ref(),
    )?;
    let visual_review = match &reviewed.visual_review {
        Some(record) => encode(record)?,
        None => json!("required"),
    };
    let data = with_fields(
        result,
        &[
            ("intent", draft.intent.clone().unwrap_or(Value::Null)),
            ("assetRevisions", json!(assets)),
            ("visualReview", visual_review),
            ("state", json!("proposal")),
            ("origin", json!("synthetic")),
        ],
    );
    let revisions = BTreeMap::from([(draft.id.clone(), draft.revision.clone())]);
    Ok(success(data, revisions))
}

/// The first phase of an application: the read-only preflight runs under the draft lock, the application
/// is recorded as prepared, and the files the editor transaction must write are answered. A replayed key
/// answers its committed outcome, so the extension must not write again.
fn apply_prepare(
    state: &ServerState,
    argument: &Argument,
    input: &ApplyInput,
) -> AgentResult<Success> {
    let store = DraftStore::open(&argument.storage)?;
    let draft = owned_draft(&store, argument, &input.draft_id)?;
    at_revision(&draft, &input.revision)?;
    let key = apply_key(&input.revision);
    let save = input.save == Some(true);
    let allow_incomplete = input.allow_incomplete == Some(true);
    let fingerprint = apply_fingerprint(&input.revision, save, allow_incomplete);
    let prepared = store.prepare_application(
        &input.draft_id,
        &input.revision,
        &key,
        &fingerprint,
        |current| preflight_apply(state, argument, current, allow_incomplete),
    )?;
    let revisions = BTreeMap::from([(draft.id.clone(), draft.revision.clone())]);
    match prepared {
        PreparedApplication::Committed(outcome) => {
            let mut data = json!({
                "draftId": input.draft_id,
                "revision": input.revision,
                "key": key,
                "committed": true,
            });
            merge_into(&mut data, outcome);
            Ok(success(data, revisions))
        }
        PreparedApplication::Ready(prepared_draft) => {
            let (texts, incomplete) = if allow_incomplete {
                (incomplete_texts(state, &prepared_draft)?, true)
            } else {
                (prepared_draft.files.clone(), false)
            };
            let files: Vec<Value> = texts
                .iter()
                .map(|(file, text)| {
                    json!({
                        "file": file,
                        "text": text,
                        "create": !base_exists(&prepared_draft, file),
                    })
                })
                .collect();
            let data = json!({
                "draftId": prepared_draft.id,
                "revision": prepared_draft.revision,
                "key": key,
                "committed": false,
                "save": save,
                "incomplete": incomplete,
                "files": files,
            });
            Ok(success(data, revisions))
        }
    }
}

/// The second phase of an application: the outcome the extension reported after its editor transaction.
fn apply_receipt(argument: &Argument, input: &ApplyInput) -> AgentResult<Success> {
    let store = DraftStore::open(&argument.storage)?;
    let key = apply_key(&input.revision);
    let save = input.save == Some(true);
    let allow_incomplete = input.allow_incomplete == Some(true);
    let fingerprint = apply_fingerprint(&input.revision, save, allow_incomplete);
    let outcome = receipt_outcome(argument, allow_incomplete)?;
    let recorded = store.record_application(&input.draft_id, &key, &fingerprint, outcome)?;
    let mut data = json!({
        "draftId": input.draft_id,
        "revision": input.revision,
        "key": key,
    });
    merge_into(&mut data, recorded);
    let revisions = BTreeMap::from([(input.draft_id.clone(), input.revision.clone())]);
    Ok(success(data, revisions))
}

/// The application's receipt as the store records it: the editor's outcome, or the typed no-change or
/// uncertain outcome the extension reports.
fn receipt_outcome(
    argument: &Argument,
    allow_incomplete: bool,
) -> AgentResult<Result<Value, CommitFailure>> {
    let status = argument
        .raw
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("");
    let message = argument
        .raw
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("the editor transaction reported no message");
    match status {
        "committed" => {
            let outcome = argument
                .raw
                .get("outcome")
                .filter(|outcome| outcome.is_object())
                .cloned()
                .ok_or_else(|| {
                    AgentError::new(
                        INVALID_ARGUMENT,
                        "a committed receipt carries the editor's outcome.",
                    )
                })?;
            let outcome = if allow_incomplete {
                with_fields(
                    outcome,
                    &[("incomplete", json!(true)), ("deployable", json!(false))],
                )
            } else {
                outcome
            };
            Ok(Ok(outcome))
        }
        "no-source-change" => {
            let code = known_code(
                argument
                    .raw
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            );
            Ok(Err(CommitFailure::NoSourceChange(AgentError::new(
                code, message,
            ))))
        }
        "uncertain" => Ok(Err(CommitFailure::Uncertain(message.to_owned()))),
        _ => Err(AgentError::new(
            INVALID_ARGUMENT,
            "status must be committed, no-source-change or uncertain.",
        )),
    }
}

/// The stable code of a refusal the extension names, or `VALIDATION_FAILED` for one it does not name.
#[must_use]
fn known_code(code: &str) -> &'static str {
    match code {
        INVALID_ARGUMENT => INVALID_ARGUMENT,
        CONFLICT => CONFLICT,
        CANCELLED => CANCELLED,
        LIMIT_EXCEEDED => LIMIT_EXCEEDED,
        PROJECT_UNAVAILABLE => PROJECT_UNAVAILABLE,
        EXPIRED_DRAFT => EXPIRED_DRAFT,
        _ => VALIDATION_FAILED,
    }
}

/// The preflight of an application, run under the draft lock before anything is recorded: the draft is
/// reviewed at the current program and images, its validation passes (or it is allowed incomplete), and
/// the pages still hold the text the draft was prepared against.
fn preflight_apply(
    state: &ServerState,
    argument: &Argument,
    current: &Draft,
    allow_incomplete: bool,
) -> AgentResult<()> {
    if current.state != DraftState::Reviewed {
        return Err(AgentError::new(
            VALIDATION_FAILED,
            "review the current draft revision before applying.",
        ));
    }
    let workspace = queries::load(state, argument, false)?;
    if current.program_revision.is_none() || current.program_revision != workspace.program_revision
    {
        return Err(AgentError::new(
            CONFLICT,
            "the PLC program changed since this draft was created; create a new proposal from current context.",
        ));
    }
    if let Some(reviewed) = current.reviewed_assets.as_ref()
        && !reviewed.is_empty()
    {
        let files: Vec<String> = reviewed.keys().cloned().collect();
        let now = image_revisions(argument, &files)?;
        if reviewed
            .iter()
            .any(|(file, revision)| now.get(file) != Some(revision))
        {
            return Err(AgentError::new(
                CONFLICT,
                "a project image changed since review; review the current assets before applying.",
            ));
        }
    }
    let result = validate_draft(state, argument, &workspace, current, None)?;
    let engineering_valid = result.get("engineeringValid").and_then(Value::as_bool) == Some(true);
    if !engineering_valid && !allow_incomplete {
        return Err(AgentError::new(
            VALIDATION_FAILED,
            "correct the draft's bindings before applying.",
        ));
    }
    check_transaction(argument, current)
}

/// The checks of the editor transaction that need no editor: the page count and kinds, the base each page
/// was prepared against, and the disk text of every base (`applyDraftFiles` in `apply.ts`).
fn check_transaction(argument: &Argument, draft: &Draft) -> AgentResult<()> {
    if draft.files.is_empty() || draft.files.len() > MAX_CHANGED_FILES {
        return Err(AgentError::new(
            LIMIT_EXCEEDED,
            "provide 1 to 32 changed files.",
        ));
    }
    for (file, text) in &draft.files {
        if !is_page_file(file) {
            return Err(AgentError::new(
                INVALID_ARGUMENT,
                "this transaction accepts HMI pages only.",
            ));
        }
        if !draft.base.iter().any(|snapshot| &snapshot.file == file) {
            return Err(AgentError::new(
                CONFLICT,
                "changed file has no base revision.",
            ));
        }
        if text.len() > MAX_PAGE_BYTES {
            return Err(AgentError::new(LIMIT_EXCEEDED, "page exceeds 1 MiB."));
        }
    }
    for expected in &draft.base {
        let current = documents::snapshot(
            &argument.root,
            &expected.file,
            argument.open.get(&expected.file),
        )?;
        if !documents::same_content(&current, expected) {
            return Err(AgentError::new(
                CONFLICT,
                format!("{} changed since the draft was created.", expected.file),
            ));
        }
    }
    Ok(())
}

/// The pages of an allowed incomplete application, each written with the incomplete marker set
/// (`incompleteFiles` of `service.ts`).
fn incomplete_texts(state: &ServerState, draft: &Draft) -> AgentResult<BTreeMap<String, String>> {
    let mut texts = BTreeMap::new();
    for (file, text) in &draft.files {
        let mut page = native::page(state, text, file)?;
        page.incomplete = true;
        texts.insert(file.clone(), native::format_page(state, &page)?);
    }
    Ok(texts)
}

/// Whether a draft's base holds the page as an existing file (the transaction replaces it) or not (it
/// creates the file).
#[must_use]
fn base_exists(draft: &Draft, file: &str) -> bool {
    draft
        .base
        .iter()
        .any(|snapshot| snapshot.file == file && snapshot.exists)
}

/// The project images the draft's pages show, with their revisions: the images of the pages and of the
/// faceplates they place (`assetRevisions` of `service.ts`).
fn reviewed_assets(
    state: &ServerState,
    argument: &Argument,
    draft: &Draft,
) -> AgentResult<BTreeMap<String, String>> {
    let mut images: BTreeSet<String> = BTreeSet::new();
    for (file, text) in &draft.files {
        let page = native::page(state, text, file)?;
        for object in &page.objects {
            if let Some(image) = &object.image {
                images.insert(image.clone());
            }
        }
        if page.objects.iter().any(|object| object.kind == "faceplate") {
            let placed = native::placed(
                state,
                &argument.root_uri,
                file,
                text,
                &native::unsaved_pages(&argument.open),
            )?;
            if let Value::Object(entries) = placed {
                for entry in entries.values() {
                    let placed_objects = entry
                        .get("model")
                        .and_then(|model| model.get("object"))
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten();
                    for object in placed_objects {
                        if let Some(image) = object.get("image").and_then(Value::as_str) {
                            images.insert(image.to_owned());
                        }
                    }
                }
            }
        }
    }
    let files: Vec<String> = images.into_iter().collect();
    image_revisions(argument, &files)
}

/// The SHA-256 of each project image, read as the preview reads it: a PNG, JPEG or SVG under `hmi/assets/`,
/// no larger than 8 MiB. A missing image is a render failure.
fn image_revisions(argument: &Argument, files: &[String]) -> AgentResult<BTreeMap<String, String>> {
    if files.len() > MAX_IMAGES {
        return Err(AgentError::new(
            LIMIT_EXCEEDED,
            "preview supports at most 32 different project images.",
        ));
    }
    let mut revisions = BTreeMap::new();
    for file in files {
        if !file.starts_with("hmi/assets/") {
            return Err(AgentError::new(
                INVALID_ARGUMENT,
                "images must be under hmi/assets/.",
            ));
        }
        let extension = file.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "svg") {
            return Err(AgentError::new(
                UNSUPPORTED_CONTEXT,
                format!("{file} must be a static PNG, JPEG or SVG image."),
            ));
        }
        let bytes = if extension == "svg" {
            let snapshot = documents::snapshot(&argument.root, file, argument.open.get(file))?;
            if !snapshot.exists {
                return Err(missing_image(file));
            }
            snapshot.text.into_bytes()
        } else {
            let path = documents::resolve(&argument.root, file)?;
            fs::read(path).map_err(|_| missing_image(file))?
        };
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(AgentError::new(
                LIMIT_EXCEEDED,
                format!("{file} exceeds the 8 MiB image limit."),
            ));
        }
        revisions.insert(file.clone(), documents::sha256_hex(&bytes));
    }
    Ok(revisions)
}

/// The refusal of a project image that is not there.
fn missing_image(file: &str) -> AgentError {
    AgentError::new(RENDER_FAILED, format!("project image is missing: {file}."))
}

/// The application key of a revision: the key the extension must report its receipt under.
#[must_use]
fn apply_key(revision: &str) -> String {
    format!("apply:{revision}")
}

/// The fingerprint of an application request, as the store compares a replay with it.
#[must_use]
fn apply_fingerprint(revision: &str, save: bool, allow_incomplete: bool) -> Value {
    json!({
        "revision": revision,
        "save": save,
        "allowIncomplete": allow_incomplete,
    })
}

/// Copies the fields of `extra` into `target` when both are objects.
fn merge_into(target: &mut Value, extra: Value) {
    if let (Value::Object(target), Value::Object(extra)) = (target, extra) {
        target.extend(extra);
    }
}

#[cfg(test)]
#[path = "review_apply_tests.rs"]
mod tests;
