//! Tutorial 10 (unit tests for Structured Text) as its README writes it: the tests of a copy of its
//! project run, listed and filtered, a bug that a test finds, and the runs of its Try-its.

use std::path::Path;
use std::process::Output;

use super::*;

/// The test that divides by zero, as the second Try-it writes `src/errors.st`.
const ERRORS: &str = "\
TEST_PROGRAM DivideByZero
VAR
    Divisor : INT;
END_VAR
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := INT#10 / Divisor);
END_TEST_PROGRAM
";

/// The test of both commands at once, as the third Try-it writes `src/both.st`.
const BOTH: &str = "\
TEST_FUNCTION_BLOCK StopWins
VAR
    Dut : StartStopLatch;
END_VAR
Dut(StartCmd := TRUE, StopCmd := TRUE);
ASSERT_FALSE(Dut.Running);
END_TEST_FUNCTION_BLOCK
";

/// The latch's `IF` as shipped, the stop first.
const SHIPPED_IF: &str = "\
IF StopCmd THEN
    Running := FALSE;
ELSIF StartCmd THEN
    Running := TRUE;
END_IF;";

/// The latch's `IF` that the third Try-it writes, the start first.
const START_WINS: &str =
    "IF StartCmd THEN Running := TRUE; ELSIF StopCmd THEN Running := FALSE; END_IF;";

/// The README of tutorial 10.
fn readme() -> String {
    std::fs::read_to_string(tutorial_project_path("10_unit_testing_101").join("README.md"))
        .expect("tutorial 10's README")
}

