//! Project-level source registry and database helpers.

use rustc_hash::FxHashMap;
use std::path::{Component, Path, PathBuf};

use crate::db::{Database, FileId, SourceDatabase};

/// Canonical key for a source file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceKey {
    /// File-backed source (canonicalized path when possible).
    Path(PathBuf),
    /// Virtual source (non-file URI or in-memory buffer).
    Virtual(String),
}

impl SourceKey {
    /// Create a file-backed source key.
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        SourceKey::Path(normalize_path(path.as_ref()))
    }

    /// Create a virtual source key.
    pub fn from_virtual(name: impl Into<String>) -> Self {
        SourceKey::Virtual(name.into())
    }

    /// Render the key as a display string.
    pub fn display(&self) -> String {
        match self {
            SourceKey::Path(path) => path.to_string_lossy().to_string(),
            SourceKey::Virtual(name) => name.clone(),
        }
    }
}

/// Tracks source keys and assigns stable file ids within a project.
#[derive(Debug, Default)]
pub struct SourceRegistry {
    next_id: u32,
    ids_by_key: FxHashMap<SourceKey, FileId>,
    keys_by_id: FxHashMap<FileId, SourceKey>,
}

impl SourceRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve a file id from a source key.
    pub fn file_id_for_key(&self, key: &SourceKey) -> Option<FileId> {
        self.ids_by_key.get(key).copied()
    }

    /// Resolve a source key from a file id.
    pub fn key_for_file_id(&self, file_id: FileId) -> Option<&SourceKey> {
        self.keys_by_id.get(&file_id)
    }

    /// Ensure a file id exists for the key (allocate if missing).
    pub fn ensure_file_id(&mut self, key: SourceKey) -> FileId {
        if let Some(existing) = self.ids_by_key.get(&key).copied() {
            return existing;
        }
        let file_id = FileId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.ids_by_key.insert(key.clone(), file_id);
        self.keys_by_id.insert(file_id, key);
        file_id
    }

    /// Insert a key with an explicit file id.
    pub fn insert_with_id(&mut self, key: SourceKey, file_id: FileId) -> FileId {
        if let Some(existing) = self.ids_by_key.get(&key).copied() {
            return existing;
        }
        if self.keys_by_id.contains_key(&file_id) {
            panic!(
                "source file id collision for {}: FileId({}) is already registered",
                key.display(),
                file_id.0
            );
        }
        self.next_id = self.next_id.max(file_id.0.saturating_add(1));
        self.ids_by_key.insert(key.clone(), file_id);
        self.keys_by_id.insert(file_id, key);
        file_id
    }

    /// Clear all registered sources.
    pub fn clear(&mut self) {
        self.next_id = 0;
        self.ids_by_key.clear();
        self.keys_by_id.clear();
    }

    /// Remove a source key and return its file id.
    pub fn remove(&mut self, key: &SourceKey) -> Option<FileId> {
        let file_id = self.ids_by_key.remove(key)?;
        self.keys_by_id.remove(&file_id);
        Some(file_id)
    }

    /// Iterate registered keys and ids.
    pub fn iter(&self) -> impl Iterator<Item = (&SourceKey, FileId)> {
        self.ids_by_key.iter().map(|(key, id)| (key, *id))
    }
}

/// Project wrapper that owns sources + semantic database.
#[derive(Debug, Default)]
pub struct Project {
    db: Database,
    sources: SourceRegistry,
}

impl Project {
    /// Create a new project.
    pub fn new() -> Self {
        Self::default()
    }

    /// Access the database (read-only).
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Access the database (mutable).
    pub fn database_mut(&mut self) -> &mut Database {
        &mut self.db
    }

