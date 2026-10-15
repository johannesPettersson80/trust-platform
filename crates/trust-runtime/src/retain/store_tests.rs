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
        entries
            .iter()
            .all(|name| !name.to_string_lossy().ends_with(".tmp")),
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

    assert_eq!(store.load().expect("load replaced snapshot"), snapshot(2));
    assert_no_temp_files(&path);
    remove_test_directory(&path);
}

const PHYSICAL_RETAIN_SOURCE: &str = r#"
TYPE
    Inner : STRUCT
        low AT %B0 : BYTE;
        high AT %B2 : BYTE;
    END_STRUCT;
    OuterType : STRUCT OVERLAP
        raw AT %B0 : DWORD;
        signed AT %B0 : DINT;
        inner AT %B0 : Inner;
    END_STRUCT;
END_TYPE
VAR_GLOBAL RETAIN outer : OuterType; END_VAR
PROGRAM Main END_PROGRAM
"#;

fn write_debug_lvalue(runtime: &mut Runtime, name: &str, value: Value) {
    let mut registry = runtime.registry().clone();
    let target = crate::harness::parse_debug_lvalue_with_abi(
        name,
        &mut registry,
        runtime.profile(),
        &[],
        runtime.prepared().storage_abi(),
    )
    .expect("debug lvalue");
    runtime.write_lvalue(&target, value, None).unwrap();
}

fn read_debug_lvalue(runtime: &mut Runtime, name: &str) -> Value {
    let mut registry = runtime.registry().clone();
    let target = crate::harness::parse_debug_lvalue_with_abi(
        name,
        &mut registry,
        runtime.profile(),
        &[],
        runtime.prepared().storage_abi(),
    )
    .expect("debug lvalue");
    runtime.read_lvalue(&target, None).unwrap()
}

#[test]
fn file_retain_v3_roundtrip_preserves_hidden_physical_bytes_through_warm_restart() {
    use crate::{harness::CompileSession, value::StorageAbi, RestartMode};

    let path = test_path("physical-v3-roundtrip");
    let mut runtime = CompileSession::from_source(PHYSICAL_RETAIN_SOURCE)
        .build_runtime()
        .unwrap();
    assert_eq!(
        runtime.prepared().storage_abi(),
        StorageAbi::UniformTemporal64
    );
    write_debug_lvalue(&mut runtime, "outer.raw", Value::DWord(0xFFCC_BBAA));
    let snapshot = runtime.retain_snapshot().unwrap();
    let Some(Value::Struct(value)) = snapshot.values().get("outer") else {
        panic!("retained physical root");
    };
    let checkpoint = value.encode_physical_checkpoint().unwrap().unwrap();
    for position in [0, 4] {
        let mut malformed = checkpoint.clone();
        malformed[position] = u8::MAX;
        assert!(
            (**value)
                .clone()
                .with_physical_checkpoint(&malformed)
                .is_err(),
            "corrupt checkpoint framing/ABI must be rejected"
        );
    }
    runtime.set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
    runtime.save_retain_store().unwrap();
    let bytes = fs::read(&path).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), RETAIN_VERSION);

    let mut restored = CompileSession::from_source(PHYSICAL_RETAIN_SOURCE)
        .build_runtime()
        .unwrap();
    restored.set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
    restored
        .restart_with_retain_store(RestartMode::Warm)
        .unwrap();
    assert_eq!(
        read_debug_lvalue(&mut restored, "outer.raw"),
        Value::DWord(0xFFCC_BBAA)
    );
    write_debug_lvalue(&mut restored, "outer.inner.low", Value::Byte(0x11));
    assert_eq!(
        read_debug_lvalue(&mut restored, "outer.raw"),
        Value::DWord(0xFFCC_BB11),
        "the unnamed middle byte must survive the durable restart"
    );
    remove_test_directory(&path);
}

