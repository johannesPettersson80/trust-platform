//! The files of a project's Rust crate that an isolated run builds with (protocol annex E8.2;
//! specification 48): every file of the crate's folder except its outputs and vendored sources, with
//! the project's `trust.toml` and `.cargo/config.toml`, at most [`MAX_FILES`] files and [`MAX_BYTES`]
//! bytes. The run's build reads the crate's registry dependencies where the project's cargo
//! configuration puts them, its vendored sources in the project included: the copied configuration's
//! relative folders are made absolute into the project ([`write`]), and `Cargo.lock` pins those
//! dependencies by the checksums cargo checks.

use std::path::{Component, Path, PathBuf};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// The most files the crate's copy holds.
pub(super) const MAX_FILES: usize = 256;
/// The most bytes the crate's copy holds.
pub(super) const MAX_BYTES: usize = 4 * 1024 * 1024;
/// The reason of a crate larger than the copy holds.
pub(super) const TOO_LARGE: &str =
    "the project's Rust crate holds more than 256 files or 4 MiB, which an isolated run does not copy";
/// The folders of the crate's folder that are not copied: outputs, vendored sources and tools' own.
const SKIPPED: [&str; 5] = ["target", "build", "vendor", ".git", "node_modules"];
/// The project's files beside the crate that the build reads.
const PROJECT_FILES: [&str; 2] = ["trust.toml", ".cargo/config.toml"];
/// The dependency tables of a manifest whose entries may name a `path`.
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// One file of the crate's copy.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct CrateFile {
    /// The file's path relative to the project, with `/`.
    pub path: String,
    /// The file's bytes, in base64: a crate may embed files that are not text.
    pub bytes: String,
}

/// The crate files of the project at `root`, in the order of their paths; none for a project
/// without a crate.
///
/// # Errors
///
/// [`TOO_LARGE`] for a crate past the limits, the dependency whose `path` leaves the crate's
/// folder, and a file that cannot be read.
pub(super) fn read(root: &Path) -> Result<Vec<CrateFile>, String> {
    let layout =
        crate::native::rust_pous::layout(root).map_err(|error| format!("trust.toml: {error}"))?;
    let folder = root.join(&layout.crate_folder);
    if !folder.join("Cargo.toml").is_file() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    collect(&folder, &mut paths)?;
    for name in PROJECT_FILES {
        let path = root.join(name);
        if path.is_file() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    if paths.len() > MAX_FILES {
        return Err(TOO_LARGE.into());
    }
    let mut files = Vec::with_capacity(paths.len());
    let mut total = 0usize;
    for path in &paths {
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        total = total.saturating_add(bytes.len());
        if total > MAX_BYTES {
            return Err(TOO_LARGE.into());
        }
        if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            check_paths_inside(path, &bytes, &folder)?;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("{} lies outside the project", path.display()))?;
        files.push(CrateFile {
            path: slashed(relative),
            bytes: base64::engine::general_purpose::STANDARD.encode(&bytes),
        });
    }
    Ok(files)
}

/// Writes `files` below `folder`, the project of an isolated run. The copied `.cargo/config.toml`
/// gets the project's folders in place of its relative ones: a source's `directory` and
/// `local-registry`, a patch's `path` and the `paths` overrides, which cargo reads relative to the
/// project at `project`, so the run's build reads them there.
///
/// # Errors
///
/// A path that leaves `folder`, bytes that are not base64, a configuration that does not read, and
/// a file that cannot be written.
pub(super) fn write(files: &[CrateFile], folder: &Path, project: &Path) -> Result<(), String> {
    for file in files {
        let relative = Path::new(&file.path);
        if relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!("the crate file {} leaves the project", file.path));
        }
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(&file.bytes)
            .map_err(|error| format!("the crate file {}: {error}", file.path))?;
        if file.path == ".cargo/config.toml" {
            bytes = config_in_place(&bytes, project)?.into_bytes();
        }
        let target = folder.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        std::fs::write(&target, bytes).map_err(|error| format!("{}: {error}", target.display()))?;
    }
    Ok(())
}

