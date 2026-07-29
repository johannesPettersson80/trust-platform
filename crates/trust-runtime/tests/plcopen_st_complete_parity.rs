use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use trust_runtime::plcopen::{export_project_to_xml, import_xml_to_project};

include!("../../../tests/support/repository_source_oracle.rs");

#[derive(Debug, Deserialize)]
struct ExpectedCompatibilityCoverage {
    supported_items: usize,
    partial_items: usize,
    unsupported_items: usize,
    verdict: String,
}

#[derive(Debug, Deserialize)]
struct ExpectedMigrationArtifact {
    detected_ecosystem: String,
    discovered_pous: usize,
    imported_pous: usize,
    imported_data_types: usize,
    discovered_configurations: usize,
    imported_configurations: usize,
    imported_resources: usize,
    imported_tasks: usize,
    imported_program_instances: usize,
    compatibility_coverage: ExpectedCompatibilityCoverage,
    unsupported_nodes: Vec<String>,
    diagnostic_codes: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct XmlProjectSignature {
    pous: Vec<(String, String, String)>,
    data_types: Vec<(String, String)>,
    configurations: Vec<String>,
}

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
        .join("codesys_st_complete")
        .join(name)
}

fn read_expected(name: &str) -> ExpectedMigrationArtifact {
    let path = fixture_path(name);
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let text = repository_source_tree_read_to_string!(
        (&path, &repository_root),
        roots = ["crates/trust-runtime/tests/fixtures/plcopen/codesys_st_complete"],
        extension = "json",
    )
    .expect("read expected migration artifact");
    serde_json::from_str(&text).expect("parse expected migration artifact")
}

fn normalize_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

fn normalize_inline_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn canonical_element(node: roxmltree::Node<'_, '_>) -> String {
    let mut out = String::new();
    let tag = node.tag_name().name().to_ascii_lowercase();
    out.push('<');
    out.push_str(&tag);

    let mut attributes = node
        .attributes()
        .map(|attribute| {
            (
                attribute.name().to_ascii_lowercase(),
                normalize_text(attribute.value()),
            )
        })
        .collect::<Vec<_>>();
    attributes.sort();
    for (name, value) in attributes {
        out.push(' ');
        out.push_str(&name);
        out.push_str("=\"");
        out.push_str(&value);
        out.push('"');
    }
    out.push('>');

    for child in node.children() {
        if child.is_element() {
            out.push_str(&canonical_element(child));
        } else if child.is_text() {
            let Some(text) = child.text() else {
                continue;
            };
            let normalized = normalize_inline_text(text);
            if !normalized.is_empty() {
                out.push_str(&normalized);
            }
        }
    }

    out.push_str("</");
    out.push_str(&tag);
    out.push('>');
    out
}

fn xml_signature(xml_text: &str) -> XmlProjectSignature {
    let doc = roxmltree::Document::parse(xml_text).expect("parse XML");

    let mut pous = doc
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name().eq_ignore_ascii_case("pou"))
        .filter_map(|pou| {
            let name = pou.attribute("name")?.trim().to_string();
            let pou_type = pou.attribute("pouType")?.trim().to_ascii_lowercase();
            let st = pou
                .children()
                .find(|child| {
                    child.is_element() && child.tag_name().name().eq_ignore_ascii_case("body")
                })
                .and_then(|body| {
                    body.descendants().find(|child| {
                        child.is_element() && child.tag_name().name().eq_ignore_ascii_case("st")
                    })
                })
                .and_then(|st| st.text())
                .map(normalize_text)
                .unwrap_or_default();
            Some((name, pou_type, st))
        })
        .collect::<Vec<_>>();
    pous.sort();

    let mut data_types = doc
        .descendants()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name().eq_ignore_ascii_case("dataType")
                && node
                    .ancestors()
                    .any(|ancestor| ancestor.tag_name().name().eq_ignore_ascii_case("dataTypes"))
        })
        .filter_map(|data_type| {
            let name = data_type.attribute("name")?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let canonical = data_type
                .children()
                .find(|child| {
                    child.is_element()
                        && (child.tag_name().name().eq_ignore_ascii_case("baseType")
                            || child.tag_name().name().eq_ignore_ascii_case("type"))
                })
                .map(canonical_element)
                .unwrap_or_default();
            Some((name, canonical))
        })
        .collect::<Vec<_>>();
    data_types.sort();

    let mut configurations = doc
        .descendants()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name().eq_ignore_ascii_case("configuration")
                && node
                    .ancestors()
                    .any(|ancestor| ancestor.tag_name().name().eq_ignore_ascii_case("instances"))
        })
        .map(canonical_element)
        .collect::<Vec<_>>();
    configurations.sort();

    XmlProjectSignature {
        pous,
        data_types,
        configurations,
    }
}