#[test]
fn file_retain_v3_preserves_a_standalone_nested_physical_value() {
    use crate::{harness::CompileSession, RestartMode};

    let source = r#"
TYPE
    Inner : STRUCT
        low AT %B0 : BYTE;
        high AT %B2 : BYTE;
    END_STRUCT;
    OuterType : STRUCT OVERLAP
        raw AT %B0 : DWORD;
        inner AT %B0 : Inner;
    END_STRUCT;
END_TYPE
VAR_GLOBAL outer : OuterType; target : OuterType; END_VAR
VAR_GLOBAL RETAIN saved : Inner; END_VAR
PROGRAM Main END_PROGRAM
"#;
    let path = test_path("physical-v3-nested-standalone");
    let mut runtime = CompileSession::from_source(source).build_runtime().unwrap();
    write_debug_lvalue(&mut runtime, "outer.raw", Value::DWord(0x00CC_BBAA));
    let copied = read_debug_lvalue(&mut runtime, "outer.inner");
    write_debug_lvalue(&mut runtime, "saved", copied);
    let snapshot = runtime.retain_snapshot().unwrap();
    assert!(snapshot.values().contains_key("saved"));
    assert!(!snapshot.values().contains_key("outer"));
    runtime.set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
    runtime.save_retain_store().unwrap();

    let mut restored = CompileSession::from_source(source).build_runtime().unwrap();
    restored.set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
    restored
        .restart_with_retain_store(RestartMode::Warm)
        .unwrap();
    let copied = read_debug_lvalue(&mut restored, "saved");
    write_debug_lvalue(&mut restored, "target.inner", copied);
    assert_eq!(
        read_debug_lvalue(&mut restored, "target.raw"),
        Value::DWord(0x00CC_BBAA),
        "the retained child gap must survive without retaining its physical parent"
    );
    remove_test_directory(&path);
}

#[test]
fn retain_manager_persists_hidden_physical_only_changes() {
    use crate::harness::CompileSession;

    let source = r#"
TYPE
    Inner : STRUCT
        low AT %B0 : BYTE;
        high AT %B2 : BYTE;
    END_STRUCT;
    OuterType : STRUCT OVERLAP
        raw AT %B0 : DWORD;
        inner AT %B0 : Inner;
    END_STRUCT;
END_TYPE
VAR_GLOBAL outer : OuterType; END_VAR
VAR_GLOBAL RETAIN saved : Inner; END_VAR
PROGRAM Main END_PROGRAM
"#;
    let mut runtime = CompileSession::from_source(source).build_runtime().unwrap();
    write_debug_lvalue(&mut runtime, "outer.raw", Value::DWord(0x00CC_BBAA));
    let copied = read_debug_lvalue(&mut runtime, "outer.inner");
    write_debug_lvalue(&mut runtime, "saved", copied);
    let first = runtime.retain_snapshot().unwrap();

    write_debug_lvalue(&mut runtime, "outer.raw", Value::DWord(0x00CC_DDAA));
    let copied = read_debug_lvalue(&mut runtime, "outer.inner");
    write_debug_lvalue(&mut runtime, "saved", copied);
    let second = runtime.retain_snapshot().unwrap();
    assert_eq!(first, second, "logical named fields are unchanged");
    let (Some(Value::Struct(first_value)), Some(Value::Struct(second_value))) =
        (first.values().get("saved"), second.values().get("saved"))
    else {
        panic!("retained nested structure")
    };
    assert!(!first_value.physical_checkpoint_eq(second_value));

    let path = test_path("physical-only-dirty-save");
    let store = FileRetainStore::new(&path);
    let mut manager = RetainManager::default();
    manager.configure(
        Some(Box::new(FileRetainStore::new(&path))),
        Some(Duration::ZERO),
        Duration::ZERO,
    );
    manager.mark_dirty();
    manager.save_snapshot(first, Duration::ZERO).unwrap();
    manager.mark_dirty();
    manager
        .save_snapshot(second.clone(), Duration::from_nanos(1))
        .unwrap();

    let persisted = store.load().unwrap();
    let Some(Value::Struct(persisted)) = persisted.values().get("saved") else {
        panic!("persisted nested structure")
    };
    let Some(Value::Struct(second)) = second.values().get("saved") else {
        panic!("second nested structure")
    };
    assert!(persisted.physical_checkpoint_eq(second));
    remove_test_directory(&path);
}

#[test]
fn retain_manager_persists_signed_zero_representation_changes() {
    let path = test_path("signed-zero-dirty-save");
    let store = FileRetainStore::new(&path);
    let mut manager = RetainManager::default();
    manager.configure(
        Some(Box::new(FileRetainStore::new(&path))),
        Some(Duration::ZERO),
        Duration::ZERO,
    );

    let mut first = RetainSnapshot::default();
    first.insert("saved", Value::Real(-0.0));
    manager.mark_dirty();
    manager.save_snapshot(first, Duration::ZERO).unwrap();

    let mut second = RetainSnapshot::default();
    second.insert("saved", Value::Real(0.0));
    manager.mark_dirty();
    manager
        .save_snapshot(second, Duration::from_nanos(1))
        .unwrap();

    let persisted = store.load().unwrap();
    let Some(Value::Real(value)) = persisted.values().get("saved") else {
        panic!("persisted REAL")
    };
    assert_eq!(value.to_bits(), 0.0_f32.to_bits());
    remove_test_directory(&path);
}

