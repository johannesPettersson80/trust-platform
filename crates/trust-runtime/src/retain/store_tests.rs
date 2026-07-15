use std::io;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaultStage {
    CreateDir,
    CreateTemp,
    Write,
    Flush,
    FileSync,
    Rename,
    ParentSync,
    Open,
    Read,
}

impl FaultStage {
    fn label(self) -> &'static str {
        match self {
            Self::CreateDir => "create-dir fault",
            Self::CreateTemp => "create-temp fault",
            Self::Write => "write fault",
            Self::Flush => "flush fault",
            Self::FileSync => "file-sync fault",
            Self::Rename => "rename fault",
            Self::ParentSync => "parent-sync fault",
            Self::Open => "open fault",
            Self::Read => "read fault",
        }
    }
}

struct FaultingFileOps {
    stage: FaultStage,
}

impl FaultingFileOps {
    fn error(&self) -> io::Error {
        io::Error::other(self.stage.label())
    }
}

impl RetainFileOps for FaultingFileOps {
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        if self.stage == FaultStage::CreateDir {
            return Err(self.error());
        }
        StdRetainFileOps.create_dir_all(path)
    }

    fn create_temp(&self, path: &Path) -> io::Result<Box<dyn RetainTempFile>> {
        if self.stage == FaultStage::CreateTemp {
            return Err(self.error());
        }
        let inner = StdRetainFileOps.create_temp(path)?;
        Ok(Box::new(FaultingTempFile {
            inner,
            stage: self.stage,
        }))
    }

    fn open_read(&self, path: &Path) -> io::Result<Box<dyn Read + Send>> {
        if self.stage == FaultStage::Open {
            return Err(self.error());
        }
        let inner = StdRetainFileOps.open_read(path)?;
        if self.stage == FaultStage::Read {
            return Ok(Box::new(FaultingReader { inner }));
        }
        Ok(inner)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        if self.stage == FaultStage::Rename {
            return Err(self.error());
        }
        StdRetainFileOps.rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        StdRetainFileOps.remove_file(path)
    }

    fn sync_parent(&self, path: &Path) -> io::Result<()> {
        if self.stage == FaultStage::ParentSync {
            return Err(self.error());
        }
        StdRetainFileOps.sync_parent(path)
    }
}

struct FaultingTempFile {
    inner: Box<dyn RetainTempFile>,
    stage: FaultStage,
}

impl Write for FaultingTempFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.stage == FaultStage::Write {
            return Err(io::Error::other(self.stage.label()));
        }
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.stage == FaultStage::Flush {
            return Err(io::Error::other(self.stage.label()));
        }
        self.inner.flush()
    }
}

impl RetainTempFile for FaultingTempFile {
    fn sync_all(&self) -> io::Result<()> {
        if self.stage == FaultStage::FileSync {
            return Err(io::Error::other(self.stage.label()));
        }
        self.inner.sync_all()
    }
}

struct FaultingReader {
    inner: Box<dyn Read + Send>,
}

impl Read for FaultingReader {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        let _ = &self.inner;
        Err(io::Error::other("read fault"))
    }
}

fn test_path(name: &str) -> PathBuf {
    let sequence = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "trust-runtime-retain-store-{}-{sequence}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    directory.join("retain.bin")
}

fn snapshot(value: i32) -> RetainSnapshot {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("count", Value::DInt(value));
    snapshot
}

fn assert_no_temp_files(path: &Path) {
    let parent = path.parent().expect("retain test parent");
    let entries = fs::read_dir(parent)
        .expect("read retain test directory")
        .map(|entry| entry.expect("read retain test entry").file_name())
        .collect::<Vec<_>>();
    assert!(
        entries.iter().all(|name| !name.to_string_lossy().ends_with(".tmp")),
        "temporary retain files must be cleaned up: {entries:?}"
    );
}

fn remove_test_directory(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir_all(parent);
    }
}

