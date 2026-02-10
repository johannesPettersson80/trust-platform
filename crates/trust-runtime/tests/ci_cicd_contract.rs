use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    for _ in 0..64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let seq = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "trust-runtime-{prefix}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!("create temp fixture dir {}: {err}", dir.display()),
        }
    }
    panic!("failed to allocate unique temp dir for fixture '{prefix}'")
}

fn copy_file_with_retry(src_path: &Path, dst_path: &Path) {
    for attempt in 0..5 {
        match std::fs::copy(src_path, dst_path) {
            Ok(_) => return,
            Err(err) if cfg!(windows) && err.raw_os_error() == Some(32) && attempt < 4 => {
                // Windows runners can briefly lock files while tests run in parallel.
                std::thread::sleep(Duration::from_millis(20 * (attempt + 1)));
            }
            Err(err) => panic!(
                "copy fixture file {} -> {}: {err}",
                src_path.display(),
                dst_path.display()
            ),
        }
    }
    unreachable!("copy_file_with_retry exhausted retries")
}

fn fixture_root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ci")
        .join(name)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create destination fixture directory");
    for entry in std::fs::read_dir(src).expect("read fixture directory") {
        let entry = entry.expect("read fixture entry");
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            copy_file_with_retry(&src_path, &dst_path);
        }
    }
}

fn copy_fixture(name: &str) -> PathBuf {
    let target = unique_temp_dir(&format!("ci-{name}"));
    copy_dir_recursive(&fixture_root(name), &target);
    target
}

fn write_generated_tests(project: &Path, case_count: usize) {
    let mut text = String::new();
    for idx in 0..case_count {
        text.push_str(&format!(
            "TEST_PROGRAM AutoCase_{idx}\nASSERT_TRUE(TRUE);\nEND_TEST_PROGRAM\n\n"
        ));
    }
    std::fs::write(project.join("sources").join("tests.st"), text)
        .expect("write generated tests fixture");
}

fn run_trust_runtime(project: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_trust-runtime"));
    command.args(args);
    command.args(["--project", project.to_str().expect("project path utf-8")]);
    command.output().expect("run trust-runtime")
}

