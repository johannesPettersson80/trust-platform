//! `trust-lsp.projectLibraries` and the library quick fix (specification 25 §7, §11.6).

use super::*;
use crate::config::ProjectConfig;
use tower_lsp::lsp_types::{CodeActionContext, Range, TextDocumentIdentifier, Url};

fn position_of(text: &str, needle: &str, after: usize) -> Position {
    let offset = text.find(needle).expect("needle") + after;
    crate::handlers::lsp_utils::offset_to_position(text, offset as u32)
}

#[test]
fn the_word_under_the_cursor_is_an_identifier() {
    // On the word or just after it; a word that starts with digits is the identifier after them.
    // Catches: `INC` missed at its end, or `12abc` taken as `12abc`.
    let text = "x := INC(a); y := 12abc;";
    assert_eq!(
        word_at(text, position_of(text, "INC", 1)).as_deref(),
        Some("INC")
    );
    assert_eq!(
        word_at(text, position_of(text, "INC", 3)).as_deref(),
        Some("INC")
    );
    assert_eq!(word_at(text, position_of(text, "12abc", 0)), None);
    assert_eq!(
        word_at(text, position_of(text, "12abc", 3)).as_deref(),
        Some("abc")
    );
    assert_eq!(word_at(text, position_of(text, ":=", 1)), None);
}

/// A server with one open document in a project whose manifest is `manifest`.
fn server_with(manifest: &str, source: &str) -> (ServerState, Url, PathBuf) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "trust-lsp-library-quick-fix-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("project");
    std::fs::write(root.join("trust-lsp.toml"), manifest).expect("manifest");
    let state = ServerState::new();
    let root_url = Url::from_directory_path(&root).expect("root url");
    state.set_workspace_config(
        root_url,
        ProjectConfig::from_contents(&root, Some(root.join("trust-lsp.toml")), manifest),
    );
    let uri = Url::from_file_path(root.join("src/Main.st")).expect("uri");
    state.open_document(uri.clone(), 1, source.to_owned());
    (state, uri, root)
}

fn actions_at(
    state: &ServerState,
    uri: &Url,
    position: Position,
    only: Option<Vec<CodeActionKind>>,
) -> Vec<CodeActionOrCommand> {
    let doc = state.get_document(uri).expect("document");
    let params = CodeActionParams {
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
    };
    code_actions(state, &doc, &params)
}

const SOURCE: &str = "PROGRAM Main\nVAR\n  c : INT;\nEND_VAR\nc := INC(c);\nEND_PROGRAM\n";

#[test]
fn the_quick_fix_adds_oscat_for_inc_when_the_project_lacks_it() {
    let (state, uri, root) = server_with("include_paths = [\"src\"]\n", SOURCE);
    let actions = actions_at(&state, &uri, position_of(SOURCE, "INC", 1), None);
    let _ = std::fs::remove_dir_all(root);
    assert_eq!(actions.len(), 1);
    let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
        panic!("a code action");
    };
    assert_eq!(action.title, "Add OSCAT to this project");
    assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
    let command = action.command.as_ref().expect("command");
    assert_eq!(command.command, "trust-lsp.libraries.add");
    assert_eq!(
        command.arguments,
        Some(vec![
            json!({ "source": { "kind": "curated", "id": "oscat" } })
        ])
    );
}

#[test]
fn no_quick_fix_when_the_manifest_names_the_library_or_quick_fixes_are_not_asked() {
    // Catches: the fix offered again after OSCAT was added, or in a refactor-only menu.
    let (state, uri, root) = server_with(
        "[dependencies]\nOSCAT = { path = \"libraries/oscat\" }\n",
        SOURCE,
    );
    let named = actions_at(&state, &uri, position_of(SOURCE, "INC", 1), None);
    let _ = std::fs::remove_dir_all(root);
    assert!(named.is_empty());
    let (state, uri, root) = server_with("include_paths = [\"src\"]\n", SOURCE);
    let refactor_only = actions_at(
        &state,
        &uri,
        position_of(SOURCE, "INC", 1),
        Some(vec![CodeActionKind::REFACTOR]),
    );
    let on_variable = actions_at(&state, &uri, position_of(SOURCE, "c :=", 0), None);
    let _ = std::fs::remove_dir_all(root);
    assert!(refactor_only.is_empty());
    assert!(on_variable.is_empty());
}

#[test]
fn the_command_answers_the_view_and_refuses_unknown_actions() {
    let root =
        std::env::temp_dir().join(format!("trust-lsp-libraries-answer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("project");
    std::fs::write(root.join("trust-lsp.toml"), "include_paths = [\"src\"]\n").expect("manifest");
    let view = answer(json!({ "action": "view", "root": root, "bundled": root.join("bundled") }))
        .expect("view");
    let insertion =
        answer(json!({ "action": "insertion", "text": "VAR\nEND_VAR\n", "snippet": "x : INT;" }))
            .expect("insertion");
    let unknown = answer(json!({ "action": "rename", "root": root }));
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(view["ok"], json!(true));
    assert_eq!(view["libraries"], json!([]));
    assert_eq!(
        view["curated"][1],
        json!({ "id": "oscat", "label": "OSCAT", "purpose": "General-purpose utility blocks and functions" })
    );
    assert_eq!(
        insertion["insertion"],
        json!({ "line": 1, "text": "    x : INT;\n" })
    );
    assert!(unknown.is_err());
}
