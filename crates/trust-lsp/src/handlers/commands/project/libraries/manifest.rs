//! A project's `trust-lsp.toml` as the Libraries panel edits it (specification 25 §11.6): one
//! `[dependencies]` entry is added, replaced or removed, and every other line, comment and table
//! stays as it was. A dependency names exactly one of `path` and `git`; a Git dependency has at
//! most one pin selector (`rev`, `tag` or `branch`).

use std::path::{Component, Path, PathBuf};

use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

/// The manifest every project and library keeps its settings in.
pub(crate) const MANIFEST_FILE: &str = "trust-lsp.toml";

/// One `[dependencies]` entry as the panel writes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DependencySpec {
    pub(super) path: Option<String>,
    pub(super) git: Option<String>,
    pub(super) version: Option<String>,
    pub(super) rev: Option<String>,
    pub(super) tag: Option<String>,
    pub(super) branch: Option<String>,
}

impl DependencySpec {
    /// A dependency on the folder `path`, as written in the manifest.
    pub(crate) fn path(path: String, version: Option<String>) -> Self {
        Self {
            path: Some(path),
            version,
            ..Self::default()
        }
    }

    /// The entry's inline table: `{ path = "…", version = "…" }`, or `{ git = "…", <pin> = "…",
    /// version = "…" }`. Empty fields are left out.
    pub(super) fn inline_table(&self) -> Result<InlineTable, String> {
        fn present(value: &Option<String>) -> Option<&str> {
            value.as_deref().filter(|text| !text.is_empty())
        }
        let mut table = InlineTable::new();
        match (present(&self.path), present(&self.git)) {
            (Some(path), None) => {
                table.insert("path", Value::from(path));
            }
            (None, Some(git)) => {
                let pins: Vec<(&str, &str)> = [
                    ("rev", &self.rev),
                    ("tag", &self.tag),
                    ("branch", &self.branch),
                ]
                .into_iter()
                .filter_map(|(key, value)| present(value).map(|value| (key, value)))
                .collect();
                if pins.len() > 1 {
                    return Err(
                        "Git dependency may set only one of rev, tag, or branch.".to_owned()
                    );
                }
                table.insert("git", Value::from(git));
                for (key, value) in pins {
                    table.insert(key, Value::from(value));
                }
            }
            _ => return Err("Library dependency must set exactly one of path or git.".to_owned()),
        }
        if let Some(version) = present(&self.version) {
            table.insert("version", Value::from(version));
        }
        table.fmt();
        Ok(table)
    }
}

/// The selector a typed Git pin stands for: a commit (7 to 40 hexadecimal digits) is `rev`, a
/// version such as `v1.2` or `2.0.1` is `tag`, anything else is `branch`. A blank pin is none.
pub(super) fn classify_git_pin(pin: &str) -> Option<(&'static str, String)> {
    let pin = pin.trim();
    if pin.is_empty() {
        return None;
    }
    let is_commit = (7..=40).contains(&pin.len()) && pin.chars().all(|ch| ch.is_ascii_hexdigit());
    let after_v = pin.strip_prefix('v').unwrap_or(pin);
    let is_version = after_v.split_once('.').is_some_and(|(major, minor)| {
        !major.is_empty()
            && major.chars().all(|ch| ch.is_ascii_digit())
            && minor.starts_with(|ch: char| ch.is_ascii_digit())
    });
    let selector = if is_commit {
        "rev"
    } else if is_version {
        "tag"
    } else {
        "branch"
    };
    Some((selector, pin.to_owned()))
}

/// `text` with the dependency `name` set to `spec`: an existing entry is replaced where it stands,
/// a new one is added at the end of `[dependencies]`, and a manifest without that table gains it at
/// its end, after a blank line.
pub(crate) fn upsert(text: &str, name: &str, spec: &DependencySpec) -> Result<String, String> {
    let inline = spec.inline_table()?;
    let mut document = parse(text)?;
    let has_content = !text.trim().is_empty();
    let dependencies = document.entry("dependencies").or_insert_with(|| {
        let mut table = Table::new();
        if has_content {
            table.decor_mut().set_prefix("\n");
        }
        Item::Table(table)
    });
    let dependencies = dependencies
        .as_table_like_mut()
        .ok_or_else(|| "The [dependencies] of trust-lsp.toml is not a table.".to_owned())?;
    let entry = Item::Value(Value::InlineTable(inline));
    // Replacing the value keeps the key where it stands, with the comment above it.
    match dependencies.get_mut(name) {
        Some(existing) => *existing = entry,
        None => {
            dependencies.insert(name, entry);
        }
    }
    Ok(document.to_string())
}

/// `text` without the dependency `name`; unchanged when it has none.
pub(super) fn remove(text: &str, name: &str) -> Result<String, String> {
    let mut document = parse(text)?;
    if let Some(dependencies) = document
        .get_mut("dependencies")
        .and_then(Item::as_table_like_mut)
    {
        dependencies.remove(name);
    }
    Ok(document.to_string())
}

fn parse(text: &str) -> Result<DocumentMut, String> {
    text.parse::<DocumentMut>().map_err(|error| {
        format!(
            "The project's trust-lsp.toml cannot be read: {}",
            error.message()
        )
    })
}

/// The `[package] version` of the library in `folder`, read from its `trust-lsp.toml` by the
/// language server's rules.
pub(super) fn package_version(folder: &Path) -> Option<String> {
    let text = std::fs::read_to_string(folder.join(MANIFEST_FILE)).ok()?;
    crate::config::manifest_dependencies(folder, &text).and_then(|(_, version)| version)
}

/// `target` relative to `root` with `/` separators, as a manifest names a dependency's folder:
/// `libraries/oscat`, `../shared/lib`. A target on another drive keeps its full path.
pub(super) fn relative_posix(root: &Path, target: &Path) -> String {
    let root = normalize(root);
    let target = normalize(target);
    let root_parts: Vec<Component<'_>> = root.components().collect();
    let target_parts: Vec<Component<'_>> = target.components().collect();
    if root_parts.first() != target_parts.first() {
        return posix(&target);
    }
    let common = root_parts
        .iter()
        .zip(&target_parts)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..root_parts.len() {
        relative.push("..");
    }
    for part in &target_parts[common..] {
        relative.push(part.as_os_str());
    }
    posix(&relative)
}

/// `path` with `.` and `..` resolved without the file system.
fn normalize(path: &Path) -> PathBuf {
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

/// `path` with `/` separators.
fn posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
