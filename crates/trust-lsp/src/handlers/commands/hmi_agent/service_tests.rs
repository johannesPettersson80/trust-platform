//! Tests of the authoring operations (specification 43, AT-04 to AT-06 and §6), run through the authoring
//! commands on a temporary project with a compiling program: composition, source copies, edits with their
//! receipts, validation, the bounded receipts of a draft and the layout of a composed control.
//!
//! Port of `editors/vscode/src/test/suite/hmi-agent-tools.test.ts` (the composition, edit, validation and
//! discard steps of "agent composes, edits, validates and renders an isolated draft", "agent refines an
//! existing dirty page without replacing the customer's other work" and "creation retries return their original
//! proposal and discarded proposals stay expired"), `hmi-agent-edits.test.ts` (the editing rules, run through the
//! edit command on the built-in catalogue), `hmi-agent-projects.test.ts` (the discover, compose, inspect, edit and
//! validate steps of its journey), `hmi-agent-completion.test.ts` ("padding and equal sizing retain readable
//! controls within the reserved region") and `hmi-agent-draft-retention.test.ts` ("edit receipts never copy the
//! pages"). The renders, previews and editor transactions of those suites are the extension's and are not ported.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;

use crate::state::ServerState;

use super::super::error::{CONFLICT, INVALID_ARGUMENT, VALIDATION_FAILED};
use super::*;

/// The workspace the tests connect.
const WORKSPACE: &str = "workspace-service";

/// The process page the copy tests use: one measured value, bound to the program's level.
const PROCESS: &str = "format = 1\ntitle = \"Process\"\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [40, 40]\nsize = [200, 64]\n\n[object.bind]\nvalue = \"Main.Level\"\n";

/// Two tanks joined by a pipe from the first's outlet to the second's inlet; the first is labelled and bound.
/// It is the page of the edit tests, as the TypeScript edit tests' base page is.
const TANKS: &str = "format = 1\ntitle = \"Tanks\"\n\n[[object]]\nid = \"a\"\ntype = \"tank\"\nat = [40, 40]\nsize = [100, 100]\nlabel = \"Keep me\"\n\n[object.bind]\nlevel = \"Main.Level\"\n\n[[object]]\nid = \"b\"\ntype = \"tank\"\nat = [240, 80]\nsize = [100, 100]\n\n[[pipe]]\nfrom = \"a.outlet\"\nto = \"b.inlet\"\n";

/// A project folder under the system temporary folder, removed when the test ends.
struct TempProject(PathBuf);