#[test]
fn retain_manager_persists_snapshot_entry_order_changes() {
    let path = test_path("snapshot-entry-order-save");
    let store = FileRetainStore::new(&path);
    let mut manager = RetainManager::default();
    manager.configure(
        Some(Box::new(FileRetainStore::new(&path))),
        Some(Duration::ZERO),
        Duration::ZERO,
    );

    let mut first = RetainSnapshot::default();
    first.insert("first", Value::DInt(1));
    first.insert("second", Value::DInt(2));
    manager.mark_dirty();
    manager.save_snapshot(first, Duration::ZERO).unwrap();

    let mut reordered = RetainSnapshot::default();
    reordered.insert("second", Value::DInt(2));
    reordered.insert("first", Value::DInt(1));
    manager.mark_dirty();
    manager
        .save_snapshot(reordered, Duration::from_nanos(1))
        .unwrap();

    let persisted = store.load().unwrap();
    assert_eq!(
        persisted
            .values()
            .keys()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    remove_test_directory(&path);
}

#[test]
fn file_retain_rejects_incompatible_temporal_checkpoint_without_mutation() {
    use crate::{
        bytecode::BytecodeVersion,
        harness::CompileSession,
        value::{Duration, StorageAbi},
        RestartMode,
    };

    let plain = "TYPE OverlayType : STRUCT elapsed : TIME; raw : LINT; END_STRUCT; END_TYPE VAR_GLOBAL RETAIN overlay : OverlayType; END_VAR PROGRAM Main END_PROGRAM";
    let legacy_context = "TYPE OverlayType : STRUCT OVERLAP elapsed AT %B0 : TIME; raw AT %B0 : LINT; END_STRUCT; END_TYPE VAR_GLOBAL RETAIN overlay : OverlayType; END_VAR PROGRAM Main END_PROGRAM";
    let legacy_module = CompileSession::from_source(plain)
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .unwrap();
    let context = CompileSession::from_source(legacy_context);
    let mut old = context
        .build_runtime_from_bytecode(&legacy_module, None)
        .unwrap();
    assert_eq!(old.prepared().storage_abi(), StorageAbi::LegacyTemporal32);
    write_debug_lvalue(&mut old, "overlay.raw", Value::LInt(0x0000_0001_0000_0001));
    let path = test_path("physical-temporal-abi");
    old.set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
    old.save_retain_store().unwrap();

    let mut current = CompileSession::from_source(legacy_context)
        .build_runtime()
        .unwrap();
    assert_eq!(
        current.prepared().storage_abi(),
        StorageAbi::UniformTemporal64
    );
    current.set_current_time(Duration::from_millis(27));
    let before = current.storage().get_global("overlay").cloned().unwrap();
    current.set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
    assert!(current
        .restart_with_retain_store(RestartMode::Warm)
        .is_err());
    assert_eq!(current.storage().get_global("overlay"), Some(&before));
    assert_eq!(current.current_time(), Duration::from_millis(27));
    assert!(!current.faulted());
    remove_test_directory(&path);
}

#[test]
fn file_retain_rejects_changed_physical_geometry_without_mutation() {
    use crate::{harness::CompileSession, value::Duration, RestartMode};

    let old_source = "TYPE OverlayType : STRUCT OVERLAP whole AT %B0 : DWORD; low AT %B0 : WORD; END_STRUCT; END_TYPE VAR_GLOBAL RETAIN overlay : OverlayType; END_VAR PROGRAM Main END_PROGRAM";
    let changed_source = "TYPE OverlayType : STRUCT OVERLAP whole AT %B0 : DWORD; low AT %B2 : WORD; END_STRUCT; END_TYPE VAR_GLOBAL RETAIN overlay : OverlayType; END_VAR PROGRAM Main END_PROGRAM";
    let path = test_path("physical-geometry-change");
    let mut old = CompileSession::from_source(old_source)
        .build_runtime()
        .unwrap();
    write_debug_lvalue(&mut old, "overlay.whole", Value::DWord(0x1122_3344));
    old.set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
    old.save_retain_store().unwrap();

    let mut current = CompileSession::from_source(changed_source)
        .build_runtime()
        .unwrap();
    current.set_current_time(Duration::from_millis(19));
    let before = current.storage().get_global("overlay").cloned().unwrap();
    current.set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
    assert!(current
        .restart_with_retain_store(RestartMode::Warm)
        .is_err());
    assert_eq!(current.storage().get_global("overlay"), Some(&before));
    assert_eq!(current.current_time(), Duration::from_millis(19));
    assert!(!current.faulted());
    remove_test_directory(&path);
}

#[test]
fn file_retain_v1_and_v2_structures_remain_readable_as_logical_values() {
    fn old_struct_file(version: u16) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1_u32.to_le_bytes());
        encode_string(&mut payload, "saved");
        payload.push(ValueTag::Struct as u8);
        encode_string(&mut payload, "LegacyShape");
        payload.extend_from_slice(&1_u32.to_le_bytes());
        encode_string(&mut payload, "member");
        payload.push(ValueTag::DInt as u8);
        payload.extend_from_slice(&42_i32.to_le_bytes());

        let mut file = Vec::new();
        file.extend_from_slice(RETAIN_MAGIC);
        file.extend_from_slice(&version.to_le_bytes());
        if version == RETAIN_V1_VERSION {
            file.extend_from_slice(&payload);
        } else {
            file.extend_from_slice(&(payload.len() as u64).to_le_bytes());
            file.extend_from_slice(&payload);
            file.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
            file.extend_from_slice(RETAIN_TRAILER);
        }
        file
    }

    for version in [RETAIN_V1_VERSION, RETAIN_V2_VERSION] {
        let path = test_path(&format!("logical-compat-v{version}"));
        fs::create_dir_all(path.parent().expect("retain test parent")).unwrap();
        fs::write(&path, old_struct_file(version)).unwrap();
        let snapshot = FileRetainStore::new(&path).load().unwrap();
        let Some(Value::Struct(value)) = snapshot.values().get("saved") else {
            panic!("legacy structure value");
        };
        assert_eq!(value.fields().get("member"), Some(&Value::DInt(42)));
        assert_eq!(value.encode_physical_checkpoint().unwrap(), None);
        remove_test_directory(&path);
    }
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
        let faulting = FileRetainStore::with_file_ops(&path, Arc::new(FaultingFileOps { stage }));

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
        let faulting = FileRetainStore::with_file_ops(&path, Arc::new(FaultingFileOps { stage }));

        let error = faulting
            .load()
            .expect_err("read-side fault must be visible");
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

