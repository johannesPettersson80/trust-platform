// Keep in project's workflow with a scripted language tool in place of `trust-lsp`: the requests the tool receives, the replies the
// workflow reads, and the files it writes or leaves alone. The real tool is tested by `tests/control_studio_keep.rs`. The replies
// carry `file://` URIs of Unix paths, so these tests run on Unix only.

#![cfg(unix)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;

use serde_json::json;

use super::*;

const MAIN: &str = "PROGRAM Main\nVAR\n    Parameters : Control.PIDParameters := (ProportionalGain := LREAL#1.0, IntegralGain := LREAL#0.5);\nEND_VAR\nEND_PROGRAM\n";

/// A tool that answers its replies in order and records each request.
struct ScriptedTool {
    replies: RefCell<VecDeque<Value>>,
    requests: RefCell<Vec<(String, Value)>>,
}

impl ScriptedTool {
    fn new(replies: Vec<Value>) -> Self {
        Self {
            replies: RefCell::new(replies.into()),
            requests: RefCell::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<(String, Value)> {
        self.requests.borrow().clone()
    }
}

impl ProjectTool for ScriptedTool {
    fn answer(&self, command: &str, _project: &Path, argument: &Value) -> Result<Value, Failure> {
        self.requests
            .borrow_mut()
            .push((command.to_owned(), argument.clone()));
        Ok(self
            .replies
            .borrow_mut()
            .pop_front()
            .expect("a scripted reply for each request"))
    }
}

/// A project folder whose `src/Main.st` holds `text`, in a fresh temporary folder that is removed on drop.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str, text: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("trust-cs-keep-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).expect("the source folder");
        fs::write(root.join("src/Main.st"), text).expect("Main.st");
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn main(&self) -> PathBuf {
        self.0.join("src/Main.st")
    }

    fn uri(&self) -> String {
        format!("file://{}", self.main().display())
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The example whose record a hand-written PID is configured from: `P1.Parameters`, declared in `src/Main.st`.
fn hand_written_loop() -> LoopDefinition {
    load_loop("examples/control/06-pid-tuning-methods/30-model-based-pi-tuning/control-loop.json")
}

/// The loop that Create PLC code configures from `Main.TIC101_Parameters`, which the generator declares.
fn generated_loop() -> LoopDefinition {
    load_loop("examples/room_heating/control-loops/tank-temperature.json")
}

fn load_loop(relative: &str) -> LoopDefinition {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let text = fs::read_to_string(&path).expect("the example loop");
    LoopDefinition::decode(&text).expect("the example loop decodes")
}

/// The reply of an edit that replaces the literal on `line` of `text` with `new_text`, and keeps `ProportionalGain`.
fn literal_edit(uri: &str, text: &str, new_text: &str, line: usize, literal: &str) -> Value {
    let column = text
        .lines()
        .nth(line)
        .and_then(|text_line| text_line.find(literal))
        .expect("the literal on its line");
    json!({
        "ok": true,
        "edit": { "changes": { uri: [{
            "range": { "start": { "line": line, "character": column },
                       "end": { "line": line, "character": column + literal.len() } },
            "newText": new_text
        }] } },
        "expected": { uri: text },
        "kept": ["ProportionalGain"]
    })
}

/// The reply that keeps `ProportionalGain` of `MAIN` at 2.5 in the hand-written declaration.
fn hand_written_reply(folder: &Folder) -> Value {
    literal_edit(&folder.uri(), MAIN, "LREAL#2.5", 2, "LREAL#1.0")
}

/// The failure's sentence, whole: its detail when the sentence is longer than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

fn running(values: &[(&str, f64)]) -> BTreeMap<String, f64> {
    values
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), *value))
        .collect()
}

#[test]
fn a_hand_written_record_is_reviewed_without_writing_and_written_on_confirmation() {
    let folder = Folder::new("written", MAIN);
    let values = running(&[("P1.Parameters.ProportionalGain", 2.5)]);
    let reply = hand_written_reply(&folder);
    let tool = ScriptedTool::new(vec![reply.clone(), reply]);

    let review =
        review_keep(folder.path(), &hand_written_loop(), &values, &[], &tool).expect("the review");
    assert_eq!(
        fs::read_to_string(folder.main()).expect("Main.st"),
        MAIN,
        "a review writes nothing"
    );
    assert_eq!(review.record, "P1.Parameters");
    assert_eq!(review.kept, ["ProportionalGain"]);
    assert_eq!(review.unknown, [] as [std::string::String; 0]);
    assert_eq!(review.files.len(), 1);
    assert_eq!(
        review.files[0].after,
        MAIN.replace("LREAL#1.0", "LREAL#2.5")
    );

    let outcome = confirm_keep(
        folder.path(),
        &hand_written_loop(),
        &values,
        &[],
        &tool,
        &review.identity,
    )
    .expect("the confirmation");
    assert_eq!(
        fs::read_to_string(folder.main()).expect("Main.st"),
        MAIN.replace("LREAL#1.0", "LREAL#2.5"),
        "the typed literal is kept, and nothing else changes"
    );
    assert_eq!(
        outcome,
        KeepOutcome {
            record: "P1.Parameters".to_owned(),
            kept: vec!["ProportionalGain".to_owned()],
            unknown: Vec::new(),
        }
    );
    let requests = tool.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].0, CONTROL_KEEP);
    assert_eq!(
        requests[0].1,
        json!({ "record": "P1.Parameters", "values": { "ProportionalGain": 2.5 } })
    );
}