/// Every file below `folder`, without the [`SKIPPED`] folders.
fn collect(folder: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", folder.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_dir() {
            let skipped = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| SKIPPED.contains(&name));
            if !skipped {
                collect(&path, paths)?;
            }
        } else if kind.is_file() {
            paths.push(path);
            if paths.len() > MAX_FILES {
                return Err(TOO_LARGE.into());
            }
        }
    }
    Ok(())
}

/// Refuses a dependency of the manifest at `manifest`, whose text is `bytes`, whose `path` leaves
/// `folder`: its sources would not be in the copy.
fn check_paths_inside(manifest: &Path, bytes: &[u8], folder: &Path) -> Result<(), String> {
    let text =
        std::str::from_utf8(bytes).map_err(|error| format!("{}: {error}", manifest.display()))?;
    let table: toml::Table =
        toml::from_str(text).map_err(|error| format!("{}: {error}", manifest.display()))?;
    let base = manifest.parent().unwrap_or(folder);
    let mut tables: Vec<&toml::Table> = Vec::new();
    for name in DEPENDENCY_TABLES {
        tables.extend(table.get(name).and_then(toml::Value::as_table));
    }
    for target in table
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::values)
    {
        for name in DEPENDENCY_TABLES {
            tables.extend(target.get(name).and_then(toml::Value::as_table));
        }
    }
    tables.extend(
        table
            .get("workspace")
            .and_then(|workspace| workspace.get("dependencies"))
            .and_then(toml::Value::as_table),
    );
    tables.extend(
        table
            .get("patch")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flat_map(|patch| patch.values().filter_map(toml::Value::as_table)),
    );
    for dependencies in tables {
        for dependency in dependencies.values() {
            let Some(path) = dependency.get("path").and_then(toml::Value::as_str) else {
                continue;
            };
            if !lexically_inside(&base.join(path), folder) {
                return Err(format!(
                    "the Rust crate's dependency {path} lies outside its folder, which an isolated run does not copy"
                ));
            }
        }
    }
    Ok(())
}

/// The cargo configuration `bytes` with its relative folders made absolute into `project`.
fn config_in_place(bytes: &[u8], project: &Path) -> Result<String, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|error| format!(".cargo/config.toml: {error}"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|error| format!(".cargo/config.toml: {error}"))?;
    let in_place = |value: &mut toml::Value| {
        if let Some(path) = value.as_str().filter(|path| Path::new(path).is_relative()) {
            *value = toml::Value::String(project.join(path).to_string_lossy().into_owned());
        }
    };
    if let Some(sources) = table.get_mut("source").and_then(toml::Value::as_table_mut) {
        for source in sources
            .iter_mut()
            .filter_map(|(_, source)| source.as_table_mut())
        {
            for key in ["directory", "local-registry"] {
                if let Some(value) = source.get_mut(key) {
                    in_place(value);
                }
            }
        }
    }
    if let Some(patches) = table.get_mut("patch").and_then(toml::Value::as_table_mut) {
        for patch in patches
            .iter_mut()
            .filter_map(|(_, patch)| patch.as_table_mut())
        {
            for dependency in patch
                .iter_mut()
                .filter_map(|(_, dependency)| dependency.as_table_mut())
            {
                if let Some(value) = dependency.get_mut("path") {
                    in_place(value);
                }
            }
        }
    }
    if let Some(paths) = table.get_mut("paths").and_then(toml::Value::as_array_mut) {
        paths.iter_mut().for_each(in_place);
    }
    toml::to_string(&table).map_err(|error| format!(".cargo/config.toml: {error}"))
}

/// Whether `path` lies inside `folder`, its `..` and `.` resolved without the file system.
fn lexically_inside(path: &Path, folder: &Path) -> bool {
    let normal = |path: &Path| {
        let mut parts: Vec<Component<'_>> = Vec::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    if matches!(parts.last(), Some(Component::Normal(_))) {
                        parts.pop();
                    } else {
                        parts.push(component);
                    }
                }
                other => parts.push(other),
            }
        }
        parts.into_iter().collect::<PathBuf>()
    };
    normal(path).starts_with(normal(folder))
}

/// `path` with `/` between its names.
fn slashed(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
#[path = "crate_files_tests.rs"]
mod tests;