const RECOVERY_SOURCE: &str = r#"
VAR_GLOBAL RETAIN
    count : DINT := DINT#5;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

/// Delegates every retain file operation to one injected fault and counts the
/// store opens that reach it.
struct CountingFaultOps {
    inner: FaultingFileOps,
    opens: Arc<AtomicUsize>,
}

impl RetainFileOps for CountingFaultOps {
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.inner.create_dir_all(path)
    }

    fn create_temp(&self, path: &Path) -> io::Result<Box<dyn RetainTempFile>> {
        self.inner.create_temp(path)
    }

    fn open_read(&self, path: &Path) -> io::Result<Box<dyn Read + Send>> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        self.inner.open_read(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.inner.rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.inner.remove_file(path)
    }

    fn sync_parent(&self, path: &Path) -> io::Result<()> {
        self.inner.sync_parent(path)
    }
}

/// A running runtime with `count` = 77 after two completed cycles.
fn recovery_harness() -> crate::harness::TestHarness {
    let mut harness = crate::harness::TestHarness::from_source(RECOVERY_SOURCE)
        .expect("compile recovery runtime");
    harness.set_input("count", Value::DInt(77));
    for _ in 0..2 {
        let cycle = harness.cycle();
        assert!(
            cycle.errors.is_empty(),
            "live cycle failed: {:?}",
            cycle.errors
        );
    }
    harness
}

