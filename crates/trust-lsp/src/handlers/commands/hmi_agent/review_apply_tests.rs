//! Tests of the review, the application and the discard of a draft (specification 43, AT-09 and §6), run through
//! the authoring commands on a temporary project with a compiling program.
//!
//! Port of `editors/vscode/src/test/suite/hmi-agent-review.test.ts` (a review that has not been recorded certifies
//! nothing), `hmi-agent-completion.test.ts` (review issues escalate after three attempts; editing invalidates the
//! visual verdict but keeps the repair history), `hmi-agent-apply.test.ts` (the conflict and no-conflict rules of
//! an application, the byte order mark, a retryable pre-commit refusal), `hmi-agent-drafts.test.ts` (discard, its
//! retry and its expired creation key; an unknown identifier cannot escape the storage) and `hmi-agent-tools.test.ts`
//! ("a changed PLC source blocks applying an older design proposal"). The editor transaction that writes the pages
//! is the extension's: here the application is prepared, the preflight is run, and the receipt is recorded.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;

use crate::state::ServerState;

use super::super::error::{
    CANCELLED, CONFLICT, EXPIRED_DRAFT, INVALID_ARGUMENT, VALIDATION_FAILED,
};
use super::super::service::{execute, APPLY, COMPOSE, DISCARD, DRAFTS, EDIT, REVIEW};

/// The workspace the tests connect.
const WORKSPACE: &str = "workspace-review";

/// Two tanks joined by a pipe, the page the tests compose from, edit and review.
const TANKS: &str = "format = 1\ntitle = \"Tanks\"\n\n[[object]]\nid = \"a\"\ntype = \"tank\"\nat = [40, 40]\nsize = [100, 100]\nlabel = \"Keep me\"\n\n[object.bind]\nlevel = \"Main.Level\"\n\n[[object]]\nid = \"b\"\ntype = \"tank\"\nat = [240, 80]\nsize = [100, 100]\n\n[[pipe]]\nfrom = \"a.outlet\"\nto = \"b.inlet\"\n";

/// A project folder under the system temporary folder, removed when the test ends.
struct TempProject(PathBuf);

