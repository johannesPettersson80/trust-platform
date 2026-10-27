//! `trust-runtime ctl --project <folder> endpoint --json` as Control Studio's command line reads it
//! (specification 11 §6.8.0a; specification 35, §0.1b). The stand-in for trust-runtime is a shell
//! script, so the process tests are Unix only.

use super::*;

/// A folder of the test, removed when the test ends, whether or not it passed.
#[cfg(unix)]
struct Scratch(PathBuf);

#[cfg(unix)]
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn scratch(name: &str) -> Scratch {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let folder = std::env::temp_dir().join(format!(
        "trust-cs-endpoint-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&folder).expect("the scratch folder");
    Scratch(folder)
}

/// Wrong behaviour caught: a missing trust-runtime reported without saying how else to name the
/// runtime, or as an engine defect instead of a setting to fix.
#[test]
fn a_missing_trust_runtime_names_itself_and_the_endpoint_option() {
    let failure = RuntimeTool::new(Some(PathBuf::from("/nonexistent/trust-runtime-test")))
        .project_endpoint(Path::new("/project"))
        .expect_err("no trust-runtime to run");
    assert!(
        failure.message.contains("/nonexistent/trust-runtime-test"),
        "{}",
        failure.message
    );
    assert!(
        failure.message.contains("--endpoint"),
        "{}",
        failure.message
    );
    assert_eq!(failure.cause, Cause::Settings);
    assert_eq!(failure.next_action, NextAction::ConnectRuntime);
}

/// Wrong behaviour caught: the engine reading the project's files itself instead of asking
/// trust-runtime, asking for another project, or dropping the token that the runtime generated.
#[cfg(unix)]
#[test]
fn the_project_runtime_and_its_token_are_asked_of_trust_runtime() {
    let folder = scratch("asked");
    let tool = stub_tool(
        &folder.0,
        r#"{"endpoint":"unix:///work/plc/.trust-runtime/control.sock","auth_token":"generated-token","source":"endpoint_file","endpoint_file_skipped":null}"#,
        0,
    );

    let reported = tool
        .project_endpoint(Path::new("/work/plc"))
        .expect("the reported endpoint");

    assert_eq!(
        reported,
        ProjectEndpoint {
            endpoint: "unix:///work/plc/.trust-runtime/control.sock".to_string(),
            auth_token: Some("generated-token".to_string()),
        }
    );
    assert_eq!(
        std::fs::read_to_string(folder.0.join("arguments")).expect("the recorded arguments"),
        "ctl\n--project\n/work/plc\nendpoint\n--json\n"
    );
}

/// Wrong behaviour caught: a JSON `null` token turned into the text "null".
#[cfg(unix)]
#[test]
fn a_report_without_a_token_offers_none() {
    let folder = scratch("no-token");
    let tool = stub_tool(
        &folder.0,
        r#"{"endpoint":"tcp://127.0.0.1:9100","auth_token":null,"source":"configured","endpoint_file_skipped":null}"#,
        0,
    );

    let reported = tool
        .project_endpoint(Path::new("/work/plc"))
        .expect("the reported endpoint");

    assert_eq!(reported.endpoint, "tcp://127.0.0.1:9100");
    assert_eq!(reported.auth_token, None);
}

/// Wrong behaviour caught: trust-runtime's reason, such as a runtime that is not running, lost
/// behind a generic message, or the failure not naming `--endpoint`.
#[cfg(unix)]
#[test]
fn a_reported_failure_keeps_the_reason_and_names_the_endpoint_option() {
    let folder = scratch("not-running");
    let tool = stub_tool(
        &folder.0,
        r#"{"error":{"code":"not_running","message":"runtime is not running: no endpoint file at /work/plc/.trust-runtime/control.json"}}"#,
        1,
    );

    let failure = tool
        .project_endpoint(Path::new("/work/plc"))
        .expect_err("the runtime is not running");

    assert_eq!(
        failure.message,
        "The project's runtime cannot be found (runtime is not running: no endpoint file at \
         /work/plc/.trust-runtime/control.json); start it, or pass --endpoint tcp://host:port or \
         unix:///path."
    );
    assert_eq!(failure.next_action, NextAction::ConnectRuntime);
}

/// Wrong behaviour caught: output that is not the documented answer taken as an endpoint.
#[cfg(unix)]
#[test]
fn an_answer_that_is_not_the_report_is_no_endpoint() {
    let folder = scratch("no-answer");
    let tool = stub_tool(&folder.0, "not json", 2);

    let failure = tool
        .project_endpoint(Path::new("/work/plc"))
        .expect_err("no endpoint in the answer");

    assert!(
        failure
            .message
            .starts_with("trust-runtime gave no endpoint for the project ("),
        "{}",
        failure.message
    );
    assert!(
        failure.message.contains("--endpoint"),
        "{}",
        failure.message
    );
    assert_eq!(failure.next_action, NextAction::ConnectRuntime);
}

/// Wrong behaviour caught: trust-runtime started while another process still holds its file open
/// for writing (`ETXTBSY`, as for a moment after it was written) reported as missing instead of
/// started again; and another failure, or a busy file that stays busy, retried for ever.
#[test]
fn a_busy_executable_is_started_again_and_other_failures_are_not() {
    let busy = || std::io::Error::from(std::io::ErrorKind::ExecutableFileBusy);
    let mut calls = 0;
    let started = started_once_free(|| {
        calls += 1;
        if calls < 3 {
            Err(busy())
        } else {
            Ok(std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    });
    assert!(started.is_ok(), "{started:?}");
    assert_eq!(calls, 3, "started once the file was free");

    let mut calls = 0;
    let missing = started_once_free(|| {
        calls += 1;
        Err(std::io::Error::from(std::io::ErrorKind::NotFound))
    });
    assert_eq!(
        missing.map_err(|error| error.kind()).err(),
        Some(std::io::ErrorKind::NotFound)
    );
    assert_eq!(calls, 1, "a missing file is not started again");

    let mut calls = 0;
    let stuck = started_once_free(|| {
        calls += 1;
        Err(busy())
    });
    assert!(stuck.is_err());
    assert_eq!(
        calls, 11,
        "a file that stays busy is given up after ten more tries"
    );
}
