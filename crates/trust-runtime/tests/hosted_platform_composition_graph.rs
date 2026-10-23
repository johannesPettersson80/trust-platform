//! Spec 34 §4.3 HOST-07 (line 357): the documented hosted compositions are
//! selectable, runtime-only keeps compiler, HIR, IDE and service crates out of its
//! build graph, and full and development include them.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

/// Crates that only the full controller and its services may bring in.
const RUNTIME_ONLY_ABSENT: [&str; 16] = [
    "serde",
    "serde_json",
    "tiny_http",
    "rumqttc",
    "opcua",
    "tokio",
    "zenoh",
    "rusqlite",
    "trust-hir",
    "trust-syntax",
    "trust-ide",
    "trust-plcopen",
    "trust-ads-core",
    "trust-tcads-native",
    "trust-wasm-analysis",
    "trust-ads-server",
];

/// Crates that the full and development compositions must include.
const FULL_PRESENT: [&str; 4] = ["trust-hir", "trust-syntax", "trust-ide", "serde"];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repository root")
        .to_path_buf()
}

/// Package names in the normal and build dependency graph of `trust-runtime` for one feature set.
fn resolved_packages(features: &str) -> BTreeSet<String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(repository_root())
        .args([
            "tree",
            "--locked",
            "-p",
            "trust-runtime",
            "--no-default-features",
            "--features",
            features,
            "--edges",
            "no-dev",
            "--prefix",
            "none",
        ])
        .output()
        .unwrap_or_else(|err| panic!("run cargo tree for `{features}`: {err}"));
    assert!(
        output.status.success(),
        "cargo tree must select the `{features}` composition: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

/// Spec 34 §4.3 HOST-07 (line 357): runtime-only omits compiler, HIR, IDE and host services from its build graph.
#[test]
fn runtime_only_build_graph_omits_compiler_and_services() {
    let graph = resolved_packages("runtime-only");
    assert!(
        graph.contains("trust-runtime-core"),
        "runtime-only must build the portable core"
    );
    for crate_name in RUNTIME_ONLY_ABSENT {
        assert!(
            !graph.contains(crate_name),
            "runtime-only build graph must not contain `{crate_name}` (spec 34 §4.3 HOST-07)"
        );
    }
}

/// Spec 34 §4.3 HOST-07 (line 357): the register-IR runtime-only consumer keeps the same omissions.
#[test]
fn runtime_only_with_register_ir_build_graph_omits_compiler_and_services() {
    let graph = resolved_packages("runtime-only,register-ir");
    for crate_name in RUNTIME_ONLY_ABSENT {
        assert!(
            !graph.contains(crate_name),
            "runtime-only + register-ir build graph must not contain `{crate_name}` (spec 34 §4.3 HOST-07)"
        );
    }
}

/// Spec 34 §4.3 HOST-07 (line 357): the full controller composition is selectable and includes compiler, HIR, IDE and services.
#[test]
fn full_composition_build_graph_includes_compiler_and_services() {
    let graph = resolved_packages("full");
    for crate_name in FULL_PRESENT {
        assert!(
            graph.contains(crate_name),
            "full composition build graph must include `{crate_name}` (spec 34 §4.3 HOST-07)"
        );
    }
}

/// Spec 34 §4.3 HOST-07 (line 357): the development composition is selectable and includes the full set.
#[test]
fn development_composition_build_graph_includes_compiler_and_services() {
    let graph = resolved_packages("development");
    for crate_name in FULL_PRESENT {
        assert!(
            graph.contains(crate_name),
            "development composition build graph must include `{crate_name}` (spec 34 §4.3 HOST-07)"
        );
    }
}
