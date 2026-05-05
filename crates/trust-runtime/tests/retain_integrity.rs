use std::env;

use smol_str::SmolStr;
use trust_runtime::harness::TestHarness;
use trust_runtime::retain::{FileRetainStore, RetainStore};
use trust_runtime::value::Value;
use trust_runtime::RetainSnapshot;

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut path = env::temp_dir();
    let pid = std::process::id();
    path.push(format!("trust_runtime_retain_integrity_{pid}_{name}.bin"));
    path
}

fn count_snapshot(value: Value) -> RetainSnapshot {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("count", value);
    snapshot
}

fn holder_snapshot(fields: impl IntoIterator<Item = (SmolStr, Value)>) -> RetainSnapshot {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert(
        "holder",
        Value::Struct(std::sync::Arc::new(
            trust_runtime::value::StructValue::from_untyped_parts(
                SmolStr::new("HolderT"),
                fields.into_iter().collect(),
            ),
        )),
    );
    snapshot
}

fn legacy_v1_count_snapshot_bytes(value: i32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"STRN");
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&5u32.to_le_bytes());
    bytes.extend_from_slice(b"count");
    bytes.push(4);
    bytes.extend_from_slice(&value.to_le_bytes());
    bytes
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn retain_store_rejects_trailing_garbage() {
    let path = temp_path("trailing");
    let _ = std::fs::remove_file(&path);
    let store = FileRetainStore::new(&path);
    store
        .store(&count_snapshot(Value::DInt(42)))
        .expect("store retain snapshot");
    let mut bytes = std::fs::read(&path).expect("read retain bytes");
    bytes.extend_from_slice(b"garbage");
    std::fs::write(&path, bytes).expect("append trailing garbage");

    let err = store
        .load()
        .expect_err("retain load must reject trailing garbage");
    assert!(
        err.to_string().contains("trailing") || err.to_string().contains("corrupt"),
        "expected trailing/corruption error, got {err}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn retain_store_rejects_payload_mutation() {
    let path = temp_path("mutation");
    let _ = std::fs::remove_file(&path);
    let store = FileRetainStore::new(&path);
    store
        .store(&count_snapshot(Value::Bool(true)))
        .expect("store retain snapshot");
    let mut bytes = std::fs::read(&path).expect("read retain bytes");
    let last = bytes.last_mut().expect("retain payload byte");
    *last ^= 0x01;
    std::fs::write(&path, bytes).expect("write mutated retain bytes");

    let err = store
        .load()
        .expect_err("retain load must reject mutated payload");
    assert!(
        err.to_string().contains("checksum") || err.to_string().contains("corrupt"),
        "expected checksum/corruption error, got {err}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn retain_store_loads_legacy_v1_snapshot() {
    let path = temp_path("legacy_v1");
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, legacy_v1_count_snapshot_bytes(17)).expect("write v1 retain bytes");
    let store = FileRetainStore::new(&path);

    let loaded = store.load().expect("load legacy v1 retain snapshot");
    assert_eq!(loaded.values().get("count"), Some(&Value::DInt(17)));
    let _ = std::fs::remove_file(path);
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn retain_orphan_global_emits_runtime_event() {
    let source = r#"
VAR_GLOBAL RETAIN
    count : DINT := 0;
END_VAR

PROGRAM Main
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let debug = harness.runtime_mut().enable_debug();
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("old_count", Value::DInt(7));

    harness
        .runtime_mut()
        .apply_retain_snapshot(&snapshot)
        .expect("orphan retained globals should be dropped with evidence");

    let events = debug.drain_runtime_events();
    assert!(
        events
            .iter()
            .any(|event| format!("{event:?}").contains("RetainOrphanDropped")),
        "expected retain orphan event, got {events:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn retain_scalar_widening_migrates_with_runtime_event() {
    let source = r#"
VAR_GLOBAL RETAIN
    count : DINT := 0;
END_VAR

PROGRAM Main
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let debug = harness.runtime_mut().enable_debug();

    harness
        .runtime_mut()
        .apply_retain_snapshot(&count_snapshot(Value::Int(7)))
        .expect("INT retained value should migrate to DINT");

    assert_eq!(harness.get_output("count"), Some(Value::DInt(7)));
    let events = debug.drain_runtime_events();
    assert!(
        events
            .iter()
            .any(|event| format!("{event:?}").contains("RetainMigrationApplied")),
        "expected retain migration event, got {events:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn retain_struct_added_field_uses_declared_default_with_migration_event() {
    let source = r#"
TYPE HolderT : STRUCT
    count : DINT;
    added : DINT := 5;
END_STRUCT;
END_TYPE

VAR_GLOBAL RETAIN
    holder : HolderT;
END_VAR

PROGRAM Main
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let debug = harness.runtime_mut().enable_debug();
    harness
        .runtime_mut()
        .apply_retain_snapshot(&holder_snapshot([(SmolStr::new("count"), Value::DInt(11))]))
        .expect("struct added field should migrate with declared default");

    let Some(Value::Struct(holder)) = harness.get_output("holder") else {
        panic!("expected retained struct holder");
    };
    assert_eq!(holder.fields().get("count"), Some(&Value::DInt(11)));
    assert_eq!(holder.fields().get("added"), Some(&Value::DInt(5)));
    let events = debug.drain_runtime_events();
    assert!(
        events
            .iter()
            .any(|event| format!("{event:?}").contains("RetainMigrationApplied")),
        "expected retain migration event, got {events:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn retain_struct_removed_field_drops_with_migration_event() {
    let source = r#"
TYPE HolderT : STRUCT
    count : DINT;
END_STRUCT;
END_TYPE

VAR_GLOBAL RETAIN
    holder : HolderT;
END_VAR

PROGRAM Main
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let debug = harness.runtime_mut().enable_debug();

    harness
        .runtime_mut()
        .apply_retain_snapshot(&holder_snapshot([
            (SmolStr::new("count"), Value::DInt(1)),
            (SmolStr::new("removed_field"), Value::DInt(7)),
        ]))
        .expect("struct removed field should be dropped with migration evidence");
    let Some(Value::Struct(holder)) = harness.get_output("holder") else {
        panic!("expected retained struct holder");
    };
    assert_eq!(holder.fields().get("count"), Some(&Value::DInt(1)));
    assert!(
        !holder.fields().contains_key("removed_field"),
        "removed field must not remain in retained struct: {holder:?}"
    );
    let events = debug.drain_runtime_events();
    assert!(
        events
            .iter()
            .any(|event| format!("{event:?}").contains("RetainMigrationApplied")),
        "expected retain migration event, got {events:?}"
    );
}
