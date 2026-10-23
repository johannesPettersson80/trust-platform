//! The HMI bundle (specification 39, §16.1, §16.4): every file under `hmi/` and its version, the
//! SHA-256 of the files in path order. One implementation for the language server and the
//! runtime.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value as Json;

pub mod checks;
mod faceplates;
mod panel_checks;
pub mod store;

#[cfg(test)]
mod check_tests;
#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod tests;

/// The bundle format this runtime reads.
pub const FORMAT: u64 = 1;
/// The most files a bundle may hold.
pub const MAX_FILES: usize = 2_000;
/// The most bytes a bundle may hold.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
/// The folders of a library's `hmi/` that a bundle takes, with the kinds of file each holds: its
/// panels and pairs (§8.7 item 2) and its symbols, a drawing and a definition each (§14.4 item 1).
pub const LIBRARY_FOLDERS: [(&str, &[&str]); 3] = [
    ("panels", &["toml"]),
    ("pairs", &["toml"]),
    ("symbols", &["svg", "toml"]),
];

/// Reads a file of a project's `hmi/` folder by its project-relative path, for a route or a check.
/// A link is not followed, and a path that leads outside `hmi/` or names an `io.toml` is refused,
/// so that no HMI route discloses the I/O configuration's credentials, as the bundle itself never
/// takes a link.
pub(crate) fn read_hmi_file(project: &Path, relative: &str) -> Option<Vec<u8>> {
    let path = project.join(relative);
    if !std::fs::symlink_metadata(&path).ok()?.is_file() {
        return None;
    }
    let folder = project.join("hmi").canonicalize().ok()?;
    let resolved = path.canonicalize().ok()?;
    let io_config = resolved
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("io.toml"));
    if !resolved.starts_with(&folder) || io_config {
        return None;
    }
    std::fs::read(&resolved).ok()
}

/// Every file under `hmi/`, by path (`hmi/pages/overview.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HmiBundle {
    /// File contents by path, in path order.
    pub files: BTreeMap<String, Vec<u8>>,
}

impl HmiBundle {
    /// The bundle of a project: every file under its `hmi/` folder, and the panels, pairs and
    /// symbols of its libraries.
    pub fn read(project: &Path) -> std::io::Result<Self> {
        let mut files = BTreeMap::new();
        let mut total = 0;
        let mut folders = vec![project.join("hmi")];
        while let Some(folder) = folders.pop() {
            let entries = match std::fs::read_dir(&folder) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            for entry in entries {
                let entry = entry?;
                let kind = entry.file_type()?;
                if kind.is_dir() {
                    folders.push(entry.path());
                } else if kind.is_file() {
                    let relative = entry
                        .path()
                        .strip_prefix(project)
                        .map_err(std::io::Error::other)?
                        .to_path_buf();
                    let path = relative
                        .components()
                        .map(|part| part.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/");
                    let bytes = std::fs::read(entry.path())?;
                    total += bytes.len();
                    if files.len() >= MAX_FILES || total > MAX_BYTES {
                        return Err(std::io::Error::other(format!(
                            "the hmi folder holds more than {MAX_FILES} files or {MAX_BYTES} bytes"
                        )));
                    }
                    files.insert(path, bytes);
                }
            }
        }
        // The panels, pairs and symbols the project's libraries ship (§8.7 item 2, §14.4 item 1),
        // unless the project has its own of that name.
        for library in crate::bundle_builder::dependency_roots(project).unwrap_or_default() {
            for (folder, kinds) in LIBRARY_FOLDERS {
                let Ok(entries) = std::fs::read_dir(library.join("hmi").join(folder)) else {
                    continue;
                };
                for entry in entries.filter_map(Result::ok) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let shipped = Path::new(&name)
                        .extension()
                        .is_some_and(|extension| kinds.iter().any(|kind| extension == *kind));
                    if !shipped || !entry.path().is_file() {
                        continue;
                    }
                    let path = format!("hmi/{folder}/{name}");
                    if files.contains_key(&path) {
                        continue;
                    }
                    let bytes = std::fs::read(entry.path())?;
                    total += bytes.len();
                    if files.len() >= MAX_FILES || total > MAX_BYTES {
                        return Err(std::io::Error::other(format!(
                            "the hmi folder holds more than {MAX_FILES} files or {MAX_BYTES} bytes"
                        )));
                    }
                    files.insert(path, bytes);
                }
            }
        }
        Ok(Self { files })
    }

