use super::extract;

#[test]
fn generated_factories_and_producer_result_fields_close_file_oracles() {
    let source = r#"
use std::path::{Path, PathBuf};

struct Report {
    artifact_path: PathBuf,
}

fn unique_temp_dir(_: &str) -> PathBuf {
    std::env::temp_dir().join("generated-factory")
}

fn produce_report(_: &Path) -> Report {
    todo!()
}

#[test]
fn generated_report() {
    let project = unique_temp_dir("report");
    let report = produce_report(&project);
    let bytes = std::fs::read(generated_output_path!(&report.artifact_path, &project)).unwrap();
    assert!(!bytes.is_empty());
}
"#;

    let inventory = extract(
        "crates/example/tests/generated_report.rs",
        "generated_report",
        source,
    )
    .expect("extract generated report");

    assert!(
        inventory.unresolved_assertion_helpers.is_empty(),
        "a producer result derived from a generated root remains generated: {:?}",
        inventory.unresolved_assertion_helpers
    );
}

#[test]
fn generated_receiver_method_result_fields_close_file_oracles() {
    let source = r#"
use std::path::{Path, PathBuf};

struct Config;
struct Producer;
struct Readiness {
    mapping_path: PathBuf,
}

fn temp_path(_: &str) -> PathBuf {
    std::env::temp_dir().join("generated-method")
}

impl Config {
    fn with_root(_: PathBuf) -> Self {
        Self
    }
}

impl Producer {
    fn with_config(_: Config) -> Self {
        Self
    }

    fn readiness(&self) -> Readiness {
        todo!()
    }
}

#[test]
fn generated_mapping() {
    let root = temp_path("mapping");
    let producer = Producer::with_config(Config::with_root(root.clone()));
    let readiness = producer.readiness();
    let bytes = std::fs::read(generated_output_path!(&readiness.mapping_path, &root)).unwrap();
    assert!(!bytes.is_empty());
}
"#;

    let inventory = extract(
        "crates/example/tests/generated_mapping.rs",
        "generated_mapping",
        source,
    )
    .expect("extract generated method result");

    assert!(
        inventory.unresolved_assertion_helpers.is_empty(),
        "a method result derived from a generated receiver remains generated: {:?}",
        inventory.unresolved_assertion_helpers
    );
}

#[test]
fn explicit_generated_test_root_closes_a_checked_output_path() {
    let source = r#"
use std::path::PathBuf;

fn project_dir() -> PathBuf {
    todo!()
}

#[test]
fn generated_project() {
    let project = generated_test_root!(project_dir());
    let output = project.join("report.json");
    let bytes = std::fs::read(&output).unwrap();
    assert!(!bytes.is_empty());
}
"#;

    let inventory = extract(
        "crates/example/tests/generated_project.rs",
        "generated_project",
        source,
    )
    .expect("extract explicit generated test root");

    assert!(inventory.unresolved_assertion_helpers.is_empty());
}

#[test]
fn external_factories_and_escaping_joins_remain_unresolved() {
    let source = r#"
use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    todo!()
}

fn temp_path(_: &str) -> PathBuf {
    std::env::temp_dir().join("generated-negative")
}

#[test]
fn external_fixture() {
    let path = fixture_path();
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn escaping_generated_path() {
    let root = temp_path("escape");
    let path = root.join("/etc/passwd");
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn generated_output_macro_with_external_root() {
    let root = fixture_path();
    let output = fixture_path();
    let bytes = std::fs::read(generated_output_path!(&output, &root)).unwrap();
    assert!(!bytes.is_empty());
}
"#;

    for name in [
        "external_fixture",
        "escaping_generated_path",
        "generated_output_macro_with_external_root",
    ] {
        let inventory = extract("crates/example/tests/generated_negative.rs", name, source)
            .expect("extract rejected generated path");
        assert_eq!(
            1,
            inventory.unresolved_assertion_helpers.len(),
            "{name} must retain its external oracle"
        );
        assert!(
            inventory.unresolved_assertion_helpers[0].ends_with(":unresolved-oracle"),
            "{name} must remain an explicit unresolved oracle"
        );
    }
}
