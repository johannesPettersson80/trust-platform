//! Persistent workspace index cache.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::Hasher;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

const CACHE_VERSION: u32 = 1;
const CACHE_FILE: &str = "index.json";

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct IndexCache {
    version: u32,
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    hash: u64,
    size: u64,
    mtime: Option<u64>,
    content: String,
}

impl Default for IndexCache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            entries: HashMap::new(),
        }
    }
}

impl IndexCache {
    pub(crate) fn load_or_default(dir: &Path) -> Self {
        let path = dir.join(CACHE_FILE);
        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(mut cache) = serde_json::from_str::<IndexCache>(&contents) else {
            return Self::default();
        };
        if cache.version != CACHE_VERSION {
            cache.version = CACHE_VERSION;
            cache.entries.clear();
        }
        cache
    }

    pub(crate) fn save(&self, dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let path = dir.join(CACHE_FILE);
        let payload = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        fs::write(path, payload)
    }

    pub(crate) fn content_for_path(&self, path: &Path) -> Option<&str> {
        let key = cache_key(path);
        let entry = self.entries.get(&key)?;
        if entry.matches_disk(path) {
            Some(entry.content.as_str())
        } else {
            None
        }
    }

    pub(crate) fn update_from_content(&mut self, path: &Path, content: String) -> u64 {
        let (size, mtime) = metadata_signature(path).unwrap_or((content.len() as u64, None));
        let hash = hash_content(&content);
        let key = cache_key(path);
        if let Some(entry) = self.entries.get_mut(&key) {
            if entry.hash == hash {
                entry.size = size;
                entry.mtime = mtime;
                return hash;
            }
        }
        let entry = CacheEntry {
            hash,
            size,
            mtime,
            content,
        };
        self.entries.insert(key, entry);
        hash
    }

    pub(crate) fn remove_path(&mut self, path: &Path) {
        self.entries.remove(&cache_key(path));
    }

    pub(crate) fn retain_paths(&mut self, paths: &[std::path::PathBuf]) {
        let mut keep = HashSet::with_capacity(paths.len());
        for path in paths {
            keep.insert(cache_key(path));
        }
        self.entries.retain(|key, _| keep.contains(key));
    }
}

impl CacheEntry {
    fn matches_disk(&self, path: &Path) -> bool {
        let Some((size, mtime)) = metadata_signature(path) else {
            return false;
        };
        if self.size != size || self.mtime != mtime {
            return false;
        }
        let Ok(current) = fs::read_to_string(path) else {
            return false;
        };
        hash_content(&current) == self.hash
    }
}

fn cache_key(path: &Path) -> String {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = normalized
                    .components()
                    .next_back()
                    .is_some_and(|last| matches!(last, Component::Normal(_)));
                if can_pop {
                    normalized.pop();
                } else if !normalized.is_absolute() {
                    normalized.push("..");
                }
            }
            Component::Normal(value) => normalized.push(value),
            Component::Prefix(value) => normalized.push(value.as_os_str()),
            Component::RootDir => normalized.push(std::path::MAIN_SEPARATOR.to_string()),
        }
    }
    normalized.to_string_lossy().to_string()
}

fn metadata_signature(path: &Path) -> Option<(u64, Option<u64>)> {
    let meta = fs::metadata(path).ok()?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs());
    Some((size, mtime))
}

fn hash_content(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(content, &mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn cache_round_trip_and_invalidate_on_change() {
        let root = temp_dir("trustlsp-index-cache");
        let cache_dir = root.join(".trust-lsp/index-cache");
        let file_path = root.join("main.st");
        fs::write(&file_path, "PROGRAM Test\nEND_PROGRAM\n").expect("write file");

        let mut cache = IndexCache::load_or_default(&cache_dir);
        assert!(cache.content_for_path(&file_path).is_none());

        let content = fs::read_to_string(&file_path).expect("read file");
        cache.update_from_content(&file_path, content.clone());
        cache.save(&cache_dir).expect("save cache");

        let cache = IndexCache::load_or_default(&cache_dir);
        let cached = cache.content_for_path(&file_path).expect("cached content");
        assert_eq!(cached, content);

        fs::write(
            &file_path,
            "PROGRAM Test\nVAR x : INT; END_VAR\nEND_PROGRAM\n",
        )
        .expect("write update");

        let cache = IndexCache::load_or_default(&cache_dir);
        assert!(cache.content_for_path(&file_path).is_none());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cache_rejects_same_size_same_second_mtime_collision() {
        let root = temp_dir("trustlsp-index-cache-metadata-collision");
        let cache_dir = root.join(".trust-lsp/index-cache");
        let file_path = root.join("Lib.st");
        let initial = "FUNCTION Add : INT\n    Add := 1;\nEND_FUNCTION\n";
        let edited = "FUNCTION Mul : INT\n    Mul := 1;\nEND_FUNCTION\n";
        assert_eq!(
            initial.len(),
            edited.len(),
            "fixture must preserve file size"
        );

        let timestamp = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        fs::write(&file_path, initial).expect("write initial file");
        set_modified_time(&file_path, timestamp);
        let initial_signature = metadata_signature(&file_path).expect("initial metadata");

        let mut cache = IndexCache::load_or_default(&cache_dir);
        cache.update_from_content(&file_path, initial.to_string());
        cache.save(&cache_dir).expect("save cache");
        let cache = IndexCache::load_or_default(&cache_dir);
        assert_eq!(
            cache.content_for_path(&file_path),
            Some(initial),
            "warm cache should return the original content"
        );

        fs::write(&file_path, edited).expect("write edited file");
        set_modified_time(&file_path, timestamp);
        let edited_signature = metadata_signature(&file_path).expect("edited metadata");
        assert_eq!(
            initial_signature, edited_signature,
            "fixture must collide on the cache metadata signature"
        );

        let cache = IndexCache::load_or_default(&cache_dir);
        assert!(
            cache.content_for_path(&file_path).is_none(),
            "cache lookup must reject stale content when disk content changed under identical size/mtime metadata"
        );

        fs::remove_dir_all(root).ok();
    }

    fn set_modified_time(path: &Path, modified: SystemTime) {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("open file for mtime update");
        let times = fs::FileTimes::new().set_modified(modified);
        file.set_times(times).expect("set modified time");
    }
}

#[cfg(test)]
#[path = "index_cache/contract_tests.rs"]
mod contract_tests;
