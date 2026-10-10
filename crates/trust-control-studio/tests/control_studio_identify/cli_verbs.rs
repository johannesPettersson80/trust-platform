//! The verbs of `control_studio_identify` that need no runtime (checklist V1.6 and V1.10c; specification 35, §0.1b): `cancel`
//! with nothing journaled, and the usage errors of `identify`. Both run the built program on a copy of lesson 30; the helpers
//! are those of the crate root, `control_studio_identify.rs`.

use std::fs;

use super::{strings, Findings, Program, Scratch, LOOP_ID, RUN_LIMIT};

/// `cancel` with nothing journaled for the loop says so and exits with status 0 (§0.1b). The runtime is not running: the
/// answer comes from the loop's journal alone.
///
/// Run time: well under a second; the copy of the example is the only work.
#[test]
fn cancel_with_an_empty_journal_says_so_and_exits_zero() {
    let scratch = Scratch::new("cancel");
    let journal = scratch.folder.join("journal");
    fs::create_dir_all(&journal).expect("an empty journal folder");
    let project = scratch.project.display().to_string();
    let journal_dir = journal.display().to_string();
    let mut program = Program::start(
        &strings(&[
            "cancel",
            "--project",
            &project,
            "--loop",
            LOOP_ID,
            "--journal-dir",
            &journal_dir,
        ]),
        false,
        &scratch.folder,
        "cancel",
    );
    let status = program.wait(RUN_LIMIT);
    let said = format!("{}\n{}", program.stdout(), program.stderr());
    assert_eq!(
        status.code(),
        Some(0),
        "cancel with nothing journaled finishes:\n{said}"
    );
    assert!(
        said.contains("No operation is journaled"),
        "cancel says that nothing is journaled:\n{said}"
    );
}

/// Two of the words that say what `identify` does (`--method`, `--recording`, `--resume`, `--reset`) are a usage error, status
/// 2, and the program does nothing: it writes no journal and no study. No runtime is needed.
///
/// Run time: well under a second.
#[test]
fn identify_with_a_second_name_on_one_loop_is_a_usage_error_for_conflicting_flags() {
    let scratch = Scratch::new("usage");
    let journal = scratch.folder.join("journal");
    let project = scratch.project.display().to_string();
    let journal_dir = journal.display().to_string();
    let conflicts: [&[&str]; 3] = [
        &["--reset", "--resume"],
        &["--method", "step", "--reset"],
        &["--recording", "rec-1", "--resume"],
    ];
    let mut findings = Findings::default();
    for flags in conflicts {
        let mut arguments = strings(&[
            "identify",
            "--project",
            &project,
            "--loop",
            LOOP_ID,
            "--journal-dir",
            &journal_dir,
        ]);
        arguments.extend(strings(flags));
        let mut program = Program::start(&arguments, false, &scratch.folder, "usage");
        let status = program.wait(RUN_LIMIT);
        findings.check(
            status.code() == Some(2),
            format!(
                "{flags:?} should be a usage error (status 2), the program ended with {status}:\n{}",
                program.stderr()
            ),
        );
    }
    findings.check(!journal.exists(), "a usage error should write no journal");
    findings.check(
        !scratch.project.join("control-studio").exists(),
        "a usage error should write no study",
    );
    findings.conclude();
}
