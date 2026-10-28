// The tool's protocol is specification 14 §7.8: one argument on standard input, one JSON object on one line of standard output, a
// usage or I/O error as exit status 2. The process tests run a shell in place of the tool, so they are Unix only.

use serde_json::json;

use super::*;

#[test]
fn a_missing_executable_is_named_in_the_failure() {
    let tool = LanguageTool::new(Some(PathBuf::from("/nonexistent/trust-lsp-test")));
    let failure = tool
        .answer("trust-lsp.hmiEquipment", Path::new("/project"), &json!({}))
        .expect_err("no tool to run");
    assert!(
        failure.message.contains("/nonexistent/trust-lsp-test"),
        "{}",
        failure.message
    );
    assert_eq!(failure.cause, Cause::Implementation);
    assert_eq!(failure.next_action, NextAction::None);
}

#[cfg(unix)]
fn shell(script: &str, input: &str, deadline: Duration) -> Result<Value, Failure> {
    run_tool(
        Path::new("/bin/sh"),
        &[OsString::from("-c"), OsString::from(script)],
        input,
        deadline,
    )
}

#[cfg(unix)]
#[test]
fn the_argument_reaches_the_tool_and_its_one_object_is_the_answer() {
    // `cat` answers with what it reads: the argument arrives on standard input, and the answer is that object.
    let answer = shell(
        "cat",
        "{\"ok\":true,\"kept\":[\"ProportionalGain\"]}",
        DEADLINE,
    )
    .expect("an answer");
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["kept"], json!(["ProportionalGain"]));
}

#[cfg(unix)]
#[test]
fn a_failing_tool_is_named_with_its_standard_error() {
    let failure =
        shell("echo 'unknown argument' >&2; exit 2", "{}", DEADLINE).expect_err("exit status 2");
    assert!(
        failure.message.contains("unknown argument"),
        "{}",
        failure.message
    );
    assert!(failure.message.contains("/bin/sh"), "{}", failure.message);
    assert_eq!(failure.cause, Cause::Implementation);
}

#[cfg(unix)]
#[test]
fn an_answer_that_is_not_one_json_object_on_one_line_is_refused() {
    assert!(shell("echo '[1]'", "{}", DEADLINE).is_err(), "an array");
    assert!(
        shell("echo '{\"a\":1}'; echo '{\"b\":2}'", "{}", DEADLINE).is_err(),
        "two lines"
    );
    assert!(shell("true", "{}", DEADLINE).is_err(), "no answer");
}

#[cfg(unix)]
#[test]
fn a_tool_that_outlasts_its_deadline_is_killed() {
    let started = Instant::now();
    let failure =
        shell("exec sleep 30", "{}", Duration::from_millis(200)).expect_err("past its deadline");
    assert!(
        failure.message.contains("did not answer within"),
        "{}",
        failure.message
    );
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the tool is killed at its deadline, not when it ends"
    );
}