impl TempProject {
    /// A project with a compiling program and a 1280 × 800 screen. The tests add their pages.
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "trust-hmi-service-{label}-{}-{}",
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
    /// extra fields the command reads (the phase, the open documents).
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

/// The identifiers of a list of objects, in the list's order.
fn ids_of(objects: &Value) -> Vec<&str> {
    objects
        .as_array()
        .expect("a list of objects")
        .iter()
        .map(|object| object["id"].as_str().expect("an identifier"))
        .collect()
}

/// Creates a draft of the tanks page as a copy of it (the refinement of an existing page), under `key`.
/// Returns the draft's identifier.
fn tanks_draft(state: &ServerState, project: &TempProject, key: &str) -> String {
    project.write("hmi/pages/tanks.toml", TANKS);
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

/// The inspection of one page of a draft at `revision`, with the given object identifiers.
fn inspect_draft(
    state: &ServerState,
    project: &TempProject,
    draft: &str,
    revision: &str,
    ids: Value,
) -> Value {
    let answer = call(
        state,
        INSPECT,
        project.argument(
            json!({
                "file": "hmi/pages/tanks.toml",
                "draftId": draft,
                "revision": revision,
                "ids": ids,
            }),
            json!({}),
        ),
    );
    ok_data(&answer).clone()
}

/// A composition request for the process overview, the tank and its level reading, under `key`.
fn overview_request(key: &str, file: &str, title: &str) -> Value {
    json!({
        "key": key,
        "file": file,
        "title": title,
        "pattern": "process-overview",
        "items": [
            {
                "id": "tank",
                "type": "tank",
                "region": "process",
                "min": 0,
                "max": 3,
                "bind": { "level": { "path": "Main.Level", "direction": "read" } },
            },
            {
                "id": "level",
                "type": "value",
                "region": "measurements",
                "unit": "m",
                "label": "Tank level",
                "bind": { "value": { "path": "Main.Level", "direction": "read" } },
            },
        ],
    })
}

/// Port of `hmi-agent-tools.test.ts` ("agent composes, edits, validates and renders an isolated draft"), its
/// composition step: a new page is a draft at revision 1, its pattern's task is recorded because the request
/// gives none, and the source page is not written (specification 43, AT-04 and §6).
#[test]
fn a_composition_creates_a_draft_at_revision_one_and_leaves_the_source_untouched() {
    let project = TempProject::new("compose");
    let state = ServerState::new();
    let answer = call(
        &state,
        COMPOSE,
        project.argument(
            overview_request("compose-tank", "hmi/pages/agent-created.toml", "Agent tank"),
            json!({}),
        ),
    );
    let data = ok_data(&answer);
    let draft = data["draftId"].as_str().expect("a draft identifier");
    assert_eq!(data["revision"], json!("1"));
    assert_eq!(answer["revisions"][draft], json!("1"));
    assert_eq!(data["file"], json!("hmi/pages/agent-created.toml"));
    assert_eq!(data["model"]["title"], json!("Agent tank"));
    assert_eq!(data["visualReview"], json!("required"));
    assert_eq!(data["intent"]["taskSource"], json!("pattern"));
    assert_eq!(
        data["intent"]["operatorTask"],
        json!("Understand process flow, equipment state and the measurements that need attention.")
    );
    assert!(!project.0.join("hmi/pages/agent-created.toml").exists());
}

/// Port of `hmi-agent-completion.test.ts` ("...") and of the request's notes in `intent.ts`: a task, rationale
/// and assumptions the request gives are recorded with the proposal, after the two standing assumptions
/// (specification 43, AT-04: "optional `operatorTask`, `rationale` and `assumptions` are persisted").
#[test]
fn a_composition_records_the_task_rationale_and_assumptions_it_is_given() {
    let project = TempProject::new("compose-notes");
    let state = ServerState::new();
    let answer = call(
        &state,
        COMPOSE,
        project.argument(
            json!({
                "key": "compose-notes",
                "file": "hmi/pages/requested.toml",
                "title": "Requested",
                "pattern": "process-overview",
                "operatorTask": "Read the tank level and the outlet valve",
                "rationale": "The tank is the only process area.",
                "assumptions": ["The outlet valve is manual."],
                "items": [{ "id": "tank", "type": "tank", "region": "process" }],
            }),
            json!({}),
        ),
    );
    let data = ok_data(&answer);
    assert_eq!(data["intent"]["taskSource"], json!("request"));
    assert_eq!(
        data["intent"]["operatorTask"],
        json!("Read the tank level and the outlet valve")
    );
    assert_eq!(
        data["intent"]["rationale"],
        json!("The tank is the only process area.")
    );
    assert_eq!(
        data["assumptions"],
        json!([
            "Preview values are synthetic.",
            "Process connections are supplied, not inferred.",
            "The outlet valve is manual.",
        ])
    );
}

/// Port of `hmi-agent-tools.test.ts` ("creation retries return their original proposal and discarded proposals
/// stay expired"), the replay rule: the same request under the same key returns the original envelope, and the
/// same key with other input is a conflict (specification 43, §6).
#[test]
fn a_creation_key_replays_its_proposal_and_refuses_other_input() {
    let project = TempProject::new("replay");
    let state = ServerState::new();
    let request = overview_request("create-retry", "hmi/pages/retried.toml", "Retry proposal");
    let first = call(
        &state,
        COMPOSE,
        project.argument(request.clone(), json!({})),
    );
    ok_data(&first);
    let replay = call(
        &state,
        COMPOSE,
        project.argument(request.clone(), json!({})),
    );
    assert_eq!(
        replay, first,
        "the same request returns the original proposal"
    );
    let mut other = request;
    other["title"] = json!("Different proposal");
    let refused = call(&state, COMPOSE, project.argument(other, json!({})));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["code"], CONFLICT);
    let drafts = call(
        &state,
        DRAFTS,
        project.argument(json!({}), json!({ "action": "list" })),
    );
    assert_eq!(
        ok_data(&drafts)["drafts"].as_array().map(Vec::len),
        Some(1),
        "the retry did not make a second proposal"
    );
}

/// Port of `hmi-agent-tools.test.ts` ("agent discovers and invokes context ...", its draft list) and of
/// `hmi-agent-completion.test.ts` ("...") in its refusal rule: a new composition refuses a page that exists and
/// names `sourceFile` for a reviewed edit of it; the context then lists the open draft by identifier (specification
/// 43, AT-01 and AT-04).
#[test]
fn a_new_composition_refuses_an_existing_page_and_context_lists_the_draft() {
    let project = TempProject::new("existing");
    project.write("hmi/pages/process.toml", PROCESS);
    let state = ServerState::new();
    let refused = call(
        &state,
        COMPOSE,
        project.argument(
            overview_request(
                "existing-destination",
                "hmi/pages/process.toml",
                "Replacement",
            ),
            json!({}),
        ),
    );
    assert_eq!(refused["code"], CONFLICT);
    let created = call(
        &state,
        COMPOSE,
        project.argument(
            overview_request("new-page", "hmi/pages/new.toml", "New page"),
            json!({}),
        ),
    );
    let draft = ok_data(&created)["draftId"]
        .as_str()
        .expect("a draft identifier")
        .to_owned();
    let context = call(&state, CONTEXT, project.argument(json!({}), json!({})));
    assert_eq!(ok_data(&context)["drafts"][0]["draftId"], json!(draft));
    assert_eq!(ok_data(&context)["drafts"][0]["revision"], json!("1"));
}

/// Port of `hmi-agent-tools.test.ts` ("agent refines an existing dirty page without replacing the customer's
/// other work"), and its copy rule: a refinement keeps the unsaved title of the page; a copy to another page is
/// retitled only when the request names a title, and the source page on disk is not written (specification 43,
/// AT-04 and §4: a dirty buffer is authoritative).
#[test]
fn a_source_copy_keeps_the_unsaved_text_and_retitles_only_the_copy() {
    let project = TempProject::new("copy");
    project.write("hmi/pages/process.toml", PROCESS);
    let state = ServerState::new();
    let unsaved = PROCESS.replace("Process", "Manual draft");
    let open =
        json!({ "hmi/pages/process.toml": { "text": unsaved, "version": 4, "dirty": true } });
    let refined = call(
        &state,
        COMPOSE,
        project.argument(
            json!({
                "key": "refine",
                "file": "hmi/pages/process.toml",
                "sourceFile": "hmi/pages/process.toml",
            }),
            json!({ "open": open.clone() }),
        ),
    );
    assert_eq!(ok_data(&refined)["model"]["title"], json!("Manual draft"));
    let copy = call(
        &state,
        COMPOSE,
        project.argument(
            json!({
                "key": "copy",
                "file": "hmi/pages/copy.toml",
                "sourceFile": "hmi/pages/process.toml",
                "title": "Copy of process",
            }),
            json!({ "open": open }),
        ),
    );
    let copied = ok_data(&copy);
    assert_eq!(copied["model"]["title"], json!("Copy of process"));
    assert_eq!(copied["model"]["object"][0]["id"], json!("level"));
    assert_eq!(
        fs::read_to_string(project.0.join("hmi/pages/process.toml")).expect("the source page"),
        PROCESS
    );
}

/// Port of `hmi-agent-edits.test.ts` ("targeted moves preserve bindings, labels, identities and other objects")
/// and of `hmi-agent-tools.test.ts` ("...returns its exact native-model change receipt"): a move changes only the
/// moved object's position, the receipt names exactly that change, a replay with the same key returns the original
/// receipt even with its fields reordered, and the draft holds the new position at its new revision (specification
/// 43, AT-05).
#[test]
fn an_edit_reports_the_moved_object_and_replays_its_receipt() {
    let project = TempProject::new("move");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "copy-tanks");
    let edited = edit(
        &state,
        &project,
        &draft,
        "1",
        "move-a",
        json!([{ "op": "move", "ids": ["a"], "delta": [16, 24] }]),
    );
    let data = ok_data(&edited);
    assert_eq!(data["revision"], json!("2"));
    assert_eq!(
        data["objectChanges"][0]["file"],
        json!("hmi/pages/tanks.toml")
    );
    assert_eq!(
        data["objectChanges"][0]["modified"],
        json!([{ "id": "a", "properties": ["at"] }])
    );
    let reordered = call(
        &state,
        EDIT,
        project.argument(
            json!({
                "operations": [{ "delta": [16, 24], "ids": ["a"], "op": "move" }],
                "key": "move-a",
                "revision": "1",
                "draftId": draft,
            }),
            json!({}),
        ),
    );
    assert_eq!(reordered, edited, "the replay returns the original outcome");
    let inspection = inspect_draft(&state, &project, &draft, "2", json!(["a"]));
    assert_eq!(inspection["origin"], json!("draft"));
    assert_eq!(inspection["objects"][0]["at"], json!([56, 64]));
}

