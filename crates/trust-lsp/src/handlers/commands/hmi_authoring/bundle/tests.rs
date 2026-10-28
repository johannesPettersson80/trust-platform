use super::*;
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "trust-hmi-bundle-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn unsaved_assets_replace_saved_and_include_new_files() {
    let project = Project::new();
    let folder = project.path().join("hmi/pages");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("main.toml"), "saved").unwrap();
    let open = BTreeMap::from([
        (folder.join("main.toml"), "edited".into()),
        (folder.join("new.toml"), "new page".into()),
    ]);
    let bundle = read_edited(project.path(), &open).unwrap();
    assert_eq!(bundle.files["hmi/pages/main.toml"], b"edited");
    assert_eq!(bundle.files["hmi/pages/new.toml"], b"new page");
    assert_eq!(std::fs::read(folder.join("main.toml")).unwrap(), b"saved");
    assert!(!folder.join("new.toml").exists());
}

#[test]
fn unsaved_dependency_removal_does_not_keep_saved_library_assets() {
    let project = Project::new();
    std::fs::create_dir_all(project.path().join("src")).unwrap();
    let library = project.path().join("old");
    std::fs::create_dir_all(library.join("src")).unwrap();
    std::fs::create_dir_all(library.join("hmi/pairs")).unwrap();
    std::fs::write(library.join("hmi/pairs/pump.toml"), "old asset").unwrap();
    let manifest = project.path().join("trust-lsp.toml");
    std::fs::write(&manifest, "[dependencies]\nOld = { path = 'old' }\n").unwrap();
    let open = BTreeMap::from([(manifest, "[dependencies]\n".into())]);
    assert!(read_edited(project.path(), &open).unwrap().files.is_empty());
}

#[test]
fn snapshot_limits_apply_to_replacements_without_double_counting() {
    let mut bundle = HmiBundle::default();
    insert(&mut bundle, "hmi/a".into(), vec![0; MAX_BYTES]).unwrap();
    insert(&mut bundle, "hmi/a".into(), vec![1; MAX_BYTES]).unwrap();
    assert!(insert(&mut bundle, "hmi/b".into(), vec![0]).is_err());
}
