//! The command line of the `trust-control-studio` program (specification 35, §0.1b) on a copy of the
//! PID commissioning example: `open` lists the loops and opens one, `--json` is exactly the method's
//! answer, and the exit statuses hold for usage errors and refusals.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::messages::OperationUpdate;
use trust_control_studio::commands::{ControlStudio, INPUTS, OPEN};
use trust_control_studio::operation::UpdateSink;

const EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control-pid-commissioning"
);
const LOOP_ID: &str = "control-pid-commissioning";

/// A folder of the test, removed when the test ends, whether or not its assertions held.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create the copy");
    for entry in fs::read_dir(from).expect("read the example") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("its type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}

/// A copy of the example as a project, in a folder of its own.
fn project_copy(name: &str) -> (Scratch, String, String) {
    let scratch = Scratch(trust_test_support::unique_temp_path(&format!(
        "trust-engine-cli-{name}"
    )));
    let _ = fs::remove_dir_all(&scratch.0);
    let project = scratch.0.join("project");
    copy_dir(Path::new(EXAMPLE), &project);
    let journal = scratch.0.join("journal");
    let project = project.display().to_string();
    let journal = journal.display().to_string();
    (scratch, project, journal)
}

/// The command line, run with `arguments`.
fn run(arguments: &[&str]) -> Output {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-control-studio").expect(
            "Cargo must provide trust-control-studio binary while executing integration tests",
        ),
    )
    .args(arguments)
    .output()
    .expect("the program runs")
}

/// The answer that the method gives for `request`, computed in this process.
fn method_answer(command: &str, request: Value) -> Value {
    struct Silent;

    impl UpdateSink for Silent {
        fn send(&self, _update: OperationUpdate) {}
    }

    ControlStudio::new(Arc::new(Silent))
        .execute(command, &[request])
        .expect("the method answers")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn open_lists_the_loops_opens_one_and_json_is_the_method_answer() {
    let (scratch, project, journal) = project_copy("open");

    let listed = run(&[
        "open",
        "--project",
        project.as_str(),
        "--journal-dir",
        journal.as_str(),
    ]);
    assert_eq!(listed.status.code(), Some(0), "{}", text(&listed.stderr));
    assert!(
        text(&listed.stdout).contains(LOOP_ID),
        "{}",
        text(&listed.stdout)
    );

    let listed_json = run(&[
        "open",
        "--project",
        project.as_str(),
        "--journal-dir",
        journal.as_str(),
        "--json",
    ]);
    assert_eq!(
        listed_json.status.code(),
        Some(0),
        "{}",
        text(&listed_json.stderr)
    );
    let answer: Value = serde_json::from_slice(&listed_json.stdout).expect("JSON");
    assert_eq!(
        answer,
        method_answer(
            OPEN,
            json!({ "projectRoot": project, "runtimeId": "simulator", "storageFolder": journal })
        )
    );

    let opened = run(&[
        "open",
        "--project",
        project.as_str(),
        "--journal-dir",
        journal.as_str(),
        "--loop",
        LOOP_ID,
    ]);
    assert_eq!(opened.status.code(), Some(0), "{}", text(&opened.stderr));
    let opened_text = text(&opened.stdout);
    assert!(
        opened_text.contains("Identify: Needs data"),
        "{opened_text}"
    );
    assert!(
        opened_text.contains("Journaled operations: none"),
        "{opened_text}"
    );

    let opened_json = run(&[
        "open",
        "--project",
        project.as_str(),
        "--journal-dir",
        journal.as_str(),
        "--loop",
        LOOP_ID,
        "--json",
    ]);
    assert_eq!(
        opened_json.status.code(),
        Some(0),
        "{}",
        text(&opened_json.stderr)
    );
    let answer: Value = serde_json::from_slice(&opened_json.stdout).expect("JSON");
    assert_eq!(
        answer,
        method_answer(
            OPEN,
            json!({
                "projectRoot": project,
                "runtimeId": "simulator",
                "storageFolder": journal,
                "loopId": LOOP_ID,
            })
        )
    );
    assert!(
        !Path::new(&journal).exists(),
        "opening a loop writes no journal"
    );
    drop(scratch);
}

#[test]
fn an_unknown_verb_or_a_token_argument_is_a_usage_error_with_status_2() {
    assert_eq!(run(&["frobnicate"]).status.code(), Some(2));
    assert_eq!(
        run(&["open", "--token", "secret"]).status.code(),
        Some(2),
        "the token is never an argument"
    );
    assert_eq!(
        run(&["open", "--endpoint", "http://runtime"]).status.code(),
        Some(2),
        "an endpoint is tcp:// or unix://"
    );
}

#[test]
fn a_refused_loop_is_status_1_with_the_failure_on_standard_error() {
    let (scratch, project, journal) = project_copy("refused");
    let refused = run(&[
        "open",
        "--project",
        project.as_str(),
        "--journal-dir",
        journal.as_str(),
        "--loop",
        "no-such-loop",
    ]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused.stderr));
    assert!(refused.stdout.is_empty(), "{}", text(&refused.stdout));
    assert!(
        text(&refused.stderr).contains("This project declares no usable loop no-such-loop."),
        "{}",
        text(&refused.stderr)
    );
    drop(scratch);
}