/// Port of `hmi-agent-tools.test.ts` ("published mutation schemas reject unknown fields before creating or changing
/// a draft"): an operation with a key it does not own is refused, and the rejected operation does not advance the
/// draft's revision (specification 43, §4 and AT-05).
#[test]
fn an_edit_refuses_an_unknown_field_and_keeps_the_revision() {
    let project = TempProject::new("typo");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "typo-copy");
    let refused = edit(
        &state,
        &project,
        &draft,
        "1",
        "malformed-move",
        json!([{ "op": "move", "ids": ["a"], "delta": [8, 0], "typo": true }]),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["code"], INVALID_ARGUMENT);
    let checked = call(
        &state,
        VALIDATE,
        project.argument(json!({ "draftId": draft, "revision": "1" }), json!({})),
    );
    assert_eq!(checked["ok"], json!(true), "{checked}");
}

/// Port of `hmi-agent-tools.test.ts` ("...invalid binding edits must be rejected before draft mutation") and of
/// `hmi-agent-edits.test.ts` ("port connections and binding directions respect the catalogue"): a binding to a path
/// the program does not declare is refused before the draft changes, and it is kept only when the customer allows
/// an unresolved binding, in which case validation reports it (specification 43, AT-05 and AT-06).
#[test]
fn an_edit_refuses_an_unresolved_binding_unless_it_is_allowed() {
    let project = TempProject::new("binding");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "binding-copy");
    let gone = json!([{
        "op": "bind",
        "id": "b",
        "property": "level",
        "binding": { "path": "Main.Gone", "direction": "read" },
    }]);
    let refused = edit(&state, &project, &draft, "1", "gone", gone.clone());
    assert_eq!(refused["code"], VALIDATION_FAILED);
    let unchanged = call(
        &state,
        VALIDATE,
        project.argument(json!({ "draftId": draft, "revision": "1" }), json!({})),
    );
    assert_eq!(unchanged["ok"], json!(true), "{unchanged}");
    let allowed = call(
        &state,
        EDIT,
        project.argument(
            json!({
                "draftId": draft,
                "revision": "1",
                "key": "gone-allowed",
                "allowUnresolved": true,
                "operations": gone,
            }),
            json!({}),
        ),
    );
    assert_eq!(ok_data(&allowed)["revision"], json!("2"));
    let checked = call(
        &state,
        VALIDATE,
        project.argument(json!({ "draftId": draft, "revision": "2" }), json!({})),
    );
    assert_eq!(ok_data(&checked)["engineeringValid"], json!(false));
    assert!(
        ok_data(&checked)["findings"]
            .as_array()
            .expect("the findings")
            .iter()
            .any(|finding| finding["rule"] == "binding"),
        "{checked}"
    );
}

