//! The namespace move's target (specification 25 §1.1). The line rules are the extension's former
//! `namespaceMove.ts` rules; every column is worked out by counting the line's UTF-16 code units.

use super::*;
use tower_lsp::lsp_types::{CodeActionContext, Range, TextDocumentIdentifier};

fn params(
    uri: &Url,
    line: u32,
    character: u32,
    only: Option<Vec<CodeActionKind>>,
) -> CodeActionParams {
    let position = Position { line, character };
    CodeActionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        range: Range {
            start: position,
            end: position,
        },
        context: CodeActionContext {
            diagnostics: Vec::new(),
            only,
            trigger_kind: None,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    }
}

fn document(uri: &Url, content: &str) -> Document {
    let state = ServerState::new();
    state.open_document(uri.clone(), 1, content.to_owned());
    state.get_document(uri).expect("document")
}

#[test]
fn namespace_and_using_lines_are_recognised_in_any_letter_case() {
    // Catches: a check that misses indentation or lower case, or one that takes END_NAMESPACE, a
    // longer word or a keyword followed by a tab for a namespace line.
    for line in [
        "NAMESPACE Company.Lib",
        "  using Company.Lib;",
        "NAMESPACE",
        "Using",
    ] {
        assert!(is_namespace_or_using_line(line), "{line}");
    }
    for line in [
        "NAMESPACEX A",
        "NAMESPACE\tA",
        "END_NAMESPACE",
        "",
        "   ",
        "VAR",
    ] {
        assert!(!is_namespace_or_using_line(line), "{line}");
    }
}

#[test]
fn the_move_starts_from_the_name_under_the_cursor_else_the_first_name() {
    // `NAMESPACE ` is 10 columns, so `Company.Library.Core` spans columns 10 to 30.
    let line = "NAMESPACE Company.Library.Core";
    assert_eq!(name_start(line, 0), Some(10), "the keyword: the first name");
    assert_eq!(name_start(line, 25), Some(10));
    assert_eq!(
        name_start(line, 30),
        Some(10),
        "the name's end column belongs to it"
    );
    // 4 columns of indentation, `USING`, one blank: the names start at column 10. `A.B` spans 10-13,
    // `Other.Thing` 15-26. Catches: a cursor on the second name moved to the first.
    let using = "    USING A.B, Other.Thing;";
    assert_eq!(name_start(using, 20), Some(15));
    assert_eq!(name_start(using, 14), Some(10), "between names: the first");
    assert_eq!(
        name_start("NAMESPACE", 3),
        None,
        "no name after the keyword"
    );
    assert_eq!(name_start("NAMESPACE   ", 3), None);
}

#[test]
fn columns_count_utf16_code_units() {
    // `😀` takes two UTF-16 units, so `X.Y` starts at column 10 + 2 = 12. Catches: columns counted
    // in characters (11) or bytes (14), which the editor would place inside the emoji.
    assert_eq!(name_start("NAMESPACE 😀X.Y", 0), Some(12));
    // An ideographic space indents by one unit: `NAMESPACE` and a blank put the names at 11; `Ü` and
    // `ï` start no name, so the first name is `n` at 12.
    assert_eq!(name_start("\u{3000}NAMESPACE Ünïcode.A", 0), Some(12));
}

#[test]
fn the_quick_fix_moves_from_the_name_on_a_namespace_line_only() {
    let uri = Url::parse("file:///project/src/plant.st").expect("uri");
    let doc = document(
        &uri,
        "NAMESPACE Company.Lib\n  VAR_GLOBAL\n  END_VAR\nEND_NAMESPACE\n",
    );
    let actions = code_actions(&doc, &params(&uri, 0, 2, None));
    assert_eq!(actions.len(), 1);
    let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
        panic!("a code action");
    };
    assert_eq!(action.title, "Move Structured Text Namespace");
    assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
    let command = action.command.as_ref().expect("command");
    assert_eq!(command.command, "trust-lsp.moveNamespace.ui");
    assert_eq!(
        command.arguments,
        Some(vec![json!({
            "uri": "file:///project/src/plant.st",
            "position": { "line": 0, "character": 10 },
        })])
    );
    // Catches: the fix offered on END_NAMESPACE or in the namespace's body, or for a refactor-only
    // request.
    assert!(code_actions(&doc, &params(&uri, 3, 2, None)).is_empty());
    assert!(code_actions(&doc, &params(&uri, 1, 4, None)).is_empty());
    assert!(code_actions(
        &doc,
        &params(&uri, 0, 2, Some(vec![CodeActionKind::REFACTOR]))
    )
    .is_empty());
    assert_eq!(
        code_actions(
            &doc,
            &params(&uri, 0, 2, Some(vec![CodeActionKind::QUICKFIX]))
        )
        .len(),
        1
    );
}

