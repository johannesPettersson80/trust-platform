//! The Libraries panel's edits of a project (specification 25 §7, §11.6): adding a curated, local or
//! Git library, removing one, updating a curated one and fixing a library's path, and where a
//! symbol's declaration goes in a Structured Text file.
//!
//! Adding OSCAT or PLCopen Motion vendors the extension's copy into the project's
//! `libraries/<folder>` (replacing an earlier copy) and names it in `[dependencies]` with its
//! package version. A local folder needs a truST library manifest; the panel creates the
//! `[package] version = "0.1.0"` template only when the user agrees. A Git library is pinned by the
//! tag, branch or commit the user typed.

use std::path::{Path, PathBuf};

use super::catalog::{self, CuratedLibrary};
use super::manifest::{
    classify_git_pin, package_version, relative_posix, remove as remove_entry, upsert,
    DependencySpec, MANIFEST_FILE,
};

/// The manifest a local library folder gets when the user agrees to make it a truST library.
pub(super) const LIBRARY_MANIFEST_TEMPLATE: &str =
    "[package]\nversion = \"0.1.0\"\n\n[project]\ninclude_paths = [\"src\"]\n";

/// What adding a library did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Added {
    /// The library was added; the message for the user.
    Done(String),
    /// The local folder has no truST library manifest and the user was not asked yet.
    NeedsManifest,
}

/// Adds the curated library `id` by vendoring the extension's copy from `bundled`.
pub(super) fn add_curated(root: &Path, bundled: &Path, id: &str) -> Result<Added, String> {
    let library = curated(id)?;
    if library.id == "plant" {
        // The Plant library is installed with its HMI components by the HMI Builder's library
        // install (specification 39), which the extension runs.
        return Err("The Plant library is added by the HMI Builder's library install.".to_owned());
    }
    let source = bundled.join(library.folder);
    if !source.is_dir() {
        return Err(format!(
            "Bundled {} files are missing from the extension.",
            library.label
        ));
    }
    let destination = root.join("libraries").join(library.folder);
    copy_directory(&source, &destination).map_err(|error| {
        format!(
            "{} could not be copied into the project: {error}",
            library.label
        )
    })?;
    let version = package_version(&destination);
    write_dependency(
        root,
        library.dependency_name,
        &DependencySpec::path(relative_posix(root, &destination), version),
    )?;
    Ok(Added::Done(format!(
        "{} added to this project.",
        library.label
    )))
}

/// Adds the local library in `folder`. Without a truST library manifest there, `create_manifest`
/// decides: `None` asks the user first, `Some(true)` writes the template, `Some(false)` refuses.
pub(super) fn add_local(
    root: &Path,
    folder: &Path,
    create_manifest: Option<bool>,
) -> Result<Added, String> {
    if !folder.is_dir() {
        return Err("No library added — the selected folder does not exist.".to_owned());
    }
    if !folder.join(MANIFEST_FILE).is_file() {
        match create_manifest {
            None => return Ok(Added::NeedsManifest),
            Some(false) => {
                return Err(
                    "No library added — the folder needs a truST library manifest.".to_owned(),
                )
            }
            Some(true) => {
                std::fs::write(folder.join(MANIFEST_FILE), LIBRARY_MANIFEST_TEMPLATE)
                    .and_then(|()| std::fs::create_dir_all(folder.join("src")))
                    .map_err(|error| {
                        format!("The library manifest could not be written: {error}")
                    })?;
            }
        }
    }
    let name = local_library_name(folder);
    let version = package_version(folder);
    write_dependency(
        root,
        &name,
        &DependencySpec::path(relative_posix(root, folder), version),
    )?;
    Ok(Added::Done(format!("{name} added to this project.")))
}

/// Adds the Git library `name` from `url`, pinned by `pin` (a tag, branch or commit; none when
/// blank), with an optional `version`.
pub(super) fn add_git(
    root: &Path,
    name: &str,
    url: &str,
    pin: Option<&str>,
    version: Option<String>,
) -> Result<Added, String> {
    if name.trim().is_empty() {
        return Err("No library added — the dependency needs a name.".to_owned());
    }
    let mut spec = DependencySpec {
        git: Some(url.to_owned()),
        version: version.filter(|version| !version.trim().is_empty()),
        ..DependencySpec::default()
    };
    match pin.and_then(classify_git_pin) {
        Some(("rev", value)) => spec.rev = Some(value),
        Some(("tag", value)) => spec.tag = Some(value),
        Some((_, value)) => spec.branch = Some(value),
        None => {}
    }
    write_dependency(root, name, &spec)?;
    Ok(Added::Done(format!("{name} added from Git.")))
}