#[test]
fn a_loop_without_a_controller_law_is_refused_before_any_request_or_write() {
    // C12: an observation loop binds no record Apply wrote, so Keep refuses it with the reason and asks the tool nothing.
    let folder = Folder::new("observation", MAIN);
    let observation = load_loop(
        "examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/control-loop.json",
    );
    let tool = ScriptedTool::new(Vec::new());
    let failure = review_keep(folder.path(), &observation, &running(&[]), &[], &tool)
        .expect_err("an observation loop has nothing to keep");
    assert_eq!(failure.next_action, NextAction::None, "{failure:?}");
    assert!(tool.requests().is_empty(), "the tool is asked nothing");
    assert_eq!(fs::read_to_string(folder.main()).expect("Main.st"), MAIN);
}

#[test]
fn a_confirmation_that_does_not_give_back_the_review_identity_writes_nothing() {
    let folder = Folder::new("identity", MAIN);
    let values = running(&[("P1.Parameters.ProportionalGain", 2.5)]);
    let reply = hand_written_reply(&folder);
    let tool = ScriptedTool::new(vec![reply.clone(), reply]);

    let failure = confirm_keep(
        folder.path(),
        &hand_written_loop(),
        &values,
        &[],
        &tool,
        "sha256:not-the-review",
    )
    .expect_err("a different identity");
    assert_eq!(text_of(&failure), keep_texts::review_expired());
    assert_eq!(fs::read_to_string(folder.main()).expect("Main.st"), MAIN);
}

#[test]
fn a_file_that_changed_after_the_review_is_refused_and_left_as_it_is() {
    let folder = Folder::new("changed", MAIN);
    let values = running(&[("P1.Parameters.ProportionalGain", 2.5)]);
    let reply = hand_written_reply(&folder);
    let tool = ScriptedTool::new(vec![reply.clone(), reply]);
    let review =
        review_keep(folder.path(), &hand_written_loop(), &values, &[], &tool).expect("the review");

    let changed = format!("{MAIN}(* edited meanwhile *)\n");
    fs::write(folder.main(), &changed).expect("the editor's change");
    let failure = confirm_keep(
        folder.path(),
        &hand_written_loop(),
        &values,
        &[],
        &tool,
        &review.identity,
    )
    .expect_err("the file changed");
    assert_eq!(text_of(&failure), keep_texts::project_changed());
    assert_eq!(fs::read_to_string(folder.main()).expect("Main.st"), changed);
}

#[test]
fn an_unsaved_file_refuses_the_keep_and_writes_nothing() {
    let folder = Folder::new("unsaved", MAIN);
    let values = running(&[("P1.Parameters.ProportionalGain", 2.5)]);
    let tool = ScriptedTool::new(vec![hand_written_reply(&folder)]);

    let failure = review_keep(
        folder.path(),
        &hand_written_loop(),
        &values,
        &[PathBuf::from("src/Main.st")],
        &tool,
    )
    .expect_err("the file has unsaved changes");
    assert_eq!(
        text_of(&failure),
        keep_texts::unsaved_changes("src/Main.st")
    );
    assert_eq!(fs::read_to_string(folder.main()).expect("Main.st"), MAIN);
}