/// Port of `hmi-agent-edits.test.ts` ("batch failure leaves the source unchanged and pinned objects cannot move"),
/// through the command: one operation that names an unknown object refuses the whole batch, and the draft keeps
/// its revision and its positions (specification 43, AT-05: "Invalid operations leave it unchanged").
#[test]
fn an_edit_batch_with_a_bad_operation_changes_nothing() {
    let project = TempProject::new("batch");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "batch-copy");
    let refused = edit(
        &state,
        &project,
        &draft,
        "1",
        "batch",
        json!([
            { "op": "move", "ids": ["a"], "delta": [8, 8] },
            { "op": "move", "ids": ["missing"], "delta": [8, 8] },
        ]),
    );
    assert_eq!(refused["code"], INVALID_ARGUMENT);
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|message| message.contains("unknown object")),
        "{refused}"
    );
    let inspection = inspect_draft(&state, &project, &draft, "1", json!(["a"]));
    assert_eq!(inspection["objects"][0]["at"], json!([40, 40]));
}

/// Port of `hmi-agent-edits.test.ts` ("deleting connected equipment requires explicit dependent removal"): removing
/// an object that a pipe joins is refused until the dependents are named, and then removes the pipe with it
/// (specification 43, AT-05).
#[test]
fn removing_connected_equipment_needs_its_dependents_named() {
    let project = TempProject::new("remove");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "remove-copy");
    let blocked = edit(
        &state,
        &project,
        &draft,
        "1",
        "remove-a",
        json!([{ "op": "remove", "ids": ["a"] }]),
    );
    assert_eq!(blocked["code"], CONFLICT);
    let removed = edit(
        &state,
        &project,
        &draft,
        "1",
        "remove-a-dependents",
        json!([{ "op": "remove", "ids": ["a"], "removeDependents": true }]),
    );
    assert_eq!(ok_data(&removed)["revision"], json!("2"));
    let inspection = inspect_draft(&state, &project, &draft, "2", Value::Null);
    assert_eq!(ids_of(&inspection["objects"]), vec!["b"]);
    assert_eq!(inspection["pipes"], json!([]));
}

