//! The vendor library stubs example through the language server, as its README walks through it
//! (`examples/vendor_library_stubs/README.md`; specification 22, "Shipped project example corpus";
//! specification 14 §6.1.2 and §7.6): a copy of the example indexed the way the server loads a
//! workspace folder, then the editor's requests on `src/Main.st`.

use super::*;
use tower_lsp::lsp_types::{
    CompletionParams, CompletionResponse, DidChangeWatchedFilesParams, FileChangeType, FileEvent,
    GotoDefinitionParams, GotoDefinitionResponse, HoverContents, HoverParams, NumberOrString,
    TextDocumentIdentifier, TextDocumentPositionParams, Url,
};

/// The example in the repository.
fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/vendor_library_stubs")
}

fn readme() -> String {
    std::fs::read_to_string(example().join("README.md")).expect("the README")
}

/// `text` with the four-space indentation of a README step's blocks removed, so that a block
/// reads as its file does.
fn dedented(text: &str) -> String {
    text.lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn copy_folder(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("a folder");
    for entry in std::fs::read_dir(from).expect("the example's folder") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("its type").is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("a copy");
        }
    }
}

/// A copy of the example in a fresh folder, as the README's step 1 makes it; without its
/// `[[libraries]]` entry unless `libraries`.
fn copied_example(name: &str, libraries: bool) -> PathBuf {
    let root = temp_dir(name);
    copy_folder(&example(), &root);
    if !libraries {
        let config = root.join("trust-lsp.toml");
        let text = std::fs::read_to_string(&config).expect("trust-lsp.toml");
        let project = text
            .split("[[libraries]]")
            .next()
            .expect("the project section");
        std::fs::write(&config, format!("{}\n", project.trim_end())).expect("trust-lsp.toml");
    }
    root
}

/// The copy at `root` indexed as the language server loads its workspace folder, with
/// `src/Main.st` open as the editor opens it; its URI and text.
async fn opened(root: &std::path::Path) -> (Arc<ServerState>, Url, String) {
    let state = Arc::new(ServerState::new());
    state.set_workspace_folders(vec![Url::from_directory_path(root).expect("root uri")]);
    index_workspace(&test_client(), &state).await;
    state.mark_index_first_pass_done();
    let main = indexed(&state, "src/Main.st");
    state.open_document(main.uri.clone(), 1, main.content.clone());
    (state, main.uri, main.content)
}

/// The indexed document whose path ends with `suffix`.
fn indexed(state: &ServerState, suffix: &str) -> crate::state::Document {
    state
        .documents()
        .into_iter()
        .find(|doc| doc.uri.path().ends_with(suffix))
        .unwrap_or_else(|| panic!("no indexed document ends with {suffix}"))
}

/// The diagnostics of the document at `uri` as `code: message`, sorted.
fn diagnostics(state: &ServerState, uri: &Url) -> Vec<String> {
    let doc = state.get_document(uri).expect("the document");
    let mut lines: Vec<String> = super::diagnostics::collect_diagnostics_with_ticket(
        state,
        &doc.uri,
        &doc.content,
        doc.file_id,
        None,
    )
    .into_iter()
    .map(|diagnostic| {
        let code = match diagnostic.code {
            Some(NumberOrString::String(code)) => code,
            _ => String::new(),
        };
        format!("{code}: {}", diagnostic.message)
    })
    .collect();
    lines.sort();
    lines
}

/// The position just inside the first `needle` of `text`, which the document `uri` holds.
fn inside(uri: &Url, text: &str, needle: &str) -> TextDocumentPositionParams {
    let offset = text.find(needle).unwrap_or_else(|| panic!("no {needle}"));
    TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        position: super::lsp_utils::offset_to_position(text, offset as u32 + 1),
    }
}

/// The hover text at the first `needle` of the document `uri`, whose text is `text`.
fn hover_text(state: &ServerState, uri: &Url, text: &str, needle: &str) -> String {
    let shown = hover(
        state,
        HoverParams {
            text_document_position_params: inside(uri, text, needle),
            work_done_progress_params: Default::default(),
        },
    )
    .unwrap_or_else(|| panic!("no hover at {needle}"));
    let HoverContents::Markup(markup) = shown.contents else {
        panic!("a markdown hover");
    };
    markup.value
}

