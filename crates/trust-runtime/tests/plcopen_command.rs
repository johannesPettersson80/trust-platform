use std::path::PathBuf;
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

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("plcopen")
        .join(name)
}

fn source_root(project_root: &std::path::Path) -> PathBuf {
    project_root.join("src")
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
        "trust-st-complete-v1"
    );
    assert!(value["compatibility_matrix"]
        .as_array()
        .expect("compatibility matrix")
        .iter()
        .any(|entry| entry["status"] == "supported"));
    assert!(!value["round_trip_limits"]
        .as_array()
        .expect("round trip limits")
        .is_empty());
    assert!(!value["known_gaps"]
        .as_array()
        .expect("known gaps")
        .is_empty());
}

#[test]
fn plcopen_export_and_import_round_trip_via_cli() {
    let source_project = unique_temp_dir("plcopen-cli-source");
    let source_dir = source_project.join("src");
    let xml_a = source_project.join("out/plcopen.xml");
    std::fs::create_dir_all(&source_dir).expect("create source root");
    std::fs::write(
        source_dir.join("main.st"),
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
        source_dir.join("pump.st"),
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
    let import_stdout = String::from_utf8_lossy(&import_result.stdout);
    assert!(
        import_stdout.contains("Migration report:"),
        "expected migration report path in import output, got:\n{import_stdout}"
    );

    let imported_sources = source_root(&imported_project);
    assert!(
        imported_sources.is_dir(),
        "import did not create src/ directory"
    );
    assert!(
        imported_project
            .join("interop")
            .join("plcopen-migration-report.json")
            .is_file(),
        "import did not emit migration report"
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
fn plcopen_export_import_json_reports_include_compatibility_diagnostics() {
    let project = unique_temp_dir("plcopen-cli-json-report");
    std::fs::create_dir_all(project.join("src")).expect("create src");
    std::fs::write(
        project.join("src/main.st"),
        r#"
PROGRAM Main
END_PROGRAM
"#,
    )
    .expect("write source");

    let output_xml = project.join("out/plcopen.json.xml");
    let export = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "export",
            "--project",
            project.to_str().expect("project utf-8"),
            "--output",
            output_xml.to_str().expect("output utf-8"),
            "--json",
        ])
        .output()
        .expect("run plcopen export json");
    assert!(
        export.status.success(),
        "expected export json success, stderr was:\n{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let export_json: serde_json::Value =
        serde_json::from_slice(&export.stdout).expect("parse export JSON report");
    assert_eq!(export_json["target"], "generic-plcopen");
    assert_eq!(export_json["pou_count"], 1);
    assert_eq!(export_json["source_count"], 1);
    assert!(export_json["source_map_path"].is_string());

    let import_project = unique_temp_dir("plcopen-cli-json-import");
    let fixture = fixture_path("codesys.xml");
    let import = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "import",
            "--input",
            fixture.to_str().expect("fixture utf-8"),
            "--project",
            import_project.to_str().expect("import project utf-8"),
            "--json",
        ])
        .output()
        .expect("run plcopen import json");
    assert!(
        import.status.success(),
        "expected import json success, stderr was:\n{}",
        String::from_utf8_lossy(&import.stderr)
    );
    let import_json: serde_json::Value =
        serde_json::from_slice(&import.stdout).expect("parse import JSON report");
    assert_eq!(import_json["detected_ecosystem"], "codesys");
    assert_eq!(import_json["compatibility_coverage"]["verdict"], "partial");
    assert!(import_json["unsupported_diagnostics"]
        .as_array()
        .expect("unsupported diagnostics array")
        .iter()
        .any(|entry| entry["code"] == "PLCO203"));

    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(import_project);
}

