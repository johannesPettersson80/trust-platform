use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

fn trust_dev_command() -> Command {
    Command::new(trust_dev_bin())
}

fn trust_runtime_command_with_dev_alias() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_trust-runtime"));
    command.env("TRUST_DEV_BIN", trust_dev_bin());
    command
}

fn trust_dev_bin() -> std::path::PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_trust-dev") {
        return path.into();
    }
    if let Ok(path) = std::env::var("TRUST_DEV_BIN") {
        return path.into();
    }
    let exe = std::env::current_exe().expect("current test exe path");
    let debug_dir = exe
        .parent()
        .and_then(|deps| deps.parent())
        .expect("target debug dir");
    debug_dir.join(format!("trust-dev{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn docs_command_generates_markdown_and_html() {
    let project = unique_temp_dir("docs-project");
    let sources = project.join("src");
    let out_dir = project.join("generated-docs");
    std::fs::create_dir_all(&sources).expect("create src");
    std::fs::write(
        sources.join("main.st"),
        r#"
// @brief Adds one to input.
// @param IN Input value.
// @return Incremented value.
FUNCTION Increment : INT
VAR_INPUT
    IN : INT;
END_VAR
Increment := IN + INT#1;
END_FUNCTION
"#,
    )
    .expect("write source");

    let output = trust_dev_command()
        .args([
            "docs",
            "--project",
            project.to_str().expect("project path utf-8"),
            "--out-dir",
            out_dir.to_str().expect("output path utf-8"),
            "--format",
            "both",
        ])
        .output()
        .expect("run trust-dev docs");

    assert!(
        output.status.success(),
        "expected docs command success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let markdown = std::fs::read_to_string(out_dir.join("api.md")).expect("read markdown output");
    let html = std::fs::read_to_string(out_dir.join("api.html")).expect("read html output");
    assert!(markdown.contains("FUNCTION `Increment`"));
    assert!(markdown.contains("**Parameters**"));
    assert!(markdown.contains("`IN`: Input value."));
    assert!(html.contains("<h3>FUNCTION <code>Increment</code></h3>"));
    assert!(html.contains("<strong>Returns:</strong> Incremented value."));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn trust_runtime_docs_alias_forwards_to_trust_dev() {
    let project = unique_temp_dir("docs-alias-project");
    let sources = project.join("src");
    let out_dir = project.join("generated-docs");
    std::fs::create_dir_all(&sources).expect("create src");
    std::fs::write(
        sources.join("main.st"),
        r#"
// @brief Does work.
PROGRAM Main
END_PROGRAM
"#,
    )
    .expect("write source");

    let output = trust_runtime_command_with_dev_alias()
        .args([
            "docs",
            "--project",
            project.to_str().expect("project path utf-8"),
            "--out-dir",
            out_dir.to_str().expect("output path utf-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("run trust-runtime docs alias");

    assert!(
        output.status.success(),
        "expected docs alias success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("trust-runtime docs"));
    assert!(stderr.contains("trust-dev docs"));
    assert!(out_dir.join("api.md").exists());

    let _ = std::fs::remove_dir_all(project);
}
