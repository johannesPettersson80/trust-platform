use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CACHE_ID: AtomicU64 = AtomicU64::new(0);

struct CacheProject {
    root: PathBuf,
}

impl CacheProject {
    fn new(label: &str) -> Self {
        let id = NEXT_CACHE_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-lsp-index-cache-contract-{}-{label}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create index cache contract project");
        Self { root }
    }

    fn write(&self, relative: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create index cache fixture parent");
        }
        fs::write(&path, contents).expect("write index cache fixture");
        path
    }

    fn cache_dir(&self) -> PathBuf {
        self.root.join(".trust-lsp/index-cache")
    }
}

impl Drop for CacheProject {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

fn entry_count(cache: &IndexCache) -> usize {
    cache.entries.len()
}

#[test]
fn absent_cache_loads_empty_current_schema() {
    let project = CacheProject::new("absent");
    let cache = IndexCache::load_or_default(&project.cache_dir());
    assert_eq!(cache.version, CACHE_VERSION);
    assert_eq!(entry_count(&cache), 0);
}

#[test]
fn malformed_cache_loads_empty_current_schema() {
    let project = CacheProject::new("malformed");
    project.write(".trust-lsp/index-cache/index.json", b"{not-json");
    let cache = IndexCache::load_or_default(&project.cache_dir());
    assert_eq!(cache.version, CACHE_VERSION);
    assert_eq!(entry_count(&cache), 0);
}

#[test]
fn unsupported_cache_version_discards_every_entry() {
    let project = CacheProject::new("old-version");
    project.write(
        ".trust-lsp/index-cache/index.json",
        br#"{
  "version": 99,
  "entries": {
    "stale.st": {
      "hash": 1,
      "size": 1,
      "mtime": 1,
      "content": "x"
    }
  }
}"#,
    );
    let cache = IndexCache::load_or_default(&project.cache_dir());
    assert_eq!(cache.version, CACHE_VERSION);
    assert_eq!(entry_count(&cache), 0);
}

