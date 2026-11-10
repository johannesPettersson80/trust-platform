use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value as JsonValue;
use smol_str::SmolStr;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    for _ in 0..64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let seq = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = trust_test_support::unique_temp_path(&format!(
            "trust-runtime-{prefix}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!("create temp dir {}: {err}", dir.display()),
        }
    }
    panic!("failed to allocate unique temp dir for '{prefix}'")
}

fn make_runtime_toml_portable(runtime_toml: String) -> String {
    #[cfg(windows)]
    {
        runtime_toml.replacen(
            "[runtime.control]\n",
            "[runtime.control]\nendpoint = \"tcp://127.0.0.1:0\"\nauth_token = \"trust-ci-token\"\n",
            1,
        )
    }
    #[cfg(not(windows))]
    runtime_toml
}

fn write_project_fixture(root: &std::path::Path, source: &str) {
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    let runtime_toml = make_runtime_toml_portable(render_runtime_toml(&SmolStr::new("main"), 100));
    let io_template = build_io_config_auto("loopback").expect("build loopback io template");
    let io_toml = render_io_toml(&io_template);
    std::fs::write(root.join("runtime.toml"), runtime_toml).expect("write runtime.toml");
    std::fs::write(root.join("io.toml"), io_toml).expect("write io.toml");
    std::fs::write(root.join("src").join("main.st"), source).expect("write main source");
}

fn run_check(project: &std::path::Path) -> Output {
    run_check_with_args(project, &["--json"])
}

fn run_check_with_args(project: &std::path::Path, args: &[&str]) -> Output {
    let mut command = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    );
    command
        .arg("check")
        .arg("--project")
        .arg(project)
        .args(args);
    command.output().expect("run trust-runtime check")
}

fn json_stdout(output: &Output) -> JsonValue {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout should be JSON ({error}); stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn reported_path_ends_with(path: &str, suffix: &str) -> bool {
    path.replace('\\', "/").ends_with(suffix)
}

#[test]
fn check_accepts_project_without_writing_program_image() {
    let project = unique_temp_dir("check-ok");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := Counter + 1;
END_PROGRAM
"#,
    );

    let program_path = project.join("program.tpi");
    std::fs::write(&program_path, b"existing-image").expect("write existing program image");

    let output = run_check(&project);
    assert!(
        output.status.success(),
        "expected check success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = json_stdout(&output);
    assert_eq!(payload["version"], 1);
    assert_eq!(payload["command"], "check");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["errors"], 0);
    assert_eq!(payload["warnings"], 0);
    assert_eq!(
        payload["source_count"].as_u64(),
        payload["sources"]
            .as_array()
            .map(|sources| sources.len() as u64)
    );
    assert!(
        payload.get("bytecode_size").is_none(),
        "the check reports no bytecode size"
    );
    assert_eq!(
        std::fs::read(&program_path).expect("read existing program image"),
        b"existing-image",
        "check must not replace program.tpi"
    );
    let _ = std::fs::remove_dir_all(project);
}

/// An issue without its `file`, whose form depends on where the test's project lies.
fn issue_without_file(issue: &JsonValue) -> JsonValue {
    let mut issue = issue.clone();
    if let Some(object) = issue.as_object_mut() {
        object.remove("file");
    }
    issue
}

/// Specification 11 §6.8.1c (the CODESYS toolkit's B16): a compile that fails on its sources'
/// diagnostics gives one issue per diagnostic, where it is. Worked by hand: the statement
/// `Counter := 1` on line 6 lacks its `;`, so the parser reports `expected ';'` at the next token,
/// `END_PROGRAM`, on line 7 from column 1 to before column 12: a missing token, E002. Catches the
/// diagnostics joined into one `compile` issue that names byte offsets in its text, which the
/// toolkit had to parse.
#[test]
fn check_reports_each_compile_diagnostic_where_it_is() {
    let project = unique_temp_dir("check-compile-error");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := 1
END_PROGRAM
"#,
    );

    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(11));
    let payload = json_stdout(&output);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["status"], "failed");
    assert_eq!(payload["errors"], 1);
    let issues = payload["issues"].as_array().expect("issues");
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert!(
        reported_path_ends_with(
            issues[0]["file"].as_str().unwrap_or_default(),
            "src/main.st"
        ),
        "{}",
        issues[0]
    );
    assert_eq!(
        issue_without_file(&issues[0]),
        serde_json::json!({
            "severity": "error", "code": "E002", "message": "expected ';'",
            "line": 7, "column": 1, "end_line": 7, "end_column": 12
        })
    );
    assert!(payload["source_count"].as_u64().unwrap_or_default() >= 1);
    assert!(
        payload.get("bytecode_size").is_none(),
        "the check reports no bytecode size"
    );
    assert!(
        !project.join("program.tpi").exists(),
        "failed check must not write program.tpi"
    );
    let _ = std::fs::remove_dir_all(project);
}