impl TempProject {
    /// A project with a compiling program and a 1280 × 800 screen. The tests add their pages.
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "trust-hmi-review-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        let project = Self(path);
        project.write(
            "src/main.st",
            "PROGRAM Main\nVAR Level : REAL; END_VAR\nEND_PROGRAM\n",
        );
        project.write(
            "hmi/screen.toml",
            "format = 1\nsize = [1280, 800]\ntheme = \"dark\"\nsymbols = \"flat\"\n",
        );
        project
    }

    /// Writes a file of the project, creating its folders.
    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("a folder")).expect("create the folder");
        fs::write(path, text).expect("write the file");
    }

    /// The argument of a command on this project: its address, its storage and the request's input, with the
    /// extra fields the command reads (the phase, the status, the open documents).
    fn argument(&self, input: Value, extra: Value) -> Value {
        let root_uri = Url::from_directory_path(&self.0)
            .expect("a folder address")
            .to_string();
        let mut argument = json!({
            "workspaceId": WORKSPACE,
            "root_uri": root_uri,
            "storage": self.0.join("private").display().to_string(),
            "open": {},
            "request": { "contractVersion": 1, "workspaceId": WORKSPACE, "input": input },
        });
        if let (Value::Object(fields), Value::Object(extra)) = (&mut argument, extra) {
            fields.extend(extra);
        }
        argument
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs an authoring command on its argument and returns its envelope.
fn call(state: &ServerState, command: &str, argument: Value) -> Value {
    execute(state, command, vec![argument]).expect("an authoring command")
}

/// The data of a successful answer.
fn ok_data(answer: &Value) -> &Value {
    assert_eq!(answer["ok"], json!(true), "{answer}");
    &answer["data"]
}

/// Composes the tanks page as a copy of itself under `key` (a refinement), without writing the page. The page
/// must already be on disk. Returns the draft's identifier.
fn copy_tanks(state: &ServerState, project: &TempProject, key: &str) -> String {
    let answer = call(
        state,
        COMPOSE,
        project.argument(
            json!({
                "key": key,
                "file": "hmi/pages/tanks.toml",
                "sourceFile": "hmi/pages/tanks.toml",
            }),
            json!({}),
        ),
    );
    ok_data(&answer)["draftId"]
        .as_str()
        .expect("a draft identifier")
        .to_owned()
}

/// Writes the tanks page to disk, then composes it as a copy under `key`. Returns the draft's identifier.
fn tanks_draft(state: &ServerState, project: &TempProject, key: &str) -> String {
    project.write("hmi/pages/tanks.toml", TANKS);
    copy_tanks(state, project, key)
}

/// An edit of a draft at `revision`, under `key`, with the given operations.
fn edit(
    state: &ServerState,
    project: &TempProject,
    draft: &str,
    revision: &str,
    key: &str,
    operations: Value,
) -> Value {
    call(
        state,
        EDIT,
        project.argument(
            json!({
                "draftId": draft,
                "revision": revision,
                "key": key,
                "operations": operations,
            }),
            json!({}),
        ),
    )
}

/// The first phase of a review of a draft at `revision`: the pages it would show, and no recorded review.
fn review_prepare(
    state: &ServerState,
    project: &TempProject,
    draft: &str,
    revision: &str,
) -> Value {
    call(
        state,
        REVIEW,
        project.argument(
            json!({ "draftId": draft, "revision": revision, "action": "review" }),
            json!({ "phase": "prepare" }),
        ),
    )
}

/// The second phase of a review: records a review of a draft at `revision` with a verdict and its issues.
fn review_record(
    state: &ServerState,
    project: &TempProject,
    draft: &str,
    revision: &str,
    verdict: &str,
    issues: Value,
) -> Value {
    call(
        state,
        REVIEW,
        project.argument(
            json!({
                "draftId": draft,
                "revision": revision,
                "action": "review",
                "visualReview": { "reviewer": "operator", "verdict": verdict, "issues": issues },
            }),
            json!({ "phase": "record" }),
        ),
    )
}

/// The first phase of an application of a draft at `revision`, with the extra argument fields (the open
/// documents).
fn apply_prepare(
    state: &ServerState,
    project: &TempProject,
    draft: &str,
    revision: &str,
    extra: Value,
) -> Value {
    call(
        state,
        APPLY,
        project.argument(
            json!({ "draftId": draft, "revision": revision, "action": "apply", "save": false }),
            merged(json!({ "phase": "prepare" }), extra),
        ),
    )
}

/// The receipt of an application of a draft at `revision`, with its status and its extra argument fields.
fn apply_receipt(
    state: &ServerState,
    project: &TempProject,
    draft: &str,
    revision: &str,
    extra: Value,
) -> Value {
    call(
        state,
        APPLY,
        project.argument(
            json!({ "draftId": draft, "revision": revision, "action": "apply" }),
            merged(json!({ "phase": "receipt" }), extra),
        ),
    )
}

/// The fields of `extra` added to `base`, both JSON objects.
fn merged(mut base: Value, extra: Value) -> Value {
    if let (Value::Object(fields), Value::Object(extra)) = (&mut base, extra) {
        fields.extend(extra);
    }
    base
}

/// The stored state of a draft as its read answers it.
fn read_draft(state: &ServerState, project: &TempProject, draft: &str) -> Value {
    call(
        state,
        DRAFTS,
        project.argument(json!({}), json!({ "action": "read", "draftId": draft })),
    )
}

/// Port of `hmi-agent-review.test.ts` ("a renderer resource failure leaves the exact proposal unreviewed"), in the
/// form the review takes here: the first phase answers the pages and changes of the proposal, and a draft is
/// certified only when its review is recorded (specification 43, §6: a review records its revision only after its
/// previews are produced).
#[test]
fn a_review_prepare_answers_the_pages_and_leaves_the_draft_unreviewed() {
    let project = TempProject::new("prepare");
    let state = ServerState::new();
    let created = call(
        &state,
        COMPOSE,
        project.argument(
            json!({
                "key": "new-page",
                "file": "hmi/pages/new.toml",
                "title": "New page",
                "pattern": "process-overview",
                "items": [{ "id": "tank", "type": "tank", "region": "process" }],
            }),
            json!({}),
        ),
    );
    let draft = ok_data(&created)["draftId"]
        .as_str()
        .expect("a draft identifier")
        .to_owned();
    let prepared = review_prepare(&state, &project, &draft, "1");
    let data = ok_data(&prepared);
    assert_eq!(data["state"], json!("proposal"));
    assert_eq!(data["changes"], json!(["hmi/pages/new.toml"]));
    assert_eq!(data["pages"][0]["baseExists"], json!(false));
    assert_eq!(data["pages"][0]["title"], json!("New page"));
    assert!(
        data["objectChanges"][0]["added"]
            .as_array()
            .is_some_and(|added| added.iter().any(|id| id == "tank")),
        "{prepared}"
    );
    let read = read_draft(&state, &project, &draft);
    assert_eq!(ok_data(&read)["state"], json!("draft"));
    assert_eq!(ok_data(&read)["visualReview"], Value::Null);
}

/// Port of `hmi-agent-review.test.ts` (the review is recorded) and `hmi-agent-apply.test.ts` (a reviewed draft is
/// the one a source application may take): recording an accepted review marks the draft reviewed at its revision,
/// with the verdict kept (specification 43, §6 and AT-09).
#[test]
fn a_recorded_review_marks_the_draft_reviewed_with_its_verdict() {
    let project = TempProject::new("record");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "record-copy");
    let recorded = review_record(&state, &project, &draft, "1", "accepted", json!([]));
    assert_eq!(
        ok_data(&recorded)["visualReview"]["verdict"],
        json!("accepted")
    );
    let read = read_draft(&state, &project, &draft);
    assert_eq!(ok_data(&read)["state"], json!("reviewed"));
    assert_eq!(ok_data(&read)["visualReview"]["verdict"], json!("accepted"));
    assert_eq!(ok_data(&read)["visualReview"]["attempt"], json!(1));
    assert_eq!(ok_data(&read)["visualReview"]["revision"], json!("1"));
}