#[test]
fn plcopen_export_target_generates_adapter_report_and_default_target_path() {
    let project = unique_temp_dir("plcopen-cli-target-export");
    std::fs::create_dir_all(project.join("src")).expect("create src");
    std::fs::write(
        project.join("src/main.st"),
        r#"
PROGRAM Main
VAR RETAIN
    Counter : INT := 0;
END_VAR
(* marker: %MW8 *)
END_PROGRAM

CONFIGURATION Plant
TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM MainInstance WITH MainTask : Main;
END_CONFIGURATION
"#,
    )
    .expect("write source");

    let export = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "export",
            "--project",
            project.to_str().expect("project utf-8"),
            "--target",
            "ab",
            "--json",
        ])
        .output()
        .expect("run plcopen export target json");
    assert!(
        export.status.success(),
        "expected target export success, stderr was:\n{}",
        String::from_utf8_lossy(&export.stderr)
    );

    let export_json: serde_json::Value =
        serde_json::from_slice(&export.stdout).expect("parse export JSON report");
    assert_eq!(export_json["target"], "allen-bradley");
    assert!(export_json["adapter_report_path"].is_string());
    assert!(export_json["adapter_diagnostics"]
        .as_array()
        .expect("adapter diagnostics array")
        .iter()
        .any(|entry| entry["code"] == "PLCO7AB1"));
    assert!(!export_json["adapter_manual_steps"]
        .as_array()
        .expect("adapter manual steps")
        .is_empty());
    assert!(!export_json["adapter_limitations"]
        .as_array()
        .expect("adapter limitations")
        .is_empty());

    let output_xml = project.join("interop/plcopen.ab.xml");
    assert!(output_xml.is_file(), "expected target default output xml");
    let adapter_report = export_json["adapter_report_path"]
        .as_str()
        .expect("adapter report path");
    assert!(
        std::path::Path::new(adapter_report).is_file(),
        "expected adapter report file"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn plcopen_export_siemens_target_generates_scl_bundle() {
    let project = unique_temp_dir("plcopen-cli-target-export-siemens");
    std::fs::create_dir_all(project.join("src")).expect("create src");
    std::fs::write(
        project.join("src/main.st"),
        r#"
PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := Counter + 1;
END_PROGRAM
"#,
    )
    .expect("write source");

    let export = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "export",
            "--project",
            project.to_str().expect("project utf-8"),
            "--target",
            "siemens",
            "--json",
        ])
        .output()
        .expect("run plcopen export siemens json");
    assert!(
        export.status.success(),
        "expected siemens target export success, stderr was:\n{}",
        String::from_utf8_lossy(&export.stderr)
    );

    let export_json: serde_json::Value =
        serde_json::from_slice(&export.stdout).expect("parse export JSON report");
    assert_eq!(export_json["target"], "siemens-tia");
    assert!(export_json["siemens_scl_bundle_dir"].is_string());
    assert!(export_json["siemens_scl_files"].is_array());

    let output_xml = project.join("interop/plcopen.siemens.xml");
    assert!(output_xml.is_file(), "expected target default output xml");

    let bundle_dir = export_json["siemens_scl_bundle_dir"]
        .as_str()
        .expect("siemens scl bundle dir");
    assert!(
        std::path::Path::new(bundle_dir).is_dir(),
        "expected siemens scl bundle directory"
    );
    let scl_files = export_json["siemens_scl_files"]
        .as_array()
        .expect("siemens scl files");
    assert!(
        !scl_files.is_empty(),
        "expected generated siemens scl files"
    );

    let mut found_main_ob = false;
    for entry in scl_files {
        let path = entry.as_str().expect("siemens scl file path");
        assert!(
            std::path::Path::new(path).is_file(),
            "expected generated siemens scl file to exist: {path}"
        );
        if path.ends_with("_ob_Main.scl") {
            let text = std::fs::read_to_string(path).expect("read main ob file");
            assert!(text.contains("ORGANIZATION_BLOCK \"Main\""));
            assert!(text.contains("END_ORGANIZATION_BLOCK"));
            found_main_ob = true;
        }
    }
    assert!(found_main_ob, "expected converted Main OB Siemens SCL file");

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn plcopen_import_json_detects_openplc_ecosystem_and_shims() {
    let import_project = unique_temp_dir("plcopen-cli-openplc-import");
    let fixture = fixture_path("openplc.xml");
    let import = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "import",
            "--input",
            fixture.to_str().expect("fixture utf-8"),
            "--project",
            import_project.to_str().expect("import project utf-8"),
            "--json",
        ])
        .output()
        .expect("run plcopen import json");
    assert!(
        import.status.success(),
        "expected import json success, stderr was:\n{}",
        String::from_utf8_lossy(&import.stderr)
    );

    let import_json: serde_json::Value =
        serde_json::from_slice(&import.stdout).expect("parse import JSON report");
    assert_eq!(import_json["detected_ecosystem"], "openplc");
    assert!(import_json["unsupported_diagnostics"]
        .as_array()
        .expect("unsupported diagnostics array")
        .iter()
        .any(|entry| entry["code"] == "PLCO203"));
    assert!(import_json["applied_library_shims"]
        .as_array()
        .expect("applied library shims array")
        .iter()
        .any(|entry| entry["vendor"] == "openplc"
            && entry["source_symbol"] == "R_EDGE"
            && entry["replacement_symbol"] == "R_TRIG"));

    let _ = std::fs::remove_dir_all(import_project);
}

