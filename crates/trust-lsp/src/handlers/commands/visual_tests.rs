//! The visual-program commands at the protocol boundary (specification 25 §11.4): the names the
//! server advertises and the extension must never share, and the shape of each answer the editors
//! draw. The rules themselves are tested in `trust-visual`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tower_lsp::lsp_types::InitializeParams;
use tower_lsp::LanguageServer;

use super::*;
use crate::test_support::test_client;

const LADDER: &str = r#"{
  "schemaVersion": 2,
  "networks": [
    {
      "id": "rung_1",
      "order": 0,
      "nodes": [
        { "id": "c1", "type": "contact", "contactType": "NO", "variable": "StartPB", "position": { "x": 100, "y": 100 } },
        { "id": "q1", "type": "coil", "coilType": "NORMAL", "variable": "Motor", "position": { "x": 300, "y": 100 } }
      ],
      "edges": [],
      "layout": { "y": 100 }
    }
  ],
  "variables": [
    { "name": "StartPB", "scope": "global", "type": "BOOL", "address": "%IX0.0" },
    { "name": "Motor", "scope": "global", "type": "BOOL", "address": "%QX0.0" }
  ],
  "metadata": { "name": "conveyor", "description": "demo", "version": "1.0.0" }
}"#;

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[tokio::test]
async fn the_visual_commands_are_advertised() {
    // The language client sends only commands the server advertises; a wrong registration answers
    // `workspace/executeCommand` but the editors could never reach it.
    let server = crate::StLanguageServer::new(test_client());
    let result = server
        .initialize(InitializeParams::default())
        .await
        .expect("initialize");
    let advertised = result
        .capabilities
        .execute_command_provider
        .expect("an executeCommand provider")
        .commands;
    for command in VISUAL_COMMANDS {
        assert!(
            advertised.iter().any(|name| name == command),
            "{command} is advertised: {advertised:?}"
        );
    }
}

#[test]
fn the_visual_commands_never_share_a_name_with_an_extension_command() {
    // The language client registers every advertised command as a VS Code command, so a name the
    // extension also registers stops the client from starting (specification 35, §0.1a).
    let extension = repository().join("editors/vscode");
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(extension.join("package.json")).expect("the extension's manifest"),
    )
    .expect("the manifest is JSON");
    let mut names: Vec<String> = manifest["contributes"]["commands"]
        .as_array()
        .expect("contributed commands")
        .iter()
        .filter_map(|command| command["command"].as_str().map(str::to_owned))
        .collect();
    names.extend(
        manifest["activationEvents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|event| {
                event
                    .as_str()?
                    .strip_prefix("onCommand:")
                    .map(str::to_owned)
            }),
    );
    collect_registrations(&extension.join("src"), &mut names);
    for command in VISUAL_COMMANDS {
        assert!(
            !names.iter().any(|name| name == command),
            "{command} is also an extension command"
        );
    }
}

fn collect_registrations(folder: &Path, names: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_registrations(&path, names);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("ts" | "tsx")
        ) && let Ok(text) = std::fs::read_to_string(&path)
        {
            let mut rest = text.as_str();
            while let Some(at) = rest.find("registerCommand(") {
                rest = &rest[at + "registerCommand(".len()..];
                if let Some(quoted) = rest.trim_start().strip_prefix('"')
                    && let Some(end) = quoted.find('"')
                {
                    names.push(quoted[..end].to_owned());
                }
            }
        }
    }
}