/// Port of `hmi-agent-completion.test.ts` ("review issues escalate after three attempts and cannot masquerade as
/// accepted"), its escalation: the third unsuccessful review asks the customer for a decision, and the attempts are
/// counted from the first (specification 43, §10.1).
#[test]
fn the_third_unsuccessful_review_requests_the_customers_decision() {
    let project = TempProject::new("escalate");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "escalate-copy");
    for (attempt, decision) in [(1, false), (2, false), (3, true)] {
        let recorded = review_record(&state, &project, &draft, "1", "needs-repair", json!([]));
        assert_eq!(
            ok_data(&recorded)["visualReview"]["attempt"],
            json!(attempt),
            "attempt {attempt}"
        );
        assert_eq!(
            ok_data(&recorded)["visualReview"]["customerDecisionRequired"],
            json!(decision),
            "attempt {attempt}"
        );
    }
}

/// Port of `hmi-agent-completion.test.ts` ("review issues escalate ... cannot masquerade as accepted"), its refusals:
/// an accepted review cannot carry an unresolved issue, and an issue must name a page of the draft and objects that
/// page holds. A refused review leaves the draft unreviewed (specification 43, §10.1).
#[test]
fn a_review_refuses_an_accepted_issue_and_an_issue_outside_the_draft() {
    let project = TempProject::new("refusals");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "refusal-copy");
    let accepted_with_issue = review_record(
        &state,
        &project,
        &draft,
        "1",
        "accepted",
        json!([{ "file": "hmi/pages/tanks.toml", "objects": ["a"], "message": "The label overlaps." }]),
    );
    assert_eq!(accepted_with_issue["code"], INVALID_ARGUMENT);
    let other_page = review_record(
        &state,
        &project,
        &draft,
        "1",
        "needs-repair",
        json!([{ "file": "hmi/pages/other.toml", "objects": [], "message": "Not in this draft." }]),
    );
    assert_eq!(other_page["code"], INVALID_ARGUMENT);
    let ghost = review_record(
        &state,
        &project,
        &draft,
        "1",
        "needs-repair",
        json!([{ "file": "hmi/pages/tanks.toml", "objects": ["ghost"], "message": "Not on the page." }]),
    );
    assert_eq!(ghost["code"], INVALID_ARGUMENT);
    let read = read_draft(&state, &project, &draft);
    assert_eq!(ok_data(&read)["state"], json!("draft"));
}