#[test]
fn plcopen_openplc_fixture_in_st_complete_bundle_import_export_smoke() {
    let example_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("plcopen_xml_st_complete");
    let fixture = example_root.join("interop/openplc.xml");
    assert!(
        fixture.is_file(),
        "expected OpenPLC fixture in ST-complete bundle"
    );

    let import_project = unique_temp_dir("plcopen-cli-openplc-example-import");
    let import = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "import",
            "--input",
            fixture.to_str().expect("fixture utf-8"),
            "--project",
            import_project.to_str().expect("project utf-8"),
            "--json",
        ])
        .output()
        .expect("run openplc example import");
    assert!(
        import.status.success(),
        "expected example import success, stderr was:\n{}",
        String::from_utf8_lossy(&import.stderr)
    );

    let import_json: serde_json::Value =
        serde_json::from_slice(&import.stdout).expect("parse import json");
    assert_eq!(import_json["detected_ecosystem"], "openplc");
    assert!(import_json["applied_library_shims"]
        .as_array()
        .expect("applied shims array")
        .iter()
        .any(|entry| entry["vendor"] == "openplc"
            && entry["source_symbol"] == "R_EDGE"
            && entry["replacement_symbol"] == "R_TRIG"));

    let output_xml = import_project.join("interop/example-roundtrip.xml");
    let export = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "export",
            "--project",
            import_project.to_str().expect("project utf-8"),
            "--output",
            output_xml.to_str().expect("output utf-8"),
            "--json",
        ])
        .output()
        .expect("run openplc example export");
    assert!(
        export.status.success(),
        "expected example export success, stderr was:\n{}",
        String::from_utf8_lossy(&export.stderr)
    );
    assert!(
        output_xml.is_file(),
        "expected example roundtrip xml output"
    );
    assert!(
        output_xml.with_extension("source-map.json").is_file(),
        "expected source-map sidecar"
    );

    let _ = std::fs::remove_dir_all(import_project);
}

#[test]
fn plcopen_import_json_reports_applied_vendor_library_shims() {
    let project = unique_temp_dir("plcopen-cli-shim-import");
    std::fs::create_dir_all(&project).expect("create temp project");
    let input_xml = project.join("siemens-shim.xml");
    std::fs::write(
        &input_xml,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <fileHeader companyName="Siemens AG" productName="TIA Portal V18" />
  <types>
    <pous>
      <pou name="MainOb1" pouType="PRG">
        <body>
          <ST><![CDATA[
PROGRAM MainOb1
VAR
  DelayTimer : SFB4;
END_VAR
DelayTimer(IN := TRUE, PT := T#1s);
END_PROGRAM
]]></ST>
        </body>
      </pou>
    </pous>
  </types>
</project>
"#,
    )
    .expect("write shim fixture");

    let import_project = unique_temp_dir("plcopen-cli-shim-import-out");
    let import = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "plcopen",
            "import",
            "--input",
            input_xml.to_str().expect("input xml utf-8"),
            "--project",
            import_project.to_str().expect("import project utf-8"),
            "--json",
        ])
        .output()
        .expect("run plcopen import json");
    assert!(
        import.status.success(),
        "expected import json success, stderr was:\n{}",
        String::from_utf8_lossy(&import.stderr)
    );

    let import_json: serde_json::Value =
        serde_json::from_slice(&import.stdout).expect("parse import JSON report");
    assert_eq!(import_json["detected_ecosystem"], "siemens-tia");
    assert!(import_json["applied_library_shims"]
        .as_array()
        .expect("applied library shims array")
        .iter()
        .any(|entry| entry["source_symbol"] == "SFB4" && entry["replacement_symbol"] == "TON"));
    assert!(import_json["unsupported_diagnostics"]
        .as_array()
        .expect("unsupported diagnostics array")
        .iter()
        .any(|entry| entry["code"] == "PLCO301"));

    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(import_project);
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