/// Port of `hmi-agent-projects.test.ts` ("...discover, compose, render, repair, validate, apply and Undo"), its
/// validation steps: moving one tank onto the other is an object overlap that validation reports; the repaired
/// draft validates with no engineering error (specification 43, AT-06 and §7 item 2).
#[test]
fn a_validation_flags_an_overlap_and_accepts_the_repaired_draft() {
    let project = TempProject::new("overlap");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "overlap-copy");
    let overlapped = edit(
        &state,
        &project,
        &draft,
        "1",
        "overlap",
        json!([{ "op": "move", "ids": ["b"], "delta": [-200, -40] }]),
    );
    assert_eq!(ok_data(&overlapped)["revision"], json!("2"));
    let flawed = call(
        &state,
        VALIDATE,
        project.argument(json!({ "draftId": draft, "revision": "2" }), json!({})),
    );
    let overlap = ok_data(&flawed)["findings"]
        .as_array()
        .expect("the findings")
        .iter()
        .find(|finding| finding["rule"] == "object-overlap")
        .expect("an object overlap");
    assert_eq!(overlap["objects"], json!(["a", "b"]));
    assert_eq!(overlap["file"], json!("hmi/pages/tanks.toml"));
    let repaired = edit(
        &state,
        &project,
        &draft,
        "2",
        "repair-overlap",
        json!([{ "op": "move", "ids": ["b"], "delta": [200, 40] }]),
    );
    assert_eq!(ok_data(&repaired)["revision"], json!("3"));
    let valid = call(
        &state,
        VALIDATE,
        project.argument(json!({ "draftId": draft, "revision": "3" }), json!({})),
    );
    assert_eq!(ok_data(&valid)["engineeringValid"], json!(true), "{valid}");
}

/// Port of `hmi-agent-completion.test.ts` ("...") in its viewport rule: a control smaller than 44 pixels at a target
/// viewport is a `viewport-touch-target` finding with the viewport's identifier, and the viewports answer in
/// request order (specification 43, §4 item 4 and §7). The design size is the viewport here, so the scale is one.
#[test]
fn a_validation_reports_a_control_below_44_pixels_in_a_viewport() {
    let project = TempProject::new("viewport");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "viewport-copy");
    let added = edit(
        &state,
        &project,
        &draft,
        "1",
        "add-control",
        json!([{
            "op": "add",
            "object": { "id": "sp", "type": "setpoint", "at": [40, 240], "size": [240, 30] },
        }]),
    );
    assert_eq!(ok_data(&added)["revision"], json!("2"));
    let checked = call(
        &state,
        VALIDATE,
        project.argument(
            json!({
                "draftId": draft,
                "revision": "2",
                "viewports": [{ "id": "design", "width": 1280, "height": 800 }],
            }),
            json!({}),
        ),
    );
    let data = ok_data(&checked);
    assert_eq!(data["viewports"][0]["id"], json!("design"));
    assert_eq!(
        data["viewports"][0]["origin"],
        json!("shared-adaptive-layout")
    );
    let touch = data["findings"]
        .as_array()
        .expect("the findings")
        .iter()
        .find(|finding| finding["rule"] == "viewport-touch-target")
        .expect("a touch-target finding");
    assert_eq!(touch["viewport"], json!("design"));
    assert_eq!(touch["object"], json!("sp"));
}