/// Port of `hmi-agent-completion.test.ts` ("editing invalidates the stored visual verdict but retains the bounded
/// repair history"): an edit clears the review of the old revision, and the next review continues the count
/// (specification 43, §10.1).
#[test]
fn an_edit_clears_the_visual_verdict_and_keeps_the_repair_history() {
    let project = TempProject::new("history");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "history-copy");
    let first = review_record(&state, &project, &draft, "1", "needs-repair", json!([]));
    assert_eq!(ok_data(&first)["visualReview"]["attempt"], json!(1));
    let edited = edit(
        &state,
        &project,
        &draft,
        "1",
        "move-a",
        json!([{ "op": "move", "ids": ["a"], "delta": [8, 0] }]),
    );
    assert_eq!(ok_data(&edited)["revision"], json!("2"));
    let read = read_draft(&state, &project, &draft);
    assert_eq!(ok_data(&read)["visualReview"], Value::Null);
    assert_eq!(ok_data(&read)["state"], json!("draft"));
    let second = review_record(&state, &project, &draft, "2", "needs-repair", json!([]));
    assert_eq!(ok_data(&second)["visualReview"]["attempt"], json!(2));
}

/// Port of `hmi-agent-apply.test.ts` ("...") and `hmi-agent-tools.test.ts` ("reviewed valid draft applies ..."): an
/// application takes a reviewed draft only, so the first phase refuses an unreviewed one (specification 43, AT-09).
#[test]
fn an_application_needs_a_reviewed_draft() {
    let project = TempProject::new("unreviewed");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "unreviewed-copy");
    let refused = apply_prepare(&state, &project, &draft, "1", json!({}));
    assert_eq!(refused["code"], VALIDATION_FAILED);
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|message| message.contains("review the current draft revision")),
        "{refused}"
    );
}

/// Port of `hmi-agent-tools.test.ts` ("...discards" and the unresolved binding journey) and of
/// `hmi-agent-apply.test.ts` (a refused preflight leaves no receipt): an invalid draft cannot be applied; once its
/// binding is corrected and reviewed at its new revision, the same draft applies (specification 43, AT-09: "a failed
/// preflight consumes no application receipt and may be retried after correcting/reviewing the proposal").
#[test]
fn an_invalid_draft_is_refused_and_applies_once_corrected_and_reviewed() {
    let project = TempProject::new("invalid");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "invalid-copy");
    let unresolved = call(
        &state,
        EDIT,
        project.argument(
            json!({
                "draftId": draft,
                "revision": "1",
                "key": "gone",
                "allowUnresolved": true,
                "operations": [{
                    "op": "bind",
                    "id": "b",
                    "property": "level",
                    "binding": { "path": "Main.Gone", "direction": "read" },
                }],
            }),
            json!({}),
        ),
    );
    assert_eq!(ok_data(&unresolved)["revision"], json!("2"));
    review_record(&state, &project, &draft, "2", "accepted", json!([]));
    let refused = apply_prepare(&state, &project, &draft, "2", json!({}));
    assert_eq!(refused["code"], VALIDATION_FAILED);
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|message| message.contains("correct the draft's bindings")),
        "{refused}"
    );
    let corrected = edit(
        &state,
        &project,
        &draft,
        "2",
        "bind-correct",
        json!([{
            "op": "bind",
            "id": "b",
            "property": "level",
            "binding": { "path": "Main.Level", "direction": "read" },
        }]),
    );
    assert_eq!(ok_data(&corrected)["revision"], json!("3"));
    review_record(&state, &project, &draft, "3", "accepted", json!([]));
    let ready = apply_prepare(&state, &project, &draft, "3", json!({}));
    assert_eq!(ok_data(&ready)["committed"], json!(false));
    assert_eq!(ok_data(&ready)["key"], json!("apply:3"));
    assert_eq!(
        ok_data(&ready)["files"][0]["file"],
        json!("hmi/pages/tanks.toml")
    );
    assert_eq!(ok_data(&ready)["files"][0]["create"], json!(false));
}

