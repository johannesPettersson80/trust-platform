use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    discover_cargo_targets, extract_rust_source, parse_rust_extract_args,
    scan_crate_root_module_graph, scan_module_graph, scan_rust_universe, scan_source_topology,
};

static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn cargo_universe_includes_product_custom_build_and_example_targets_only() {
    let root = PathBuf::from("/repo");
    let metadata = serde_json::json!({
        "packages": [
            {
                "name": "product-crate",
                "source": null,
                "manifest_path": "/repo/crates/product/Cargo.toml",
                "targets": [
                    {"name": "product_crate", "kind": ["lib"], "src_path": "/repo/crates/product/src/lib.rs"},
                    {"name": "browser", "kind": ["cdylib", "rlib"], "src_path": "/repo/crates/product/src/browser.rs"},
                    {"name": "builder", "kind": ["custom-build"], "src_path": "/repo/crates/product/custom_build.rs"},
                    {"name": "demo", "kind": ["example"], "src_path": "/repo/crates/product/examples/demo.rs"},
                    {"name": "integration", "kind": ["test"], "src_path": "/repo/crates/product/tests/integration.rs"}
                ]
            },
            {
                "name": "patched-product",
                "source": null,
                "manifest_path": "/repo/third_party/patched/Cargo.toml",
                "targets": [
                    {"name": "patched_product", "kind": ["lib"], "src_path": "/repo/third_party/patched/lib/custom.rs"}
                ]
            },
            {
                "name": "registry-dependency",
                "source": "registry+https://example.invalid/index",
                "manifest_path": "/cargo/registry/dependency/Cargo.toml",
                "targets": [
                    {"name": "dependency", "kind": ["lib"], "src_path": "/cargo/registry/dependency/src/lib.rs"}
                ]
            },
            {
                "name": "xtask",
                "source": null,
                "manifest_path": "/repo/xtask/Cargo.toml",
                "targets": [
                    {"name": "xtask", "kind": ["bin"], "src_path": "/repo/xtask/src/main.rs"}
                ]
            },
            {
                "name": "verification-cases",
                "source": null,
                "manifest_path": "/repo/crates/verification-cases/Cargo.toml",
                "targets": [
                    {"name": "verification-cases", "kind": ["lib"], "src_path": "/repo/crates/verification-cases/src/lib.rs"}
                ]
            }
        ]
    });

    let universe = discover_cargo_targets(&root, &metadata);
    assert!(
        universe.diagnostics.is_empty(),
        "{:?}",
        universe.diagnostics
    );
    let paths = universe
        .targets
        .iter()
        .map(|target| target.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        vec![
            "crates/product/custom_build.rs",
            "crates/product/examples/demo.rs",
            "crates/product/src/browser.rs",
            "crates/product/src/lib.rs",
            "third_party/patched/lib/custom.rs"
        ],
        paths
    );
    assert!(universe.targets.iter().any(|target| {
        target.target_kind == "custom-build" && target.module == "product_crate::build_script"
    }));
    assert!(!paths.contains(&"crates/product/tests/integration.rs"));
    assert!(!paths.contains(&"xtask/src/main.rs"));
    assert!(!paths.contains(&"crates/verification-cases/src/lib.rs"));
}