    /// The version: the SHA-256 of each file's path, a zero byte, its contents and a zero byte, in
    /// path order, as hexadecimal.
    pub fn version(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for (path, bytes) in &self.files {
            hasher.update(path.as_bytes());
            hasher.update([0]);
            hasher.update(bytes);
            hasher.update([0]);
        }
        hex::encode(hasher.finalize()).to_string()
    }

    /// The first 16 hexadecimal digits of the version: the page version of §12.8.
    pub fn short_version(&self) -> String {
        self.version()[..16].to_string()
    }

    /// The bundle as sent: text files as text, others in base64.
    pub fn to_json(&self) -> Json {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        let files: serde_json::Map<String, Json> = self
            .files
            .iter()
            .map(|(path, bytes)| {
                let content = match std::str::from_utf8(bytes) {
                    Ok(text) if is_text(path) => Json::String(text.to_string()),
                    _ => serde_json::json!({ "base64": STANDARD.encode(bytes) }),
                };
                (path.clone(), content)
            })
            .collect();
        serde_json::json!({ "format": FORMAT, "version": self.version(), "files": files })
    }

    /// A bundle as received; its version must match its content and every path must be a file
    /// under `hmi/`.
    pub fn from_json(value: &Json) -> Result<Self, String> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        if value.get("format").and_then(Json::as_u64) != Some(FORMAT) {
            return Err(format!("the bundle format must be {FORMAT}"));
        }
        let listed = value
            .get("files")
            .and_then(Json::as_object)
            .ok_or("the bundle has no files")?;
        if listed.len() > MAX_FILES {
            return Err(format!("a bundle holds at most {MAX_FILES} files"));
        }
        let mut files = BTreeMap::new();
        let mut total = 0;
        for (path, content) in listed {
            if !is_bundle_path(path) {
                return Err(format!("\"{path}\" is not a path of a file under hmi/"));
            }
            let bytes = match content {
                Json::String(text) => text.as_bytes().to_vec(),
                Json::Object(object) => object
                    .get("base64")
                    .and_then(Json::as_str)
                    .and_then(|data| STANDARD.decode(data).ok())
                    .ok_or_else(|| format!("\"{path}\" is neither text nor {{ \"base64\" }}"))?,
                _ => return Err(format!("\"{path}\" is neither text nor {{ \"base64\" }}")),
            };
            total += bytes.len();
            if total > MAX_BYTES {
                return Err(format!("a bundle holds at most {MAX_BYTES} bytes"));
            }
            files.insert(path.clone(), bytes);
        }
        let bundle = Self { files };
        let claimed = value
            .get("version")
            .and_then(Json::as_str)
            .unwrap_or_default();
        if claimed != bundle.version() {
            return Err("the bundle's version does not match its content".to_string());
        }
        Ok(bundle)
    }

    /// Writes the files below `root`, which gets an `hmi/` folder.
    pub fn write(&self, root: &Path) -> std::io::Result<()> {
        for (path, bytes) in &self.files {
            let file = root.join(path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(file, bytes)?;
        }
        Ok(())
    }
}

/// Files sent as text.
fn is_text(path: &str) -> bool {
    [".toml", ".json", ".svg", ".txt", ".md"]
        .iter()
        .any(|ending| path.ends_with(ending))
}

/// A relative path under `hmi/` with plain parts: no `..`, no backslash, no empty part.
fn is_bundle_path(path: &str) -> bool {
    path.strip_prefix("hmi/").is_some_and(|rest| {
        !rest.is_empty()
            && !path.contains('\\')
            && rest
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
    })
}