/// A copy of tutorial 10's project, as step 1 makes it.
fn copied_project() -> PathBuf {
    let project = unique_temp_dir("tutorial-10");
    copy_folder(&tutorial_project_path("10_unit_testing_101"), &project);
    project
}

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create the copy");
    for entry in std::fs::read_dir(from).expect("read the project") {
        let entry = entry.expect("a project entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("its type").is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}

/// `trust-dev test --project .` with `args`, run in `project` as the README has the reader run it.
fn test_in(project: &Path, args: &[&str]) -> Output {
    trust_dev_command()
        .args(["test", "--project", "."])
        .args(args)
        .current_dir(project)
        .output()
        .expect("run trust-dev test")
}

/// What a terminal shows of `output`: its standard output, then its standard error, with the
/// paths written with `/`, as the README's Linux and macOS write them (Windows writes `\`).
fn shown(output: &Output) -> String {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}

/// `text` with each time that varies from run to run, such as `[0ms]` and `(265ms)`, written as
/// `[…ms]` and `(…ms)`.
fn without_times(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(['[', '(']) {
        let (before, from) = rest.split_at(at);
        kept.push_str(before);
        let (open, after) = from.split_at(1);
        let close = if open == "[" { "ms]" } else { "ms)" };
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        kept.push_str(open);
        if digits > 0 && after[digits..].starts_with(close) {
            kept.push('…');
            kept.push_str(close);
            rest = &after[digits + close.len()..];
        } else {
            rest = after;
        }
    }
    kept.push_str(rest);
    kept
}

/// The answers the README quotes, its `text` blocks, in their order, without the indentation of
/// the step that holds them.
fn quoted_answers(readme: &str) -> Vec<String> {
    let mut answers = Vec::new();
    let mut open: Option<(usize, Vec<String>)> = None;
    for line in readme.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some((fence, lines)) = open.as_mut() {
            if trimmed == "```" && indent == *fence {
                answers.push(lines.join("\n"));
                open = None;
            } else {
                lines.push(line.get(*fence..).unwrap_or_default().to_string());
            }
        } else if trimmed == "```text" {
            open = Some((indent, Vec::new()));
        }
    }
    answers
}

/// Replaces `from` with `to` once in the project's `file`, as the README has the reader edit it.
fn edit(project: &Path, file: &str, from: &str, to: &str) {
    let path = project.join(file);
    let text = std::fs::read_to_string(&path).expect("read the file");
    assert_eq!(text.matches(from).count(), 1, "{file} holds `{from}` once");
    std::fs::write(&path, text.replacen(from, to, 1)).expect("write the file");
}

#[test]
fn tutorial_10_runs_lists_filters_and_finds_a_bug_as_its_readme_writes_it() {
    // examples/tutorials/10_unit_testing_101/README.md (specification 22, "Test POU discovery",
    // "Test case execution" and "Test reporting and CI output"; specification 07, MP-014). On a
    // copy of the project, step 4's run, step 5's list and step 7's run without the limit answer
    // what the README quotes, the times aside; step 6's filter runs StartStopSequence alone, in
    // either case; with the limit back the three tests pass and the exit code is 0. Without the
    // assertion of line 6, the missing limit fails the ASSERT_FALSE of line 21, then line 17, so
    // step 7's test ended at its first failed assertion, as "What happened" says; the JSON report
    // types the failed assertion's values, INT 5 expected and 7 actual. Catches a README that
    // quotes what the runner no longer prints, a run that goes on after a failed assertion, and a
    // failure reported without its line, its message or exit code 1.
    let readme = readme();
    for command in [
        "trust-dev test --project .\n",
        "trust-dev test --project . --list",
        "trust-dev test --project . --filter StartStop",
        "`--filter startstop`",
        "write `LimitAdd := Sum;` in place of\n   `LimitAdd := MaxValue;`",
    ] {
        assert!(readme.contains(command), "the README has `{command}`");
    }
    let quoted = quoted_answers(&readme);
    assert_eq!(quoted.len(), 3, "steps 4, 5 and 7 quote an answer");
    let project = copied_project();

    let run = test_in(&project, &[]);
    let list = test_in(&project, &["--list"]);
    let filtered = test_in(&project, &["--filter", "StartStop"]);
    let lower = test_in(&project, &["--filter", "startstop"]);
    edit(
        &project,
        "src/main.st",
        "    LimitAdd := MaxValue;",
        "    LimitAdd := Sum;",
    );
    let broken = test_in(&project, &[]);
    let json = test_in(&project, &["--output", "json", "--filter", "LimitAdd"]);
    let tests = std::fs::read_to_string(project.join("src/tests.st")).expect("read the tests");
    let second: String = tests
        .lines()
        .skip(5)
        .take(4)
        .map(|line| format!("{line}\n"))
        .collect();
    edit(&project, "src/tests.st", &second, "");
    let later = test_in(&project, &["--filter", "LimitAdd"]);
    std::fs::write(project.join("src/tests.st"), &tests).expect("restore the tests");
    edit(
        &project,
        "src/main.st",
        "    LimitAdd := Sum;\nELSE",
        "    LimitAdd := MaxValue;\nELSE",
    );
    let fixed = test_in(&project, &[]);
    let _ = std::fs::remove_dir_all(&project);

    assert_eq!(run.status.code(), Some(0), "step 4: {}", shown(&run));
    assert_eq!(
        without_times(shown(&run).trim_end()),
        without_times(&quoted[0]),
        "step 4's answer"
    );
    assert_eq!(shown(&list).trim_end(), quoted[1], "step 5's answer");
    for (case, output) in [("StartStop", &filtered), ("startstop", &lower)] {
        let text = shown(output);
        assert!(
            text.contains("PASS [1/1] TEST_FUNCTION_BLOCK::StartStopSequence")
                && text.contains("1 passed, 0 failed, 0 errors"),
            "--filter {case} runs StartStopSequence alone: {text}"
        );
    }
    assert!(readme.contains("`1 passed, 0 failed, 0 errors`"));
    let step_7 = format!(
        "{}\nexit {}",
        shown(&broken).trim_end(),
        broken.status.code().unwrap_or(-1)
    );
    assert_eq!(
        without_times(&step_7),
        without_times(&quoted[2]),
        "step 7's answer"
    );
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).expect("a JSON report");
    assert_eq!(
        report["tests"][0]["assertion"],
        serde_json::json!({
            "name": "ASSERT_EQUAL",
            "expected": { "type": "INT", "value": 5 },
            "actual": { "type": "INT", "value": 7 },
        }),
        "the failed assertion's values: {report}"
    );
    let later = shown(&later);
    assert!(
        later.contains("FAIL [1/1] TEST_PROGRAM::LimitAddAndScaling src/tests.st:17")
            && later.contains("reason   : ASSERT_FALSE expected FALSE, got TRUE"),
        "without the assertion of line 6, the ASSERT_FALSE fails: {later}"
    );
    assert_eq!(
        fixed.status.code(),
        Some(0),
        "the limit back: {}",
        shown(&fixed)
    );
    assert!(shown(&fixed).contains("3 passed, 0 failed, 0 errors"));
}