#[test]
fn saved_cache_is_one_complete_reloadable_document() {
    let project = CacheProject::new("saved-document");
    let source = project.write("main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let mut cache = IndexCache::default();
    cache.update_from_content(&source, fs::read_to_string(&source).unwrap());
    cache.save(&project.cache_dir()).expect("save cache");

    let serialized =
        fs::read_to_string(project.cache_dir().join(CACHE_FILE)).expect("read cache document");
    let value: serde_json::Value =
        serde_json::from_str(&serialized).expect("cache is complete JSON");
    assert_eq!(value["version"].as_u64(), Some(CACHE_VERSION as u64));
    assert_eq!(value["entries"].as_object().map(|map| map.len()), Some(1));

    let reloaded = IndexCache::load_or_default(&project.cache_dir());
    assert_eq!(
        reloaded.content_for_path(&source),
        Some("PROGRAM Main\nEND_PROGRAM\n")
    );
}

#[test]
fn cache_retains_multiple_independent_source_entries() {
    let project = CacheProject::new("multiple");
    let first = project.write("first.st", "PROGRAM First\nEND_PROGRAM\n");
    let second = project.write("second.st", "PROGRAM Second\nEND_PROGRAM\n");
    let mut cache = IndexCache::default();
    cache.update_from_content(&first, fs::read_to_string(&first).unwrap());
    cache.update_from_content(&second, fs::read_to_string(&second).unwrap());

    assert_eq!(entry_count(&cache), 2);
    assert_eq!(
        cache.content_for_path(&first),
        Some("PROGRAM First\nEND_PROGRAM\n")
    );
    assert_eq!(
        cache.content_for_path(&second),
        Some("PROGRAM Second\nEND_PROGRAM\n")
    );
}

#[test]
fn updating_same_content_refreshes_without_duplicate_entry() {
    let project = CacheProject::new("same-content");
    let source = project.write("main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let mut cache = IndexCache::default();
    let first_hash = cache.update_from_content(&source, "PROGRAM Main\nEND_PROGRAM\n".to_string());
    let second_hash = cache.update_from_content(&source, "PROGRAM Main\nEND_PROGRAM\n".to_string());

    assert_eq!(first_hash, second_hash);
    assert_eq!(entry_count(&cache), 1);
    assert_eq!(
        cache.content_for_path(&source),
        Some("PROGRAM Main\nEND_PROGRAM\n")
    );
}

#[test]
fn updating_changed_content_replaces_entry_and_hash() {
    let project = CacheProject::new("changed-content");
    let source = project.write("main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let mut cache = IndexCache::default();
    let first_hash = cache.update_from_content(&source, "PROGRAM Main\nEND_PROGRAM\n".to_string());
    project.write(
        "main.st",
        "PROGRAM Main\nVAR x : INT; END_VAR\nEND_PROGRAM\n",
    );
    let second_hash = cache.update_from_content(
        &source,
        "PROGRAM Main\nVAR x : INT; END_VAR\nEND_PROGRAM\n".to_string(),
    );

    assert_ne!(first_hash, second_hash);
    assert_eq!(entry_count(&cache), 1);
    assert_eq!(
        cache.content_for_path(&source),
        Some("PROGRAM Main\nVAR x : INT; END_VAR\nEND_PROGRAM\n")
    );
}

#[test]
fn deleted_source_never_produces_cache_hit() {
    let project = CacheProject::new("deleted");
    let source = project.write("main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let mut cache = IndexCache::default();
    cache.update_from_content(&source, "PROGRAM Main\nEND_PROGRAM\n".to_string());
    fs::remove_file(&source).expect("delete source fixture");
    assert!(cache.content_for_path(&source).is_none());
}

#[test]
fn non_utf8_source_never_produces_cache_hit() {
    let project = CacheProject::new("non-utf8");
    let source = project.write("main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let mut cache = IndexCache::default();
    cache.update_from_content(&source, "PROGRAM Main\nEND_PROGRAM\n".to_string());
    project.write("main.st", [0xff, 0xfe, 0xfd]);
    assert!(cache.content_for_path(&source).is_none());
}

#[test]
fn remove_path_removes_only_selected_identity() {
    let project = CacheProject::new("remove");
    let first = project.write("first.st", "PROGRAM First\nEND_PROGRAM\n");
    let second = project.write("second.st", "PROGRAM Second\nEND_PROGRAM\n");
    let mut cache = IndexCache::default();
    cache.update_from_content(&first, fs::read_to_string(&first).unwrap());
    cache.update_from_content(&second, fs::read_to_string(&second).unwrap());
    cache.remove_path(&first);

    assert!(cache.content_for_path(&first).is_none());
    assert!(cache.content_for_path(&second).is_some());
    assert_eq!(entry_count(&cache), 1);
}

#[test]
fn retain_paths_removes_every_unselected_identity() {
    let project = CacheProject::new("retain");
    let first = project.write("first.st", "PROGRAM First\nEND_PROGRAM\n");
    let second = project.write("second.st", "PROGRAM Second\nEND_PROGRAM\n");
    let third = project.write("third.st", "PROGRAM Third\nEND_PROGRAM\n");
    let mut cache = IndexCache::default();
    for path in [&first, &second, &third] {
        cache.update_from_content(path, fs::read_to_string(path).unwrap());
    }
    cache.retain_paths(&[first.clone(), third.clone()]);

    assert!(cache.content_for_path(&first).is_some());
    assert!(cache.content_for_path(&second).is_none());
    assert!(cache.content_for_path(&third).is_some());
    assert_eq!(entry_count(&cache), 2);
}

#[test]
fn lexical_path_aliases_share_one_cache_identity() {
    let project = CacheProject::new("path-alias");
    fs::create_dir_all(project.root.join("nested")).expect("create alias parent");
    let source = project.write("main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let alias = project.root.join("nested/../main.st");
    let mut cache = IndexCache::default();
    cache.update_from_content(&alias, "PROGRAM Main\nEND_PROGRAM\n".to_string());
    cache.update_from_content(&source, "PROGRAM Main\nEND_PROGRAM\n".to_string());

    assert_eq!(entry_count(&cache), 1);
    assert_eq!(
        cache.content_for_path(&source),
        Some("PROGRAM Main\nEND_PROGRAM\n")
    );
}

#[test]
fn retain_paths_accepts_normalized_alias_of_cached_path() {
    let project = CacheProject::new("retain-alias");
    fs::create_dir_all(project.root.join("nested")).expect("create alias parent");
    let source = project.write("main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let alias = project.root.join("nested/../main.st");
    let mut cache = IndexCache::default();
    cache.update_from_content(&source, "PROGRAM Main\nEND_PROGRAM\n".to_string());
    cache.retain_paths(&[alias]);

    assert_eq!(entry_count(&cache), 1);
    assert!(cache.content_for_path(&source).is_some());
}

#[test]
fn content_hash_is_stable_and_content_sensitive() {
    assert_eq!(hash_content("same"), hash_content("same"));
    assert_ne!(hash_content("same"), hash_content("different"));
    assert_ne!(hash_content("A"), hash_content("a"));
}

#[test]
fn metadata_signature_requires_existing_regular_input() {
    let project = CacheProject::new("metadata");
    let source = project.write("main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let (size, _) = metadata_signature(&source).expect("source metadata");
    assert_eq!(size, fs::read(&source).unwrap().len() as u64);
    assert!(metadata_signature(&project.root.join("missing.st")).is_none());
}

#[test]
fn failed_save_does_not_mutate_in_memory_entries() {
    let project = CacheProject::new("save-failure");
    let source = project.write("main.st", "PROGRAM Main\nEND_PROGRAM\n");
    let blocker = project.write("not-a-directory", "file");
    let mut cache = IndexCache::default();
    cache.update_from_content(&source, "PROGRAM Main\nEND_PROGRAM\n".to_string());

    assert!(cache.save(&blocker.join("cache")).is_err());
    assert_eq!(entry_count(&cache), 1);
    assert_eq!(
        cache.content_for_path(&source),
        Some("PROGRAM Main\nEND_PROGRAM\n")
    );
}
