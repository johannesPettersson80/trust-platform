use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{extract, extract_from_context, extract_from_repository, ContextSource};

static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn assertion_fixture_root(label: &str) -> PathBuf {
    let ordinal = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-test-assertions-{label}-{}-{ordinal}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn follows_unique_local_helper_and_records_exact_assertion_sites() {
    let source = r#"
fn expect_rejected(value: Result<(), String>) {
    let error = value.expect_err("must reject");
    assert!(error.contains("invalid"));
}

#[test]
fn rejects_invalid() {
    expect_rejected(Err("invalid".into()));
}
"#;
    let inventory = extract("crates/example/tests/reject.rs", "rejects_invalid", source)
        .expect("extract assertions");

    assert_eq!(2, inventory.assertions.len());
    assert!(inventory
        .assertions
        .iter()
        .any(|site| site.kind == "method:expect_err"));
    assert!(inventory
        .assertions
        .iter()
        .any(|site| site.kind == "macro:assert"));
    assert!(inventory.unresolved_assertion_helpers.is_empty());
}

#[test]
fn unresolved_assertion_helper_is_explicit() {
    let source = r#"
#[test]
fn delegates() {
    expect_external_contract();
}
"#;
    let inventory = extract("crates/example/tests/delegate.rs", "delegates", source)
        .expect("extract assertions");

    assert_eq!(
        vec!["delegates::expect_external_contract:unresolved"],
        inventory.unresolved_assertion_helpers
    );
}

#[test]
fn follows_unique_helper_from_sibling_module_and_binds_source_closure() {
    let test_path = "crates/example/tests/contract/case.rs";
    let helper_path = "crates/example/tests/common.rs";
    let inventory = extract_from_context(
        test_path,
        "delegates",
        vec![
            ContextSource {
                path: test_path.to_string(),
                module: vec!["case".to_string()],
                source: "use crate::common::*;\n#[test]\nfn delegates() { check_contract(); }\n"
                    .to_string(),
            },
            ContextSource {
                path: helper_path.to_string(),
                module: vec!["common".to_string()],
                source: "fn check_contract() { assert!(true); }\n".to_string(),
            },
        ],
    )
    .expect("extract cross-file assertions");

    assert!(inventory.unresolved_assertion_helpers.is_empty());
    assert_eq!(1, inventory.assertions.len());
    assert_eq!("common::check_contract", inventory.assertions[0].function);
    assert_eq!(
        vec![helper_path.to_string(), test_path.to_string()],
        inventory.source_digests.keys().cloned().collect::<Vec<_>>()
    );
}

#[test]
fn resolves_qualified_named_renamed_glob_and_super_imports_exactly() {
    let test_path = "crates/example/tests/contract/part.rs";
    let common_path = "crates/example/tests/common.rs";
    let root_path = "crates/example/tests/contract.rs";
    let inventory = extract_from_context(
        test_path,
        "delegates",
        vec![
            ContextSource {
                path: root_path.to_string(),
                module: Vec::new(),
                source: "fn check_parent() { assert!(true); }\n".to_string(),
            },
            ContextSource {
                path: common_path.to_string(),
                module: vec!["common".to_string()],
                source: concat!(
                    "fn check_named() { assert!(true); }\n",
                    "fn check_renamed() { assert!(true); }\n",
                    "fn check_globbed() { assert!(true); }\n",
                    "fn check_qualified() { assert!(true); }\n",
                )
                .to_string(),
            },
            ContextSource {
                path: test_path.to_string(),
                module: vec!["part".to_string()],
                source: concat!(
                    "use crate::common::{check_named, check_renamed as alias, *};\n",
                    "use super::*;\n",
                    "#[test]\n",
                    "fn delegates() {\n",
                    "  check_named();\n",
                    "  alias();\n",
                    "  check_globbed();\n",
                    "  crate::common::check_qualified();\n",
                    "  check_parent();\n",
                    "}\n",
                )
                .to_string(),
            },
        ],
    )
    .expect("resolve exact imported helpers");

    assert!(inventory.unresolved_assertion_helpers.is_empty());
    assert_eq!(5, inventory.assertions.len());
    let assertion_sources = inventory
        .assertions
        .iter()
        .map(|record| {
            serde_json::to_value(record)
                .expect("serialize assertion")
                .get("source_path")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        vec![
            Some(common_path.to_string()),
            Some(common_path.to_string()),
            Some(common_path.to_string()),
            Some(common_path.to_string()),
            Some(root_path.to_string()),
        ],
        assertion_sources
    );
}

#[test]
fn rejects_same_file_helper_from_unrelated_inline_module() {
    let inventory = extract(
        "crates/example/tests/contract.rs",
        "delegates",
        concat!(
            "mod unrelated { fn run_case() { assert!(true); } }\n",
            "#[test]\n",
            "fn delegates() { run_case(); }\n",
        ),
    )
    .expect("extract conservative attribution");

    assert!(inventory.assertions.is_empty());
    assert_eq!(
        vec!["delegates::run_case:unresolved"],
        inventory.unresolved_assertion_helpers
    );
}

#[test]
fn ignores_inactive_host_platform_cfg_duplicate_helpers() {
    let inventory = extract(
        "crates/example/src/project.rs",
        "uses_platform_helper",
        concat!(
            "#[cfg(windows)]\n",
            "fn platform_helper() { assert!(cfg!(windows)); }\n",
            "#[cfg(not(windows))]\n",
            "fn platform_helper() { assert!(cfg!(unix)); }\n",
            "#[test]\n",
            "fn uses_platform_helper() { platform_helper(); }\n",
        ),
    )
    .expect("extract active platform helper only");

    assert!(inventory.unresolved_assertion_helpers.is_empty());
    assert_eq!(1, inventory.assertions.len());
    assert_eq!("platform_helper", inventory.assertions[0].function);
}

#[test]
fn follows_mutually_exclusive_feature_cfg_helper_variants() {
    let inventory = extract(
        "crates/example/src/protocol.rs",
        "uses_feature_helper",
        concat!(
            "#[cfg(feature = \"wire\")]\n",
            "fn protocol_helper() { assert_eq!(\"wire\", \"wire\"); }\n",
            "#[cfg(not(feature = \"wire\"))]\n",
            "fn protocol_helper() { assert_eq!(\"offline\", \"offline\"); }\n",
            "#[test]\n",
            "fn uses_feature_helper() { protocol_helper(); }\n",
        ),
    )
    .expect("extract both feature helper variants");

    assert!(inventory.unresolved_assertion_helpers.is_empty());
    assert_eq!(2, inventory.assertions.len());
    assert!(inventory
        .assertions
        .iter()
        .all(|assertion| assertion.function == "protocol_helper"));
}

#[test]
fn ignores_assertion_like_methods_reached_in_production_functions() {
    let source = r#"
fn type_check() {
    checker
        .stmt()
        .check_statement_list_with_labels(&statement_list);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_type_error() {
        type_check();
        assert!(true);
    }
}
"#;
    let inventory = extract(
        "crates/example/src/type_check.rs",
        "reports_type_error",
        source,
    )
    .expect("extract type-check assertions");

    assert!(inventory.unresolved_assertion_helpers.is_empty());
}

#[test]
fn retains_unresolved_custom_methods_in_test_only_helpers() {
    let source = r#"
#[cfg(test)]
mod tests {
    fn check_contract() {
        fixture.check_contract();
    }

    #[test]
    fn reports_contract() {
        check_contract();
        assert!(true);
    }
}

"#;
    let inventory = extract("crates/example/src/contract.rs", "reports_contract", source)
        .expect("extract test-only method helper");

    assert_eq!(
        vec!["tests::check_contract::check_contract:unresolved-method"],
        inventory.unresolved_assertion_helpers
    );
}

#[test]
fn assert_eq_method_is_recorded_as_oracle_site() {
    let source = r#"
#[test]
fn compares_runtime_value() {
    harness.assert_eq("count", 1);
}
"#;
    let inventory = extract(
        "crates/example/src/harness.rs",
        "compares_runtime_value",
        source,
    )
    .expect("extract assertion method");

    assert!(inventory.unresolved_assertion_helpers.is_empty());
    assert_eq!(1, inventory.assertions.len());
    assert_eq!("method:assert_eq", inventory.assertions[0].kind);
}

#[test]
fn qualified_external_calls_do_not_collide_with_local_function_names() {
    let source = r#"
fn temp_dir() {
    let _ = std::env::temp_dir();
}

#[test]
fn from_source() {
    temp_dir();
    let _harness = TestHarness::from_source("PROGRAM main END_PROGRAM").unwrap();
    assert!(true);
}
"#;
    let inventory = extract(
        "crates/example/src/harness.rs",
        "from_source",
        source,
    )
    .expect("extract qualified external calls");

    assert!(inventory.unresolved_assertion_helpers.is_empty());
    assert_eq!(
        vec!["method:unwrap", "macro:assert"],
        inventory
            .assertions
            .iter()
            .map(|assertion| assertion.kind.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn checked_arithmetic_methods_are_not_assertion_helpers() {
    let source = r#"
#[cfg(test)]
mod tests {
    fn charge(total: usize) -> Option<usize> {
        total.checked_add(1)
    }

    #[test]
    fn accepts_boundary() {
        assert_eq!(charge(1), Some(2));
    }
}
"#;
    let inventory = extract(
        "crates/example/src/resource_limits.rs",
        "accepts_boundary",
        source,
    )
    .expect("extract checked arithmetic assertions");

    assert!(inventory.unresolved_assertion_helpers.is_empty());
}

#[test]
fn binds_static_include_and_named_insta_snapshot_oracles() {
    let root = assertion_fixture_root("external-oracles");
    let test_path = "crates/example/tests/contract.rs";
    let source_path = root.join(test_path);
    fs::create_dir_all(source_path.parent().unwrap().join("fixtures")).unwrap();
    fs::create_dir_all(source_path.parent().unwrap().join("snapshots")).unwrap();
    fs::write(
        &source_path,
        concat!(
            "#[test]\n",
            "fn contract() {\n",
            "  assert_eq!(include_str!(\"fixtures/expected.txt\"), \"ok\");\n",
            "  insta::assert_snapshot!(\"contract_output\", \"ok\");\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        source_path.parent().unwrap().join("fixtures/expected.txt"),
        "ok",
    )
    .unwrap();
    fs::write(
        source_path
            .parent()
            .unwrap()
            .join("snapshots/contract__contract_output.snap"),
        "---\nsource: contract.rs\nexpression: \"ok\"\n---\nok\n",
    )
    .unwrap();

    let inventory = extract_from_repository(&root, test_path, "contract", "rust_integration_test")
        .expect("extract external assertion oracles");

    assert_eq!(2, inventory.schema_version);
    assert_eq!(
        vec![
            "crates/example/tests/contract.rs",
            "crates/example/tests/fixtures/expected.txt",
            "crates/example/tests/snapshots/contract__contract_output.snap",
        ],
        inventory
            .source_digests
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn binds_static_runtime_fixture_reads_and_rejects_dynamic_paths() {
    let root = assertion_fixture_root("runtime-fixtures");
    let test_path = "crates/example/tests/contract.rs";
    let source_path = root.join(test_path);
    fs::create_dir_all(source_path.parent().unwrap().join("fixtures")).unwrap();
    fs::write(
        &source_path,
        concat!(
            "#[test]\n",
            "fn static_fixture() {\n",
            "  let value = std::fs::read_to_string(\"fixtures/expected.txt\").unwrap();\n",
            "  assert_eq!(value, \"ok\");\n",
            "}\n",
            "fn fixture_path() -> std::path::PathBuf { todo!() }\n",
            "fn temp_dir(_: &str) -> std::path::PathBuf { todo!() }\n",
            "#[test]\n",
            "fn dynamic_fixture() {\n",
            "  let value = std::fs::read(fixture_path()).unwrap();\n",
            "  assert!(!value.is_empty());\n",
            "}\n",
            "#[test]\n",
            "fn generated_output() {\n",
            "  let root = temp_dir(\"generated-output\");\n",
            "  let project = root.join(\"project-1\");\n",
            "  let output = project.join(\"io.toml\");\n",
            "  let value = std::fs::read_to_string(&output).unwrap();\n",
            "  assert!(!value.is_empty());\n",
            "}\n",
            "#[test]\n",
            "fn generated_parent_escape() {\n",
            "  let root = temp_dir(\"generated-parent\");\n",
            "  let output = root.join(\"../outside\");\n",
            "  let value = std::fs::read_to_string(&output).unwrap();\n",
            "  assert!(!value.is_empty());\n",
            "}\n",
            "#[test]\n",
            "fn generated_absolute_escape() {\n",
            "  let root = temp_dir(\"generated-absolute\");\n",
            "  let output = root.join(\"/etc/passwd\");\n",
            "  let value = std::fs::read_to_string(&output).unwrap();\n",
            "  assert!(!value.is_empty());\n",
            "}\n",
            "#[test]\n",
            "fn generated_dynamic_escape() {\n",
            "  let root = temp_dir(\"generated-dynamic\");\n",
            "  let output = root.join(fixture_path());\n",
            "  let value = std::fs::read_to_string(&output).unwrap();\n",
            "  assert!(!value.is_empty());\n",
            "}\n",
            "#[test]\n",
            "fn generated_reassigned_escape() {\n",
            "  let root = temp_dir(\"generated-reassigned\");\n",
            "  let mut output = root.join(\"inside\");\n",
            "  output = fixture_path();\n",
            "  let value = std::fs::read_to_string(&output).unwrap();\n",
            "  assert!(!value.is_empty());\n",
            "}\n",
            "struct Report { path: std::path::PathBuf }\n",
            "fn report(_: std::path::PathBuf) -> Report { todo!() }\n",
            "#[test]\n",
            "fn generated_report_path_is_not_trusted() {\n",
            "  let root = temp_dir(\"generated-report\");\n",
            "  let report = report(root);\n",
            "  let value = std::fs::read_to_string(&report.path).unwrap();\n",
            "  assert!(!value.is_empty());\n",
            "}\n",
        ),
    )
    .unwrap();
    fs::write(
        source_path.parent().unwrap().join("fixtures/expected.txt"),
        "ok",
    )
    .unwrap();

    let static_inventory =
        extract_from_repository(&root, test_path, "static_fixture", "rust_integration_test")
            .expect("extract static runtime fixture");
    assert!(static_inventory.unresolved_assertion_helpers.is_empty());
    assert!(static_inventory
        .source_digests
        .contains_key("crates/example/tests/fixtures/expected.txt"));

    let dynamic_inventory =
        extract_from_repository(&root, test_path, "dynamic_fixture", "rust_integration_test")
            .expect("extract dynamic runtime fixture");
    assert_eq!(
        vec!["dynamic_fixture::std::fs::read:unresolved-oracle"],
        dynamic_inventory.unresolved_assertion_helpers
    );

    let generated_inventory = extract_from_repository(
        &root,
        test_path,
        "generated_output",
        "rust_integration_test",
    )
    .expect("extract generated output read");
    assert!(
        generated_inventory.unresolved_assertion_helpers.is_empty(),
        "test-created project output is not an external fixture oracle"
    );
    assert_eq!(
        static_inventory.source_digests,
        generated_inventory.source_digests,
        "generated outputs do not add source-closure dependencies"
    );
    for name in [
        "generated_parent_escape",
        "generated_absolute_escape",
        "generated_dynamic_escape",
        "generated_reassigned_escape",
        "generated_report_path_is_not_trusted",
    ] {
        let inventory = extract_from_repository(
            &root,
            test_path,
            name,
            "rust_integration_test",
        )
        .expect("extract rejected generated-output path");
        assert!(
            inventory
                .unresolved_assertion_helpers
                .iter()
                .any(|helper| helper.ends_with(
                    "std::fs::read_to_string:unresolved-oracle"
                )),
            "{name} must remain unresolved"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_runtime_observation_is_unresolved_only_when_reachable() {
    let source = concat!(
        "fn read_host_metric() {\n",
        "  let _ = std::fs::read_to_string(\"/proc/self/status\");\n",
        "  assert!(true);\n",
        "}\n",
        "#[test]\n",
        "fn ordinary_case() { assert_eq!(1, 1); }\n",
        "#[test]\n",
        "fn host_metric_case() { read_host_metric(); }\n",
    );

    let ordinary = extract("crates/example/tests/runtime.rs", "ordinary_case", source)
        .expect("unreachable host observation must not block extraction");
    assert!(ordinary.unresolved_assertion_helpers.is_empty());

    let observed = extract(
        "crates/example/tests/runtime.rs",
        "host_metric_case",
        source,
    )
    .expect("reachable host observation remains an explicit unresolved oracle");
    assert_eq!(
        vec!["read_host_metric::std::fs::read_to_string:unresolved-oracle"],
        observed.unresolved_assertion_helpers
    );
}
