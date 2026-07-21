use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::model::{CargoTarget, RustScannedSource, ToolingRustScan};
use super::{extract_rust_source, scan_module_graph};

pub fn scan_tooling_rust_universe(root: &Path) -> ToolingRustScan {
    let mut diagnostics = Vec::new();
    let mut sources = BTreeMap::new();
    let mut cargo_targets = Vec::new();
    let mut represented_xtask = BTreeSet::new();
    let mut test_only_xtask = BTreeSet::new();
    let xtask_entry = root.join("xtask/src/main.rs");
    if !xtask_entry.is_file() {
        diagnostics.push("missing Rust tooling entrypoint xtask/src/main.rs".to_string());
    } else {
        cargo_targets.push(CargoTarget {
            package: "tooling".to_string(),
            target: "xtask".to_string(),
            target_kind: "tooling".to_string(),
            path: "xtask/src/main.rs".to_string(),
            module: "xtask".to_string(),
        });
        match scan_module_graph(root, &xtask_entry, "xtask") {
            Ok(graph) => {
                diagnostics.extend(
                    graph
                        .diagnostics
                        .into_iter()
                        .map(|item| format!("xtask module graph: {item}")),
                );
                test_only_xtask.extend(graph.test_only_sources);
                for source in graph.sources {
                    represented_xtask.insert(source.path.clone());
                    add_source(
                        root,
                        &mut sources,
                        &mut diagnostics,
                        &source.path,
                        &source.module,
                        "tooling:xtask:tooling",
                    );
                }
            }
            Err(error) => diagnostics.push(format!("xtask module graph failed: {error}")),
        }
    }

    for path in rust_files_under(&root.join("xtask/src")) {
        let relative = relative_path(root, &path);
        if !represented_xtask.contains(&relative) && !test_only_xtask.contains(&relative) {
            diagnostics.push(format!("unrepresented xtask Rust source {relative}"));
        }
    }

    let fuzz_targets = fuzz_target_files(root);
    if fuzz_targets.is_empty() {
        diagnostics.push("Rust tooling fuzz target denominator is empty".to_string());
    }
    for path in fuzz_targets {
        let relative = relative_path(root, &path);
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("target")
            .replace('-', "_");
        cargo_targets.push(CargoTarget {
            package: "tooling".to_string(),
            target: stem.clone(),
            target_kind: "fuzz".to_string(),
            path: relative.clone(),
            module: format!("fuzz::{stem}"),
        });
        add_source(
            root,
            &mut sources,
            &mut diagnostics,
            &relative,
            &format!("fuzz::{stem}"),
            &format!("tooling:{stem}:fuzz"),
        );
    }

    if sources.is_empty() {
        diagnostics.push("Rust tooling source denominator is empty".to_string());
    }
    diagnostics.sort();
    diagnostics.dedup();
    ToolingRustScan {
        schema_version: 1,
        cargo_targets,
        sources: sources.into_values().collect(),
        diagnostics,
    }
}

fn add_source(
    root: &Path,
    sources: &mut BTreeMap<(String, String), RustScannedSource>,
    diagnostics: &mut Vec<String>,
    path: &str,
    module: &str,
    binding: &str,
) {
    let source = match fs::read_to_string(root.join(path)) {
        Ok(source) => source,
        Err(error) => {
            diagnostics.push(format!("unreadable Rust tooling source {path}: {error}"));
            return;
        }
    };
    let extracted = match extract_rust_source(module, &source) {
        Ok(extracted) => extracted,
        Err(error) => {
            diagnostics.push(format!("unsupported Rust tooling syntax in {path}: {error}"));
            return;
        }
    };
    diagnostics.extend(
        extracted
            .diagnostics
            .into_iter()
            .map(|item| format!("{path}: {item}")),
    );
    sources.insert(
        (path.to_string(), module.to_string()),
        RustScannedSource {
            path: path.to_string(),
            module: module.to_string(),
            cargo_bindings: vec![binding.to_string()],
            facts: extracted.facts,
        },
    );
}

fn fuzz_target_files(root: &Path) -> Vec<PathBuf> {
    let mut directories = vec![root.join("fuzz/fuzz_targets")];
    if let Ok(entries) = fs::read_dir(root.join("crates")) {
        for entry in entries.flatten() {
            directories.push(entry.path().join("fuzz/fuzz_targets"));
        }
    }
    let mut files = directories
        .into_iter()
        .flat_map(|directory| rust_files_under(&directory))
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    files
}

fn rust_files_under(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "rs"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
