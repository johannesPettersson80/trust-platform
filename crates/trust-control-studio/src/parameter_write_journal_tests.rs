use std::fs;
use std::path::PathBuf;

use trust_control::analysis::parameter_writes::ParameterWriteAction;
use trust_control::loop_definition::LoopDefinition;

use super::*;

/// Lesson 45 (root locus): its project and loop name the journal file.
const LESSON_45: &str = include_str!(
    "../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"
);

fn lesson_45() -> LoopDefinition {
    LoopDefinition::decode(LESSON_45).expect("the shipped lesson decodes")
}

/// A folder of the test's own, empty at its start.
fn folder(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "trust-parameter-write-journal-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    root
}

#[test]
fn a_journaled_write_is_read_until_it_is_cleared() {
    let root = folder("roundtrip");
    let journal = WriteJournal::of(&root, &lesson_45());
    assert_eq!(journal.read().expect("no journal yet"), None);
    journal
        .record("transaction-1", ParameterWriteAction::Apply)
        .expect("the identifier is journaled");
    assert_eq!(
        journal.read().expect("the journal is read"),
        Some(JournaledWrite {
            transaction_id: "transaction-1".to_string(),
            action: ParameterWriteAction::Apply,
        })
    );
    journal.clear().expect("the journal is cleared");
    assert_eq!(journal.read().expect("the journal is read"), None);
    journal
        .clear()
        .expect("clearing an empty journal is not an error");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_restore_is_journaled_as_a_restore() {
    let root = folder("restore");
    let journal = WriteJournal::of(&root, &lesson_45());
    journal
        .record("transaction-2", ParameterWriteAction::Restore)
        .expect("the identifier is journaled");
    let written = journal.read().expect("read").expect("a journaled write");
    assert_eq!(written.action, ParameterWriteAction::Restore);
    journal.clear().expect("cleared");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_journal_file_is_one_file_per_loop_in_the_control_studio_folder() {
    let root = folder("path");
    let journal = WriteJournal::of(&root, &lesson_45());
    assert!(journal.file.starts_with(root.join("control-studio")));
    assert!(journal
        .file
        .to_string_lossy()
        .ends_with(".parameter-write.json"));
}

#[test]
fn a_damaged_journal_is_refused_rather_than_ignored() {
    let root = folder("damaged");
    let journal = WriteJournal::of(&root, &lesson_45());
    fs::create_dir_all(journal.file.parent().expect("the folder")).expect("the folder is made");
    fs::write(&journal.file, "not a journal").expect("the file is written");
    let error = journal.read().expect_err("a damaged journal is refused");
    assert!(
        error.message.contains("could not be used"),
        "{}",
        error.message
    );
    let _ = fs::remove_dir_all(&root);
}
