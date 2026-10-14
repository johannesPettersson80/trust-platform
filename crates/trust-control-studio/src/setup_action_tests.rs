//! Tests of the loop setup methods' file rules (`setup_action.rs`): the project identifier, and the all-or-none write of the loop
//! files (B1.3, §0.1b). The discovery and the save through the compiled catalog are proved by `tests/control_studio_setup.rs`.

use std::fs;
use std::path::PathBuf;

use trust_control::loop_definition::LoopDefinition;
use trust_control::setup::save::PlannedLoop;

use super::{project_id, write_all_or_none};

/// A loop file of the project that the engine's examples ship with: a valid loop of the contract.
const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

/// An empty folder for one test, removed before it runs.
fn scratch(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("trust-setup-action-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("a scratch folder");
    root
}

fn planned(file_name: &str, loop_definition: &LoopDefinition) -> PlannedLoop {
    PlannedLoop {
        file_name: file_name.to_string(),
        loop_definition: loop_definition.clone(),
    }
}

#[test]
fn the_project_identifier_is_a_project_scheme_with_a_sha256_hex_digest() {
    let root = scratch("project-id");
    let id = project_id(&root);
    let digest = id.strip_prefix("project:").expect("the project scheme");
    assert_eq!(digest.len(), 64, "a SHA-256 digest in hexadecimal");
    assert!(digest
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    assert_eq!(
        project_id(&root),
        id,
        "the same folder keeps its identifier"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_failed_write_removes_the_files_this_save_created_and_nothing_else() {
    let root = scratch("all-or-none");
    // The second file's path is a folder, so its file cannot be created after the first one was.
    fs::create_dir_all(root.join("control-loops/b.json")).expect("a folder where b.json goes");
    let loop_definition = LoopDefinition::decode(EXAMPLE).expect("the example loop decodes");
    let plans = [
        planned("control-loops/a.json", &loop_definition),
        planned("control-loops/b.json", &loop_definition),
    ];
    assert!(write_all_or_none(&root, &plans).is_err());
    assert!(
        !root.join("control-loops/a.json").exists(),
        "the first file this save created is removed again"
    );
    assert!(
        root.join("control-loops/b.json").is_dir(),
        "a path that was never created by this save is left as it was"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_saved_loop_file_decodes_and_a_second_save_never_overwrites_it() {
    let root = scratch("save-once");
    let loop_definition = LoopDefinition::decode(EXAMPLE).expect("the example loop decodes");
    let plans = [planned("control-loops/a.json", &loop_definition)];
    write_all_or_none(&root, &plans).expect("the first save writes the file");
    let text = fs::read_to_string(root.join("control-loops/a.json")).expect("the file is read");
    assert!(
        text.ends_with("}\n"),
        "a final line break, as setup writes it"
    );
    LoopDefinition::decode(&text).expect("the saved file decodes as a loop definition");
    let before = text.clone();
    assert!(
        write_all_or_none(&root, &plans).is_err(),
        "a second save over the file is refused"
    );
    let after = fs::read_to_string(root.join("control-loops/a.json")).expect("the file is read");
    assert_eq!(before, after, "the refused save changes nothing");
    let _ = fs::remove_dir_all(&root);
}
