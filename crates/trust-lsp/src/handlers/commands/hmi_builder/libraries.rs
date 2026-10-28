//! The bundled libraries an HMI needs, installed into the project (specification 39, §3.1 item 9,
//! §4.10; specification 25 §7): New HMI Screen, a new page or panel, and the Libraries panel's
//! Plant entry bring the extension's `Hmi` and `Plant` packages, with the packages they depend on,
//! into the project's `libraries/<folder>` and name the requested ones in `trust-lsp.toml`. Source
//! and visuals travel together; a library's own example projects stay in the library. Nothing is
//! written here: the answer lists the files to create and the manifest's new text, and the files
//! read, so the extension applies them in one edit and checks first that none changed meanwhile.
//!
//! `{ root, bundled, libraries: [{ name, folder }], open? }` → `{ create: [{ path, text }],
//! manifest: { path, text } | null, checked: [{ path, sha256 }] }`. `root` is the project,
//! `bundled` the extension's `media/libraries`, `open` the texts of the project's files open in an
//! editor by their project-relative path; every answered path is project-relative with `/`;
//! `sha256` is the hex digest of the text read, null for a file that did not exist.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::super::project::libraries::manifest::{upsert, DependencySpec, MANIFEST_FILE};
use super::decode;
use crate::config::manifest_dependencies;

/// A project's manifest before it names anything.
const NEW_MANIFEST: &str = "[project]\ninclude_paths = [\"src\"]\n";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    root: PathBuf,
    bundled: PathBuf,
    libraries: Vec<Requested>,
    #[serde(default)]
    open: BTreeMap<String, String>,
}

/// A library to install: its dependency name and its folder under `bundled`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Requested {
    name: String,
    folder: String,
}

/// The answer of `trust-lsp.hmiLibraries`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    let plan = plan(&args)?;
    Ok(json!({
        "ok": true,
        "create": plan.create.iter().map(|(path, text)| json!({ "path": path, "text": text })).collect::<Vec<_>>(),
        "manifest": plan.manifest.map(|text| json!({ "path": MANIFEST_FILE, "text": text })),
        "checked": plan.checked.iter().map(|(path, digest)| json!({ "path": path, "sha256": digest })).collect::<Vec<_>>(),
    }))
}

/// What installing the libraries takes.
struct Plan {
    /// The library files the project lacks, by project-relative path.
    create: BTreeMap<String, String>,
    /// The manifest's new text, when it changes.
    manifest: Option<String>,
    /// Each project file read and the digest of its text, none when it did not exist.
    checked: BTreeMap<String, Option<String>>,
}

fn plan(args: &Args) -> Result<Plan, String> {
    let project = Project {
        root: &args.root,
        open: &args.open,
    };
    let mut packages = Packages {
        base: &args.bundled,
        visited: BTreeSet::new(),
        visiting: BTreeSet::new(),
        files: BTreeMap::new(),
    };
    let mut checked = BTreeMap::new();
    let current = project.read(MANIFEST_FILE)?;
    checked.insert(MANIFEST_FILE.to_owned(), current.as_deref().map(digest));
    let mut manifest = current.clone().unwrap_or_else(|| NEW_MANIFEST.to_owned());
    for library in &args.libraries {
        let named =
            manifest_dependencies(&args.root, &manifest).is_some_and(|(dependencies, _)| {
                dependencies
                    .iter()
                    .any(|dependency| dependency.name.eq_ignore_ascii_case(&library.name))
            });
        if named {
            continue;
        }
        let package = packages.package(&library.folder)?;
        let version = manifest_dependencies(&args.bundled.join(&library.folder), &package)
            .and_then(|(_, version)| version);
        manifest = upsert(
            &manifest,
            &library.name,
            &DependencySpec::path(format!("libraries/{}", library.folder), version),
        )?;
    }
    let mut create = BTreeMap::new();
    for (path, text) in packages.files {
        let current = project.read(&path)?;
        checked.insert(path.clone(), current.as_deref().map(digest));
        match current {
            Some(current) if current != text => {
                return Err(format!(
                    "{} differs from the bundled library. Review that library update first; your version was preserved.",
                    args.root.join(&path).display()
                ));
            }
            Some(_) => {}
            None => {
                create.insert(path, text);
            }
        }
    }
    Ok(Plan {
        create,
        manifest: (current.as_deref() != Some(manifest.as_str())).then_some(manifest),
        checked,
    })
}

