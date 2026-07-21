use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::cargo_universe::discover_cargo_targets;
use super::model::{RustScannedSource, RustUniverseScan};
use super::product_census::cross_check_product_census;
use super::{extract_rust_source, scan_module_graph};

pub fn scan_rust_universe(root: &Path, metadata: &serde_json::Value) -> RustUniverseScan {
    let cargo = discover_cargo_targets(root, metadata);
    let mut diagnostics = cargo.diagnostics;
    let mut sources: BTreeMap<(String, String), RustScannedSource> = BTreeMap::new();
    let mut test_only_sources = std::collections::BTreeSet::new();

    for target in &cargo.targets {
        let binding = format!(
            "{}:{}:{}",
            target.package, target.target, target.target_kind
        );
        let graph = match scan_module_graph(root, &root.join(&target.path), &target.module) {
            Ok(graph) => graph,
            Err(error) => {
                diagnostics.push(format!(
                    "cargo target {binding} module scan failed: {error}"
                ));
                continue;
            }
        };
        diagnostics.extend(
            graph
                .diagnostics
                .into_iter()
                .map(|diagnostic| format!("cargo target {binding}: {diagnostic}")),
        );
        test_only_sources.extend(graph.test_only_sources);
        for source_unit in graph.sources {
            let key = (source_unit.path.clone(), source_unit.module.clone());
            if let Some(existing) = sources.get_mut(&key) {
                if !existing.cargo_bindings.contains(&binding) {
                    existing.cargo_bindings.push(binding.clone());
                    existing.cargo_bindings.sort();
                }
                continue;
            }
            let source = match fs::read_to_string(root.join(&source_unit.path)) {
                Ok(source) => source,
                Err(error) => {
                    diagnostics.push(format!(
                        "cargo target {binding}: unreadable Rust source {}: {error}",
                        source_unit.path
                    ));
                    continue;
                }
            };
            let extracted = match extract_rust_source(&source_unit.module, &source) {
                Ok(extracted) => extracted,
                Err(error) => {
                    diagnostics.push(format!(
                        "cargo target {binding}: unsupported Rust syntax in {}: {error}",
                        source_unit.path
                    ));
                    continue;
                }
            };
            diagnostics.extend(
                extracted
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| format!("{}: {diagnostic}", source_unit.path)),
            );
            sources.insert(
                key,
                RustScannedSource {
                    path: source_unit.path,
                    module: source_unit.module,
                    cargo_bindings: vec![binding.clone()],
                    facts: extracted.facts,
                },
            );
        }
    }

    if cargo.targets.is_empty() {
        diagnostics.push("cargo product target denominator is empty".to_string());
    }
    if sources.is_empty() {
        diagnostics.push("Rust product source denominator is empty".to_string());
    }
    if !sources.is_empty() && sources.values().all(|source| source.facts.is_empty()) {
        diagnostics.push("Rust parser returned zero facts for every product source".to_string());
    }
    let represented_sources = sources.keys().map(|(path, _module)| path.clone()).collect();
    diagnostics.extend(cross_check_product_census(
        root,
        metadata,
        &represented_sources,
        &test_only_sources,
    ));
    diagnostics.sort();
    diagnostics.dedup();

    RustUniverseScan {
        schema_version: 1,
        cargo_targets: cargo.targets,
        sources: sources.into_values().collect(),
        diagnostics,
    }
}