#[test]
fn the_companion_command_answers_both_files_beside_the_program() {
    // Specification 17 §3: `<base>.st` and `<base>.visual.runtime.st`. A wrong answer misses a file
    // name or the kind the editor shows.
    let answer = execute(
        VISUAL_COMPANION_COMMAND,
        vec![json!({ "file": "/work/conveyor.ladder.json", "text": LADDER })],
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(answer["kind"], json!("ladder"));
    assert_eq!(answer["companion"]["file"], json!("conveyor.st"));
    assert_eq!(
        answer["runtimeEntry"]["file"],
        json!("conveyor.visual.runtime.st")
    );
    let companion = answer["companion"]["text"]
        .as_str()
        .expect("companion text");
    assert!(
        companion.contains("FUNCTION_BLOCK FB_conveyor_LADDER"),
        "{companion}"
    );
    assert!(companion.contains("Motor := StartPB;"), "{companion}");
    let entry = answer["runtimeEntry"]["text"].as_str().expect("entry text");
    assert!(entry.contains("  Motor AT %QX0.0 : BOOL;"), "{entry}");
}

#[test]
fn a_file_that_is_no_visual_program_is_named_so() {
    // The save hook sends any file the glob matches; Rust decides. A wrong answer would write a
    // companion for `package.json`.
    let answer = execute(
        VISUAL_COMPANION_COMMAND,
        vec![json!({ "file": "package.json", "text": "{}" })],
    );
    assert_eq!(answer["ok"], json!(false));
    assert_eq!(answer["notVisual"], json!(true));
}

#[test]
fn a_ladder_edit_answers_the_program_as_text_in_its_own_member_order() {
    // The program travels as text so the file keeps its member order; serde_json's map would sort
    // `schemaVersion` after `networks`. The replacement count is answered beside it.
    let answer = execute(
        VISUAL_EDIT_COMMAND,
        vec![
            json!({ "kind": "ladder", "op": "replaceSymbol", "programText": LADDER, "find": "Motor", "replace": "Pump" }),
        ],
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(answer["replacements"], json!(2));
    let text = answer["programText"].as_str().expect("program text");
    assert!(
        text.starts_with("{\"schemaVersion\":2,\"networks\":"),
        "{text}"
    );
    assert!(text.contains("\"variable\":\"Pump\""), "{text}");
    assert!(
        text.contains("\"version\":\"1.0.0\""),
        "unknown members survive: {text}"
    );
}

#[test]
fn the_glob_and_bad_arguments() {
    let glob = execute(VISUAL_SOURCES_COMMAND, vec![json!({})]);
    assert_eq!(
        glob["glob"],
        json!("**/*.{ladder.json,blockly.json,statechart.json,sfc.json,sfc}")
    );
    let refused = execute(VISUAL_VALIDATE_COMMAND, vec![]);
    assert_eq!(refused["ok"], json!(false));
    let unknown = execute(
        VISUAL_VALIDATE_COMMAND,
        vec![json!({ "kind": "fbd", "text": "{}" })],
    );
    assert_eq!(unknown["ok"], json!(false));
    let edit = execute(
        VISUAL_EDIT_COMMAND,
        vec![json!({ "kind": "ladder", "op": "explode", "programText": LADDER })],
    );
    assert_eq!(edit["ok"], json!(false));
}

#[test]
fn validate_answers_the_ladder_schema_verdict() {
    // Specification 16 §2: a file without `schemaVersion: 2` is refused with the actionable
    // message; a valid one passes.
    let ok = execute(
        VISUAL_VALIDATE_COMMAND,
        vec![json!({ "kind": "ladder", "text": LADDER })],
    );
    assert_eq!(ok["ok"], json!(true), "{ok}");
    let legacy = execute(
        VISUAL_VALIDATE_COMMAND,
        vec![
            json!({ "kind": "ladder", "text": "{\"networks\":[],\"variables\":[],\"metadata\":{\"name\":\"a\",\"description\":\"\"}}" }),
        ],
    );
    assert_eq!(legacy["ok"], json!(false));
    let message = legacy["errors"][0]["message"].as_str().expect("message");
    assert!(
        message.contains("schemaVersion must be exactly 2."),
        "{message}"
    );
}

#[test]
fn parse_opens_a_ladder_file_or_names_the_empty_program_it_shows_instead() {
    // The editor shows the program the server opens; for a file it cannot read the answer still
    // carries the empty program called "Invalid Ladder Program", so the editor never invents one.
    let opened = execute(
        VISUAL_PARSE_COMMAND,
        vec![json!({ "kind": "ladder", "text": LADDER, "now": "T" })],
    );
    assert_eq!(opened["ok"], json!(true), "{opened}");
    assert!(opened["programText"]
        .as_str()
        .expect("program")
        .starts_with("{\"schemaVersion\":2,"));
    let broken = execute(
        VISUAL_PARSE_COMMAND,
        vec![json!({ "kind": "ladder", "text": "{\"schemaVersion\":1}", "now": "T" })],
    );
    assert_eq!(broken["ok"], json!(false));
    let fallback: Value =
        serde_json::from_str(broken["programText"].as_str().expect("fallback program"))
            .expect("JSON");
    assert_eq!(
        fallback["metadata"]["name"],
        json!("Invalid Ladder Program")
    );
    assert_eq!(fallback["metadata"]["created"], json!("T"));
}

#[test]
fn a_wire_edit_answers_what_happened() {
    // The editor shows "Wire connected." or "Nodes are already connected." from the answer; the
    // program text comes back either way.
    let wire = |from: &str, to: &str| {
        execute(
            VISUAL_EDIT_COMMAND,
            vec![
                json!({ "kind": "ladder", "op": "connect", "programText": LADDER, "rungIndex": 0, "fromNodeId": from, "toNodeId": to }),
            ],
        )
    };
    assert_eq!(wire("c1", "q1")["wire"], json!("already-connected"));
    let added = wire("q1", "c1");
    assert_eq!(added["wire"], json!("added"), "{added}");
    assert!(added["programText"]
        .as_str()
        .expect("program")
        .contains("\"id\":\"edge_rung_1_1\""));
    assert_eq!(wire("c1", "c1")["wire"], json!("invalid"));
}

#[test]
fn new_and_import_answer_the_file_name_and_the_name_rule() {
    // The four new-program commands and the four imports ask the server; a refused name or file
    // comes back as one message the input box or error shows.
    let check = execute(
        VISUAL_NEW_COMMAND,
        vec![json!({ "kind": "statechart", "name": "my chart", "checkOnly": true })],
    );
    assert_eq!(check["ok"], json!(false));
    assert_eq!(
        check["errors"][0]["message"],
        json!(
            "Statechart program name must only contain letters, numbers, hyphens, and underscores."
        )
    );
    let created = execute(
        VISUAL_NEW_COMMAND,
        vec![json!({ "kind": "sfc", "name": "filler", "created": "T" })],
    );
    assert_eq!(created["fileName"], json!("filler.sfc.json"), "{created}");
    assert!(created["text"]
        .as_str()
        .expect("text")
        .starts_with("{\n  \"name\": \"filler\",\n"));
    let imported = execute(
        VISUAL_IMPORT_COMMAND,
        vec![json!({ "kind": "ladder", "file": "/in/old.json", "text": "{\"networks\":[]}" })],
    );
    assert_eq!(
        imported["targetFile"],
        json!("old.json.ladder.json"),
        "{imported}"
    );
    // Without a workspace nothing is inside it; /in-other is no folder of /in.
    assert_eq!(imported["inWorkspace"], json!(false), "{imported}");
    for (workspace, inside) in [("/in", true), ("/in-other", false)] {
        let answer = execute(
            VISUAL_IMPORT_COMMAND,
            vec![json!({
                "kind": "ladder",
                "file": "/in/old.json",
                "text": "{\"networks\":[]}",
                "workspace": workspace,
            })],
        );
        assert_eq!(
            answer["inWorkspace"],
            json!(inside),
            "{workspace}: {answer}"
        );
    }
    let refused = execute(
        VISUAL_IMPORT_COMMAND,
        vec![json!({ "kind": "ladder", "file": "/in/old.json", "text": "{}" })],
    );
    assert_eq!(refused["ok"], json!(false));
}

#[test]
fn a_typed_number_answers_the_program_with_the_stored_value_or_the_reason() {
    // The Ladder properties panel sends the text of a number field; the answer carries the program
    // with the number stored and the `value`, or `ok: false` with the reason the panel shows. A
    // wrong shape would leave the panel without its value or its message.
    let program = r#"{"schemaVersion":2,"networks":[{"id":"rung_1","order":0,"nodes":[{"id":"t1","type":"timer","timerType":"TON","instance":"T_1","input":"","presetMs":1000,"qOutput":"T_1_Q","etOutput":"T_1_ET","position":{"x":100,"y":100}}],"edges":[],"layout":{"y":100}}],"variables":[],"metadata":{"name":"n","description":""}}"#;
    let typed = |field: &str, text: &str| {
        execute(
            VISUAL_EDIT_COMMAND,
            vec![
                json!({ "kind": "ladder", "op": "setNumber", "programText": program, "rungIndex": 0, "elementIndex": 0, "field": field, "text": text }),
            ],
        )
    };
    let stored = typed("presetMs", "2.5e3");
    assert_eq!(stored["ok"], json!(true), "{stored}");
    assert_eq!(stored["value"], json!(2500));
    assert!(stored["programText"]
        .as_str()
        .expect("program")
        .contains("\"presetMs\":2500"));
    let moved = typed("x", "130");
    assert_eq!(moved["value"], json!(140), "{moved}");
    let refused = typed("presetMs", "");
    assert_eq!(
        refused,
        json!({ "ok": false, "error": "refused", "errors": [{ "message": "Preset (ms) must be a number." }] })
    );
    let unknown = typed("pt", "5");
    assert_eq!(unknown["ok"], json!(false));
}

#[test]
fn a_typed_delay_or_mapping_value_answers_what_the_statechart_stores() {
    // The Statechart panels send the text of the timer and of a mapping value; `after` is null for
    // no delay and a refused delay names the rule. A wrong shape would store the text unread.
    let ask = |op: &str, text: &str| {
        execute(
            VISUAL_EDIT_COMMAND,
            vec![json!({ "kind": "statechart", "op": op, "text": text })],
        )
    };
    assert_eq!(
        ask("afterDelay", "1500"),
        json!({ "ok": true, "after": 1500 })
    );
    assert_eq!(ask("afterDelay", ""), json!({ "ok": true, "after": null }));
    let refused = ask("afterDelay", "-5");
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["error"], json!("refused"));
    assert_eq!(
        refused["errors"][0]["message"],
        json!("Auto-transition timer must be a number of milliseconds from 0, or empty for none.")
    );
    assert_eq!(
        ask("mappingValue", "true"),
        json!({ "ok": true, "value": true })
    );
    assert_eq!(
        ask("mappingValue", "42"),
        json!({ "ok": true, "value": 42 })
    );
    assert_eq!(
        ask("mappingValue", "on"),
        json!({ "ok": true, "value": "on" })
    );
    assert_eq!(ask("explode", "1")["ok"], json!(false));
}

#[test]
fn a_new_action_mapping_is_added_or_refused_as_the_statechart_stores_it() {
    // Specification 25 §11.4: the Add dialog's name goes to the language server with the panel's
    // mappings; the answer is the mappings with the new action and its name, or the reason.
    // Catches the mappings answered in another shape, which the panel would not store.
    let add = |name: &str| {
        execute(
            VISUAL_EDIT_COMMAND,
            vec![json!({
                "kind": "statechart",
                "op": "addMapping",
                "mappings": { "START_MOTOR": { "action": "WRITE_OUTPUT", "address": "%QX0.1", "value": true } },
                "name": name,
            })],
        )
    };
    assert_eq!(
        add(" STOP_MOTOR "),
        json!({
            "ok": true,
            "name": "STOP_MOTOR",
            "mappings": {
                "START_MOTOR": { "action": "WRITE_OUTPUT", "address": "%QX0.1", "value": true },
                "STOP_MOTOR": { "action": "WRITE_OUTPUT", "address": "%QX0.0", "value": true },
            },
        })
    );
    assert_eq!(
        add("START_MOTOR"),
        json!({ "ok": false, "error": "refused", "errors": [{ "message": "An action with this name already exists." }] })
    );
}
