use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn conformance_command_supports_update_verify_and_mismatch_taxonomy() {
    let root = unique_temp_dir("conformance-suite");
    let cases_timers = root.join("cases/timers/cfm_timers_smoke_case_001");
    let cases_memory = root.join("cases/memory_map/cfm_memory_map_compile_error_001");
    let expected_timers = root.join("expected/timers");
    let expected_memory = root.join("expected/memory_map");
    let reports = root.join("reports");

    std::fs::create_dir_all(&cases_timers).expect("create timer case dir");
    std::fs::create_dir_all(&cases_memory).expect("create memory case dir");
    std::fs::create_dir_all(&expected_timers).expect("create expected timers dir");
    std::fs::create_dir_all(&expected_memory).expect("create expected memory dir");
    std::fs::create_dir_all(&reports).expect("create reports dir");

    std::fs::write(
        cases_timers.join("program.st"),
        r#"
PROGRAM Main
VAR
    x : INT := 1;
END_VAR
END_PROGRAM
"#,
    )
    .expect("write timer program");
    std::fs::write(
        cases_timers.join("manifest.toml"),
        r#"
id = "cfm_timers_smoke_case_001"
category = "timers"
description = "smoke case"
cycles = 1
watch_globals = ["x"]
"#,
    )
    .expect("write timer manifest");

    std::fs::write(
        cases_memory.join("program.st"),
        r#"
PROGRAM Main
VAR
    out AT %M* : BOOL;
END_VAR
out := TRUE;
END_PROGRAM
"#,
    )
    .expect("write compile-error program");
    std::fs::write(
        cases_memory.join("manifest.toml"),
        r#"
id = "cfm_memory_map_compile_error_001"
category = "memory_map"
kind = "compile_error"
"#,
    )
    .expect("write compile-error manifest");

    let update = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["conformance", "--suite-root"])
    .arg(&root)
    .args(["--update-expected", "--output"])
    .arg(root.join("reports/update.json"))
    .output()
    .expect("run conformance update");
    assert!(
        update.status.success(),
        "update mode should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&update.stdout),
        String::from_utf8_lossy(&update.stderr)
    );

    let verify = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["conformance", "--suite-root"])
    .arg(&root)
    .args(["--output"])
    .arg(root.join("reports/verify.json"))
    .output()
    .expect("run conformance verify");
    assert!(
        verify.status.success(),
        "verify mode should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&verify.stdout),
        String::from_utf8_lossy(&verify.stderr)
    );
    let verify_summary: serde_json::Value =
        serde_json::from_slice(&verify.stdout).expect("parse verify summary");
    assert_eq!(verify_summary["version"], 1);
    assert_eq!(verify_summary["profile"], "trust-conformance-v1");
    assert_eq!(verify_summary["summary"]["total"], 2);
    assert_eq!(verify_summary["summary"]["passed"], 2);
    assert_eq!(verify_summary["ordering"], "case_id_asc");

    std::fs::write(
        root.join("expected/timers/cfm_timers_smoke_case_001.json"),
        r#"{"version":1,"kind":"runtime","case_id":"cfm_timers_smoke_case_001","category":"timers","trace":[]}"#,
    )
    .expect("mutate expected for mismatch");

    let mismatch = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["conformance", "--suite-root"])
    .arg(&root)
    .args(["--output"])
    .arg(root.join("reports/mismatch.json"))
    .output()
    .expect("run conformance mismatch");
    assert!(
        !mismatch.status.success(),
        "mismatch run should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&mismatch.stdout),
        String::from_utf8_lossy(&mismatch.stderr)
    );
    let mismatch_summary: serde_json::Value =
        serde_json::from_slice(&mismatch.stdout).expect("parse mismatch summary");
    assert_eq!(mismatch_summary["summary"]["failed"], 1);
    let results = mismatch_summary["results"]
        .as_array()
        .expect("results array");
    assert!(
        results.iter().any(|entry| {
            entry["case_id"] == "cfm_timers_smoke_case_001"
                && entry["reason"]["code"] == "expected_mismatch"
        }),
        "expected mismatch reason code in summary"
    );

    let _ = std::fs::remove_dir_all(root);
}