#[test]
fn cancel_names_only_an_operation_journaled_for_the_loop() {
    let (scratch, project, journal) = project_copy("cancel");
    let cancelled = run(&[
        "cancel",
        "--project",
        project.as_str(),
        "--journal-dir",
        journal.as_str(),
        "--loop",
        LOOP_ID,
        "operation-1",
    ]);
    assert_eq!(
        cancelled.status.code(),
        Some(2),
        "a named operation that is not journaled is a usage error, even when nothing is journaled: {}",
        text(&cancelled.stderr)
    );
    assert!(
        text(&cancelled.stderr)
            .contains("Operation operation-1 is not journaled for loop control-pid-commissioning."),
        "{}",
        text(&cancelled.stderr)
    );
    drop(scratch);
}

/// The arguments as the command line takes them.
fn borrowed(arguments: &[String]) -> Vec<&str> {
    arguments.iter().map(String::as_str).collect()
}

/// The command line, run with `arguments` and `input` on standard input.
fn run_with_input(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-control-studio").expect(
            "Cargo must provide trust-control-studio binary while executing integration tests",
        ),
    )
    .args(arguments)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("the program runs");
    child
        .stdin
        .take()
        .expect("its standard input")
        .write_all(input.as_bytes())
        .expect("the input is written");
    child.wait_with_output().expect("the program ends")
}

/// Specification 35 §0.1b, A8.1 and B4.3: `inputs` records one change of the study's inputs, from a file or from standard input,
/// and `--json` is the method's answer. Catches a command line that cannot declare the requirements Tune designs to (only `serve`
/// could), a change the verb reshapes, and a change that is no JSON object taken for a refusal (status 1) instead of a usage
/// error (status 2).
#[test]
fn inputs_records_a_requirements_change_and_json_is_the_method_answer() {
    let (scratch, project, journal) = project_copy("inputs");
    let (_method, method_project, method_journal) = project_copy("inputs-method");
    // The requirements the page's editor sends for one controlled output (B4.3).
    let change = json!({
        "referenceChange": 5.0,
        "maximumError": 0.5,
        "settlingBand": 0.2,
        "settlingTimeSeconds": 40.0,
        "durationSeconds": 80,
        "scenarioDefinitions": []
    });
    let file = scratch.0.join("change.json");
    fs::write(&file, change.to_string()).expect("the change file");
    let file = file.display().to_string();
    let inputs = |extra: &[&'static str], change: &str| -> Vec<String> {
        let mut arguments = vec![
            "inputs".to_owned(),
            "--project".to_owned(),
            project.clone(),
            "--journal-dir".to_owned(),
            journal.clone(),
            "--loop".to_owned(),
            LOOP_ID.to_owned(),
            "--change".to_owned(),
            change.to_owned(),
        ];
        arguments.extend(extra.iter().map(|argument| (*argument).to_owned()));
        arguments
    };
    let recorded = run(&borrowed(&inputs(&["--json"], &file)));
    assert_eq!(
        recorded.status.code(),
        Some(0),
        "{}",
        text(&recorded.stderr)
    );
    let answer: Value = serde_json::from_slice(&recorded.stdout).expect("JSON");
    assert_eq!(
        answer,
        method_answer(
            INPUTS,
            json!({ "projectRoot": method_project, "runtimeId": "simulator", "loopId": LOOP_ID,
                "change": change, "storageFolder": method_journal })
        )
    );
    let requirements = &answer["result"]["inputs"]["requirements"];
    assert_eq!(requirements["settlingTimeSeconds"], json!(40.0), "{answer}");
    assert_eq!(requirements["maximumError"], json!(0.5), "{answer}");
    assert_eq!(answer["result"]["changed"], json!(true), "{answer}");

    // The same change again, from standard input: the study already holds it.
    let again = run_with_input(&borrowed(&inputs(&[], "-")), &change.to_string());
    assert_eq!(again.status.code(), Some(0), "{}", text(&again.stderr));
    let shown = text(&again.stdout);
    assert!(
        shown.contains("The study already held these inputs."),
        "{shown}"
    );
    assert!(shown.contains("settlingTimeSeconds: 40"), "{shown}");
    assert!(shown.contains("Stages:"), "{shown}");

    let not_an_object = run_with_input(&borrowed(&inputs(&[], "-")), "[1, 2]");
    assert_eq!(
        not_an_object.status.code(),
        Some(2),
        "{}",
        text(&not_an_object.stderr)
    );
    assert!(
        text(&not_an_object.stderr).contains("is not a JSON object"),
        "{}",
        text(&not_an_object.stderr)
    );
}