#[test]
fn tutorial_10_try_its_answer_as_its_readme_writes_them() {
    // examples/tutorials/10_unit_testing_101/README.md, "Try it yourself" (specification 22, "Test
    // case execution", "Test POU discovery" and "Test reporting and CI output"): a filter that
    // matches no test runs none, says so with the count it filtered out and ends with 0; a test
    // in src/errors.st that divides by zero ends as an error, not a failure, and runs first, as
    // the files are ordered; with the latch's start winning the three tests still pass, and the
    // test StopWins of src/both.st fails at its line 6, then passes on the latch as shipped.
    // Catches a filter without its message or with a failing exit code, a runtime error counted
    // as a failure, tests run out of their files' order, and a README that quotes what the runner
    // no longer prints.
    let readme = readme();
    let project = copied_project();
    let nothing = test_in(&project, &["--filter", "Pump"]);

    std::fs::write(project.join("src/errors.st"), ERRORS).expect("write src/errors.st");
    let error = test_in(&project, &[]);
    std::fs::remove_file(project.join("src/errors.st")).expect("rm src/errors.st");

    edit(&project, "src/main.st", SHIPPED_IF, START_WINS);
    let unchecked = test_in(&project, &[]);
    std::fs::write(project.join("src/both.st"), BOTH).expect("write src/both.st");
    let checked = test_in(&project, &[]);
    edit(&project, "src/main.st", START_WINS, SHIPPED_IF);
    let shipped = test_in(&project, &[]);
    let _ = std::fs::remove_dir_all(&project);

    for (here_document, file) in [(ERRORS, "src/errors.st"), (BOTH, "src/both.st")] {
        let indented: String = here_document
            .lines()
            .map(|line| format!("  {line}\n"))
            .collect();
        assert!(
            readme.contains(&format!("cat > {file} <<'ST'\n{indented}  ST\n")),
            "the README writes {file} as the test does"
        );
    }
    assert!(
        readme.contains(&format!("write\n  `{START_WINS}` in place of the")),
        "the README writes the start-first IF as the test does"
    );

    let refusal = "0 tests matched filter \"Pump\" (3 tests discovered, all filtered out)";
    assert!(shown(&nothing).contains(refusal), "{}", shown(&nothing));
    assert_eq!(
        nothing.status.code(),
        Some(0),
        "a filter that matches nothing"
    );
    assert!(readme.contains(&format!("`{refusal}`")));

    let error_text = shown(&error);
    assert_eq!(error.status.code(), Some(1), "{error_text}");
    for quoted in [
        "ERROR [1/4] TEST_PROGRAM::DivideByZero src/errors.st:5",
        "reason   : division by zero",
        "3 passed, 0 failed, 1 errors",
    ] {
        assert!(
            error_text.contains(quoted),
            "the run shows `{quoted}`: {error_text}"
        );
        assert!(
            readme.contains(&format!("`{quoted}`")),
            "the README quotes `{quoted}`"
        );
    }

    assert_eq!(unchecked.status.code(), Some(0), "{}", shown(&unchecked));
    assert!(shown(&unchecked).contains("3 passed, 0 failed, 0 errors"));
    let checked_text = shown(&checked);
    assert_eq!(checked.status.code(), Some(1), "{checked_text}");
    for quoted in [
        "FAIL [1/4] TEST_FUNCTION_BLOCK::StopWins src/both.st:6",
        "ASSERT_FALSE expected FALSE, got TRUE",
    ] {
        assert!(
            checked_text.contains(quoted),
            "the run shows `{quoted}`: {checked_text}"
        );
        assert!(
            readme.contains(&format!("`{quoted}`")),
            "the README quotes `{quoted}`"
        );
    }
    assert_eq!(shipped.status.code(), Some(0), "{}", shown(&shipped));
    assert!(shown(&shipped).contains("4 passed, 0 failed, 0 errors"));
}