fn assert_migration_report(case_name: &str, report: &trust_runtime::plcopen::PlcopenImportReport) {
    let expected = read_expected(&format!("{case_name}.expected-migration.json"));

    assert_eq!(report.detected_ecosystem, expected.detected_ecosystem);
    assert_eq!(report.discovered_pous, expected.discovered_pous);
    assert_eq!(report.imported_pous, expected.imported_pous);
    assert_eq!(report.imported_data_types, expected.imported_data_types);
    assert_eq!(
        report.discovered_configurations,
        expected.discovered_configurations
    );
    assert_eq!(
        report.imported_configurations,
        expected.imported_configurations
    );
    assert_eq!(report.imported_resources, expected.imported_resources);
    assert_eq!(report.imported_tasks, expected.imported_tasks);
    assert_eq!(
        report.imported_program_instances,
        expected.imported_program_instances
    );

    assert_eq!(
        report.compatibility_coverage.supported_items,
        expected.compatibility_coverage.supported_items
    );
    assert_eq!(
        report.compatibility_coverage.partial_items,
        expected.compatibility_coverage.partial_items
    );
    assert_eq!(
        report.compatibility_coverage.unsupported_items,
        expected.compatibility_coverage.unsupported_items
    );
    assert_eq!(
        report.compatibility_coverage.verdict,
        expected.compatibility_coverage.verdict
    );

    let mut actual_nodes = report.unsupported_nodes.clone();
    actual_nodes.sort();
    let mut expected_nodes = expected.unsupported_nodes;
    expected_nodes.sort();
    assert_eq!(actual_nodes, expected_nodes);

    let mut actual_codes = report
        .unsupported_diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.clone())
        .collect::<Vec<_>>();
    actual_codes.sort();
    actual_codes.dedup();

    let mut expected_codes = expected.diagnostic_codes;
    expected_codes.sort();
    expected_codes.dedup();

    assert_eq!(actual_codes, expected_codes);
}

fn run_case(case_name: &str) {
    let source_xml = fixture_path(&format!("{case_name}.xml"));
    let project_a = unique_temp_dir(&format!("plcopen-st-complete-{case_name}-a"));
    let import_report_a =
        import_xml_to_project(&source_xml, &project_a).expect("import fixture into project A");

    assert_migration_report(case_name, &import_report_a);

    let export_a = project_a.join("interop/export-a.xml");
    export_project_to_xml(&project_a, &export_a).expect("export project A to XML");
    let signature_a = xml_signature(&std::fs::read_to_string(&export_a).expect("read export A"));

    assert!(
        !signature_a.pous.is_empty(),
        "expected at least one POU in exported signature"
    );
    assert!(
        !signature_a.configurations.is_empty(),
        "expected at least one configuration in exported signature"
    );

    let project_b = unique_temp_dir(&format!("plcopen-st-complete-{case_name}-b"));
    import_xml_to_project(&export_a, &project_b).expect("import export A into project B");
    let export_b = project_b.join("interop/export-b.xml");
    export_project_to_xml(&project_b, &export_b).expect("export project B to XML");
    let signature_b = xml_signature(&std::fs::read_to_string(&export_b).expect("read export B"));

    assert_eq!(
        signature_a, signature_b,
        "expected deterministic PLCopen ST-complete round-trip signature for case '{case_name}'"
    );

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}

#[test]
fn plcopen_codesys_st_complete_small_parity() {
    run_case("small");
}

#[test]
fn plcopen_codesys_st_complete_medium_parity() {
    run_case("medium");
}

#[test]
fn plcopen_codesys_st_complete_large_parity() {
    run_case("large");
}