    /// Run a function against the database.
    pub fn with_database<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Database) -> R,
    {
        f(&self.db)
    }

    /// Run a function against the database (mutable).
    pub fn with_database_mut<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Database) -> R,
    {
        f(&mut self.db)
    }

    /// Insert/update source text and return its file id.
    pub fn set_source_text(&mut self, key: SourceKey, text: String) -> FileId {
        let file_id = self.sources.ensure_file_id(key);
        self.db.set_source_text(file_id, text);
        file_id
    }

    /// Lookup file id for a key.
    pub fn file_id_for_key(&self, key: &SourceKey) -> Option<FileId> {
        self.sources.file_id_for_key(key)
    }

    /// Lookup key for a file id.
    pub fn key_for_file_id(&self, file_id: FileId) -> Option<&SourceKey> {
        self.sources.key_for_file_id(file_id)
    }

    /// Access the source registry.
    pub fn sources(&self) -> &SourceRegistry {
        &self.sources
    }

    /// Remove a source and return its file id.
    pub fn remove_source(&mut self, key: &SourceKey) -> Option<FileId> {
        let file_id = self.sources.remove(key)?;
        self.db.remove_source_text(file_id);
        Some(file_id)
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    if let Ok(canon) = path.canonicalize() {
        return strip_windows_device_prefix(canon);
    }
    normalize_path_lossy_without_canonicalize(path)
}

fn normalize_path_lossy_without_canonicalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(windows)]
fn strip_windows_device_prefix(path: PathBuf) -> PathBuf {
    let raw = match path.to_str() {
        Some(raw) => raw,
        None => return path,
    };

    if let Some(rest) = raw
        .strip_prefix(r"\\?\UNC\")
        .or_else(|| raw.strip_prefix(r"\?\UNC\"))
        .or_else(|| raw.strip_prefix(r"\\.\UNC\"))
        .or_else(|| raw.strip_prefix(r"\??\UNC\"))
    {
        let mut unc = String::from(r"\\");
        unc.push_str(rest);
        return PathBuf::from(unc);
    }

    if let Some(rest) = raw
        .strip_prefix(r"\\?\")
        .or_else(|| raw.strip_prefix(r"\?\"))
        .or_else(|| raw.strip_prefix(r"\\.\"))
        .or_else(|| raw.strip_prefix(r"\??\"))
    {
        return PathBuf::from(rest);
    }

    path
}

#[cfg(not(windows))]
fn strip_windows_device_prefix(path: PathBuf) -> PathBuf {
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    #[should_panic(expected = "source file id collision")]
    fn insert_with_id_rejects_file_id_collision() {
        let mut registry = SourceRegistry::new();
        let first = SourceKey::from_virtual("first.st");
        let second = SourceKey::from_virtual("second.st");

        assert_eq!(registry.insert_with_id(first, FileId(7)), FileId(7));
        registry.insert_with_id(second, FileId(7));
    }

    #[test]
    fn insert_with_id_existing_key_returns_existing_id() {
        let mut registry = SourceRegistry::new();
        let key = SourceKey::from_virtual("main.st");

        assert_eq!(registry.insert_with_id(key.clone(), FileId(7)), FileId(7));
        assert_eq!(registry.insert_with_id(key, FileId(9)), FileId(7));
    }

    #[test]
    fn noncanonical_fallback_path_does_not_collide_with_canonical_path() {
        let root =
            std::env::temp_dir().join(format!("trust-hir-path-collision-{}", std::process::id()));
        let real_dir = root.join("real");
        fs::create_dir_all(&real_dir).expect("create temp source dir");
        let real_file = real_dir.join("main.st");
        fs::write(&real_file, "PROGRAM Main\nEND_PROGRAM\n").expect("write temp source");

        let canonical = SourceKey::from_path(&real_file);
        let fallback_path = real_dir.join("missing").join("..").join("main.st");
        let noncanonical =
            SourceKey::Path(normalize_path_lossy_without_canonicalize(&fallback_path));
        fs::remove_dir_all(&root).ok();

        assert_ne!(
            canonical, noncanonical,
            "a path that cannot be canonicalized must not collapse to an existing canonical source key"
        );
    }

    #[test]
    fn noncanonical_fallback_path_removes_current_dir_components() {
        let root = format!("trust-hir-current-dir-fallback-{}", std::process::id());
        let input = PathBuf::from(".").join(&root).join("main.st");
        let expected = PathBuf::from(root).join("main.st");
        let normalized = normalize_path(&input);

        assert_eq!(normalized, expected);
        assert_eq!(normalized.to_string_lossy(), expected.to_string_lossy());
    }
}
