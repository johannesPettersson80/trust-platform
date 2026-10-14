//! Keep in project against the real `trust-lsp` (specification 35, B6.5 and §0.1b; specification 14 §7.8): the review and the
//! confirmation of a Keep on copies of two examples. The test needs the `trust-lsp` executable of the target folder
//! (`cargo build -p trust-lsp`), or `TRUST_LSP_BIN`. No runtime runs here: the running values are given, so no time scale applies.
//!
//! - `examples/control/06-pid-tuning-methods/30-model-based-pi-tuning` declares its record by hand (`P1.Parameters`, in
//!   `src/Main.st`): a kept gain takes the typed literal of its initializer, and nothing else changes.
//! - `examples/room_heating` declares `Main.TIC101_Parameters` through Create PLC code: the gains are kept in `plant.toml` and in
//!   the generated declaration, and the other instance keeps its own values.

// The shared helpers serve the other test files of this crate. This file uses the `trust-lsp` helper and the folder helpers.
mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::{copy_dir, trust_lsp, RemoveDir};
use trust_control::commissioning::keep_texts;
use trust_control::failure::Failure;
use trust_control::loop_definition::LoopDefinition;
use trust_control_studio::language_tool::LanguageTool;
use trust_control_studio::workflow::keep::{confirm_keep, review_keep};

const HAND_WRITTEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning"
);
const ROOM_HEATING: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/room_heating");

/// A copy of an example in a fresh temporary folder, removed when the returned guard drops. The path is canonical, as the tool
/// spells the files it names.
fn copy_of(example: &str, name: &str) -> (PathBuf, RemoveDir) {
    let root = std::env::temp_dir().join(format!("trust-keep-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    copy_dir(Path::new(example), &root);
    let root = fs::canonicalize(&root).expect("the copy");
    (root.clone(), RemoveDir(root))
}

fn load_loop(root: &Path, file: &str) -> LoopDefinition {
    let text = fs::read_to_string(root.join(file)).expect("the loop file");
    LoopDefinition::decode(&text).expect("the loop decodes")
}

fn values(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), *value))
        .collect()
}

/// Every file of the copy with its bytes, so that a refused keep can be shown to have written nothing.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, folder: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(folder).expect("a folder of the copy") {
            let path = entry.expect("an entry").path();
            // The language tool's index cache and library lock, which loading the project writes for `trust-lsp command` as for
            // the editor's language server and `trust-lsp check`, are not changes of Keep.
            if path
                .file_name()
                .is_some_and(|name| name == ".trust-lsp" || name == "trust-lsp.lock")
            {
                continue;
            }
            if path.is_dir() {
                collect(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).expect("inside the copy");
                files.insert(relative.to_path_buf(), fs::read(&path).expect("a file"));
            }
        }
    }
    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

