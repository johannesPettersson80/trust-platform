//! The projects of a workspace folder (specification 14 §7.6): every folder that holds a project
//! config is a project, a file belongs to the deepest project folder that holds it, and each
//! project is analyzed on its own (§10.2), seeing its library folders but no other project.

use super::*;
use tower_lsp::lsp_types::{NumberOrString, Url};

/// Line A's program, whose pump the line's own file declares.
const LINE_A_MAIN: &str = "
PROGRAM Main
VAR
    pump : FB_Pump;
END_VAR
pump(run := TRUE);
END_PROGRAM
";

/// Line A's pump, which uses the shared library's `FB_Limit`.
const LINE_A_PUMP: &str = "
FUNCTION_BLOCK FB_Pump
VAR_INPUT
    run : BOOL;
END_VAR
VAR
    limit : FB_Limit;
END_VAR
limit(value := 1);
END_FUNCTION_BLOCK
";

/// Line B's program, with the same name as line A's.
const LINE_B_MAIN: &str = "
PROGRAM Main
VAR
    valve : FB_Valve;
END_VAR
valve(open := TRUE);
END_PROGRAM
";

const LINE_B_VALVE: &str = "
FUNCTION_BLOCK FB_Valve
VAR_INPUT
    open : BOOL;
END_VAR
END_FUNCTION_BLOCK
";

/// The shared library, which line A names and line B does not.
const LIMIT: &str = "
FUNCTION_BLOCK FB_Limit
VAR_INPUT
    value : INT;
END_VAR
END_FUNCTION_BLOCK
";

const LINE_A_CONFIG: &str = "[[libraries]]\nname = \"shared\"\npath = \"../shared\"\n";

/// A folder of two lines and a shared library; each line is a project when `configs`.
fn plant(name: &str, configs: bool) -> PathBuf {
    let base = temp_dir(name);
    let mut files = vec![
        ("line_a/src/Main.st", LINE_A_MAIN),
        ("line_a/src/Pump.st", LINE_A_PUMP),
        ("line_b/src/Main.st", LINE_B_MAIN),
        ("line_b/src/Valve.st", LINE_B_VALVE),
        ("shared/Limit.st", LIMIT),
    ];
    if configs {
        files.push(("line_a/trust-lsp.toml", LINE_A_CONFIG));
        files.push(("line_b/trust-lsp.toml", ""));
    }
    for (path, text) in files {
        let file = base.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
        std::fs::write(&file, text).expect("write a file");
    }
    base
}

/// A fresh server state with `root` as its only workspace folder, indexed as the language server
/// loads one.
async fn index_folder(root: &std::path::Path) -> ServerState {
    let state = ServerState::new();
    state.set_workspace_folders(vec![Url::from_directory_path(root).expect("root uri")]);
    index_workspace(&test_client(), &state).await;
    state.mark_index_first_pass_done();
    state
}

/// The indexed document whose path ends with `suffix`.
fn document(state: &ServerState, suffix: &str) -> crate::state::Document {
    state
        .documents()
        .into_iter()
        .find(|doc| doc.uri.path().ends_with(suffix))
        .unwrap_or_else(|| panic!("no indexed document ends with {suffix}"))
}