/// Specification 11 §6.8.1c: each diagnostic is its own issue, and a column counts UTF-16 code
/// units, as `trust-lsp check` and the editor count it. Worked by hand: `Missing` on line 6
/// follows `Counter := (`, 12 characters, so it spans columns 13 to before 20; `Absent` on line 7
/// follows `(* größe *) Counter := (`, 24 code units (`ö` and `ß` one each, two bytes each), so it
/// spans columns 25 to before 31. Catches a column counted in bytes (27 and 33 for `Absent`), and
/// the two diagnostics joined into one issue.
#[test]
fn check_reports_each_undefined_name_at_its_utf16_column() {
    let project = unique_temp_dir("check-two-names");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := (Missing);
(* größe *) Counter := (Absent) + 1;
END_PROGRAM
"#,
    );

    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(11));
    let payload = json_stdout(&output);
    let undefined: Vec<JsonValue> = payload["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .filter(|issue| issue["code"] == "E101")
        .map(issue_without_file)
        .collect();
    assert_eq!(
        undefined,
        [
            serde_json::json!({
                "severity": "error", "code": "E101", "message": "undefined identifier 'Missing'",
                "line": 6, "column": 13, "end_line": 6, "end_column": 20
            }),
            serde_json::json!({
                "severity": "error", "code": "E101", "message": "undefined identifier 'Absent'",
                "line": 7, "column": 25, "end_line": 7, "end_column": 31
            }),
        ]
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_reports_config_error_as_json_issue() {
    let project = unique_temp_dir("check-config-error");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    let runtime_path = project.join("runtime.toml");
    let mut runtime_text = std::fs::read_to_string(&runtime_path).expect("read runtime.toml");
    runtime_text.push_str("\n[runtime.extra]\nflag = true\n");
    std::fs::write(&runtime_path, runtime_text).expect("write invalid runtime.toml");

    let output = run_check(&project);
    assert!(
        !output.status.success(),
        "expected check failure for invalid runtime.toml"
    );
    let payload = json_stdout(&output);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["status"], "failed");
    assert!(payload["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(|issue| issue["code"] == "config.runtime"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_reports_a_runtime_resource_the_configuration_lacks() {
    // The runtime runs the configuration's resource that runtime.toml names, so a name the
    // program lacks stopped it at start while the check said ok; now the check reports it.
    let project = unique_temp_dir("check-resource-mismatch");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
END_PROGRAM

CONFIGURATION Config
RESOURCE MainRes ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    let output = run_check(&project);
    assert!(
        !output.status.success(),
        "a resource the program lacks fails the check"
    );
    let payload = json_stdout(&output);
    let issue = payload["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .find(|issue| issue["code"] == "config.resource")
        .cloned()
        .unwrap_or_else(|| panic!("a resource issue in {payload}"));
    assert_eq!(
        issue["message"],
        "runtime.toml names resource 'main', but the program's configuration declares resource 'MainRes'; use the same name in both"
    );
    let runtime_path = project.join("runtime.toml");
    let text = std::fs::read_to_string(&runtime_path).expect("read runtime.toml");
    assert!(text.contains("name = \"main\""), "{text}");
    std::fs::write(
        &runtime_path,
        text.replacen("name = \"main\"", "name = \"MainRes\"", 1),
    )
    .expect("write runtime.toml");
    let output = run_check(&project);
    assert!(
        output.status.success(),
        "matching names pass: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_aggregates_required_and_optional_config_errors_with_stable_exit_code() {
    let project = unique_temp_dir("check-config-aggregate");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    std::fs::remove_file(project.join("io.toml")).expect("remove required io.toml");
    std::fs::write(project.join("ads.toml"), "connections = [").expect("write invalid ads.toml");
    std::fs::write(project.join("opcua_client.toml"), "connections = [")
        .expect("write invalid opcua_client.toml");

    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(10));
    let payload = json_stdout(&output);
    let issues = payload["issues"].as_array().expect("issues array");
    for expected in ["config.io", "config.ads", "config.opcua_client"] {
        assert!(
            issues.iter().any(|issue| issue["code"] == expected),
            "missing {expected} in {issues:?}"
        );
    }
    assert_eq!(payload["errors"].as_u64(), Some(3));
    assert!(
        payload.get("bytecode_size").is_none(),
        "the check reports no bytecode size"
    );
    assert!(!project.join("program.tpi").exists());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_reports_source_layout_error_with_invalid_config_exit_code() {
    let project = unique_temp_dir("check-source-layout");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    std::fs::remove_dir_all(project.join("src")).expect("remove source directory");

    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(10));
    let payload = json_stdout(&output);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["status"], "failed");
    assert_eq!(payload["source_count"], 0);
    assert_eq!(payload["sources"], serde_json::json!([]));
    assert!(
        payload.get("bytecode_size").is_none(),
        "the check reports no bytecode size"
    );
    assert!(payload["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(|issue| issue["code"] == "sources"));
    assert!(!project.join("program.tpi").exists());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_ci_reports_sources_override_and_local_dependencies() {
    let project = unique_temp_dir("check-ci-dependencies");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    std::fs::rename(project.join("src"), project.join("custom_sources"))
        .expect("rename source directory");
    let dependency_root = project.join("deps").join("lib-a");
    std::fs::create_dir_all(dependency_root.join("src"))
        .expect("create dependency source directory");
    std::fs::write(
        dependency_root.join("src").join("lib.st"),
        r#"
FUNCTION LibValue : INT
LibValue := 7;
END_FUNCTION
"#,
    )
    .expect("write dependency source");
    std::fs::write(
        project.join("trust-lsp.toml"),
        "[dependencies]\nLibA = \"deps/lib-a\"\n",
    )
    .expect("write dependency manifest");

    let output = run_check_with_args(&project, &["--ci", "--sources", "custom_sources"]);
    assert!(
        output.status.success(),
        "expected CI check success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = json_stdout(&output);
    assert_eq!(payload["version"], 1);
    assert_eq!(payload["command"], "check");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["source_count"], 2);
    assert_eq!(
        payload["resolved_dependencies"],
        serde_json::json!(["LibA"])
    );
    assert_eq!(
        payload["dependency_roots"]
            .as_array()
            .expect("dependency roots")
            .len(),
        1
    );
    let sources = payload["sources"].as_array().expect("sources");
    assert!(sources.iter().any(|path| path
        .as_str()
        .is_some_and(|path| reported_path_ends_with(path, "custom_sources/main.st"))));
    assert!(sources.iter().any(|path| path
        .as_str()
        .is_some_and(|path| reported_path_ends_with(path, "deps/lib-a/src/lib.st"))));
    assert!(!project.join("program.tpi").exists());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_mixed_config_and_compile_errors_uses_config_exit_and_reports_both() {
    let project = unique_temp_dir("check-mixed-errors");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
Main :=
END_PROGRAM
"#,
    );
    std::fs::remove_file(project.join("io.toml")).expect("remove required io.toml");

    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(10));
    let payload = json_stdout(&output);
    let issues = payload["issues"].as_array().expect("issues");
    assert!(issues.iter().any(|issue| issue["code"] == "config.io"));
    // `Main :=` lacks its expression, reported at `END_PROGRAM` on line 4.
    assert!(
        issues
            .iter()
            .any(|issue| issue["code"] == "E002" && issue["line"] == 4 && issue["column"] == 1),
        "{issues:?}"
    );
    assert_eq!(payload["errors"], issues.len());
    assert!(payload["source_count"].as_u64().unwrap_or_default() >= 1);
    assert!(
        payload.get("bytecode_size").is_none(),
        "the check reports no bytecode size"
    );
    assert!(!project.join("program.tpi").exists());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_human_output_reports_success_and_failure() {
    let valid_project = unique_temp_dir("check-human-ok");
    write_project_fixture(
        &valid_project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    let success = run_check_with_args(&valid_project, &[]);
    assert!(success.status.success());
    let success_stdout = String::from_utf8_lossy(&success.stdout);
    assert!(success_stdout.contains("Project check passed"));
    assert!(success_stdout.contains("Sources: 1 file(s)"));

    let invalid_project = unique_temp_dir("check-human-failed");
    write_project_fixture(
        &invalid_project,
        r#"
PROGRAM Main
Main :=
END_PROGRAM
"#,
    );
    let failure = run_check_with_args(&invalid_project, &[]);
    assert_eq!(failure.status.code(), Some(11));
    let failure_stderr = String::from_utf8_lossy(&failure.stderr);
    assert!(failure_stderr.contains("Project check failed"));
    // An issue with a position reads `file:line:column: message` (specification 11 §6.8.1c).
    assert!(
        failure_stderr.contains("main.st:4:1: expected expression"),
        "{failure_stderr}"
    );

    let _ = std::fs::remove_dir_all(valid_project);
    let _ = std::fs::remove_dir_all(invalid_project);
}

// Specification 11 §6.8.1c: a construct that the build does not compile yet (specification 44
// NX-09) is one issue with the code `compile` at the construct's place. Here `MUX` of two strings,
// which type checks: worked by hand, the call's first token `MUX` is line 6, column 6 of
// `src/main.st`. Catches the place dropped (one issue without `file`, `line` and `column`, the
// contributor report on #407) and the place of another token.
#[test]
fn check_reports_an_uncompiled_construct_where_it_is() {
    let project = unique_temp_dir("check-uncompiled-place");
    write_project_fixture(
        &project,
        "PROGRAM Main\nVAR\n    k : INT := 0;\n    s : STRING;\nEND_VAR\ns := MUX(k, 'a', 'b');\nEND_PROGRAM\n",
    );
    let output = run_check(&project);
    assert_eq!(
        output.status.code(),
        Some(11),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let payload = json_stdout(&output);
    let issues = payload["issues"].as_array().expect("the issues");
    assert_eq!(issues.len(), 1, "{payload}");
    let issue = &issues[0];
    assert_eq!(issue["code"], "compile", "{issue}");
    assert!(
        reported_path_ends_with(issue["file"].as_str().unwrap_or_default(), "src/main.st"),
        "{issue}"
    );
    assert_eq!(issue["line"], 6, "{issue}");
    assert_eq!(issue["column"], 6, "{issue}");
    assert!(
        issue["message"]
            .as_str()
            .unwrap_or_default()
            .contains("MUX of values that are not scalars"),
        "{issue}"
    );
    let _ = std::fs::remove_dir_all(project);
}

// docs/specs/07-standard-functions.md 2.10: the project's vendor_profile selects its
// conversion rules, so a name that is not a vendor profile fails the check instead of
// running the project under other rules than intended.
#[test]
fn check_reports_an_unknown_vendor_profile() {
    let project = unique_temp_dir("check-vendor-profile");
    write_project_fixture(&project, "PROGRAM Main\nEND_PROGRAM\n");
    std::fs::write(
        project.join("trust-lsp.toml"),
        "[project]\nvendor_profile = \"codesys\"\n",
    )
    .expect("write trust-lsp.toml");
    let output = run_check(&project);
    assert!(
        output.status.success(),
        "codesys is a vendor profile: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    std::fs::write(
        project.join("trust-lsp.toml"),
        "[project]\nvendor_profile = \"codesis\"\n",
    )
    .expect("write trust-lsp.toml");
    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(11));
    let payload = json_stdout(&output);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["issues"][0]["code"], "compile");
    let message = payload["issues"][0]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("unknown vendor_profile 'codesis'")
            && message.contains(
                "supported: beckhoff, codesys, gxworks3, iec, mitsubishi, siemens, twincat"
            ),
        "{message}"
    );
    let _ = std::fs::remove_dir_all(project);
}
