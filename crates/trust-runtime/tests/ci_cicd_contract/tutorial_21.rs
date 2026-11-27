//! Tutorial 21 (CI for a PLC project) as its README writes it: tutorial 11's project built,
//! validated and tested in CI mode, and each step made to fail with the exit code the README gives.

use super::*;

/// The README of tutorial 21.
fn readme() -> String {
    std::fs::read_to_string(
        repo_root().join("examples/tutorials/21_ci_cd_project_pipeline/README.md"),
    )
    .expect("tutorial 21's README")
}

/// `program` with `args` and `--project .`, run in `project`, as the README has the reader run it.
fn in_project(project: &Path, program: &Path, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .args(["--project", "."])
        .current_dir(project)
        .output()
        .expect("run a truST command")
}

/// `trust-runtime` with `args` in `project`.
fn trust_runtime(project: &Path, args: &[&str]) -> Output {
    let program = PathBuf::from(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect("Cargo provides trust-runtime"),
    );
    in_project(project, &program, args)
}

/// `trust-dev` with `args` in `project`.
fn trust_dev(project: &Path, args: &[&str]) -> Output {
    in_project(project, &trust_dev_bin(), args)
}

/// The JSON report a command wrote on its standard output.
fn report(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "a JSON report ({error}):\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// `text` with its paths written with `/`, as the README's Linux and macOS write them (Windows
/// writes `\`).
fn portable(text: &str) -> String {
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.to_string()
    }
}

/// Replaces `from` with `to` once in the project's `file`, as step 6 has the reader edit it.
fn edit(project: &Path, file: &str, from: &str, to: &str) {
    let path = project.join(file);
    let text = std::fs::read_to_string(&path).expect("read the file");
    assert!(text.contains(from), "{file} holds `{from}`");
    std::fs::write(&path, text.replacen(from, to, 1)).expect("write the file");
}

#[test]
fn tutorial_21_steps_pass_and_each_failure_ends_with_its_exit_code() {
    // examples/tutorials/21_ci_cd_project_pipeline/README.md (specification 22, "Developer CI
    // exit classification": an invalid project or configuration is 10, a build failure 11, a
    // failed test 12). On a copy of tutorial 11's project, steps 2 to 5 end with 0 and write the
    // reports the README quotes; step 6's failed test ends with 12 and its JUnit failure, the
    // misspelled driver with 10 and the quoted error, and the build failure with 11 and the issue
    // E002. Catches CI exit codes that do not tell the kinds of failure apart, a project the steps
    // refuse (tutorial 11 once had no runtime.toml or io.toml, and step 3 exited 10), a report
    // without what the README shows, and a README that runs or quotes what the commands no longer
    // do.
    let _shared = beside_measurements();
    let readme = readme();
    for command in [
        "trust-runtime build --project . --ci",
        "trust-runtime validate --project . --ci",
        "trust-dev test --project . --ci --output junit > /tmp/trust-tutorial-21-junit.xml",
        "trust-dev test --project . --ci --output json > /tmp/trust-tutorial-21-report.json",
    ] {
        assert!(readme.contains(command), "the README runs `{command}`");
    }
    let project = unique_temp_dir("tutorial-21");
    copy_dir_recursive(
        &repo_root().join("examples/tutorials/11_unit_testing_102"),
        &project,
    );

    let build = trust_runtime(&project, &["build", "--ci"]);
    let validate = trust_runtime(&project, &["validate", "--ci"]);
    let junit = trust_dev(&project, &["test", "--ci", "--output", "junit"]);
    let json = trust_dev(&project, &["test", "--ci", "--output", "json"]);
    edit(
        &project,
        "src/tests.st",
        "ASSERT_EQUAL(EXPECTED := INT#800, ACTUAL := Dut.PumpSpeedCmd);",
        "ASSERT_EQUAL(EXPECTED := INT#900, ACTUAL := Dut.PumpSpeedCmd);",
    );
    let failed_test = trust_dev(&project, &["test", "--ci", "--output", "junit"]);
    edit(
        &project,
        "io.toml",
        "driver = \"simulated\"",
        "driver = \"simulatd\"",
    );
    let invalid = trust_runtime(&project, &["validate", "--ci"]);
    let main = project.join("src/main.st");
    let source = std::fs::read_to_string(&main).expect("read src/main.st");
    std::fs::write(&main, format!("{source}THIS IS NOT ST\n")).expect("write src/main.st");
    let broken = trust_runtime(&project, &["build", "--ci"]);
    let _ = std::fs::remove_dir_all(&project);

    assert_success(&build, "step 2, build --ci");
    let built = report(&build);
    assert_eq!(built["status"], "ok", "{built}");
    assert_eq!(built["source_count"], 2, "{built}");
    assert_eq!(
        built["image"].as_str().map(portable).as_deref(),
        Some("./program.tpi"),
        "{built}"
    );
    assert_success(&validate, "step 3, validate --ci");
    let validated = report(&validate);
    assert_eq!(validated["resource"], "TankControlRes", "{validated}");
    assert_eq!(validated["io_driver"], "simulated", "{validated}");
    assert_success(&junit, "step 4, test --ci --output junit");
    let junit = String::from_utf8_lossy(&junit.stdout).into_owned();
    let suite = r#"<testsuite name="trust-runtime" tests="2" failures="0" errors="0" skipped="0">"#;
    for quoted in [
        suite,
        "TEST_PROGRAM::TankControlWithMockIo",
        "TEST_FUNCTION_BLOCK::TankControlBand",
    ] {
        assert!(
            junit.contains(quoted),
            "the JUnit report holds `{quoted}`:\n{junit}"
        );
        assert!(readme.contains(quoted), "the README quotes `{quoted}`");
    }
    assert_success(&json, "step 5, test --ci --output json");
    let summary = report(&json)["summary"].clone();
    assert_eq!(
        (&summary["passed"], &summary["failed"], &summary["total"]),
        (
            &serde_json::json!(2),
            &serde_json::json!(0),
            &serde_json::json!(2)
        ),
        "{summary}"
    );

    assert_eq!(failed_test.status.code(), Some(12), "a failed test");
    let failure = r#"<failure message="ASSERT_EQUAL failed: expected 900, actual 800">"#;
    let report_text = portable(&String::from_utf8_lossy(&failed_test.stdout));
    for quoted in [failure, "at: src/tests.st:10"] {
        assert!(
            report_text.contains(quoted),
            "the report holds `{quoted}`:\n{report_text}"
        );
        assert!(readme.contains(quoted), "the README quotes `{quoted}`");
    }
    let stderr = String::from_utf8_lossy(&failed_test.stderr).into_owned();
    assert!(stderr.contains("Error: 1 ST test(s) failed"), "{stderr}");

    assert_eq!(invalid.status.code(), Some(10), "an invalid configuration");
    let refusal = "Error: invalid config 'unsupported io.driver 'simulatd''";
    assert!(
        String::from_utf8_lossy(&invalid.stderr).contains(refusal),
        "validate's error: {}",
        String::from_utf8_lossy(&invalid.stderr)
    );
    assert!(readme.contains(refusal), "the README quotes `{refusal}`");

    assert_eq!(broken.status.code(), Some(11), "a build failure");
    let issues = report(&broken);
    assert_eq!(issues["status"], "failed", "{issues}");
    assert!(
        issues["issues"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue["code"] == "E002")),
        "{issues}"
    );
    assert!(
        readme.contains("`E002`"),
        "the README names the issue's code"
    );
}
