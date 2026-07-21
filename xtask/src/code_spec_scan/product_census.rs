use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::cargo_universe::is_product_manifest;

pub fn cross_check_product_census(
    root: &Path,
    metadata: &serde_json::Value,
    represented_sources: &BTreeSet<String>,
    test_only_sources: &BTreeSet<String>,
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    let represented_manifests = metadata_manifests(root, metadata);
    let mut files = Vec::new();
    for relative_root in ["crates", "third_party"] {
        collect_files(
            root,
            &root.join(relative_root),
            &mut files,
            &mut diagnostics,
        );
    }
    files.sort();
    files.dedup();
    for path in files {
        let Ok(relative) = path.strip_prefix(root) else {
            diagnostics.push(format!(
                "product census path escapes repository: {}",
                path.display()
            ));
            continue;
        };
        if !is_product_file(relative) {
            continue;
        }
        let normalized = relative.to_string_lossy().replace('\\', "/");
        if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            if !represented_manifests.contains(&normalized) {
                diagnostics.push(format!("unrepresented product manifest {normalized}"));
            }
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && !represented_sources.contains(&normalized)
            && !test_only_sources.contains(&normalized)
        {
            diagnostics.push(format!("unrepresented product Rust source {normalized}"));
        }
    }
    diagnostics.sort();
    diagnostics.dedup();
    diagnostics
}

fn metadata_manifests(root: &Path, metadata: &serde_json::Value) -> BTreeSet<String> {
    metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|package| package.get("source").is_none_or(serde_json::Value::is_null))
        .filter_map(|package| {
            package
                .get("manifest_path")
                .and_then(serde_json::Value::as_str)
        })
        .filter_map(|manifest| Path::new(manifest).strip_prefix(root).ok())
        .filter(|manifest| is_product_manifest(manifest))
        .map(|manifest| manifest.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
    diagnostics: &mut Vec<String>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            diagnostics.push(format!(
                "product census cannot read {}: {error}",
                directory.display()
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                diagnostics.push(format!("product census directory entry failed: {error}"));
                continue;
            }
        };
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if !is_product_file(relative) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                diagnostics.push(format!(
                    "product census cannot stat {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        if file_type.is_symlink() {
            diagnostics.push(format!(
                "product census rejects symlink {}",
                relative.display()
            ));
        } else if file_type.is_dir() {
            collect_files(root, &path, files, diagnostics);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
}

fn is_product_file(relative: &Path) -> bool {
    let normalized = relative.to_string_lossy().replace('\\', "/");
    if normalized == "crates/verification-cases"
        || normalized.starts_with("crates/verification-cases/")
    {
        return false;
    }
    !relative.components().any(|component| {
        component.as_os_str() == "target"
            || component.as_os_str() == "tests"
            || component.as_os_str() == "benches"
            || component.as_os_str() == "fuzz"
    })
}