/// The diagnostics of the document `suffix` as `code: message`.
fn diagnostics(state: &ServerState, suffix: &str) -> Vec<String> {
    let doc = document(state, suffix);
    super::diagnostics::collect_diagnostics_with_ticket(
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
    .collect()
}

fn errors(state: &ServerState, suffix: &str) -> Vec<String> {
    diagnostics(state, suffix)
        .into_iter()
        .filter(|line| line.starts_with('E'))
        .collect()
}

/// Catches nested project configs ignored, the G276 finding: a folder of several projects was one
/// project, and every `PROGRAM Main` collided with the others'. Without the lines' configs the
/// folder is one project and the programs collide (E104); with them neither line has an error.
#[tokio::test]
async fn each_project_of_a_folder_is_analyzed_on_its_own() {
    let one = index_folder(&plant("nested-projects-one", false)).await;
    assert!(
        errors(&one, "line_a/src/Main.st")
            .iter()
            .any(|error| error.starts_with("E104")),
        "without the lines' configs the folder is one project: {:?}",
        errors(&one, "line_a/src/Main.st")
    );

    let state = index_folder(&plant("nested-projects-two", true)).await;
    for file in [
        "line_a/src/Main.st",
        "line_a/src/Pump.st",
        "line_b/src/Main.st",
        "line_b/src/Valve.st",
    ] {
        assert!(
            errors(&state, file).is_empty(),
            "{file} resolves every name it writes in its own project: {:?}",
            errors(&state, file)
        );
    }
}

/// Catches a project that sees a neighbour's files or misses its library folder: line A's pump
/// finds the shared `FB_Limit` through line A's `[[libraries]]`, while the declarations completion
/// reads in line B hold nothing of line A or of the library, which line B does not name.
#[tokio::test]
async fn a_project_sees_its_library_folder_and_not_its_neighbour() {
    let state = index_folder(&plant("nested-projects-library", true)).await;
    assert!(
        errors(&state, "line_a/src/Pump.st").is_empty(),
        "{:?}",
        errors(&state, "line_a/src/Pump.st")
    );
    let line_b = document(&state, "line_b/src/Main.st");
    let reachable: Vec<String> = state.with_database(|db| {
        db.file_symbols_with_every_declaration(line_b.file_id)
            .iter()
            .map(|symbol| symbol.name.to_string())
            .collect()
    });
    for other in ["FB_Pump", "FB_Limit"] {
        assert!(
            !reachable.iter().any(|name| name == other),
            "completion in line B offers nothing of line A or the library: '{other}' in \
             {reachable:?}"
        );
    }
}

/// Catches a file's configuration taken from the workspace folder instead of its own project, and
/// a file opened after indexing that is analyzed outside its project: line B's file reads line
/// B's configuration, and a new file in line A finds line A's pump.
#[tokio::test]
async fn a_file_follows_its_own_project_also_when_opened_later() {
    let base = plant("nested-projects-open", true);
    let state = index_folder(&base).await;
    let line_b = document(&state, "line_b/src/Main.st");
    let config = state
        .workspace_config_for_uri(&line_b.uri)
        .expect("line B's configuration");
    assert_eq!(
        config.root.canonicalize().expect("line B"),
        base.join("line_b").canonicalize().expect("line B")
    );

    let new_file = base.join("line_a/src/Second.st");
    let text =
        "PROGRAM Second\nVAR\n    pump : FB_Pump;\nEND_VAR\npump(run := FALSE);\nEND_PROGRAM\n";
    std::fs::write(&new_file, text).expect("write the new file");
    let uri = Url::from_file_path(&new_file).expect("the new file's uri");
    state.open_document(uri, 1, text.to_string());
    assert!(
        errors(&state, "line_a/src/Second.st").is_empty(),
        "a file opened later is analyzed in its project: {:?}",
        errors(&state, "line_a/src/Second.st")
    );
}

/// Catches a file opened from another folder analyzed on its own (§7.2.1 with §7.6): it belongs,
/// while it is open, to the root project of the first workspace folder, so its program collides
/// with that project's program of the same name, as it collided with the one project before a
/// folder held several; and it joins no nested project, so its `Main` meets neither line's.
#[tokio::test]
async fn a_file_from_another_folder_belongs_to_the_first_folders_root_project() {
    let base = plant("nested-projects-elsewhere-root", true);
    std::fs::write(base.join("Shared.st"), "PROGRAM Shared\nEND_PROGRAM\n")
        .expect("the root's program");
    let state = index_folder(&base).await;
    let file = temp_dir("nested-projects-elsewhere").join("Outside.st");
    let text = "PROGRAM Shared\nEND_PROGRAM\n\nPROGRAM Main\nEND_PROGRAM\n";
    std::fs::write(&file, text).expect("the file from elsewhere");
    let uri = Url::from_file_path(&file).expect("its uri");
    state.open_document(uri, 1, text.to_string());
    let outside = errors(&state, "/Outside.st");
    assert!(
        outside
            .iter()
            .any(|error| error.starts_with("E104") && error.contains("Shared")),
        "the file from elsewhere is in the root project while open: {outside:?}"
    );
    assert!(
        !outside.iter().any(|error| error.contains("Main")),
        "it joins no nested project: {outside:?}"
    );
}