fn run_release_gate_report(
    output_dir: &Path,
    gate_artifacts_dir: &Path,
    required_gates: &[&str],
    job_statuses: &[(&str, &str)],
) -> Output {
    let script = repo_root()
        .join("scripts")
        .join("generate_release_gate_report.py");
    let checklist = repo_root().join("docs").join("release-gate-checklist.md");
    let mut command = Command::new("python3");
    command
        .arg(script)
        .arg("--output-dir")
        .arg(output_dir)
        .arg("--gate-artifacts-dir")
        .arg(gate_artifacts_dir);
    for gate in required_gates {
        command.arg("--required-gate").arg(gate);
    }
    for (job, status) in job_statuses {
        command.arg("--job-status").arg(format!("{job}={status}"));
    }
    command.arg("--checklist").arg(checklist);
    command.output().expect("run release gate report script")
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} should succeed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ci_json_summary_mode_contract_is_stable() {
    let project = copy_fixture("green");

    let build = run_trust_runtime(&project, &["build", "--ci"]);
    assert_success(&build, "build --ci");
    let build_json: serde_json::Value =
        serde_json::from_slice(&build.stdout).expect("parse build --ci JSON");
    assert_eq!(build_json["version"], 1);
    assert_eq!(build_json["command"], "build");
    assert_eq!(build_json["status"], "ok");
    assert_eq!(build_json["project"], project.display().to_string());
    assert!(
        build_json["source_count"]
            .as_u64()
            .is_some_and(|count| count >= 2),
        "expected at least two source files in build output"
    );

    let validate = run_trust_runtime(&project, &["validate", "--ci"]);
    assert_success(&validate, "validate --ci");
    let validate_json: serde_json::Value =
        serde_json::from_slice(&validate.stdout).expect("parse validate --ci JSON");
    assert_eq!(validate_json["version"], 1);
    assert_eq!(validate_json["command"], "validate");
    assert_eq!(validate_json["status"], "ok");
    assert_eq!(validate_json["project"], project.display().to_string());
    assert_eq!(validate_json["resource"], "Res");

    let test = run_trust_runtime(&project, &["test", "--ci", "--output", "json"]);
    assert_success(&test, "test --ci --output json");
    let test_json: serde_json::Value =
        serde_json::from_slice(&test.stdout).expect("parse test --ci --output json");
    assert_eq!(test_json["version"], 1);
    assert_eq!(test_json["project"], project.display().to_string());
    assert_eq!(test_json["summary"]["failed"], 0);
    assert_eq!(test_json["summary"]["errors"], 0);
    assert!(
        test_json["summary"]["duration_ms"].is_number(),
        "expected summary duration_ms in JSON output"
    );
    assert!(
        test_json["summary"]["total"]
            .as_u64()
            .is_some_and(|total| total >= 1),
        "expected at least one discovered test"
    );
    assert!(test_json["tests"]
        .as_array()
        .is_some_and(|tests| tests.iter().any(|case| case["name"] == "CI_Passes")));
    assert!(test_json["tests"]
        .as_array()
        .is_some_and(|tests| tests.iter().all(|case| case["duration_ms"].is_number())));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ci_template_workflow_passes_on_green_fixture() {
    let project = copy_fixture("green");

    let build = run_trust_runtime(&project, &["build", "--ci"]);
    assert_success(&build, "build --ci");

    let validate = run_trust_runtime(&project, &["validate", "--ci"]);
    assert_success(&validate, "validate --ci");

    let tests = run_trust_runtime(&project, &["test", "--ci", "--output", "junit"]);
    assert_success(&tests, "test --ci --output junit");
    let junit = String::from_utf8_lossy(&tests.stdout);
    assert!(
        junit.contains("<testsuite"),
        "expected junit testsuite output"
    );
    assert!(
        junit.contains("failures=\"0\""),
        "expected no junit failures"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ci_clean_setup_first_passing_test_is_under_ten_minutes() {
    let project = copy_fixture("green");
    let started = Instant::now();

    let build = run_trust_runtime(&project, &["build", "--ci"]);
    assert_success(&build, "build --ci");

    let validate = run_trust_runtime(&project, &["validate", "--ci"]);
    assert_success(&validate, "validate --ci");

    let tests = run_trust_runtime(&project, &["test", "--ci", "--output", "json"]);
    assert_success(&tests, "test --ci --output json");

    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(600),
        "expected first passing test path under 10 minutes, got {:.2}s",
        elapsed.as_secs_f64()
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ci_single_test_feedback_is_under_two_seconds_for_small_project() {
    let project = copy_fixture("green");
    let started = Instant::now();

    let tests = run_trust_runtime(
        &project,
        &["test", "--ci", "--output", "json", "--filter", "CI_Passes"],
    );
    assert_success(&tests, "test --ci --output json --filter CI_Passes");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "expected single-test feedback under 2 seconds, got {:.3}s",
        elapsed.as_secs_f64()
    );

    let payload: serde_json::Value =
        serde_json::from_slice(&tests.stdout).expect("parse filtered test JSON");
    assert_eq!(
        payload["summary"]["total"], 1,
        "expected filter to execute exactly one test case"
    );
    assert_eq!(payload["summary"]["failed"], 0);
    assert_eq!(payload["summary"]["errors"], 0);

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ci_run_all_tests_scales_roughly_linearly_for_small_projects() {
    let small = copy_fixture("green");
    write_generated_tests(&small, 10);
    let started_small = Instant::now();
    let small_run = run_trust_runtime(&small, &["test", "--ci", "--output", "json"]);
    assert_success(&small_run, "test --ci --output json (10 cases)");
    let small_elapsed = started_small.elapsed();
    let small_json: serde_json::Value =
        serde_json::from_slice(&small_run.stdout).expect("parse small test JSON");
    assert_eq!(small_json["summary"]["total"], 10);

    let large = copy_fixture("green");
    write_generated_tests(&large, 40);
    let started_large = Instant::now();
    let large_run = run_trust_runtime(&large, &["test", "--ci", "--output", "json"]);
    assert_success(&large_run, "test --ci --output json (40 cases)");
    let large_elapsed = started_large.elapsed();
    let large_json: serde_json::Value =
        serde_json::from_slice(&large_run.stdout).expect("parse large test JSON");
    assert_eq!(large_json["summary"]["total"], 40);

    // 4x tests should stay within a loose envelope and not show runaway per-test cost.
    let small_per_test = small_elapsed.as_secs_f64() / 10.0;
    let large_per_test = large_elapsed.as_secs_f64() / 40.0;
    assert!(
        large_elapsed <= small_elapsed.saturating_mul(12),
        "expected bounded scaling envelope: small={:.3}s large={:.3}s",
        small_elapsed.as_secs_f64(),
        large_elapsed.as_secs_f64()
    );
    assert!(
        large_per_test <= (small_per_test * 3.0),
        "expected per-test cost to stay bounded: small={small_per_test:.5}s/test large={large_per_test:.5}s/test"
    );
    assert!(
        large_per_test <= 0.100,
        "expected small-project all-tests throughput under 100ms/test, got {large_per_test:.5}s/test"
    );
    assert!(
        large_elapsed <= Duration::from_secs(10),
        "expected small-project all-tests run under 10 seconds, got {:.3}s",
        large_elapsed.as_secs_f64()
    );

    let _ = std::fs::remove_dir_all(small);
    let _ = std::fs::remove_dir_all(large);
}

#[test]
fn ci_template_workflow_fails_on_broken_fixture_with_expected_code_and_report() {
    let project = copy_fixture("broken");

    let build = run_trust_runtime(&project, &["build", "--ci"]);
    assert_success(&build, "build --ci");

    let validate = run_trust_runtime(&project, &["validate", "--ci"]);
    assert_success(&validate, "validate --ci");

    let tests = run_trust_runtime(&project, &["test", "--ci", "--output", "junit"]);
    assert!(
        !tests.status.success(),
        "broken fixture test command should fail"
    );
    assert_eq!(
        tests.status.code(),
        Some(12),
        "expected deterministic CI test-failure exit code"
    );
    let junit = String::from_utf8_lossy(&tests.stdout);
    assert!(
        junit.contains("<testsuite"),
        "expected junit testsuite output"
    );
    assert!(
        junit.contains("<failure"),
        "expected junit failure entry for broken fixture"
    );
    let stderr = String::from_utf8_lossy(&tests.stderr);
    assert!(
        stderr.contains("ST test(s) failed"),
        "expected deterministic CI error message in stderr"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ci_template_file_contains_expected_command_sequence() {
    let template = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(".github")
        .join("workflows")
        .join("templates")
        .join("trust-runtime-project-ci.yml");
    let text = std::fs::read_to_string(&template).expect("read CI template");
    assert!(
        text.contains("target/debug/trust-runtime build --project . --ci"),
        "template must include build --ci step"
    );
    assert!(
        text.contains("target/debug/trust-runtime validate --project . --ci"),
        "template must include validate --ci step"
    );
    assert!(
        text.contains("target/debug/trust-runtime test --project . --ci --output junit"),
        "template must include junit test step"
    );
    assert!(
        text.contains("Upload JUnit report"),
        "template must keep junit artifact upload"
    );
}

#[test]
fn ci_flake_probe_script_emits_machine_readable_sample() {
    let project = copy_fixture("green");
    let report_dir = unique_temp_dir("ci-flake-probe");
    std::fs::create_dir_all(&report_dir).expect("create flake probe report dir");
    let report_path = report_dir.join("sample.json");

    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scripts")
        .join("probe_st_test_flake.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--runtime-bin")
        .arg(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("--project")
        .arg(&project)
        .arg("--runs")
        .arg("3")
        .arg("--filter")
        .arg("CI_Passes")
        .arg("--output-json")
        .arg(&report_path)
        .arg("--max-failures")
        .arg("3")
        .output()
        .expect("run flake probe script");
    assert!(
        output.status.success(),
        "flake probe should succeed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json_text = std::fs::read_to_string(&report_path).expect("read flake report");
    let payload: serde_json::Value = serde_json::from_str(&json_text).expect("parse flake report");
    assert_eq!(payload["version"], 1);
    assert_eq!(payload["project"], project.display().to_string());
    assert_eq!(payload["runs"], 3);
    let passes = payload["passes"].as_u64().expect("passes count");
    let failures = payload["failures"].as_u64().expect("failures count");
    assert_eq!(passes + failures, 3, "passes + failures must equal runs");
    assert!(
        payload["flake_rate_percent"].is_number(),
        "flake rate should be numeric"
    );
    let samples = payload["samples"].as_array().expect("sample array");
    assert_eq!(samples.len(), 3, "expected one sample per run");
    for (index, sample) in samples.iter().enumerate() {
        assert_eq!(
            sample["run"].as_u64(),
            Some((index + 1) as u64),
            "sample run index should be stable"
        );
        let status = sample["status"].as_str().expect("sample status");
        assert!(
            status == "pass" || status == "fail",
            "sample status should be pass/fail"
        );
    }

    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(report_dir);
}

#[test]
fn ci_release_gate_report_fails_when_required_gate_artifact_is_missing() {
    let report_dir = unique_temp_dir("ci-gate-report-missing");
    let artifacts_dir = unique_temp_dir("ci-gate-artifacts-missing");
    std::fs::create_dir_all(artifacts_dir.join("gate-fmt")).expect("create gate-fmt marker dir");
    std::fs::create_dir_all(artifacts_dir.join("gate-clippy"))
        .expect("create gate-clippy marker dir");

    let output = run_release_gate_report(
        &report_dir,
        &artifacts_dir,
        &["gate-fmt", "gate-clippy", "gate-vscode-extension"],
        &[
            ("fmt", "success"),
            ("clippy", "success"),
            ("vscode-extension", "success"),
        ],
    );
    assert!(
        !output.status.success(),
        "missing gate artifact should fail report generation.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(1));

    let report_path = report_dir.join("release-gate-report.json");
    let report = std::fs::read_to_string(&report_path).expect("read generated gate report");
    let payload: serde_json::Value = serde_json::from_str(&report).expect("parse gate report JSON");
    assert_eq!(payload["verdict"], "FAIL");
    assert!(payload["missing_required_gate_artifacts"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item == "gate-vscode-extension")));

    let _ = std::fs::remove_dir_all(report_dir);
    let _ = std::fs::remove_dir_all(artifacts_dir);
}

#[test]
fn ci_release_gate_report_passes_when_required_gate_artifacts_are_present() {
    let report_dir = unique_temp_dir("ci-gate-report-pass");
    let artifacts_dir = unique_temp_dir("ci-gate-artifacts-pass");
    std::fs::create_dir_all(artifacts_dir.join("gate-fmt")).expect("create gate-fmt marker dir");
    std::fs::create_dir_all(artifacts_dir.join("gate-clippy"))
        .expect("create gate-clippy marker dir");
    std::fs::create_dir_all(artifacts_dir.join("gate-vscode-extension"))
        .expect("create gate-vscode marker dir");

    let output = run_release_gate_report(
        &report_dir,
        &artifacts_dir,
        &["gate-fmt", "gate-clippy", "gate-vscode-extension"],
        &[
            ("fmt", "success"),
            ("clippy", "success"),
            ("vscode-extension", "success"),
        ],
    );
    assert!(
        output.status.success(),
        "all required gate artifacts should pass report generation.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report_path = report_dir.join("release-gate-report.json");
    let report = std::fs::read_to_string(&report_path).expect("read generated gate report");
    let payload: serde_json::Value = serde_json::from_str(&report).expect("parse gate report JSON");
    assert_eq!(payload["verdict"], "PASS");
    assert_eq!(
        payload["missing_required_gate_artifacts"].as_array(),
        Some(&Vec::new())
    );

    let _ = std::fs::remove_dir_all(report_dir);
    let _ = std::fs::remove_dir_all(artifacts_dir);
}

#[test]
fn ci_nightly_workflow_exposes_dispatch_artifacts_and_gate_enforcement() {
    let workflow = repo_root()
        .join(".github")
        .join("workflows")
        .join("nightly-reliability.yml");
    let text = std::fs::read_to_string(workflow).expect("read nightly reliability workflow");
    assert!(
        text.contains("workflow_dispatch:"),
        "nightly workflow must expose manual workflow_dispatch entrypoint"
    );
    assert!(
        text.contains("scripts/runtime_load_test.sh"),
        "nightly workflow must run runtime load test script"
    );
    assert!(
        text.contains("scripts/runtime_soak_test.sh"),
        "nightly workflow must run runtime soak test script"
    );
    assert!(
        text.contains("scripts/probe_st_test_flake.py"),
        "nightly workflow must run ST flake probe sampling"
    );
    assert!(
        text.contains("--runs 20"),
        "nightly flake probe should sample multiple runs per night"
    );
    assert!(
        text.contains("--max-failures 0"),
        "nightly flake probe should fail workflow on any sampled flake"
    );
    assert!(
        text.contains("artifacts/nightly/st-test-flake-sample.json"),
        "nightly workflow must persist machine-readable flake sample output"
    );
    assert!(
        text.contains("--enforce-gates"),
        "nightly summary step must enforce reliability gates"
    );
    assert!(
        text.contains("name: Upload reliability artifacts"),
        "nightly workflow must upload artifacts for post-run triage"
    );
    assert!(
        text.contains("name: nightly-reliability-${{ github.run_id }}"),
        "nightly artifact upload should be run-scoped"
    );
    assert!(
        text.contains("path: artifacts/nightly/"),
        "nightly artifact upload path should include reliability outputs"
    );
}

#[test]
fn ci_reliability_summary_gate_returns_non_zero_on_budget_breach() {
    let report_dir = unique_temp_dir("ci-reliability-summary");
    std::fs::create_dir_all(&report_dir).expect("create reliability summary dir");
    let load_log = report_dir.join("load.log");
    let soak_log = report_dir.join("soak.log");
    std::fs::write(
        &load_log,
        "# timestamp task stats\n2026-02-07T00:00:00Z task=Main min_ms=1.0 avg_ms=2.0 max_ms=55.0 last_ms=3.0 overruns=0\n",
    )
    .expect("write load log");
    std::fs::write(
        &soak_log,
        "# timestamp status cpu_pct mem_rss_kb process_alive\n2026-02-07T00:00:00Z state=running cpu=20 mem_rss_kb=1024 process_alive=true\n",
    )
    .expect("write soak log");

    let script = repo_root()
        .join("scripts")
        .join("summarize_runtime_reliability.py");
    let summary_json = report_dir.join("summary.json");
    let summary_md = report_dir.join("summary.md");
    let output = Command::new("python3")
        .arg(script)
        .arg("--load-log")
        .arg(&load_log)
        .arg("--soak-log")
        .arg(&soak_log)
        .arg("--output-json")
        .arg(&summary_json)
        .arg("--output-md")
        .arg(&summary_md)
        .arg("--enforce-gates")
        .arg("--max-load-max-ms")
        .arg("20")
        .arg("--max-load-jitter-ms")
        .arg("20")
        .arg("--max-soak-rss-kb")
        .arg("2048")
        .arg("--max-soak-cpu-pct")
        .arg("95")
        .output()
        .expect("run reliability summary script");
    assert!(
        !output.status.success(),
        "reliability summary should fail when load max exceeds budget.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(1));

    let report = std::fs::read_to_string(summary_json).expect("read reliability summary JSON");
    let payload: serde_json::Value =
        serde_json::from_str(&report).expect("parse reliability summary JSON");
    assert_eq!(payload["gate"]["passed"], false);
    assert!(payload["gate"]["failures"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| {
            item.as_str()
                .is_some_and(|line| line.contains("load: worst_max_ms"))
        })));

    let _ = std::fs::remove_dir_all(report_dir);
}

#[test]
fn ci_vscode_extension_job_contract_wires_failure_to_release_gate() {
    let workflow = repo_root().join(".github").join("workflows").join("ci.yml");
    let text = std::fs::read_to_string(workflow).expect("read CI workflow");

    let vscode_start = text
        .find("  vscode-extension:")
        .expect("vscode-extension job exists");
    let report_start = text
        .find("  release-gate-report:")
        .expect("release-gate-report job exists");
    assert!(
        report_start > vscode_start,
        "release gate report should follow vscode-extension job"
    );
    let vscode_block = &text[vscode_start..report_start];
    assert!(
        vscode_block.contains("xvfb-run -a npm test"),
        "vscode-extension job must execute integration tests under xvfb in CI"
    );
    assert!(
        !vscode_block.contains("continue-on-error: true"),
        "vscode-extension tests must fail the job when tests fail"
    );
    assert!(
        vscode_block.contains("if: ${{ success() }}"),
        "vscode gate marker upload should only happen on successful test run"
    );

    let release_block = &text[report_start..];
    assert!(
        release_block
            .contains("needs: [fmt, clippy, test, msrv, docs, vscode-extension, mp001-parity]"),
        "release gate report must depend on vscode-extension job status"
    );
    assert!(
        release_block.contains("--required-gate gate-vscode-extension"),
        "release gate report must require vscode extension gate artifact"
    );
    assert!(
        release_block.contains("--required-gate gate-mp001-parity"),
        "release gate report must require MP-001 parity gate artifact"
    );
}