/// Port of `hmi-agent-apply.test.ts` ("intervening manual edit prevents every draft file from changing" and
/// "changed text in an open page is a conflict ..."): when a page of the draft changed on disk since the draft was
/// prepared, the application refuses with a conflict and writes nothing. Restoring the page lets the same draft apply
/// (specification 43, AT-09 and §8 repair 2).
#[test]
fn an_application_refuses_a_page_changed_on_disk_and_writes_nothing() {
    let project = TempProject::new("disk");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "disk-copy");
    edit(
        &state,
        &project,
        &draft,
        "1",
        "move-a",
        json!([{ "op": "move", "ids": ["a"], "delta": [8, 0] }]),
    );
    review_record(&state, &project, &draft, "2", "accepted", json!([]));
    let customer = format!("{TANKS}# changed by the customer\n");
    project.write("hmi/pages/tanks.toml", &customer);
    let refused = apply_prepare(&state, &project, &draft, "2", json!({}));
    assert_eq!(refused["code"], CONFLICT);
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|message| message
                .contains("hmi/pages/tanks.toml changed since the draft was created")),
        "{refused}"
    );
    assert_eq!(
        fs::read_to_string(project.0.join("hmi/pages/tanks.toml")).expect("the customer's page"),
        customer
    );
    project.write("hmi/pages/tanks.toml", TANKS);
    let ready = apply_prepare(&state, &project, &draft, "2", json!({}));
    assert_eq!(ok_data(&ready)["committed"], json!(false));
    assert_eq!(ok_data(&ready)["files"][0]["create"], json!(false));
}

/// Port of `hmi-agent-apply.test.ts` ("opening the prepared pages without changing them does not block application",
/// "an edit undone by a later edit leaves the text, so it is no conflict" and "changed text in an open page is a
/// conflict"): an open editor copy with the page's text is no conflict whatever its version; an unsaved buffer with
/// other text is one (specification 43, §8 repair 2 and AT-09).
#[test]
fn open_documents_conflict_only_when_their_text_differs() {
    let project = TempProject::new("open");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "open-copy");
    edit(
        &state,
        &project,
        &draft,
        "1",
        "move-a",
        json!([{ "op": "move", "ids": ["a"], "delta": [8, 0] }]),
    );
    review_record(&state, &project, &draft, "2", "accepted", json!([]));
    let same_text =
        json!({ "hmi/pages/tanks.toml": { "text": TANKS, "version": 7, "dirty": false } });
    let ready = apply_prepare(&state, &project, &draft, "2", json!({ "open": same_text }));
    assert_eq!(ok_data(&ready)["committed"], json!(false), "{ready}");
    // The prepared application ends without a source change, so the same key is prepared again (AT-09).
    let _ = apply_receipt(
        &state,
        &project,
        &draft,
        "2",
        json!({ "status": "no-source-change", "code": CANCELLED, "message": "not applied" }),
    );
    let other_text = json!({ "hmi/pages/tanks.toml": { "text": "customer change", "version": 5, "dirty": true } });
    let conflict = apply_prepare(&state, &project, &draft, "2", json!({ "open": other_text }));
    assert_eq!(conflict["code"], CONFLICT);
    assert!(
        conflict["message"]
            .as_str()
            .is_some_and(|message| message
                .contains("hmi/pages/tanks.toml changed since the draft was created")),
        "{conflict}"
    );
}

