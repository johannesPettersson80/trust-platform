//! The active bundle as a snapshot (specification 39, §21.5 item 4). Its files are read and hashed
//! once when it is activated — at start, deploy and rollback — and again only when they change on
//! disk; requests use the snapshot.
//!
//! Whether the files changed is told by a stamp, which reads no file: the libraries the bundle
//! draws from, and every entry under the project's `hmi/` folder and each library's `hmi/` folder
//! with its size, modification and creation time, and on Unix its inode and change time. These
//! folders hold whatever [`HmiBundle::read`] collects, so the stamp follows it as it changes. A
//! deploy or rollback replaces the `hmi/` folder, which the stamp always tells.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use crate::hmi::bundle::{HmiBundle, MAX_FILES};
use crate::hmi::catalogue::Catalogue;
use crate::hmi::presentation::{value_formats, ValueFormat};

/// The most project folders whose snapshots are kept; a runtime serves one.
const MAX_KEPT: usize = 16;
/// The most entries a stamp looks at; a bundle holds at most [`MAX_FILES`] files.
const MAX_ENTRIES: usize = 2 * MAX_FILES;

/// The snapshots, newest first, by project folder.
static SNAPSHOTS: Mutex<Vec<(PathBuf, Arc<Snapshot>)>> = Mutex::new(Vec::new());

/// The active bundle's version and what requests derive from its content.
#[derive(Debug)]
pub(super) struct Snapshot {
    stamp: Stamp,
    /// How often this folder's bundle has been read, this time included.
    pub(super) reads: u64,
    /// The short version (§12.8, §16.4); empty when the files cannot be read.
    pub(super) version: String,
    /// The display formats the pages' values inherit (§5.9).
    pub(super) formats: BTreeMap<String, ValueFormat>,
}

/// The snapshot of the bundle in `project`, read again only when its files have changed.
pub(super) fn current(project: &Path) -> Arc<Snapshot> {
    // The stamp is taken before the files are read: a change in between is seen by the next
    // request, never hidden behind a newer stamp.
    let stamp = Stamp::of(project);
    let reads = {
        let mut kept = kept();
        match kept.iter().position(|(root, _)| root == project) {
            Some(index) if kept[index].1.stamp == stamp => {
                // The folder in use stays first, so other folders are let go before it.
                let entry = kept.remove(index);
                let snapshot = entry.1.clone();
                kept.insert(0, entry);
                return snapshot;
            }
            Some(index) => kept[index].1.reads,
            None => 0,
        }
    };
    let snapshot = Arc::new(Snapshot::read(project, stamp, reads + 1));
    let mut kept = kept();
    kept.retain(|(root, _)| root != project);
    kept.insert(0, (project.to_path_buf(), snapshot.clone()));
    kept.truncate(MAX_KEPT);
    snapshot
}

fn kept() -> MutexGuard<'static, Vec<(PathBuf, Arc<Snapshot>)>> {
    SNAPSHOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Snapshot {
    /// Reads and hashes the bundle once, with what requests derive from it.
    fn read(project: &Path, stamp: Stamp, reads: u64) -> Self {
        let (version, formats) = match HmiBundle::read(project) {
            Ok(bundle) => {
                let (symbols, _) = crate::hmi::my_symbols::project_symbols(project);
                let catalogue = Catalogue::shipped().with_symbols(&symbols);
                (bundle.short_version(), value_formats(&bundle, &catalogue))
            }
            Err(_) => (String::new(), BTreeMap::new()),
        };
        Self {
            stamp,
            reads,
            version,
            formats,
        }
    }
}

/// What the bundle's folders look like on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    libraries: Vec<PathBuf>,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    /// Inode and change time on Unix, which a replaced file or folder never keeps.
    identity: (u64, i64, i64),
}

impl Stamp {
    fn of(project: &Path) -> Self {
        let libraries = crate::bundle_builder::dependency_roots(project).unwrap_or_default();
        let mut entries = Vec::new();
        for folder in std::iter::once(project).chain(libraries.iter().map(PathBuf::as_path)) {
            walk(&folder.join("hmi"), &mut entries);
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Self { libraries, entries }
    }
}

/// The folder and every entry below it, a link stamped by what it points to; links to folders
/// are not followed, as the bundle does not follow them.
fn walk(top: &Path, entries: &mut Vec<Entry>) {
    let mut folders = vec![top.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(metadata) = std::fs::metadata(&folder) else {
            continue;
        };
        entries.push(entry(folder.clone(), &metadata));
        let Ok(children) = std::fs::read_dir(&folder) else {
            continue;
        };
        for child in children.filter_map(Result::ok) {
            if entries.len() >= MAX_ENTRIES {
                return;
            }
            let path = child.path();
            match child.file_type() {
                Ok(kind) if kind.is_dir() => folders.push(path),
                Ok(_) => {
                    if let Ok(metadata) = std::fs::metadata(&path).or_else(|_| child.metadata()) {
                        entries.push(entry(path, &metadata));
                    }
                }
                Err(_) => {}
            }
        }
    }
}

fn entry(path: PathBuf, metadata: &std::fs::Metadata) -> Entry {
    Entry {
        path,
        len: metadata.len(),
        modified: metadata.modified().ok(),
        created: metadata.created().ok(),
        identity: identity(metadata),
    }
}

#[cfg(unix)]
fn identity(metadata: &std::fs::Metadata) -> (u64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    (metadata.ino(), metadata.ctime(), metadata.ctime_nsec())
}

#[cfg(not(unix))]
fn identity(_metadata: &std::fs::Metadata) -> (u64, i64, i64) {
    (0, 0, 0)
}

#[cfg(test)]
mod tests;
