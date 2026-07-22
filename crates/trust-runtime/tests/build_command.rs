use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value as JsonValue;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    for _ in 0..64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let sequence = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "trust-runtime-{prefix}-{}-{nanos}-{sequence}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return path,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("create temp directory {}: {error}", path.display()),
        }
    }
    panic!("failed to allocate unique temp directory for {prefix}")
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture parent directory");
    }
    std::fs::write(path, contents).expect("write fixture file");
}

fn run_build(project: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("build")
        .arg("--project")
        .arg(project)
        .args(args)
        .output()
        .expect("run trust-runtime build")
}

fn json_stdout(output: &Output) -> JsonValue {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "build stdout should be JSON ({error}); stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn build_ci_reports_sources_override_and_local_dependencies() {
    let project = unique_temp_dir("build-ci-dependencies");
    write_file(
        &project.join("custom_sources/main.st"),
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
Value := LibValue();
END_PROGRAM
"#,
    );
    let dependency = project.join("deps/lib-a");
    write_file(
        &dependency.join("src/lib.st"),
        r#"
FUNCTION LibValue : INT
LibValue := 7;
END_FUNCTION
"#,
    );
    write_file(
        &project.join("trust-lsp.toml"),
        "[dependencies]\nLibA = \"deps/lib-a\"\n",
    );

    let output = run_build(&project, &["--ci", "--sources", "custom_sources"]);
    assert!(
        output.status.success(),
        "expected successful build; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = json_stdout(&output);
    assert_eq!(payload["version"], 1);
    assert_eq!(payload["command"], "build");
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["project"], project.display().to_string());
    assert_eq!(
        payload["program"],
        project.join("program.stbc").display().to_string()
    );
    assert_eq!(
        payload["source_count"].as_u64(),
        payload["sources"]
            .as_array()
            .map(|sources| sources.len() as u64)
    );
    assert_eq!(payload["source_count"], 2);
    let sources = payload["sources"].as_array().expect("source array");
    assert!(sources.iter().any(|path| {
        path.as_str()
            .is_some_and(|path| path.ends_with("custom_sources/main.st"))
    }));
    assert!(sources.iter().any(|path| {
        path.as_str()
            .is_some_and(|path| path.ends_with("deps/lib-a/src/lib.st"))
    }));
    assert_eq!(
        payload["resolved_dependencies"],
        serde_json::json!(["LibA"])
    );
    assert_eq!(
        payload["dependency_roots"],
        serde_json::json!([dependency.canonicalize().expect("canonical dependency")])
    );
    assert!(project.join("program.stbc").is_file());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn build_compile_failure_preserves_existing_program_and_exits_11() {
    let project = unique_temp_dir("build-compile-failure");
    write_file(
        &project.join("src/main.st"),
        r#"
PROGRAM Main
VAR
    Counter : INT;
END_VAR
Counter :=
END_PROGRAM
"#,
    );
    let program_path = project.join("program.stbc");
    std::fs::write(&program_path, b"previous-bytecode").expect("write existing bytecode");

    let output = run_build(&project, &["--ci"]);
    assert_eq!(output.status.code(), Some(11));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("expected expression"),
        "expected parser failure in stderr; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read(&program_path).expect("read preserved bytecode"),
        b"previous-bytecode"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn build_human_mode_discovers_current_project_and_summarizes_sources() {
    let project = unique_temp_dir("build-human");
    write_file(&project.join("runtime.toml"), "");
    write_file(&project.join("io.toml"), "");
    for index in 0..6 {
        write_file(
            &project.join(format!("src/program_{index}.st")),
            &format!("PROGRAM Program{index}\nEND_PROGRAM\n"),
        );
    }

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("build")
        .current_dir(&project)
        .output()
        .expect("run trust-runtime build from project directory");
    assert!(
        output.status.success(),
        "expected successful human build; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Wrote"));
    assert!(stdout.contains("Sources: 6 file(s)"));
    assert!(stdout.contains(" - ... +1"));
    assert!(project.join("program.stbc").is_file());

    let _ = std::fs::remove_dir_all(project);
}