/// The project's files as the editor has them.
struct Project<'a> {
    root: &'a Path,
    open: &'a BTreeMap<String, String>,
}

impl Project<'_> {
    /// The text of the project file `path` (project-relative, `/`): an open editor's, else the
    /// disk's; none when the file does not exist.
    fn read(&self, path: &str) -> Result<Option<String>, String> {
        if let Some(text) = self.open.get(path) {
            return Ok(Some(text.clone()));
        }
        let file = self.root.join(path);
        match std::fs::read(&file) {
            Ok(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| format!("{} is not UTF-8 text.", file.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("{} cannot be read: {error}", file.display())),
        }
    }
}

/// The extension's packages, collected with the packages they depend on.
struct Packages<'a> {
    base: &'a Path,
    visited: BTreeSet<String>,
    visiting: BTreeSet<String>,
    /// Each package file by its path in the project.
    files: BTreeMap<String, String>,
}

impl Packages<'_> {
    /// The manifest of the package in `folder` (relative to the collection, `/`), after its
    /// dependencies and its files have been collected.
    fn package(&mut self, folder: &str) -> Result<String, String> {
        if !folder.split('/').all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'_'
                        || byte == b'-'
                })
        }) {
            return Err(
                "Bundled library dependencies must stay inside the package collection.".to_owned(),
            );
        }
        let source = self.base.join(folder);
        let manifest = read_utf8(&source.join(MANIFEST_FILE))?;
        if self.visited.contains(folder) {
            return Ok(manifest);
        }
        if !self.visiting.insert(folder.to_owned()) {
            return Err("The bundled library contains a dependency cycle.".to_owned());
        }
        let dependencies = manifest_dependencies(&source, &manifest)
            .map(|(dependencies, _)| dependencies)
            .unwrap_or_default();
        for dependency in dependencies {
            let (Some(path), None) = (&dependency.path, &dependency.git) else {
                return Err("A bundled dependency must name a local package.".to_owned());
            };
            let relative = within(self.base, path)
                .ok_or_else(|| "A bundled dependency leaves the package collection.".to_owned())?;
            let nested = self.package(&relative)?;
            let version = manifest_dependencies(&self.base.join(&relative), &nested)
                .and_then(|(_, version)| version);
            if let Some(wanted) = &dependency.version
                && version.as_ref() != Some(wanted)
            {
                return Err(format!(
                    "The bundled {} version does not match its contract.",
                    dependency.name
                ));
            }
        }
        self.collect(&source, &format!("libraries/{folder}"), true)?;
        self.visiting.remove(folder);
        self.visited.insert(folder.to_owned());
        Ok(manifest)
    }

    /// Every file under `source` at its path below `target`, in name order; the top folder's
    /// `examples` is left out: a library's example projects document it and are not part of a
    /// project using it.
    fn collect(&mut self, source: &Path, target: &str, top: bool) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(source)
            .map_err(|error| format!("{} cannot be read: {error}", source.display()))?
            .collect::<Result<_, _>>()
            .map_err(|error| format!("{} cannot be read: {error}", source.display()))?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let kind = entry
                .file_type()
                .map_err(|error| format!("{} cannot be read: {error}", entry.path().display()))?;
            let at = format!("{target}/{name}");
            if kind.is_dir() {
                if !(top && name == "examples") {
                    self.collect(&entry.path(), &at, false)?;
                }
            } else if kind.is_file() {
                self.files.insert(at, read_utf8(&entry.path())?);
            }
        }
        Ok(())
    }
}

/// `path` relative to `base` with `/`, both taken without the file system; none when it is `base`
/// itself or lies outside it.
fn within(base: &Path, path: &Path) -> Option<String> {
    let (base, path) = (lexical(base), lexical(path));
    let relative = path.strip_prefix(&base).ok()?;
    let parts: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// `path` with `.` and `..` resolved without the file system.
fn lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn read_utf8(file: &Path) -> Result<String, String> {
    let bytes = std::fs::read(file)
        .map_err(|error| format!("{} cannot be read: {error}", file.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8 text.", file.display()))
}

/// The hex SHA-256 digest of a text.
fn digest(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

#[cfg(test)]
#[path = "libraries_tests.rs"]
mod tests;