/// The completion items at the end of the first `typed` of `text`, which the document `uri`
/// holds now, as (label, detail), sorted.
fn completed(state: &ServerState, uri: &Url, text: &str, typed: &str) -> Vec<(String, String)> {
    let offset = text.find(typed).unwrap_or_else(|| panic!("no {typed}")) + typed.len();
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: super::lsp_utils::offset_to_position(text, offset as u32),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    };
    let items = match completion(state, params).expect("a completion") {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    };
    let mut items: Vec<(String, String)> = items
        .into_iter()
        .map(|item| (item.label, item.detail.unwrap_or_default()))
        .collect();
    items.sort();
    items
}

/// The README's steps 3 to 5. Catches a stub folder the editor no longer indexes from
/// `[[libraries]]` (the program's diagnostics), a hover or a definition that no longer reaches the
/// stub, and member completion that leaves out the block's inputs and outputs (specification 14
/// §6.1.2; it answered nothing for `TCON` before 27 November 2026).
#[tokio::test]
async fn the_example_resolves_hovers_navigates_and_completes_as_its_readme_writes_it() {
    let readme = readme();
    let root = copied_example("vendor-stubs-walkthrough", true);
    let (state, uri, main) = opened(&root).await;

    // Step 3: the Problems view lists nothing for Main.st.
    assert!(readme.contains("The Problems view lists nothing for it"));
    assert_eq!(diagnostics(&state, &uri), Vec::<String>::new());

    // Step 4: the hover shows the stub's declaration, and the definition opens the stub's file at
    // the block's name.
    let stub = std::fs::read_to_string(root.join("vendor/siemens/tcon.st")).expect("tcon.st");
    assert!(
        dedented(&readme).contains(stub.trim()),
        "the README's step 4 shows the stub's declaration"
    );
    assert_eq!(
        hover_text(&state, &uri, &main, "TCON;"),
        format!("```st\n{}\n```", stub.trim())
    );
    let target = match goto_definition(
        &state,
        GotoDefinitionParams {
            text_document_position_params: inside(&uri, &main, "TCON;"),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    ) {
        Some(GotoDefinitionResponse::Scalar(location)) => location,
        Some(GotoDefinitionResponse::Array(locations)) if locations.len() == 1 => {
            locations[0].clone()
        }
        other => panic!("one definition of TCON, not {other:?}"),
    };
    assert!(readme.contains("`vendor/siemens/tcon.st` opens at"));
    assert!(
        target.uri.path().ends_with("vendor/siemens/tcon.st"),
        "{}",
        target.uri
    );
    assert_eq!(
        target.range.start.line, 0,
        "FUNCTION_BLOCK TCON is the file's first line"
    );

    // Step 5: `Connect.` offers TCON's inputs and outputs, and `Extra : NO` offers NORM_X.
    assert!(readme.contains("`DONE`, `ERROR` and `REQ` as `BOOL`, and `STATUS` as `WORD`"));
    let members = main.replace(
        "Connect(REQ := Request);",
        "Connect(REQ := Request);\nConnect.",
    );
    state.update_document(&uri, 2, members.clone());
    let expected: Vec<(String, String)> = [
        ("DONE", "BOOL"),
        ("ERROR", "BOOL"),
        ("REQ", "BOOL"),
        ("STATUS", "WORD"),
    ]
    .into_iter()
    .map(|(label, detail)| (label.to_string(), detail.to_string()))
    .collect();
    assert_eq!(completed(&state, &uri, &members, "Connect."), expected);
    assert!(readme.contains("type `Extra : NO`") && readme.contains("completion offers `NORM_X`"));
    let typed = main.replace(
        "    Engineering : REAL;",
        "    Engineering : REAL;\n    Extra : NO",
    );
    state.update_document(&uri, 3, typed.clone());
    let types = completed(&state, &uri, &typed, "Extra : NO");
    assert!(
        types.iter().any(|(label, _)| label == "NORM_X"),
        "{types:?}"
    );
}

/// The README's "What happened": without the `[[libraries]]` entry each declaration of a vendor
/// block reports E102 and each call E103. Catches a stub folder that the editor indexes without
/// the entry, which would make the entry seem unneeded, and a change of what the program reports.
#[tokio::test]
async fn without_its_libraries_entry_the_program_reports_each_block_and_call() {
    let readme = readme();
    assert!(readme.contains("the error E102, `cannot resolve type 'TCON'`"));
    assert!(readme.contains("the error E103, `'Connect' is not callable`"));
    let root = copied_example("vendor-stubs-without-libraries", false);
    let (state, uri, _) = opened(&root).await;
    let mut expected = vec![
        "E102: cannot resolve type 'TCON'",
        "E102: cannot resolve type 'NORM_X'",
        "E102: cannot resolve type 'SCALE_X'",
        "E103: 'Connect' is not callable",
        "E103: 'Normalizer' is not callable",
        "E103: 'Scaler' is not callable",
    ];
    expected.sort_unstable();
    assert_eq!(diagnostics(&state, &uri), expected);
}

/// The README's two experiments. Catches a call whose misspelled input the stub's interface no
/// longer reports, and a stub file added to the library folder that stays unknown once the editor
/// reports it (as VS Code's file watcher does), or that brings more than the unused variable's
/// warning.
#[tokio::test]
async fn the_try_its_report_a_misspelled_input_and_take_a_new_stub_file() {
    let readme = readme();
    let root = copied_example("vendor-stubs-try-its", true);
    let (state, uri, main) = opened(&root).await;

    assert!(readme.contains("`Connect(REQUEST := Request);`"));
    assert!(readme.contains("the error E105, `unknown parameter 'REQUEST'`"));
    let misspelled = main.replace("Connect(REQ := Request);", "Connect(REQUEST := Request);");
    state.update_document(&uri, 2, misspelled);
    assert_eq!(
        diagnostics(&state, &uri),
        ["E105: unknown parameter 'REQUEST'"]
    );

    // The block as the README writes it, in its `st` block after "Create `vendor/siemens/my_block.st`".
    let after = readme
        .split("Create `vendor/siemens/my_block.st`")
        .nth(1)
        .expect("the README's second experiment");
    let block: String = after
        .lines()
        .skip_while(|line| line.trim() != "```st")
        .skip(1)
        .take_while(|line| line.trim() != "```")
        .map(|line| format!("{}\n", line.strip_prefix("  ").unwrap_or(line)))
        .collect();
    assert!(
        block.starts_with("FUNCTION_BLOCK MY_VENDOR_FB\n"),
        "{block}"
    );
    let stub_folder = indexed(&state, "vendor/siemens/tcon.st")
        .uri
        .to_file_path()
        .expect("the stub's path")
        .parent()
        .expect("the stub folder")
        .to_path_buf();
    let new_stub = stub_folder.join("my_block.st");
    std::fs::write(&new_stub, block).expect("my_block.st");
    did_change_watched_files(
        &test_client(),
        &state,
        DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri: Url::from_file_path(&new_stub).expect("my_block.st uri"),
                typ: FileChangeType::CREATED,
            }],
        },
    )
    .await;

    assert!(readme.contains("add `Station : MY_VENDOR_FB;` after `Engineering : REAL;`"));
    assert!(readme.contains("the warning W001, `unused variable 'Station'`"));
    let with_station = main.replace(
        "    Engineering : REAL;",
        "    Engineering : REAL;\n    Station : MY_VENDOR_FB;",
    );
    state.update_document(&uri, 3, with_station.clone());
    assert_eq!(
        diagnostics(&state, &uri),
        ["W001: unused variable 'Station'"]
    );
    let shown = hover_text(&state, &uri, &with_station, "MY_VENDOR_FB;");
    assert!(shown.contains("FUNCTION_BLOCK MY_VENDOR_FB"), "{shown}");
}
