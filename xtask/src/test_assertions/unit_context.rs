use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::code_spec_scan::scan_crate_root_module_graph_including_tests;

const PROBE_ROOT_MODULE: &str = "__trust_assertion_probe__";

pub(super) fn target_root(repository_root: &Path, selected_path: &Path) -> Result<PathBuf> {
    if selected_path.is_absolute()
        || selected_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        bail!("Rust unit-test path must stay repository-relative")
    }
    let absolute_selected = repository_root.join(selected_path);
    if !absolute_selected.is_file() {
        bail!(
            "Rust unit-test source does not exist: {}",
            absolute_selected.display()
        )
    }
    let package_root = package_root(repository_root, &absolute_selected)?;
    let relative = absolute_selected
        .strip_prefix(&package_root)
        .with_context(|| {
            format!(
                "Rust unit-test source escaped package root: {}",
                absolute_selected.display()
            )
        })?;

    if relative == Path::new("build.rs") {
        return Ok(absolute_selected);
    }
    if let Some(root) = conventional_standalone_target(&package_root, relative)? {
        return Ok(root);
    }

    let manifest_path = package_root.join("Cargo.toml");
    let manifest_source = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read Cargo manifest {}", manifest_path.display()))?;
    let manifest: toml::Value = toml::from_str(&manifest_source)
        .with_context(|| format!("parse Cargo manifest {}", manifest_path.display()))?;
    let mut candidates = Vec::new();
    if let Some(path) = manifest
        .get("lib")
        .and_then(toml::Value::as_table)
        .and_then(|library| library.get("path"))
        .and_then(toml::Value::as_str)
    {
        let root = package_root.join(path);
        if root.is_file() {
            candidates.push(root);
        }
    }
    for default in ["src/lib.rs", "src/main.rs"] {
        let root = package_root.join(default);
        if root.is_file() && !candidates.contains(&root) {
            candidates.push(root);
        }
    }
    for path in manifest
        .get("bin")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
        .filter_map(|binary| binary.get("path"))
        .filter_map(toml::Value::as_str)
    {
        let root = package_root.join(path);
        if root.is_file() && !candidates.contains(&root) {
            candidates.push(root);
        }
    }
    if let Some(root) = unique_declaring_root(&candidates, &absolute_selected)? {
        return Ok(root);
    }
    let selected_display = selected_path.to_string_lossy().replace('\\', "/");
    let mut reachable = Vec::new();
    for candidate in candidates {
        let graph = scan_crate_root_module_graph_including_tests(
            repository_root,
            &candidate,
            PROBE_ROOT_MODULE,
        )?;
        if graph
            .sources
            .iter()
            .any(|source| source.path == selected_display)
        {
            reachable.push(candidate);
        }
    }
    match reachable.as_slice() {
        [root] => Ok(root.clone()),
        [] => bail!(
            "cannot identify Cargo target root for Rust unit-test source {}",
            absolute_selected.display()
        ),
        _ => bail!(
            "Rust unit-test source belongs to multiple Cargo target roots: {}",
            absolute_selected.display()
        ),
    }
}

fn unique_declaring_root(candidates: &[PathBuf], selected: &Path) -> Result<Option<PathBuf>> {
    let direct = candidates
        .iter()
        .filter(|candidate| candidate.as_path() == selected)
        .cloned()
        .collect::<Vec<_>>();
    if let [root] = direct.as_slice() {
        return Ok(Some(root.clone()));
    }
    let declaring = candidates
        .iter()
        .map(|candidate| {
            root_declares_selected_module(candidate, selected)
                .map(|declares| declares.map(|_| candidate.clone()))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    Ok(match declaring.as_slice() {
        [root] => Some(root.clone()),
        _ => None,
    })
}

fn root_declares_selected_module(root: &Path, selected: &Path) -> Result<Option<()>> {
    let Some(parent) = root.parent() else {
        return Ok(None);
    };
    let Ok(relative) = selected.strip_prefix(parent) else {
        return Ok(None);
    };
    let Some(first) = relative.components().find_map(|component| match component {
        Component::Normal(value) => Some(value),
        _ => None,
    }) else {
        return Ok(None);
    };
    let module = Path::new(first)
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Rust unit-test source has a non-UTF-8 module name: {}",
                selected.display()
            )
        })?;
    let source = fs::read_to_string(root)
        .with_context(|| format!("read Cargo target {}", root.display()))?;
    let file = syn::parse_file(&source)
        .with_context(|| format!("parse Cargo target {}", root.display()))?;
    Ok(file.items.iter().find_map(|item| match item {
        syn::Item::Mod(item) if item.ident == module => Some(()),
        _ => None,
    }))
}

fn package_root(repository_root: &Path, selected: &Path) -> Result<PathBuf> {
    let canonical_repository = repository_root
        .canonicalize()
        .context("canonicalize repository root for Rust unit-test context")?;
    let mut directory = selected.parent();
    while let Some(candidate) = directory {
        if !candidate.starts_with(&canonical_repository) {
            break;
        }
        if candidate.join("Cargo.toml").is_file() {
            return Ok(candidate.to_path_buf());
        }
        if candidate == canonical_repository {
            break;
        }
        directory = candidate.parent();
    }
    bail!(
        "cannot identify Cargo package for Rust unit-test source {}",
        selected.display()
    )
}

fn conventional_standalone_target(package_root: &Path, relative: &Path) -> Result<Option<PathBuf>> {
    for directory in ["src/bin", "examples", "benches"] {
        let prefix = Path::new(directory);
        let Ok(target_relative) = relative.strip_prefix(prefix) else {
            continue;
        };
        let components = target_relative
            .components()
            .filter_map(|component| match component {
                Component::Normal(value) => Some(value.to_owned()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let Some(first) = components.first() else {
            bail!(
                "Rust unit-test source does not name a target below {directory}: {}",
                relative.display()
            )
        };
        if components.len() == 1 && Path::new(first).extension().is_some() {
            return Ok(Some(package_root.join(relative)));
        }
        let stem = Path::new(first)
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Rust unit-test target has a non-UTF-8 name: {}",
                    relative.display()
                )
            })?;
        let candidates = [
            package_root.join(prefix).join(format!("{stem}.rs")),
            package_root.join(prefix).join(stem).join("main.rs"),
        ];
        let existing = candidates
            .into_iter()
            .filter(|candidate| candidate.is_file())
            .collect::<Vec<_>>();
        return match existing.as_slice() {
            [root] => Ok(Some(root.clone())),
            [] => bail!(
                "cannot identify standalone Cargo target root for {}",
                relative.display()
            ),
            _ => bail!(
                "standalone Cargo target root is ambiguous for {}",
                relative.display()
            ),
        };
    }
    Ok(None)
}
