use std::path::{Component, Path};

use super::model::{CargoTarget, CargoUniverse};

const LIBRARY_TARGET_KINDS: [&str; 6] =
    ["lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"];

pub fn discover_cargo_targets(root: &Path, metadata: &serde_json::Value) -> CargoUniverse {
    let mut universe = CargoUniverse {
        targets: Vec::new(),
        diagnostics: Vec::new(),
    };
    let Some(packages) = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
    else {
        universe
            .diagnostics
            .push("cargo metadata did not contain packages".to_string());
        return universe;
    };
    for package in packages {
        if !package.get("source").is_none_or(serde_json::Value::is_null) {
            continue;
        }
        let Some(package_name) = package.get("name").and_then(serde_json::Value::as_str) else {
            universe
                .diagnostics
                .push("cargo metadata package has no name".to_string());
            continue;
        };
        let Some(manifest_path) = package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
        else {
            universe
                .diagnostics
                .push(format!("cargo package {package_name} has no manifest_path"));
            continue;
        };
        let Ok(relative_manifest) = Path::new(manifest_path).strip_prefix(root) else {
            continue;
        };
        if !is_product_manifest(relative_manifest) {
            continue;
        }
        let Some(targets) = package.get("targets").and_then(serde_json::Value::as_array) else {
            universe
                .diagnostics
                .push(format!("cargo package {package_name} has no targets"));
            continue;
        };
        for target in targets {
            let kinds = target
                .get("kind")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>();
            let Some(target_kind) = product_target_kind(&kinds) else {
                continue;
            };
            let Some(target_name) = target.get("name").and_then(serde_json::Value::as_str) else {
                universe.diagnostics.push(format!(
                    "cargo package {package_name} has a target without a name"
                ));
                continue;
            };
            let Some(source_path) = target.get("src_path").and_then(serde_json::Value::as_str)
            else {
                universe.diagnostics.push(format!(
                    "cargo target {package_name}/{target_name} has no src_path"
                ));
                continue;
            };
            let Ok(relative) = Path::new(source_path).strip_prefix(root) else {
                universe.diagnostics.push(format!(
                    "local cargo target {package_name}/{target_name} escapes repository root"
                ));
                continue;
            };
            let package_module = package_name.replace('-', "_");
            let target_module = target_name.replace('-', "_");
            let module = match target_kind {
                "custom-build" => format!("{package_module}::build_script"),
                "example" => format!("{package_module}::example::{target_module}"),
                _ => target_module,
            };
            universe.targets.push(CargoTarget {
                package: package_name.to_string(),
                target: target_name.to_string(),
                target_kind: target_kind.to_string(),
                path: relative.to_string_lossy().replace('\\', "/"),
                module,
            });
        }
    }
    universe.targets.sort_by(|left, right| {
        (&left.path, &left.package, &left.target).cmp(&(&right.path, &right.package, &right.target))
    });
    universe.diagnostics.sort();
    universe.diagnostics.dedup();
    universe
}

fn product_target_kind(kinds: &[&str]) -> Option<&'static str> {
    if kinds.contains(&"custom-build") {
        Some("custom-build")
    } else if kinds.iter().any(|kind| LIBRARY_TARGET_KINDS.contains(kind)) {
        Some("lib")
    } else if kinds.contains(&"bin") {
        Some("bin")
    } else if kinds.contains(&"example") {
        Some("example")
    } else {
        None
    }
}

pub(super) fn is_product_manifest(relative_manifest: &Path) -> bool {
    let components = relative_manifest.components().collect::<Vec<_>>();
    match components.as_slice() {
        [Component::Normal(root), Component::Normal(crate_name), ..]
            if *root == "crates" && *crate_name != "verification-cases" =>
        {
            true
        }
        [Component::Normal(root), ..] if *root == "third_party" => true,
        _ => false,
    }
}