#[test]
fn a_generated_record_writes_only_the_settings_that_differ_through_the_equipment_edit() {
    // The TypeScript test "keeps a record Create PLC code declared through its equipment settings", in the rules' order.
    let folder = Folder::new("generated", MAIN);
    let values = running(&[("Main.TIC101_Parameters.ProportionalGain", 33.3)]);
    let keep_reply = json!({ "ok": false, "generated": { "instance": "Main.TIC101", "settings": {
        "{instance}_Parameters.ProportionalGain": 33.3,
        "{instance}_Parameters.IntegralGain": 0.0667,
        "{instance}_Parameters.TrackingTime": 1.0
    } } });
    // The equipment has 1 as the gain and the same integral gain; it has no tracking-time setting.
    let describe_reply = json!({ "ok": true, "owned": true, "settings": [
        { "member": "{instance}_Parameters.ProportionalGain", "value": 1.0 },
        { "member": "{instance}_Parameters.IntegralGain", "value": 0.0667 }
    ] });
    let apply_reply =
        json!({ "ok": true, "expected": {}, "edit": { "changes": {} }, "creates": [] });
    let tool = ScriptedTool::new(vec![keep_reply, describe_reply, apply_reply]);

    let review =
        review_keep(folder.path(), &generated_loop(), &values, &[], &tool).expect("the review");
    assert_eq!(review.record, "Main.TIC101_Parameters");
    assert_eq!(review.kept, ["ProportionalGain"]);
    assert_eq!(review.unknown, ["TrackingTime"]);
    assert!(
        review.files.is_empty(),
        "the equipment edit changes no file here"
    );

    let requests = tool.requests();
    assert_eq!(
        requests[0].1,
        json!({ "record": "Main.TIC101_Parameters", "values": { "ProportionalGain": 33.3 } })
    );
    assert_eq!(
        requests[1],
        (
            HMI_EQUIPMENT.to_owned(),
            json!({ "instance": "Main.TIC101" })
        )
    );
    assert_eq!(
        requests[2],
        (
            HMI_EQUIPMENT.to_owned(),
            json!({ "instance": "Main.TIC101",
                "changes": { "settings": { "{instance}_Parameters.ProportionalGain": 33.3 } } })
        ),
        "one equipment edit with the value that differs, as the HMI Builder's Keep in project makes it"
    );
}

#[test]
fn equipment_declared_by_hand_refuses_the_keep_and_says_where_the_values_go() {
    let folder = Folder::new("declared", MAIN);
    let values = running(&[("Main.TIC101_Parameters.ProportionalGain", 33.3)]);
    let keep_reply = json!({ "ok": false, "generated": { "instance": "Main.TIC101", "settings": {
        "{instance}_Parameters.ProportionalGain": 33.3 } } });
    let tool = ScriptedTool::new(vec![
        keep_reply,
        json!({ "ok": true, "owned": false, "settings": [] }),
    ]);

    let failure = review_keep(folder.path(), &generated_loop(), &values, &[], &tool)
        .expect_err("declared by hand");
    assert_eq!(
        text_of(&failure),
        keep_texts::declared_in_plc("Main.TIC101")
    );
}

#[test]
fn a_runtime_without_values_is_refused_before_the_tool_runs() {
    let folder = Folder::new("values", MAIN);
    let tool = ScriptedTool::new(Vec::new());
    let failure = review_keep(
        folder.path(),
        &hand_written_loop(),
        &BTreeMap::new(),
        &[],
        &tool,
    )
    .expect_err("no values");
    assert_eq!(text_of(&failure), keep_texts::no_values("P1.Parameters"));
    assert_eq!(
        tool.requests(),
        [] as [(std::string::String, serde_json::Value); 0]
    );
}

#[test]
fn a_loop_without_a_pid_law_has_no_record_to_keep() {
    let folder = Folder::new("law", MAIN);
    let mut loop_definition = hand_written_loop();
    loop_definition.controller.law = None;
    let values = running(&[("P1.Parameters.ProportionalGain", 2.5)]);
    let tool = ScriptedTool::new(Vec::new());
    let failure =
        review_keep(folder.path(), &loop_definition, &values, &[], &tool).expect_err("no record");
    assert_eq!(text_of(&failure), keep_texts::no_record());
    assert_eq!(
        tool.requests(),
        [] as [(std::string::String, serde_json::Value); 0]
    );
}

#[test]
fn a_refusal_of_the_tool_is_the_failure_of_the_keep() {
    let folder = Folder::new("refusal", MAIN);
    let values = running(&[("P1.Parameters.ProportionalGain", 2.5)]);
    let tool = ScriptedTool::new(vec![json!({ "ok": false, "errors": [
        { "message": "P1.Parameters is not declared in PROGRAM Main of this project." }
    ] })]);
    let failure =
        review_keep(folder.path(), &hand_written_loop(), &values, &[], &tool).expect_err("refused");
    assert_eq!(
        text_of(&failure),
        "P1.Parameters is not declared in PROGRAM Main of this project."
    );
}

#[test]
fn an_edit_without_the_text_it_was_prepared_from_is_a_malformed_reply() {
    let folder = Folder::new("malformed", MAIN);
    let values = running(&[("P1.Parameters.ProportionalGain", 2.5)]);
    let uri = folder.uri();
    let tool = ScriptedTool::new(vec![json!({
        "ok": true,
        "edit": { "changes": { uri.as_str(): [] } },
        "expected": {}
    })]);
    let failure = review_keep(folder.path(), &hand_written_loop(), &values, &[], &tool)
        .expect_err("no expected text");
    assert_eq!(
        text_of(&failure),
        keep_texts::tool_answer_invalid(CONTROL_KEEP)
    );
}
