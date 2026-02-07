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

fn pou_signatures(xml_text: &str) -> Vec<(String, String, String)> {
    let doc = roxmltree::Document::parse(xml_text).expect("parse XML");
    let mut items = doc
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "pou")
        .filter_map(|pou| {
            let name = pou.attribute("name")?.to_string();
            let pou_type = pou.attribute("pouType")?.to_string();
            let body = pou
                .children()
                .find(|child| child.is_element() && child.tag_name().name() == "body")
                .and_then(|body| {
                    body.children()
                        .find(|child| child.is_element() && child.tag_name().name() == "ST")
                        .and_then(|st| st.text())
                })
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            Some((name, pou_type, body))
        })
        .collect::<Vec<_>>();
    items.sort();
    items
}

#[test]
fn plcopen_profile_json_emits_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["plcopen", "profile", "--json"])
        .output()
        .expect("run trust-runtime plcopen profile");

    assert!(
        output.status.success(),
        "expected plcopen profile success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse profile JSON");
    assert_eq!(
        value
            .get("namespace")
            .and_then(serde_json::Value::as_str)
            .expect("namespace"),
        "http://www.plcopen.org/xml/tc6_0200"
    );
    assert_eq!(
        value
            .get("profile")
            .and_then(serde_json::Value::as_str)
            .expect("profile"),
        "trust-st-strict-v1"
    );
}

#[test]
fn plcopen_export_and_import_round_trip_via_cli() {
    let source_project = unique_temp_dir("plcopen-cli-source");
    let source_root = source_project.join("sources");
    let xml_a = source_project.join("out/plcopen.xml");
    std::fs::create_dir_all(&source_root).expect("create source root");
    std::fs::write(
        source_root.join("main.st"),
        r#"
PROGRAM Main
VAR
    Counter : INT := 1;
END_VAR
END_PROGRAM
"#,
    )
    .expect("write main");
    std::fs::write(
        source_root.join("pump.st"),
        r#"
FUNCTION_BLOCK PumpController
VAR_INPUT
    Enable : BOOL;
END_VAR
END_FUNCTION_BLOCK
"#,
    )
    .expect("write pump");

    let export_a = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "export",
            "--project",
            source_project.to_str().expect("source project utf-8"),
            "--output",
            xml_a.to_str().expect("xml output utf-8"),
        ])
        .output()
        .expect("run trust-runtime plcopen export");

    assert!(
        export_a.status.success(),
        "expected export success, stderr was:\n{}",
        String::from_utf8_lossy(&export_a.stderr)
    );
    assert!(xml_a.is_file(), "export did not produce xml output");
    let source_map = xml_a.with_extension("source-map.json");
    assert!(
        source_map.is_file(),
        "export did not produce source-map sidecar"
    );

    let imported_project = unique_temp_dir("plcopen-cli-import");
    let import_result = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "import",
            "--input",
            xml_a.to_str().expect("xml input utf-8"),
            "--project",
            imported_project.to_str().expect("import project utf-8"),
        ])
        .output()
        .expect("run trust-runtime plcopen import");

    assert!(
        import_result.status.success(),
        "expected import success, stderr was:\n{}",
        String::from_utf8_lossy(&import_result.stderr)
    );

    let imported_sources = imported_project.join("sources");
    assert!(
        imported_sources.is_dir(),
        "import did not create sources directory"
    );
    let imported_files = std::fs::read_dir(&imported_sources)
        .expect("read imported sources")
        .filter_map(Result::ok)
        .count();
    assert!(
        imported_files >= 2,
        "expected at least two imported source files"
    );

    let xml_b = imported_project.join("out/plcopen-roundtrip.xml");
    let export_b = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "export",
            "--project",
            imported_project.to_str().expect("import project utf-8"),
            "--output",
            xml_b.to_str().expect("roundtrip xml output utf-8"),
        ])
        .output()
        .expect("run trust-runtime plcopen export roundtrip");

    assert!(
        export_b.status.success(),
        "expected roundtrip export success, stderr was:\n{}",
        String::from_utf8_lossy(&export_b.stderr)
    );
    let a_text = std::fs::read_to_string(&xml_a).expect("read first export");
    let b_text = std::fs::read_to_string(&xml_b).expect("read second export");
    assert_eq!(
        pou_signatures(&a_text),
        pou_signatures(&b_text),
        "expected semantic POU signature stability after command round-trip"
    );

    let _ = std::fs::remove_dir_all(source_project);
    let _ = std::fs::remove_dir_all(imported_project);
}

#[test]
fn plcopen_import_fails_for_missing_input() {
    let project = unique_temp_dir("plcopen-cli-missing-input");
    let missing = project.join("does-not-exist.xml");

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "import",
            "--input",
            missing.to_str().expect("missing path utf-8"),
            "--project",
            project.to_str().expect("project path utf-8"),
        ])
        .output()
        .expect("run trust-runtime plcopen import");

    assert!(
        !output.status.success(),
        "expected import command failure for missing input"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist"),
        "expected missing input message, got:\n{stderr}"
    );

    let _ = std::fs::remove_dir_all(project);
}