#[test]
fn the_context_names_a_namespace_line_and_its_name_from_the_open_text() {
    // Line 2 `  USING Company.Lib;`: 2 + 5 + 1 = column 8. The open text differs from the file on
    // disk, which does not exist. Catches: a context read from the disk.
    let state = ServerState::new();
    let uri = Url::parse("file:///nowhere/main.st").expect("uri");
    state.open_document(
        uri.clone(),
        1,
        "PROGRAM Main\nEND_PROGRAM\n  USING Company.Lib;\n".to_owned(),
    );
    let answer = answer(
        &state,
        json!({ "action": "context", "textDocument": { "uri": uri }, "position": { "line": 2, "character": 3 } }),
    )
    .expect("answer");
    assert_eq!(answer["onNamespaceLine"], json!(true));
    assert_eq!(answer["namePosition"], json!({ "line": 2, "character": 8 }));
    let body = answer_for(&state, &uri, 0);
    assert_eq!(body["onNamespaceLine"], json!(false));
    assert_eq!(body["namePosition"], Value::Null);
    let beyond = answer_for(&state, &uri, 40);
    assert_eq!(beyond["onNamespaceLine"], json!(false));
}

fn answer_for(state: &ServerState, uri: &Url, line: u32) -> Value {
    answer(
        state,
        json!({ "action": "context", "textDocument": { "uri": uri }, "position": { "line": line, "character": 0 } }),
    )
    .expect("answer")
}

#[test]
fn a_closed_file_is_read_from_disk() {
    let path = std::env::temp_dir().join(format!("trust-lsp-namespace-{}.st", std::process::id()));
    std::fs::write(&path, "NAMESPACE Disk.Only\nEND_NAMESPACE\n").expect("file");
    let uri = Url::from_file_path(&path).expect("uri");
    let state = ServerState::new();
    let found = answer_for(&state, &uri, 0);
    let _ = std::fs::remove_file(&path);
    assert_eq!(found["onNamespaceLine"], json!(true));
    assert_eq!(found["namePosition"], json!({ "line": 0, "character": 10 }));
}

#[test]
fn a_new_path_is_checked_as_the_move_takes_it() {
    // The move accepts identifiers joined by dots, none a keyword. Catches: the extension's former
    // check, which let `1A.B`, `Plant__Core` or `Company.PROGRAM` through to a move that then failed.
    let state = ServerState::new();
    let problem = |path: &str| {
        answer(&state, json!({ "action": "path", "newPath": path })).expect("answer")["problem"]
            .clone()
    };
    assert_eq!(problem(""), json!("Namespace path is required."));
    assert_eq!(problem("   "), json!("Namespace path is required."));
    for path in [
        ".A",
        "A.",
        "A..B",
        "1A.B",
        "Plant__Core",
        "Company.PROGRAM",
        "A B",
    ] {
        assert_eq!(
            problem(path),
            json!("Namespace path must use dot-separated identifiers."),
            "{path}"
        );
    }
    assert_eq!(problem("Company.Library.Core"), Value::Null);
}

#[test]
fn the_default_target_is_one_folder_per_part_below_the_first_workspace_folder() {
    // `Company.Library.Core` → `<first folder>/Company/Library/Core.st`, and the path the move takes
    // is the trimmed text. Without a workspace folder there is no default.
    let state = ServerState::new();
    let none = answer(&state, json!({ "action": "path", "newPath": "A.B" })).expect("answer");
    assert_eq!(none["target"], Value::Null);
    state.set_workspace_folders(vec![
        Url::parse("file:///work/first").expect("uri"),
        Url::parse("file:///work/second").expect("uri"),
    ]);
    let found = answer(
        &state,
        json!({ "action": "path", "newPath": " Company.Library.Core " }),
    )
    .expect("answer");
    assert_eq!(found["path"], json!("Company.Library.Core"));
    assert_eq!(
        found["target"],
        json!("file:///work/first/Company/Library/Core.st")
    );
    let single = answer(&state, json!({ "action": "path", "newPath": "Core" })).expect("answer");
    assert_eq!(single["target"], json!("file:///work/first/Core.st"));
}
