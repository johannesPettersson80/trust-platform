//! The summary a failed reload answers with (specification 25 §6.3).

use super::reload_failure_summary;

/// Each line that names a source file is one compiler error, in any letter case, `.pou` too, and
/// the count takes its singular for one. Catches the raw paths shown instead of the count, a count
/// of every line of the error, and `1 errors`.
#[test]
fn compiler_errors_are_counted_by_the_lines_that_name_a_source_file() {
    let two =
        "build failed\nsrc/Main.st:4:7: undeclared variable x\nlib/Valve.POU:12:1: type mismatch\n";
    assert_eq!(
        reload_failure_summary(two),
        "Compile failed — 2 errors. Open Problems, then try again."
    );
    assert_eq!(
        reload_failure_summary("src/main.st:1:1: expected ;"),
        "Compile failed — 1 error. Open Problems, then try again."
    );
}

/// Without such a line the answer is the first line that is not blank, trimmed; 160 characters
/// stay whole, 161 are cut to 157 and `...`; an error of blank lines says that it gave no reason.
/// Catches a summary of the whole multi-line error, a cut one character early or late, spaces left
/// before the `...`, and an empty answer.
#[test]
fn other_errors_answer_their_first_line_cut_to_160_characters() {
    assert_eq!(
        reload_failure_summary("\n  not connected to the simulator  \nmore detail"),
        "not connected to the simulator"
    );
    let whole = "a".repeat(160);
    assert_eq!(reload_failure_summary(&whole), whole);
    assert_eq!(
        reload_failure_summary(&"d".repeat(161)),
        format!("{}...", "d".repeat(157))
    );
    // 150 letters, 7 spaces and 4 letters: the first 157 characters end in the spaces, which go.
    let long = format!("{}{}{}", "b".repeat(150), " ".repeat(7), "c".repeat(4));
    assert_eq!(
        reload_failure_summary(&long),
        format!("{}...", "b".repeat(150))
    );
    assert_eq!(
        reload_failure_summary(" \n\t\n"),
        "Update did not report a reason."
    );
}