#[test]
fn rust_universe_joins_cargo_targets_module_graphs_and_extracted_facts() {
    let root = fixture_root("rust-universe");
    let package_root = root.join("crates/fixture");
    fs::create_dir_all(package_root.join("src")).unwrap();
    fs::write(
        package_root.join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.0.0'\n",
    )
    .unwrap();
    fs::write(
        package_root.join("src/lib.rs"),
        "mod nested;\npub fn root_fn() {}\n",
    )
    .unwrap();
    fs::write(
        package_root.join("src/nested.rs"),
        "pub fn nested_fn() {}\n",
    )
    .unwrap();
    let metadata = serde_json::json!({
        "packages": [{
            "name": "fixture",
            "source": null,
            "manifest_path": package_root.join("Cargo.toml"),
            "targets": [{
                "name": "fixture",
                "kind": ["lib"],
                "src_path": package_root.join("src/lib.rs")
            }]
        }]
    });

    let scan = scan_rust_universe(&root, &metadata);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    assert_eq!(1, scan.cargo_targets.len());
    let facts = scan
        .sources
        .iter()
        .flat_map(|source| source.facts.iter())
        .map(|fact| fact.qualified_symbol.as_str())
        .collect::<Vec<_>>();
    assert!(facts.contains(&"fixture::root_fn"));
    assert!(facts.contains(&"fixture::nested::nested_fn"));
    assert_eq!(2, scan.sources.len());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rust_universe_reports_unrepresented_product_manifests_sources_and_build_scripts() {
    let root = fixture_root("rust-census");
    let represented = root.join("crates/represented");
    fs::create_dir_all(represented.join("src")).unwrap();
    fs::write(
        represented.join("Cargo.toml"),
        "[package]\nname='represented'\nversion='0.0.0'\n",
    )
    .unwrap();
    fs::write(represented.join("src/lib.rs"), "pub fn live() {}\n").unwrap();
    fs::write(represented.join("src/orphan.rs"), "pub fn orphan() {}\n").unwrap();
    fs::write(represented.join("build.rs"), "fn main() {}\n").unwrap();
    let missing = root.join("crates/missing");
    fs::create_dir_all(missing.join("src")).unwrap();
    fs::write(
        missing.join("Cargo.toml"),
        "[package]\nname='missing'\nversion='0.0.0'\n",
    )
    .unwrap();
    fs::write(missing.join("src/lib.rs"), "pub fn absent() {}\n").unwrap();
    let metadata = serde_json::json!({
        "packages": [{
            "name": "represented",
            "source": null,
            "manifest_path": represented.join("Cargo.toml"),
            "targets": [{
                "name": "represented",
                "kind": ["lib"],
                "src_path": represented.join("src/lib.rs")
            }]
        }]
    });

    let scan = scan_rust_universe(&root, &metadata);
    assert!(scan
        .diagnostics
        .iter()
        .any(|item| item.contains("unrepresented product manifest crates/missing/Cargo.toml")));
    assert!(scan
        .diagnostics
        .iter()
        .any(|item| item
            .contains("unrepresented product Rust source crates/represented/src/orphan.rs")));
    assert!(
        scan.diagnostics
            .iter()
            .any(|item| item
                .contains("unrepresented product Rust source crates/represented/build.rs"))
    );
    fs::remove_dir_all(root).unwrap();
}

pub(super) fn fixture_root(label: &str) -> PathBuf {
    let serial = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-code-spec-{label}-{}-{serial}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(root.join("src")).unwrap();
    root
}

#[test]
fn resolves_nested_path_and_literal_include_sources_but_excludes_cfg_test_modules() {
    let root = fixture_root("module-graph");
    fs::write(
        root.join("src/lib.rs"),
        r#"
mod nested;
#[path = "custom.rs"] mod placed;
include!("included.rs");
#[cfg(test)] mod tests;
"#,
    )
    .unwrap();
    fs::write(root.join("src/nested.rs"), "fn nested() {}\n").unwrap();
    fs::write(root.join("src/custom.rs"), "mod sibling;\nfn placed() {}\n").unwrap();
    fs::write(root.join("src/sibling.rs"), "fn sibling() {}\n").unwrap();
    fs::write(root.join("src/included.rs"), "fn included() {}\n").unwrap();
    fs::write(root.join("src/tests.rs"), "fn test_helper() {}\n").unwrap();

    let graph = scan_module_graph(&root, &root.join("src/lib.rs"), "fixture").unwrap();
    assert!(graph.diagnostics.is_empty(), "{:?}", graph.diagnostics);
    let sources = graph
        .sources
        .iter()
        .map(|source| (source.path.as_str(), source.module.as_str()))
        .collect::<Vec<_>>();
    assert!(sources.contains(&("src/lib.rs", "fixture")));
    assert!(sources.contains(&("src/nested.rs", "fixture::nested")));
    assert!(sources.contains(&("src/custom.rs", "fixture::placed")));
    assert!(sources.contains(&("src/sibling.rs", "fixture::placed::sibling")));
    assert!(sources.contains(&("src/included.rs", "fixture")));
    assert!(!sources.iter().any(|(path, _)| *path == "src/tests.rs"));
    assert_eq!(vec!["src/tests.rs"], graph.test_only_sources);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unresolved_modules_and_dynamic_includes_are_blocking_diagnostics() {
    let root = fixture_root("module-errors");
    fs::write(
        root.join("src/lib.rs"),
        "mod missing;\ninclude!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n",
    )
    .unwrap();

    let graph = scan_module_graph(&root, &root.join("src/lib.rs"), "fixture").unwrap();
    assert!(
        graph
            .diagnostics
            .iter()
            .any(|item| item.contains("unresolved module fixture::missing")),
        "{:?}",
        graph.diagnostics
    );
    assert!(
        graph
            .diagnostics
            .iter()
            .any(|item| item.contains("unsupported include! expression")),
        "{:?}",
        graph.diagnostics
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn crate_root_scan_resolves_sibling_modules_for_integration_targets() {
    let root = fixture_root("integration-module-graph");
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(root.join("tests/contract.rs"), "mod common;\n").unwrap();
    fs::write(root.join("tests/common.rs"), "fn check_contract() {}\n").unwrap();

    let graph =
        scan_crate_root_module_graph(&root, &root.join("tests/contract.rs"), "fixture").unwrap();

    assert!(graph.diagnostics.is_empty(), "{:?}", graph.diagnostics);
    assert!(graph
        .sources
        .iter()
        .any(|source| { source.path == "tests/common.rs" && source.module == "fixture::common" }));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_topology_binds_literal_include_to_all_parent_and_root_bytes() {
    let root = fixture_root("include-topology");
    fs::create_dir_all(root.join("src/database")).unwrap();
    fs::write(root.join("src/lib.rs"), "mod database;\n").unwrap();
    fs::write(
        root.join("src/database.rs"),
        "include!(\"database/database_part_01.rs\");\n",
    )
    .unwrap();
    fs::write(
        root.join("src/database/database_part_01.rs"),
        "fn included() {}\n",
    )
    .unwrap();

    let topology = scan_source_topology(
        &root,
        &root.join("src/lib.rs"),
        "fixture",
        &root.join("src/database/database_part_01.rs"),
    )
    .unwrap();

    assert!(
        topology.diagnostics.is_empty(),
        "{:?}",
        topology.diagnostics
    );
    assert_eq!(1, topology.bindings.len());
    assert_eq!("fixture::database", topology.bindings[0].module);
    assert_eq!(
        vec![
            "src/lib.rs",
            "src/database.rs",
            "src/database/database_part_01.rs"
        ],
        topology.bindings[0].topology_paths
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_topology_authenticates_moved_include_location() {
    let root = fixture_root("moved-include-topology");
    fs::create_dir_all(root.join("src/database")).unwrap();
    fs::write(root.join("src/lib.rs"), "mod database;\n").unwrap();
    fs::write(
        root.join("src/database.rs"),
        "include!(\"database/database_part_02.rs\");\n",
    )
    .unwrap();
    fs::write(
        root.join("src/database/database_part_02.rs"),
        "fn moved() {}\n",
    )
    .unwrap();

    let topology = scan_source_topology(
        &root,
        &root.join("src/lib.rs"),
        "fixture",
        &root.join("src/database/database_part_02.rs"),
    )
    .unwrap();

    assert_eq!(1, topology.bindings.len());
    assert_eq!(
        "src/database/database_part_02.rs",
        topology.bindings[0].path
    );
    assert_eq!(
        "src/database/database_part_02.rs",
        topology.bindings[0].topology_paths[2]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_topology_rejects_removed_include_source() {
    let root = fixture_root("removed-include-topology");
    fs::write(
        root.join("src/lib.rs"),
        "include!(\"database_part_01.rs\");\n",
    )
    .unwrap();

    let error = scan_source_topology(
        &root,
        &root.join("src/lib.rs"),
        "fixture",
        &root.join("src/database_part_01.rs"),
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("canonicalize selected Rust source"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_topology_leaves_unreferenced_include_fragment_orphaned() {
    let root = fixture_root("orphan-include-topology");
    fs::write(root.join("src/lib.rs"), "fn live() {}\n").unwrap();
    fs::write(root.join("src/database_part_01.rs"), "fn orphan() {}\n").unwrap();

    let topology = scan_source_topology(
        &root,
        &root.join("src/lib.rs"),
        "fixture",
        &root.join("src/database_part_01.rs"),
    )
    .unwrap();

    assert!(topology.bindings.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn parses_closed_rust_extract_cli_arguments() {
    let arguments = [
        "--path".to_string(),
        "crates/trust-runtime/src/stdlib/time.rs".to_string(),
        "--module".to_string(),
        "trust_runtime::stdlib::time".to_string(),
    ];
    let parsed = parse_rust_extract_args(&arguments).unwrap();
    assert_eq!("crates/trust-runtime/src/stdlib/time.rs", parsed.path);
    assert_eq!("trust_runtime::stdlib::time", parsed.module);
    assert!(parsed.topology_for.is_empty());

    let topology = parse_rust_extract_args(&[
        "--path".to_string(),
        "crates/trust-hir/src/lib.rs".to_string(),
        "--module".to_string(),
        "trust_hir".to_string(),
        "--topology-for".to_string(),
        "crates/trust-hir/src/db/queries/database/database_part_02.rs".to_string(),
        "--topology-for".to_string(),
        "crates/trust-hir/src/db/queries/database/database_part_01.rs".to_string(),
    ])
    .unwrap();
    assert_eq!(
        vec![
            "crates/trust-hir/src/db/queries/database/database_part_01.rs",
            "crates/trust-hir/src/db/queries/database/database_part_02.rs",
        ],
        topology.topology_for
    );

    let error = parse_rust_extract_args(&["--unknown".to_string()]).unwrap_err();
    assert!(error
        .to_string()
        .contains("unknown code-spec-rust argument"));
}

#[test]
fn extracts_functions_methods_types_constants_and_registration_calls() {
    let source = r#"
pub struct Clock;
pub const TICKS: u64 = 1;
pub fn runtime_clock_value(name: &str) -> u64 { name.len() as u64 }
fn current_dt() -> u64 { 0 }
impl Clock {
    pub fn now(&self) -> u64 { current_dt() }
}
pub fn register(lib: &mut Lib) {
    lib.register("CURRENT_DT", current_dt);
}
"#;

    let scan = extract_rust_source("stdlib::time", source).unwrap();
    let symbols = scan
        .facts
        .iter()
        .map(|fact| (fact.qualified_symbol.as_str(), fact.item_kind.as_str()))
        .collect::<Vec<_>>();

    assert!(symbols.contains(&("stdlib::time::runtime_clock_value", "rust_function")));
    assert!(symbols.contains(&("stdlib::time::current_dt", "rust_function")));
    assert!(symbols.contains(&("stdlib::time::Clock::now", "rust_method")));
    assert!(symbols.contains(&("stdlib::time::Clock", "rust_public_type")));
    assert!(symbols.contains(&("stdlib::time::TICKS", "rust_public_constant")));
    assert!(symbols
        .iter()
        .any(|(symbol, kind)| { *kind == "rust_registration" && symbol.contains("CURRENT_DT") }));
}

#[test]
fn extracts_top_level_cargo_fuzz_target_macro_as_a_behavior_fact() {
    let source = r#"
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = trust_syntax::parser::parse_bytes(data);
});
"#;

    let scan = extract_rust_source("fuzz::syntax_parse", source).unwrap();
    let fact = scan
        .facts
        .iter()
        .find(|fact| fact.item_kind == "rust_fuzz_target")
        .expect("top-level fuzz_target! must be part of the tooling denominator");

    assert_eq!("fuzz::syntax_parse::fuzz_target[1]", fact.qualified_symbol);
    assert_eq!(
        "fuzz_target!(|data: &[u8]| {\n    let _ = trust_syntax::parser::parse_bytes(data);\n});",
        &source[fact.body_start..fact.body_end]
    );
    assert_eq!("fuzz_target!(closure)", fact.signature);
}

#[test]
fn reports_exact_item_byte_ranges_and_ignores_cfg_test_items() {
    let source = r#"
fn live() { work(); }
#[cfg(test)]
fn helper() { panic!(); }
#[cfg(not(test))]
fn production_only() {}
#[cfg(any(test, feature = "diagnostics"))]
fn feature_or_test() {}
"#;
    let scan = extract_rust_source("example", source).unwrap();
    assert_eq!(3, scan.facts.len());
    let fact = scan
        .facts
        .iter()
        .find(|fact| fact.qualified_symbol == "example::live")
        .unwrap();
    assert_eq!(
        "fn live() { work(); }",
        &source[fact.body_start..fact.body_end]
    );
    assert_eq!(2, fact.line);
    assert!(scan
        .facts
        .iter()
        .any(|fact| fact.qualified_symbol == "example::production_only"));
    assert!(scan
        .facts
        .iter()
        .any(|fact| fact.qualified_symbol == "example::feature_or_test"));
}

#[test]
fn inline_module_impl_methods_have_distinct_qualified_owners() {
    let source = r#"
mod first {
    struct Clock;
    impl Clock { fn now(&self) {} }
}
mod second {
    struct Clock;
    impl Clock { fn now(&self) {} }
}
"#;
    let scan = extract_rust_source("example", source).unwrap();
    let methods = scan
        .facts
        .iter()
        .filter(|fact| fact.item_kind == "rust_method")
        .map(|fact| fact.qualified_symbol.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        vec!["example::first::Clock::now", "example::second::Clock::now"],
        methods
    );
}

#[test]
fn mutually_exclusive_cfg_variants_have_distinct_normalized_signatures() {
    let source = r#"
#[cfg(windows)]
fn platform_path() { windows(); }
#[cfg(not(windows))]
fn platform_path() { unix(); }
#[cfg(unix)]
mod imp {
    struct Driver;
    impl Driver { fn connect(&self) {} }
}
#[cfg(not(unix))]
mod imp {
    struct Driver;
    impl Driver { fn connect(&self) {} }
}
"#;
    let scan = extract_rust_source("example", source).unwrap();
    let variants = scan
        .facts
        .iter()
        .filter(|fact| fact.qualified_symbol == "example::platform_path")
        .collect::<Vec<_>>();
    assert_eq!(2, variants.len());
    assert_ne!(variants[0].signature, variants[1].signature);
    assert!(variants
        .iter()
        .any(|fact| fact.signature.contains("cfg(windows)")));
    assert!(variants
        .iter()
        .any(|fact| fact.signature.contains("cfg(not(windows))")));
    let methods = scan
        .facts
        .iter()
        .filter(|fact| fact.qualified_symbol.ends_with("Driver::connect"))
        .collect::<Vec<_>>();
    assert_eq!(2, methods.len());
    assert_ne!(methods[0].signature, methods[1].signature);
    assert!(methods
        .iter()
        .any(|fact| fact.signature.contains("cfg(unix)")));
    assert!(methods
        .iter()
        .any(|fact| fact.signature.contains("cfg(not(unix))")));
}

#[test]
fn rejects_unparseable_rust_instead_of_returning_an_empty_pass() {
    let error = extract_rust_source("broken", "fn broken( {").unwrap_err();
    assert!(error.to_string().contains("parse Rust source"));
}
