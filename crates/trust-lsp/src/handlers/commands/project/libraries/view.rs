//! The Libraries panel's entries (specification 25 §7): one per `[dependencies]` entry of the
//! project's `trust-lsp.toml`, read by the language server's rules.
//!
//! - A curated library (Plant, OSCAT, PLCopen Motion) is `bundled` and lives where its entry points;
//!   another library lives where the language server resolved it (a Git library's checkout), else
//!   where its entry points.
//! - Its status is `resolved` when that folder exists, `resolving` for a Git library whose folder
//!   does not exist yet, else `failed`.
//! - Its version comes from the vendored copy's `[package] version` first for a curated library
//!   (spec 25 §7: the vendored copy wins over stale project information), and from the language
//!   server's resolution first for another.
//! - A curated library whose bundled copy has another version offers an update to it.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};

use super::catalog;
use super::manifest::{package_version, MANIFEST_FILE};
use super::symbols::{library_sources, library_symbols, locale_order};
use crate::config::{manifest_dependencies, ProjectDependency};

/// A library as the language server resolved it (`trust-lsp.projectInfo`).
#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct ResolvedLibrary {
    #[serde(default)]
    pub(super) name: String,
    #[serde(default)]
    pub(super) version: Option<String>,
    #[serde(default)]
    pub(super) path: Option<PathBuf>,
}

/// The panel's entries for the project in `root`, ordered by label; `bundled` is the extension's
/// `media/libraries` folder.
pub(super) fn entries(
    root: &Path,
    bundled: &Path,
    resolved: &[ResolvedLibrary],
) -> Result<Vec<Value>, String> {
    let text = std::fs::read_to_string(root.join(MANIFEST_FILE))
        .map_err(|error| format!("The project's trust-lsp.toml cannot be read: {error}"))?;
    let (dependencies, _) = manifest_dependencies(root, &text)
        .ok_or_else(|| "The project's trust-lsp.toml cannot be read.".to_owned())?;
    let mut entries: Vec<(String, Value)> = dependencies
        .iter()
        .map(|dependency| {
            let info = resolved
                .iter()
                .find(|library| library.name == dependency.name);
            entry(dependency, bundled, info)
        })
        .collect();
    entries.sort_by(|(left, _), (right, _)| locale_order(left, right));
    Ok(entries.into_iter().map(|(_, entry)| entry).collect())
}

/// One entry, with its label for the order.
fn entry(
    dependency: &ProjectDependency,
    bundled: &Path,
    info: Option<&ResolvedLibrary>,
) -> (String, Value) {
    let curated = catalog::by_dependency_name(&dependency.name);
    let label = curated.map_or_else(
        || dependency.name.clone(),
        |library| library.label.to_owned(),
    );
    let is_git = dependency.git.is_some();
    let source = match (curated, is_git) {
        (Some(_), _) => "bundled",
        (None, true) => "git",
        (None, false) => "local",
    };
    let manifest_path = dependency.path.clone();
    let path = if curated.is_some() {
        manifest_path
    } else {
        info.and_then(|library| library.path.clone())
            .or(manifest_path)
    };
    let exists = path.as_deref().is_some_and(Path::exists);
    let status = if exists {
        "resolved"
    } else if is_git {
        "resolving"
    } else {
        "failed"
    };
    let vendored_version = path.as_deref().and_then(package_version);
    let resolved_version = info.and_then(|library| library.version.clone());
    let version = if curated.is_some() {
        vendored_version
            .or_else(|| dependency.version.clone())
            .or(resolved_version)
    } else {
        resolved_version
            .or(vendored_version)
            .or_else(|| dependency.version.clone())
    };
    let files = match (exists, path.as_deref()) {
        (true, Some(path)) => library_sources(path),
        _ => Vec::new(),
    };
    let symbols = library_symbols(&files);
    let update = curated.and_then(|library| {
        let next = package_version(&bundled.join(library.folder))?;
        let current = version.clone()?;
        (current != next)
            .then(|| json!({ "current": current, "next": next, "curatedId": library.id }))
    });
    let detail = match status {
        "failed" => "Path is missing. Fix or remove this library.".to_owned(),
        "resolving" => "Waiting for the dependency resolver to pin this Git library.".to_owned(),
        _ => format!(
            "{} {} available",
            symbols.len(),
            if symbols.len() == 1 {
                "symbol"
            } else {
                "symbols"
            }
        ),
    };
    let value = json!({
        "name": dependency.name,
        "label": label,
        "version": version,
        "source": source,
        "path": path.map(|path| path.display().to_string()),
        "firstSource": files.first().map(|file| file.display().to_string()),
        "status": status,
        "detail": detail,
        "symbols": symbols,
        "updateAvailable": update,
        "canFixPath": status == "failed" && dependency.path.is_some(),
    });
    (label, value)
}

/// The curated libraries for the panel's cards.
pub(super) fn curated_cards() -> Value {
    Value::Array(
        catalog::CURATED
            .iter()
            .map(|library| {
                json!({ "id": library.id, "label": library.label, "purpose": library.purpose })
            })
            .collect(),
    )
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
