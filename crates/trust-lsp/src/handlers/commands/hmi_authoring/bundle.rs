//! Read-only authoring snapshot; never use the saved dependency graph as an overlay base.
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use trust_runtime::hmi::bundle::{HmiBundle, LIBRARY_FOLDERS, MAX_BYTES, MAX_FILES};

fn key(root: &Path, path: &Path) -> io::Result<String> {
    let relative = path.strip_prefix(root).map_err(io::Error::other)?;
    if relative
        .components()
        .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(io::Error::other(
            "HMI asset path must stay inside its package",
        ));
    }
    Ok(relative
        .components()
        .map(|p| p.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn insert(bundle: &mut HmiBundle, name: String, bytes: Vec<u8>) -> io::Result<()> {
    let previous = bundle.files.get(&name).map_or(0, Vec::len);
    let size = bundle.files.values().map(Vec::len).sum::<usize>() - previous;
    if size.saturating_add(bytes.len()) > MAX_BYTES
        || (!bundle.files.contains_key(&name) && bundle.files.len() >= MAX_FILES)
    {
        return Err(io::Error::other(
            "HMI authoring snapshot exceeds the existing bundle limits",
        ));
    }
    bundle.files.insert(name, bytes);
    Ok(())
}

/// The files [`assets`] takes from a folder: every file below it, or its own files of some kinds.
#[derive(Clone, Copy)]
enum Take<'a> {
    All,
    Kinds(&'a [&'a str]),
}

impl Take<'_> {
    fn recursive(self) -> bool {
        matches!(self, Self::All)
    }

    fn file(self, path: &Path) -> bool {
        match self {
            Self::All => true,
            Self::Kinds(kinds) => path
                .extension()
                .is_some_and(|extension| kinds.iter().any(|kind| extension == *kind)),
        }
    }
}

fn assets(
    root: &Path,
    folder: &Path,
    open: &BTreeMap<PathBuf, String>,
    take: Take<'_>,
) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let mut result = HmiBundle::default();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() && take.recursive() {
                pending.push(entry.path());
            } else if kind.is_file() && take.file(&entry.path()) {
                let path = entry.path();
                let bytes = match open.get(&path) {
                    Some(text) => text.as_bytes().to_vec(),
                    None => {
                        let mut bytes = Vec::new();
                        std::fs::File::open(&path)?
                            .take(MAX_BYTES as u64 + 1)
                            .read_to_end(&mut bytes)?;
                        bytes
                    }
                };
                insert(&mut result, key(root, &path)?, bytes)?;
            }
            // Match the native bundle reader: never follow filesystem symlinks.
        }
    }
    for (path, text) in open {
        if path.starts_with(folder)
            && (take.recursive() || (path.parent() == Some(folder) && take.file(path)))
        {
            insert(&mut result, key(root, path)?, text.as_bytes().to_vec())?;
        }
    }
    Ok(result.files)
}

/// The bundle as the runtime reads it (`HmiBundle::read`), with unsaved editor text in place of
/// the saved files: every file under the project's `hmi/`, and each library's panels, pairs and
/// symbols that the project does not replace with a file of the same name (specification 39,
/// §8.7 item 2, §14.4 item 1, §21.1).
pub(crate) fn read_edited(root: &Path, open: &BTreeMap<PathBuf, String>) -> io::Result<HmiBundle> {
    let mut bundle = HmiBundle {
        files: assets(root, &root.join("hmi"), open, Take::All)?,
    };
    let dependencies = super::sources::dependency_roots(root, open).map_err(io::Error::other)?;
    for library in dependencies {
        for (folder, kinds) in LIBRARY_FOLDERS {
            let folder = library.join("hmi").join(folder);
            for (path, bytes) in assets(&library, &folder, open, Take::Kinds(kinds))? {
                if !bundle.files.contains_key(&path) {
                    insert(&mut bundle, path, bytes)?;
                }
            }
        }
    }
    Ok(bundle)
}

pub(crate) fn faceplate_text<'a>(bundle: &'a HmiBundle, name: &str) -> Option<(&'a str, &'a [u8])> {
    let path = format!("hmi/panels/{name}.toml");
    bundle
        .files
        .get_key_value(&path)
        .map(|(key, bytes)| (key.as_str(), bytes.as_slice()))
}
pub(crate) fn faceplate(
    bundle: &HmiBundle,
    name: &str,
) -> Option<
    Result<
        trust_runtime::hmi::page_files::HmiFaceplate,
        Vec<trust_runtime::hmi::page_files::PageFileError>,
    >,
> {
    let (path, bytes) = faceplate_text(bundle, name)?;
    Some(super::documents::faceplate(
        path,
        &String::from_utf8_lossy(bytes),
    ))
}

pub(crate) fn pairs_of(
    bundle: &HmiBundle,
    name: &str,
) -> Vec<trust_runtime::hmi::page_files::HmiPair> {
    bundle
        .files
        .iter()
        .filter(|(p, _)| p.starts_with("hmi/pairs/") && p.ends_with(".toml"))
        .filter_map(|(p, b)| super::pairs::parse_pair(p, &String::from_utf8_lossy(b)).ok())
        .filter(|p| p.faceplate == name)
        .collect()
}

#[cfg(test)]
mod tests;