/// Port of `hmi-agent-completion.test.ts` ("padding and equal sizing retain readable controls within the reserved
/// region"): with a 24-pixel padding and equal sizes on both axes, both controls take the larger size, and they stack
/// in the padded region with the 24-pixel gap, at the positions the layout gives (specification 43, §6 and AT-04).
#[test]
fn padding_and_equal_sizing_keep_controls_readable_inside_their_region() {
    let project = TempProject::new("padding");
    let state = ServerState::new();
    let answer = call(
        &state,
        COMPOSE,
        project.argument(
            json!({
                "key": "settings-controls",
                "file": "hmi/pages/settings.toml",
                "title": "Settings",
                "pattern": "settings",
                "layout": { "padding": 24, "equalSize": "both" },
                "items": [
                    { "id": "a", "type": "setpoint", "region": "controls", "size": [200, 64] },
                    { "id": "b", "type": "setpoint", "region": "controls", "size": [240, 80] },
                ],
            }),
            json!({}),
        ),
    );
    let objects: Vec<&Value> = ok_data(&answer)["model"]["object"]
        .as_array()
        .expect("the objects")
        .iter()
        .filter(|object| object["id"] != "page-heading")
        .collect();
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0]["size"], json!([240, 80]));
    assert_eq!(objects[1]["size"], json!([240, 80]));
    assert_eq!(objects[0]["at"], json!([56, 112]));
    assert_eq!(objects[1]["at"], json!([56, 216]));
}

/// Port of `hmi-agent-draft-retention.test.ts` ("edit receipts never copy the pages: a hundred edits keep one copy
/// and a bounded receipt list"), its bound: a draft keeps its newest 64 edit receipts. A retry of an older edit is
/// answered as a changed revision (`CONFLICT`) and is never applied again, while the newest edits still replay
/// (specification 43, §8 repair 1).
#[test]
fn a_draft_keeps_the_receipts_of_its_newest_64_edits() {
    let project = TempProject::new("receipts");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "receipt-copy");
    let mut responses = Vec::new();
    for index in 0..65_u64 {
        let answer = edit(
            &state,
            &project,
            &draft,
            &(index + 1).to_string(),
            &format!("step-{index}"),
            json!([{ "op": "move", "ids": ["a"], "delta": [8, 0] }]),
        );
        assert_eq!(answer["ok"], json!(true), "{answer}");
        responses.push(answer);
    }
    let oldest = edit(
        &state,
        &project,
        &draft,
        "1",
        "step-0",
        json!([{ "op": "move", "ids": ["a"], "delta": [8, 0] }]),
    );
    assert_eq!(oldest["code"], CONFLICT);
    let newest = edit(
        &state,
        &project,
        &draft,
        "65",
        "step-64",
        json!([{ "op": "move", "ids": ["a"], "delta": [8, 0] }]),
    );
    assert_eq!(newest, responses[64]);
}

/// Port of `hmi-agent-edits.test.ts` ("equal sizing uses the reference and refuses locked or incompatible faceplate
/// geometry"), for the equal-size and locking rules the command can show: both axes take the reference's size, the
/// binding is kept, and a locked object is not resized (specification 43, AT-05: "equal-size names selected IDs, a
/// reference object and width/height/both").
#[test]
fn equal_sizing_takes_the_reference_size_and_refuses_a_locked_object() {
    let project = TempProject::new("equal");
    let state = ServerState::new();
    let draft = tanks_draft(&state, &project, "equal-copy");
    let resized = edit(
        &state,
        &project,
        &draft,
        "1",
        "resize-b",
        json!([{ "op": "resize", "id": "b", "size": [160, 80] }]),
    );
    assert_eq!(ok_data(&resized)["revision"], json!("2"));
    let equalized = edit(
        &state,
        &project,
        &draft,
        "2",
        "equal-a",
        json!([{ "op": "equal-size", "ids": ["a"], "reference": "b", "axis": "both" }]),
    );
    assert_eq!(ok_data(&equalized)["revision"], json!("3"));
    let inspection = inspect_draft(&state, &project, &draft, "3", json!(["a"]));
    assert_eq!(inspection["objects"][0]["size"], json!([160, 80]));
    assert_eq!(
        inspection["objects"][0]["bind"]["level"]["path"],
        json!("Main.Level")
    );
    let locked = edit(
        &state,
        &project,
        &draft,
        "3",
        "lock-a",
        json!([{ "op": "update", "id": "a", "properties": { "locked": true } }]),
    );
    assert_eq!(ok_data(&locked)["revision"], json!("4"));
    let refused = edit(
        &state,
        &project,
        &draft,
        "4",
        "equal-locked-a",
        json!([{ "op": "equal-size", "ids": ["a"], "reference": "b", "axis": "width" }]),
    );
    assert_eq!(refused["code"], CONFLICT);
}