#[test]
fn warm_restart_store_faults_name_the_stage_and_keep_running_state() {
    use crate::RestartMode;

    for stage in [FaultStage::Open, FaultStage::Read] {
        // The folder must not carry the stage label: the error text includes the
        // path, so a label in the path would satisfy the assertion below by itself.
        let path = test_path(&format!("warm-{stage:?}"));
        FileRetainStore::new(&path)
            .store(&snapshot(123))
            .expect("seed a readable checkpoint");
        let opens = Arc::new(AtomicUsize::new(0));
        let faulting = FileRetainStore::with_file_ops(
            &path,
            Arc::new(CountingFaultOps {
                inner: FaultingFileOps { stage },
                opens: Arc::clone(&opens),
            }),
        );
        let mut harness = recovery_harness();
        harness
            .runtime_mut()
            .set_retain_store(Some(Box::new(faulting)), None);
        assert_eq!(harness.get_output("count"), Some(Value::DInt(77)));
        assert_eq!(harness.runtime().cycle_counter(), 2);

        let error = harness
            .restart_with_retain(RestartMode::Warm)
            .expect_err("warm restart must fail on a store fault");
        match &error {
            RuntimeError::RetainStore(message) => assert!(
                message.contains(stage.label()),
                "{stage:?}: expected {} in {message}",
                stage.label()
            ),
            other => panic!("{stage:?}: expected RetainStore, got {other:?}"),
        }
        assert_eq!(
            opens.load(Ordering::SeqCst),
            1,
            "{stage:?}: one load attempt"
        );
        assert_eq!(
            harness.get_output("count"),
            Some(Value::DInt(77)),
            "{stage:?}: retained value changed"
        );
        assert_eq!(
            harness.runtime().cycle_counter(),
            2,
            "{stage:?}: cycle counter changed"
        );
        assert!(
            !harness.runtime().faulted(),
            "{stage:?}: a refused restart must not latch a fault"
        );
        remove_test_directory(&path);
    }
}

#[test]
fn cold_restart_with_a_failing_store_opens_nothing_and_reinitializes_retained_values() {
    use crate::RestartMode;

    let path = test_path("cold-open-fault");
    FileRetainStore::new(&path)
        .store(&snapshot(123))
        .expect("seed a readable checkpoint");
    let opens = Arc::new(AtomicUsize::new(0));
    let faulting = FileRetainStore::with_file_ops(
        &path,
        Arc::new(CountingFaultOps {
            inner: FaultingFileOps {
                stage: FaultStage::Open,
            },
            opens: Arc::clone(&opens),
        }),
    );
    let mut harness = recovery_harness();
    harness
        .runtime_mut()
        .set_retain_store(Some(Box::new(faulting)), None);
    assert_eq!(harness.get_output("count"), Some(Value::DInt(77)));

    harness
        .restart_with_retain(RestartMode::Cold)
        .expect("cold restart must not read the retain store");
    assert_eq!(
        opens.load(Ordering::SeqCst),
        0,
        "cold restart must open the retain store zero times"
    );
    assert_eq!(
        harness.get_output("count"),
        Some(Value::DInt(5)),
        "cold restart initializes RETAIN variables from their declarations"
    );
    assert!(!harness.runtime().faulted());
    remove_test_directory(&path);
}

#[test]
fn rename_fault_before_warm_restart_leaves_the_last_good_checkpoint_to_restore() {
    use crate::RestartMode;

    // The warm restart only reads the store; rename is reached by the save that
    // precedes it, so the restart must restore the checkpoint that save preserved.
    let path = test_path("rename-then-warm");
    let standard = FileRetainStore::new(&path);
    standard
        .store(&snapshot(1))
        .expect("seed last good checkpoint");
    let last_good = fs::read(&path).expect("read last good checkpoint");
    let faulting = FileRetainStore::with_file_ops(
        &path,
        Arc::new(FaultingFileOps {
            stage: FaultStage::Rename,
        }),
    );
    let error = faulting
        .store(&snapshot(2))
        .expect_err("rename fault must fail the save");
    assert!(error.to_string().contains(FaultStage::Rename.label()));
    assert_eq!(
        fs::read(&path).expect("read preserved checkpoint"),
        last_good,
        "a rename fault must preserve the last good checkpoint"
    );

    let mut harness = recovery_harness();
    harness
        .runtime_mut()
        .set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
    assert_eq!(harness.get_output("count"), Some(Value::DInt(77)));
    harness
        .restart_with_retain(RestartMode::Warm)
        .expect("warm restart loads the last good checkpoint");
    assert_eq!(harness.get_output("count"), Some(Value::DInt(1)));
    assert!(!harness.runtime().faulted());
    remove_test_directory(&path);
}
