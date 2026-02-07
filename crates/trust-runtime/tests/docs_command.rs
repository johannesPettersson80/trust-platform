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

#[test]
fn docs_command_generates_markdown_and_html() {
    let project = unique_temp_dir("docs-project");
    let sources = project.join("sources");
    let out_dir = project.join("generated-docs");
    std::fs::create_dir_all(&sources).expect("create sources");
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

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
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
        .expect("run trust-runtime docs");

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