/// The files that differ between two snapshots: changed, added or removed.
fn changed_files(
    before: &BTreeMap<PathBuf, Vec<u8>>,
    after: &BTreeMap<PathBuf, Vec<u8>>,
) -> Vec<PathBuf> {
    before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The failure's sentence, whole: its detail when the sentence is longer than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

/// The number that `member` holds in the settings table of `instance` in `plant.toml`.
fn setting_in_table(plant: &str, instance: &str, member: &str) -> f64 {
    let header = format!("[equipment.\"{instance}\".settings]");
    let table = plant.split_once(&header).expect("the settings table").1;
    let table = table.split("\n[").next().unwrap_or(table);
    let key = format!("\"{member}\"");
    let line = table
        .lines()
        .find(|line| line.trim_start().starts_with(&key))
        .expect("the setting");
    line.split_once('=')
        .expect("a key and a value")
        .1
        .trim()
        .trim_matches('"')
        .parse()
        .expect("a number")
}

/// The number that the initializer of `record`, declared in `src/Main.st`, gives `field`.
fn declared_field(main: &str, record: &str, field: &str) -> f64 {
    let declaration = main
        .split_once(&format!("{record} : "))
        .expect("the declaration")
        .1;
    let marker = format!("{field} := ");
    let rest = declaration
        .split_once(&marker)
        .expect("the field's initializer")
        .1;
    let number: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
        .collect();
    number.parse().expect("a number")
}

#[test]
fn a_hand_written_record_is_reviewed_and_then_kept_in_its_declaration() {
    let (root, _guard) = copy_of(HAND_WRITTEN, "hand");
    let loop_definition = load_loop(&root, "control-loop.json");
    let running = values(&[
        ("P1.Parameters.ProportionalGain", 2.5),
        ("P1.Parameters.IntegralGain", 0.2),
    ]);
    let tool = LanguageTool::new(Some(trust_lsp()));
    let before = fs::read_to_string(root.join("src/Main.st")).expect("Main.st");
    assert_eq!(before.matches("ProportionalGain := LREAL#0.0").count(), 1);
    assert_eq!(before.matches("IntegralGain := LREAL#0.0").count(), 1);

    let review = review_keep(&root, &loop_definition, &running, &[], &tool).expect("the review");
    assert_eq!(
        fs::read_to_string(root.join("src/Main.st")).expect("Main.st"),
        before,
        "a review writes nothing"
    );
    assert_eq!(review.record, "P1.Parameters");
    assert_eq!(review.kept, ["IntegralGain", "ProportionalGain"]);
    assert_eq!(review.unknown, [] as [std::string::String; 0]);

    let outcome = confirm_keep(
        &root,
        &loop_definition,
        &running,
        &[],
        &tool,
        &review.identity,
    )
    .expect("the confirmation");
    assert_eq!(outcome.kept, ["IntegralGain", "ProportionalGain"]);
    let expected = before
        .replace(
            "ProportionalGain := LREAL#0.0",
            "ProportionalGain := LREAL#2.5",
        )
        .replace("IntegralGain := LREAL#0.0", "IntegralGain := LREAL#0.2");
    assert_eq!(
        fs::read_to_string(root.join("src/Main.st")).expect("Main.st"),
        expected,
        "the initializer holds the values in the same form, and nothing else changed"
    );
}

#[test]
fn a_review_whose_file_changes_before_confirmation_is_refused_and_writes_nothing() {
    let (root, _guard) = copy_of(HAND_WRITTEN, "changed");
    let loop_definition = load_loop(&root, "control-loop.json");
    let running = values(&[("P1.Parameters.ProportionalGain", 2.5)]);
    let tool = LanguageTool::new(Some(trust_lsp()));
    let review = review_keep(&root, &loop_definition, &running, &[], &tool).expect("the review");

    let before = fs::read_to_string(root.join("src/Main.st")).expect("Main.st");
    let edited = format!("{before}(* changed after the review *)\n");
    fs::write(root.join("src/Main.st"), &edited).expect("the change");
    let failure = confirm_keep(
        &root,
        &loop_definition,
        &running,
        &[],
        &tool,
        &review.identity,
    )
    .expect_err("the review is no longer the current one");
    assert_eq!(text_of(&failure), keep_texts::review_expired());
    assert_eq!(
        fs::read_to_string(root.join("src/Main.st")).expect("Main.st"),
        edited
    );
}

#[test]
fn a_file_with_unsaved_changes_refuses_the_keep_and_leaves_every_file_unchanged() {
    let (root, _guard) = copy_of(HAND_WRITTEN, "unsaved");
    let loop_definition = load_loop(&root, "control-loop.json");
    let running = values(&[("P1.Parameters.ProportionalGain", 2.5)]);
    let tool = LanguageTool::new(Some(trust_lsp()));
    let before = snapshot(&root);

    let failure = confirm_keep(
        &root,
        &loop_definition,
        &running,
        &[PathBuf::from("src/Main.st")],
        &tool,
        "sha256:unused",
    )
    .expect_err("the file has unsaved changes");
    assert_eq!(
        text_of(&failure),
        keep_texts::unsaved_changes("src/Main.st")
    );
    assert_eq!(
        changed_files(&before, &snapshot(&root)),
        [] as [PathBuf; 0],
        "every file is left as it was"
    );
}

#[test]
fn a_record_that_create_plc_code_declared_is_kept_in_plant_toml_and_its_declaration() {
    let (root, _guard) = copy_of(ROOM_HEATING, "generated");
    let loop_definition = load_loop(&root, "control-loops/tank-temperature.json");
    let running = values(&[
        ("Main.TIC101_Parameters.ProportionalGain", 35.5),
        ("Main.TIC101_Parameters.IntegralGain", 0.08),
    ]);
    let tool = LanguageTool::new(Some(trust_lsp()));

    let review = review_keep(&root, &loop_definition, &running, &[], &tool).expect("the review");
    assert_eq!(review.record, "Main.TIC101_Parameters");
    assert_eq!(review.kept, ["IntegralGain", "ProportionalGain"]);
    let outcome = confirm_keep(
        &root,
        &loop_definition,
        &running,
        &[],
        &tool,
        &review.identity,
    )
    .expect("the confirmation");
    assert_eq!(outcome.kept, ["IntegralGain", "ProportionalGain"]);
    assert_eq!(outcome.unknown, [] as [std::string::String; 0]);

    let plant = fs::read_to_string(root.join("plant.toml")).expect("plant.toml");
    assert_eq!(
        setting_in_table(
            &plant,
            "Main.TIC101",
            "{instance}_Parameters.ProportionalGain"
        ),
        35.5
    );
    assert_eq!(
        setting_in_table(&plant, "Main.TIC101", "{instance}_Parameters.IntegralGain"),
        0.08
    );
    assert_eq!(
        setting_in_table(
            &plant,
            "Main.TIC102",
            "{instance}_Parameters.ProportionalGain"
        ),
        28.6,
        "the other loop keeps its own value"
    );
    let main = fs::read_to_string(root.join("src/Main.st")).expect("Main.st");
    assert_eq!(
        declared_field(&main, "TIC101_Parameters", "ProportionalGain"),
        35.5
    );
    assert_eq!(
        declared_field(&main, "TIC101_Parameters", "IntegralGain"),
        0.08
    );
}

#[test]
fn an_unsaved_plant_file_refuses_the_equipment_keep_and_leaves_every_file_unchanged() {
    let (root, _guard) = copy_of(ROOM_HEATING, "unsaved-plant");
    let loop_definition = load_loop(&root, "control-loops/tank-temperature.json");
    let running = values(&[("Main.TIC101_Parameters.ProportionalGain", 35.5)]);
    let tool = LanguageTool::new(Some(trust_lsp()));
    let before = snapshot(&root);

    let failure = review_keep(
        &root,
        &loop_definition,
        &running,
        &[PathBuf::from("plant.toml")],
        &tool,
    )
    .expect_err("plant.toml has unsaved changes");
    assert_eq!(text_of(&failure), keep_texts::unsaved_changes("plant.toml"));
    assert_eq!(
        changed_files(&before, &snapshot(&root)),
        [] as [PathBuf; 0],
        "every file is left as it was"
    );
}