#[test]
fn file_retain_store_replaces_snapshot_atomically() {
    let path = test_path("atomic-replace");
    let store = FileRetainStore::new(&path);
    store.store(&snapshot(1)).expect("store initial snapshot");
    store.store(&snapshot(2)).expect("replace snapshot");

    assert_eq!(
        store.load().expect("load replaced snapshot"),
        snapshot(2)
    );
    assert_no_temp_files(&path);
    remove_test_directory(&path);
}

#[test]
fn file_retain_store_pre_publish_failure_matrix_preserves_last_good_snapshot() {
    for stage in [
        FaultStage::CreateDir,
        FaultStage::CreateTemp,
        FaultStage::Write,
        FaultStage::Flush,
        FaultStage::FileSync,
        FaultStage::Rename,
    ] {
        let path = test_path(stage.label());
        let standard = FileRetainStore::new(&path);
        standard
            .store(&snapshot(1))
            .expect("seed last good snapshot");
        let last_good = fs::read(&path).expect("read last good snapshot");
        let faulting = FileRetainStore::with_file_ops(
            &path,
            Arc::new(FaultingFileOps { stage }),
        );

        let error = faulting
            .store(&snapshot(2))
            .expect_err("pre-publish fault must fail the save");
        assert!(
            error.to_string().contains(stage.label()),
            "expected {} in {error}",
            stage.label()
        );
        assert_eq!(
            fs::read(&path).expect("read preserved snapshot"),
            last_good,
            "{stage:?} must preserve the last good snapshot"
        );
        assert_eq!(
            standard.load().expect("load preserved snapshot"),
            snapshot(1)
        );
        assert_no_temp_files(&path);
        remove_test_directory(&path);
    }
}

#[test]
fn file_retain_store_parent_sync_failure_is_visible_after_publish() {
    let path = test_path("parent-sync");
    let standard = FileRetainStore::new(&path);
    standard
        .store(&snapshot(1))
        .expect("seed last good snapshot");
    let faulting = FileRetainStore::with_file_ops(
        &path,
        Arc::new(FaultingFileOps {
            stage: FaultStage::ParentSync,
        }),
    );

    let error = faulting
        .store(&snapshot(2))
        .expect_err("parent sync failure must be visible");
    assert!(error.to_string().contains("parent-sync fault"));
    assert_eq!(
        standard.load().expect("load published snapshot"),
        snapshot(2),
        "rename publishes before the parent directory sync is attempted"
    );
    assert_no_temp_files(&path);
    remove_test_directory(&path);
}

#[test]
fn file_retain_store_open_and_read_failures_are_visible() {
    for stage in [FaultStage::Open, FaultStage::Read] {
        let path = test_path(stage.label());
        FileRetainStore::new(&path)
            .store(&snapshot(1))
            .expect("seed readable snapshot");
        let faulting = FileRetainStore::with_file_ops(
            &path,
            Arc::new(FaultingFileOps { stage }),
        );

        let error = faulting.load().expect_err("read-side fault must be visible");
        assert!(
            error.to_string().contains(stage.label()),
            "expected {} in {error}",
            stage.label()
        );
        remove_test_directory(&path);
    }
}

struct FailOnceStore {
    calls: Arc<AtomicUsize>,
}

impl RetainStore for FailOnceStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Ok(RetainSnapshot::default())
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(RuntimeError::RetainStore("synthetic save failure".into()));
        }
        Ok(())
    }
}

#[test]
fn retain_manager_retries_failed_save_without_a_new_dirty_mark() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut manager = RetainManager::default();
    manager.configure(
        Some(Box::new(FailOnceStore {
            calls: Arc::clone(&calls),
        })),
        Some(Duration::ZERO),
        Duration::ZERO,
    );
    manager.mark_dirty();

    manager
        .save_snapshot(snapshot(7), Duration::ZERO)
        .expect_err("first save must expose the injected failure");
    assert!(
        manager.should_save(Duration::ZERO),
        "a failed save must remain eligible for retry"
    );

    manager
        .save_snapshot(snapshot(7), Duration::ZERO)
        .expect("retry must succeed without another dirty mark");
    assert!(!manager.should_save(Duration::ZERO));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