/// Port of `hmi-agent-apply.test.ts` ("a byte order mark on disk is not a change once the page is opened"): the
/// page's text is read without its byte order mark, so a page that carries one on disk is the same page as its
/// editor copy (specification 43, §8 repair 2).
#[test]
fn a_byte_order_mark_on_disk_is_not_a_change() {
    let project = TempProject::new("bom");
    let state = ServerState::new();
    project.write("hmi/pages/tanks.toml", &format!("\u{feff}{TANKS}"));
    let draft = copy_tanks(&state, &project, "bom-copy");
    review_record(&state, &project, &draft, "1", "accepted", json!([]));
    let ready = apply_prepare(&state, &project, &draft, "1", json!({}));
    assert_eq!(ok_data(&ready)["committed"], json!(false), "{ready}");
    assert_eq!(ok_data(&ready)["files"][0]["text"], json!(TANKS));
}

/// Port of `hmi-agent-tools.test.ts` ("a changed PLC source blocks applying an older design proposal"): an application
/// is refused when the program changed since the proposal was composed, even if the variable names still match
/// (specification 43, §4 and AT-09).
#[test]
fn a_changed_plc_program_blocks_an_older_proposal() {
    let project = TempProject::new("program");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "program-copy");
    review_record(&state, &project, &draft, "1", "accepted", json!([]));
    project.write(
        "src/main.st",
        "PROGRAM Main\nVAR Level : REAL := 2.0; END_VAR\nEND_PROGRAM\n",
    );
    let refused = apply_prepare(&state, &project, &draft, "1", json!({}));
    assert_eq!(refused["code"], CONFLICT);
    assert!(
        refused["message"].as_str().is_some_and(
            |message| message.contains("the PLC program changed since this draft was created")
        ),
        "{refused}"
    );
}

/// Port of `hmi-agent-apply.test.ts` ("a definite pre-commit cancellation leaves the same application key
/// retryable"), in the form a command can take: a receipt that reports no source change removes the prepared
/// receipt, so the same application can be prepared again (specification 43, AT-09).
#[test]
fn a_refused_commit_that_changed_nothing_leaves_the_application_retryable() {
    let project = TempProject::new("retry");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "retry-copy");
    review_record(&state, &project, &draft, "1", "accepted", json!([]));
    let prepared = apply_prepare(&state, &project, &draft, "1", json!({}));
    assert_eq!(ok_data(&prepared)["committed"], json!(false));
    let stopped = apply_receipt(
        &state,
        &project,
        &draft,
        "1",
        json!({ "status": "no-source-change", "code": CANCELLED, "message": "cancelled before the write" }),
    );
    assert_eq!(stopped["ok"], json!(false));
    assert_eq!(stopped["code"], CANCELLED);
    let retried = apply_prepare(&state, &project, &draft, "1", json!({}));
    assert_eq!(ok_data(&retried)["committed"], json!(false), "{retried}");
    assert_eq!(ok_data(&retried)["key"], json!("apply:1"));
}

/// Port of `hmi-agent-drafts.test.ts` ("application receipt survives restart and prevents repeated source
/// application") and of `hmi-agent-apply.test.ts`'s committed outcome: a recorded commit is replayed with its outcome
/// and never prepared again, and the applied draft keeps only its receipts (specification 43, §6 and §8 repair 1).
#[test]
fn a_committed_application_is_replayed_and_the_applied_draft_expires() {
    let project = TempProject::new("applied");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "applied-copy");
    review_record(&state, &project, &draft, "1", "accepted", json!([]));
    apply_prepare(&state, &project, &draft, "1", json!({}));
    let receipt = apply_receipt(
        &state,
        &project,
        &draft,
        "1",
        json!({ "status": "committed", "outcome": { "changed": ["hmi/pages/tanks.toml"] } }),
    );
    assert_eq!(receipt["ok"], json!(true), "{receipt}");
    let replay = apply_prepare(&state, &project, &draft, "1", json!({}));
    assert_eq!(ok_data(&replay)["committed"], json!(true), "{replay}");
    assert!(
        ok_data(&replay).get("files").is_none(),
        "no second write is prepared"
    );
    let read = read_draft(&state, &project, &draft);
    assert_eq!(read["code"], EXPIRED_DRAFT);
    let listed = call(
        &state,
        DRAFTS,
        project.argument(json!({}), json!({ "action": "list" })),
    );
    assert_eq!(ok_data(&listed)["drafts"], json!([]));
}

