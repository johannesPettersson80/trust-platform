use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use trust_runtime::plcopen::import_xml_to_project;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("plcopen")
        .join(name)
}

fn read_json(path: &Path) -> serde_json::Value {
    let bytes = std::fs::read(path).expect("read migration report");
    serde_json::from_slice(&bytes).expect("parse migration report")
}

fn approx_eq(left: f64, right: f64, epsilon: f64) -> bool {
    (left - right).abs() <= epsilon
}

#[test]
fn migration_import_codesys_fixture_reports_coverage_and_loss() {
    let project = unique_temp_dir("plcopen-migration-codesys");
    let fixture = fixture_path("codesys.xml");

    let report = import_xml_to_project(&fixture, &project).expect("import codesys fixture");

    assert_eq!(report.detected_ecosystem, "codesys");
    assert_eq!(report.discovered_pous, 3);
    assert_eq!(report.imported_pous, 2);
    assert!(approx_eq(report.source_coverage_percent, 66.67, 0.01));
    assert!(report.semantic_loss_percent > 0.0);
    assert!(report.migration_report_path.is_file());
    assert_eq!(report.compatibility_coverage.verdict, "partial");
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO203"));

    let migration = read_json(&report.migration_report_path);
    assert_eq!(migration["detected_ecosystem"], "codesys");
    assert_eq!(migration["discovered_pous"], 3);
    assert_eq!(migration["imported_pous"], 2);
    assert_eq!(migration["skipped_pous"], 1);
    assert_eq!(migration["compatibility_coverage"]["verdict"], "partial");
    assert!(migration["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .any(|entry| entry["status"] == "skipped"));
    assert!(migration["unsupported_diagnostics"]
        .as_array()
        .expect("unsupported_diagnostics array")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "PLCO203"));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn migration_import_twincat_fixture_handles_vendor_variants() {
    let project = unique_temp_dir("plcopen-migration-twincat");
    let fixture = fixture_path("twincat.xml");

    let report = import_xml_to_project(&fixture, &project).expect("import twincat fixture");

    assert_eq!(report.detected_ecosystem, "beckhoff-twincat");
    assert_eq!(report.discovered_pous, 3);
    assert_eq!(report.imported_pous, 2);
    assert!(report.written_sources.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("MainTc.st"))
    }));
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("missing body/ST")));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO204"));

    let migration = read_json(&report.migration_report_path);
    assert_eq!(migration["detected_ecosystem"], "beckhoff-twincat");
    assert!(migration["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .any(|entry| {
            entry["name"] == "NoBody"
                && entry["status"] == "skipped"
                && entry["reason"] == "missing body/ST"
        }));
    assert!(migration["unsupported_diagnostics"]
        .as_array()
        .expect("unsupported_diagnostics array")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "PLCO204"));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn migration_import_siemens_fixture_reports_vendor_coverage() {
    let project = unique_temp_dir("plcopen-migration-siemens");
    let fixture = fixture_path("siemens.xml");

    let report = import_xml_to_project(&fixture, &project).expect("import siemens fixture");

    assert_eq!(report.detected_ecosystem, "siemens-tia");
    assert_eq!(report.discovered_pous, 3);
    assert_eq!(report.imported_pous, 2);
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO501"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO203"));

    let migration = read_json(&report.migration_report_path);
    assert_eq!(migration["detected_ecosystem"], "siemens-tia");
    assert!(migration["unsupported_nodes"]
        .as_array()
        .expect("unsupported_nodes array")
        .iter()
        .any(|entry| entry == "instances"));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn migration_import_rockwell_fixture_reports_vendor_coverage() {
    let project = unique_temp_dir("plcopen-migration-rockwell");
    let fixture = fixture_path("rockwell.xml");

    let report = import_xml_to_project(&fixture, &project).expect("import rockwell fixture");

    assert_eq!(report.detected_ecosystem, "rockwell-studio5000");
    assert_eq!(report.discovered_pous, 3);
    assert_eq!(report.imported_pous, 2);
    assert_eq!(report.compatibility_coverage.verdict, "partial");
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO203"));

    let migration = read_json(&report.migration_report_path);
    assert_eq!(migration["detected_ecosystem"], "rockwell-studio5000");
    assert_eq!(migration["compatibility_coverage"]["verdict"], "partial");

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn migration_import_schneider_fixture_detects_vendor_precedence() {
    let project = unique_temp_dir("plcopen-migration-schneider");
    let fixture = fixture_path("schneider.xml");

    let report = import_xml_to_project(&fixture, &project).expect("import schneider fixture");

    assert_eq!(report.detected_ecosystem, "schneider-ecostruxure");
    assert_eq!(report.discovered_pous, 2);
    assert_eq!(report.imported_pous, 1);
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO402"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO204"));

    let migration = read_json(&report.migration_report_path);
    assert_eq!(migration["detected_ecosystem"], "schneider-ecostruxure");

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn migration_import_openplc_fixture_reports_vendor_coverage() {
    let project = unique_temp_dir("plcopen-migration-openplc");
    let fixture = fixture_path("openplc.xml");

    let report = import_xml_to_project(&fixture, &project).expect("import openplc fixture");

    assert_eq!(report.detected_ecosystem, "openplc");
    assert_eq!(report.discovered_pous, 2);
    assert_eq!(report.imported_pous, 1);
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO203"));
    assert!(report
        .unsupported_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PLCO301"));
    assert!(report
        .applied_library_shims
        .iter()
        .any(|entry| entry.vendor == "openplc"
            && entry.source_symbol == "R_EDGE"
            && entry.replacement_symbol == "R_TRIG"));

    let migration = read_json(&report.migration_report_path);
    assert_eq!(migration["detected_ecosystem"], "openplc");
    assert!(migration["unsupported_diagnostics"]
        .as_array()
        .expect("unsupported_diagnostics array")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "PLCO203"));
    assert!(migration["applied_library_shims"]
        .as_array()
        .expect("applied library shims array")
        .iter()
        .any(|entry| entry["vendor"] == "openplc"
            && entry["source_symbol"] == "R_EDGE"
            && entry["replacement_symbol"] == "R_TRIG"));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn migration_semantic_loss_scoring_reflects_import_completeness() {
    let clean_project = unique_temp_dir("plcopen-migration-clean");
    let clean_xml = clean_project.join("clean.xml");
    std::fs::write(
        &clean_xml,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="Main" pouType="PROGRAM"><body><ST><![CDATA[
PROGRAM Main
END_PROGRAM
]]></ST></body></pou>
      <pou name="Helper" pouType="FUNCTION"><body><ST><![CDATA[
FUNCTION Helper : INT
Helper := 1;
END_FUNCTION
]]></ST></body></pou>
    </pous>
  </types>
</project>
"#,
    )
    .expect("write clean fixture");

    let clean_report = import_xml_to_project(&clean_xml, &clean_project).expect("import clean xml");
    assert!(approx_eq(clean_report.source_coverage_percent, 100.0, 0.01));
    assert!(approx_eq(clean_report.semantic_loss_percent, 0.0, 0.01));
    assert_eq!(clean_report.compatibility_coverage.verdict, "full");

    let lossy_project = unique_temp_dir("plcopen-migration-lossy");
    let lossy_fixture = fixture_path("codesys.xml");
    let lossy_report =
        import_xml_to_project(&lossy_fixture, &lossy_project).expect("import lossy fixture");

    assert!(lossy_report.semantic_loss_percent > clean_report.semantic_loss_percent);
    assert!(lossy_report.source_coverage_percent < clean_report.source_coverage_percent);
    assert_eq!(lossy_report.compatibility_coverage.verdict, "partial");

    let _ = std::fs::remove_dir_all(clean_project);
    let _ = std::fs::remove_dir_all(lossy_project);
}