/// Removes the dependency `name`; the message for the user.
pub(super) fn remove(root: &Path, name: &str) -> Result<String, String> {
    let text = read_manifest(root)?;
    write_manifest(root, &remove_entry(&text, name)?)?;
    Ok(format!("{name} removed from this project."))
}

/// The versions an update of the curated library `id` goes from and to, for the confirmation:
/// `(label, current, next)`, `unknown` where a copy has no version.
pub(super) fn update_plan(
    root: &Path,
    bundled: &Path,
    id: &str,
) -> Result<(&'static str, String, String), String> {
    let library = curated(id)?;
    let unknown = || "unknown".to_owned();
    let next = package_version(&bundled.join(library.folder)).unwrap_or_else(unknown);
    let current =
        package_version(&root.join("libraries").join(library.folder)).unwrap_or_else(unknown);
    Ok((library.label, current, next))
}

/// Points the dependency `name` at `folder`, which must hold a truST library manifest; the message
/// for the user.
pub(super) fn fix_path(root: &Path, name: &str, folder: &Path) -> Result<String, String> {
    if !folder.join(MANIFEST_FILE).is_file() {
        return Err(
            "No library path changed — the selected folder needs a truST library manifest."
                .to_owned(),
        );
    }
    let version = package_version(folder);
    write_dependency(
        root,
        name,
        &DependencySpec::path(relative_posix(root, folder), version),
    )?;
    Ok(format!("{name} path updated."))
}

/// Where a declaration snippet goes in a Structured Text `text`: the line after the first `VAR` line
/// (not `VAR_INPUT` and the like), indented as that next line is (four blanks when it is not
/// indented), as `(zero-based line, text with its newline)`. `None` when the snippet is no
/// `name : …` declaration or the text has no `VAR` line; the panel then inserts it at the cursor.
pub(super) fn insertion(text: &str, snippet: &str) -> Option<(usize, String)> {
    let snippet = snippet.trim();
    if !is_declaration(snippet) {
        return None;
    }
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let var_line = lines.iter().position(|line| is_var_line(line))?;
    let next = lines.get(var_line + 1).copied().unwrap_or("");
    let indent_length = next.len() - next.trim_start().len();
    let indent = if indent_length > 0 && indent_length < next.len() {
        &next[..indent_length]
    } else {
        "    "
    };
    Some((var_line + 1, format!("{indent}{snippet}\n")))
}

/// `name :` at the start: an identifier, blanks, a colon.
fn is_declaration(snippet: &str) -> bool {
    let mut chars = snippet.char_indices();
    match chars.next() {
        Some((_, first)) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    let name_end = chars
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_'))
        .map_or(snippet.len(), |(index, _)| index);
    snippet[name_end..].trim_start().starts_with(':')
}

/// A line that opens a `VAR` block (any letter case): `VAR` not followed by a letter, digit or `_`.
fn is_var_line(line: &str) -> bool {
    let line = line.trim_start();
    line.get(..3)
        .is_some_and(|head| head.eq_ignore_ascii_case("VAR"))
        && !line[3..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// The dependency name of a local library: its folder's name with every character other than a
/// letter, a digit or `_` made `_`; `LocalLibrary` when the folder has no name.
fn local_library_name(folder: &Path) -> String {
    let name: String = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        "LocalLibrary".to_owned()
    } else {
        name
    }
}

fn curated(id: &str) -> Result<&'static CuratedLibrary, String> {
    catalog::by_id(id).ok_or_else(|| format!("Unknown curated library: {id}"))
}

fn write_dependency(root: &Path, name: &str, spec: &DependencySpec) -> Result<(), String> {
    let text = read_manifest(root)?;
    write_manifest(root, &upsert(&text, name, spec)?)
}

fn read_manifest(root: &Path) -> Result<String, String> {
    std::fs::read_to_string(root.join(MANIFEST_FILE))
        .map_err(|error| format!("The project's trust-lsp.toml cannot be read: {error}"))
}

fn write_manifest(root: &Path, text: &str) -> Result<(), String> {
    std::fs::write(root.join(MANIFEST_FILE), text)
        .map_err(|error| format!("The project's trust-lsp.toml cannot be written: {error}"))
}

/// Replaces `destination` with a copy of `source`.
fn copy_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    if destination.is_dir() {
        std::fs::remove_dir_all(destination)?;
    } else if destination.exists() {
        std::fs::remove_file(destination)?;
    }
    copy_tree(source, destination)
}

fn copy_tree(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let target: PathBuf = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "edits_tests.rs"]
mod tests;