/// Port of `hmi-agent-drafts.test.ts` ("uncertain source application cannot be bypassed with another key or draft
/// edit"): when the outcome of a write is not known, the draft keeps its receipt, and neither a retry nor an edit
/// proceeds until the outcome is inspected (specification 43, AT-09: "an unknown error or refusal retains the
/// uncertain receipt").
#[test]
fn an_uncertain_application_blocks_retries_and_edits() {
    let project = TempProject::new("uncertain");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "uncertain-copy");
    review_record(&state, &project, &draft, "1", "accepted", json!([]));
    apply_prepare(&state, &project, &draft, "1", json!({}));
    let receipt = apply_receipt(
        &state,
        &project,
        &draft,
        "1",
        json!({ "status": "uncertain", "message": "the write was interrupted" }),
    );
    assert_eq!(receipt["code"], CONFLICT, "{receipt}");
    let retry = apply_prepare(&state, &project, &draft, "1", json!({}));
    assert_eq!(retry["code"], CONFLICT);
    let attempted_edit = edit(
        &state,
        &project,
        &draft,
        "1",
        "after-uncertain",
        json!([{ "op": "move", "ids": ["a"], "delta": [8, 0] }]),
    );
    assert_eq!(attempted_edit["code"], CONFLICT);
}

/// Port of `hmi-agent-drafts.test.ts` ("discard removes only the draft, and an unknown id cannot escape storage" and
/// "creation replay survives restart without rebuilding or reviving a discarded proposal"): a discard keeps its
/// identity and its creation key, its retry returns the same outcome, the same creation key is then expired, and a
/// discard names the revision it discards (specification 43, §6 and AT-09).
#[test]
fn a_discarded_draft_stays_expired_and_its_discard_is_repeatable() {
    let project = TempProject::new("discard");
    let state = ServerState::new();
    let input = json!({
        "key": "discard-me",
        "file": "hmi/pages/discard.toml",
        "title": "Discard",
        "pattern": "process-overview",
        "items": [{ "id": "tank", "type": "tank", "region": "process" }],
    });
    let created = call(&state, COMPOSE, project.argument(input.clone(), json!({})));
    let draft = ok_data(&created)["draftId"]
        .as_str()
        .expect("a draft identifier")
        .to_owned();
    let discard_argument = |revision: &str| {
        project.argument(
            json!({ "draftId": draft, "revision": revision, "action": "discard" }),
            json!({}),
        )
    };
    let discarded = call(&state, DISCARD, discard_argument("1"));
    assert_eq!(
        ok_data(&discarded),
        &json!({ "draftId": draft, "discarded": true })
    );
    let retry = call(&state, DISCARD, discard_argument("1"));
    assert_eq!(retry, discarded, "a retry returns the original outcome");
    let changed = call(&state, DISCARD, discard_argument("9"));
    assert_eq!(changed["code"], CONFLICT);
    let expired = call(&state, COMPOSE, project.argument(input, json!({})));
    assert_eq!(expired["code"], EXPIRED_DRAFT);
    let listed = call(
        &state,
        DRAFTS,
        project.argument(json!({}), json!({ "action": "list" })),
    );
    assert_eq!(ok_data(&listed)["drafts"], json!([]));
}

/// Port of `hmi-agent-drafts.test.ts` ("discard removes only the draft, and an unknown id cannot escape storage"):
/// an identifier that is not a draft identifier is refused before any file is opened, and an unknown draft is
/// expired (specification 43, §4 and §6).
#[test]
fn an_unknown_draft_identifier_cannot_escape_the_storage() {
    let project = TempProject::new("escape");
    let state = ServerState::new();
    let escaped = read_draft(&state, &project, "../escape");
    assert_eq!(escaped["code"], INVALID_ARGUMENT);
    let unknown = read_draft(&state, &project, "00000000-0000-4000-8000-000000000000");
    assert_eq!(unknown["code"], EXPIRED_DRAFT);
}
