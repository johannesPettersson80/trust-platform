//! Specification 39, §7.8: the editor's rename and Find All References reach the bindings of the
//! project's HMI files.

use super::*;

const PROGRAM: &str = "FUNCTION_BLOCK Pump\nVAR_INPUT\n    Start : BOOL;\nEND_VAR\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\n    Running := Start;\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    P101 : Pump;\nEND_VAR\n    P101(Start := TRUE);\nEND_PROGRAM\n";
const PAGE: &str = "format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"pump\"\ntype = \"status\"\nat = [0, 0]\nsize = [240, 48]\n\n[object.bind]\nstate = \"Main.P101.Running\"\n";

/// The server with the project open and its program in the editor.
fn opened(
    name: &str,
) -> (
    PathBuf,
    ServerState,
    tower_lsp::lsp_types::Url,
    tower_lsp::lsp_types::Url,
) {
    let root = temp_dir(name);
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::create_dir_all(root.join("hmi/pages")).expect("pages");
    std::fs::write(root.join("src/main.st"), PROGRAM).expect("source");
    std::fs::write(root.join("hmi/pages/overview.toml"), PAGE).expect("page");
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_directory_path(&root).expect("root uri");
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(root_uri, ProjectConfig::load(&root));
    let source =
        tower_lsp::lsp_types::Url::from_file_path(root.join("src/main.st")).expect("source uri");
    state.open_document(source.clone(), 1, PROGRAM.to_string());
    let page = tower_lsp::lsp_types::Url::from_file_path(root.join("hmi/pages/overview.toml"))
        .expect("page uri");
    (root, state, source, page)
}

#[test]
fn renaming_a_bound_member_also_edits_the_binding() {
    let (root, state, source, page) = opened("trustlsp-hmi-rename-handler");
    let params = tower_lsp::lsp_types::RenameParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: source.clone(),
            },
            position: position_at(PROGRAM, "Running : BOOL"),
        },
        new_name: "IsRunning".to_string(),
        work_done_progress_params: Default::default(),
    };
    let edit = rename(&state, params).expect("a rename").expect("edits");
    let changes = edit.changes.expect("changes");
    assert!(
        changes.get(&source).is_some_and(|edits| edits.len() >= 2),
        "the program: {changes:?}"
    );
    let binding = changes.get(&page).expect("the binding's edit");
    assert_eq!(binding.len(), 1, "{binding:?}");
    assert_eq!(binding[0].new_text, "IsRunning");
    assert_eq!(
        (
            binding[0].range.start.line,
            binding[0].range.start.character
        ),
        (10, 19)
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

#[test]
fn find_all_references_of_a_bound_member_includes_the_binding() {
    let (root, state, source, page) = opened("trustlsp-hmi-references-handler");
    let params = tower_lsp::lsp_types::ReferenceParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: source.clone(),
            },
            position: position_at(PROGRAM, "Running : BOOL"),
        },
        context: tower_lsp::lsp_types::ReferenceContext {
            include_declaration: true,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let found = references(&state, params).expect("references");
    assert!(
        found.iter().any(|location| location.uri == source),
        "{found:?}"
    );
    let binding: Vec<_> = found
        .iter()
        .filter(|location| location.uri == page)
        .collect();
    assert_eq!(binding.len(), 1, "{found:?}");
    assert_eq!(
        (
            binding[0].range.start.line,
            binding[0].range.start.character,
            binding[0].range.end.character
        ),
        (10, 19, 26)
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}

/// The tank example: a program in a configuration that uses the Control and Alarm libraries
/// through `[dependencies]` (the 1D acceptance project, step 8).
#[test]
fn renaming_a_bound_variable_of_a_project_with_dependencies_also_edits_the_binding() {
    const MAIN: &str = include_str!("../../../../../examples/hmi_tank_level/src/Main.st");
    const GAUGE: &str = "format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"valve\"\ntype = \"gauge\"\nat = [0, 0]\nsize = [200, 200]\n\n[object.bind]\nvalue = \"Tank.ValveOpening\"\n";
    let root = temp_dir("trustlsp-hmi-rename-dependencies");
    let libraries = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../libraries");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::create_dir_all(root.join("hmi/pages")).expect("pages");
    std::fs::write(
        root.join("trust-lsp.toml"),
        format!(
            "[project]\ninclude_paths = [\"src\"]\n\n[dependencies]\nControl = {{ path = {:?}, version = \"0.1.0\" }}\nAlarm = {{ path = {:?}, version = \"0.1.0\" }}\nHmi = {{ path = {:?}, version = \"0.1.0\" }}\n",
            libraries.join("control").to_string_lossy(),
            libraries.join("alarm").to_string_lossy(),
            libraries.join("hmi").to_string_lossy()
        ),
    )
    .expect("manifest");
    std::fs::write(root.join("src/main.st"), MAIN).expect("source");
    std::fs::write(root.join("hmi/pages/overview.toml"), GAUGE).expect("page");
    let state = ServerState::new();
    let root_uri = tower_lsp::lsp_types::Url::from_directory_path(&root).expect("root uri");
    state.set_workspace_folders(vec![root_uri.clone()]);
    state.set_workspace_config(root_uri, ProjectConfig::load(&root));
    let source =
        tower_lsp::lsp_types::Url::from_file_path(root.join("src/main.st")).expect("source uri");
    state.open_document(source.clone(), 1, MAIN.to_string());
    let page = tower_lsp::lsp_types::Url::from_file_path(root.join("hmi/pages/overview.toml"))
        .expect("page uri");
    let params = tower_lsp::lsp_types::RenameParams {
        text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
            text_document: tower_lsp::lsp_types::TextDocumentIdentifier {
                uri: source.clone(),
            },
            position: position_at(MAIN, "ValveOpening : LREAL"),
        },
        new_name: "InletValve".to_string(),
        work_done_progress_params: Default::default(),
    };
    let edit = rename(&state, params).expect("a rename").expect("edits");
    let changes = edit.changes.expect("changes");
    let binding = changes.get(&page);
    assert!(
        binding.is_some_and(|edits| edits.len() == 1 && edits[0].new_text == "InletValve"),
        "the binding's edit: {changes:?}"
    );
    std::fs::remove_dir_all(root).expect("remove temp dir");
}
