use std::env;

use smol_str::SmolStr;
use trust_runtime::harness::TestHarness;
use trust_runtime::retain::{FileRetainStore, RetainStore};
use trust_runtime::value::{ArrayValue, Value};
use trust_runtime::{RestartMode, RetainSnapshot};

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

fn rewrite_v2_payload_value(path: &std::path::Path, before: &[u8], after: &[u8]) {
    assert_eq!(before.len(), after.len());
    let mut bytes = std::fs::read(path).expect("read retain image");
    assert_eq!(&bytes[..4], b"STRN");
    assert_eq!(u16::from_le_bytes(bytes[4..6].try_into().unwrap()), 2);
    let payload_len = u64::from_le_bytes(bytes[6..14].try_into().unwrap()) as usize;
    let payload_start = 14;
    let payload_end = payload_start + payload_len;
    let matches = bytes[payload_start..payload_end]
        .windows(before.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == before).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one finite payload value");
    let value_start = payload_start + matches[0];
    bytes[value_start..value_start + after.len()].copy_from_slice(after);
    let checksum = crc32fast::hash(&bytes[payload_start..payload_end]);
    bytes[payload_end..payload_end + 4].copy_from_slice(&checksum.to_le_bytes());
    std::fs::write(path, bytes).expect("write checksum-valid retain image");
}

#[test]
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
fn retain_store_rejects_nonfinite_values_without_replacing_last_good_snapshot() {
    let path = temp_path("nonfinite_save");
    let _ = std::fs::remove_file(&path);
    let store = FileRetainStore::new(&path);
    store
        .store(&count_snapshot(Value::DInt(42)))
        .expect("store last good retain snapshot");
    let last_good = std::fs::read(&path).expect("read last good retain snapshot");
    let nested = Value::Array(Box::new(
        ArrayValue::from_untyped_parts(vec![Value::Real(f32::NAN)], vec![(0, 0)])
            .expect("valid raw retain array"),
    ));
    let cases = [
        Value::Real(f32::NAN),
        Value::Real(f32::INFINITY),
        Value::Real(f32::NEG_INFINITY),
        Value::LReal(f64::NAN),
        Value::LReal(f64::INFINITY),
        Value::LReal(f64::NEG_INFINITY),
        nested,
    ];

    for value in cases {
        let err = store
            .store(&count_snapshot(value))
            .expect_err("non-finite retain save must fail");
        assert!(
            err.to_string().contains("non-finite"),
            "expected non-finite retain error, got {err}"
        );
        assert_eq!(
            std::fs::read(&path).expect("read retained last good snapshot"),
            last_good,
            "a rejected save must not replace the last good snapshot"
        );
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn retain_nonfinite_snapshot_fails_warm_reload_without_partial_apply() {
    struct Case {
        name: &'static str,
        type_name: &'static str,
        initializer: &'static str,
        persisted: Value,
        current: Value,
        before: Vec<u8>,
        after: Vec<u8>,
    }

    let cases = [
        Case {
            name: "real_nan",
            type_name: "REAL",
            initializer: "REAL#2.0",
            persisted: Value::Real(1.25),
            current: Value::Real(7.25),
            before: 1.25_f32.to_le_bytes().to_vec(),
            after: f32::NAN.to_le_bytes().to_vec(),
        },
        Case {
            name: "real_pos_inf",
            type_name: "REAL",
            initializer: "REAL#2.0",
            persisted: Value::Real(1.25),
            current: Value::Real(7.25),
            before: 1.25_f32.to_le_bytes().to_vec(),
            after: f32::INFINITY.to_le_bytes().to_vec(),
        },
        Case {
            name: "real_neg_inf",
            type_name: "REAL",
            initializer: "REAL#2.0",
            persisted: Value::Real(1.25),
            current: Value::Real(7.25),
            before: 1.25_f32.to_le_bytes().to_vec(),
            after: f32::NEG_INFINITY.to_le_bytes().to_vec(),
        },
        Case {
            name: "lreal_nan",
            type_name: "LREAL",
            initializer: "LREAL#2.0",
            persisted: Value::LReal(1.25),
            current: Value::LReal(7.25),
            before: 1.25_f64.to_le_bytes().to_vec(),
            after: f64::NAN.to_le_bytes().to_vec(),
        },
        Case {
            name: "lreal_pos_inf",
            type_name: "LREAL",
            initializer: "LREAL#2.0",
            persisted: Value::LReal(1.25),
            current: Value::LReal(7.25),
            before: 1.25_f64.to_le_bytes().to_vec(),
            after: f64::INFINITY.to_le_bytes().to_vec(),
        },
        Case {
            name: "lreal_neg_inf",
            type_name: "LREAL",
            initializer: "LREAL#2.0",
            persisted: Value::LReal(1.25),
            current: Value::LReal(7.25),
            before: 1.25_f64.to_le_bytes().to_vec(),
            after: f64::NEG_INFINITY.to_le_bytes().to_vec(),
        },
    ];

    for case in cases {
        let source = format!(
            r#"
VAR_GLOBAL RETAIN
    sentinel : DINT := DINT#1;
    reading : {} := {};
END_VAR

PROGRAM Main
END_PROGRAM
"#,
            case.type_name, case.initializer
        );
        let mut harness = TestHarness::from_source(&source).expect("compile retain harness");
        harness.set_input("sentinel", Value::DInt(77));
        harness.set_input("reading", case.current.clone());

        let path = temp_path(case.name);
        let _ = std::fs::remove_file(&path);
        let store = FileRetainStore::new(&path);
        let mut persisted = RetainSnapshot::default();
        persisted.insert("sentinel", Value::DInt(123));
        persisted.insert("reading", case.persisted);
        store
            .store(&persisted)
            .expect("store structurally valid finite retain snapshot");
        rewrite_v2_payload_value(&path, &case.before, &case.after);

        harness
            .runtime_mut()
            .set_retain_store(Some(Box::new(FileRetainStore::new(&path))), None);
        let err = harness
            .restart_with_retain(RestartMode::Warm)
            .expect_err("checksum-valid non-finite retain load must fail");
        assert!(
            err.to_string().contains("non-finite"),
            "expected non-finite retain error, got {err}"
        );
        assert_eq!(harness.get_output("sentinel"), Some(Value::DInt(77)));
        assert_eq!(harness.get_output("reading"), Some(case.current));
        let _ = std::fs::remove_file(path);
    }
}

#[test]
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
fn retain_bounded_string_is_canonicalized_on_load() {
    let source = r#"
VAR_GLOBAL RETAIN
    label : STRING[3] := 'OK';
END_VAR

PROGRAM Main
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let debug = harness.runtime_mut().enable_debug();
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("label", Value::String("TOO-LONG".into()));

    harness
        .runtime_mut()
        .apply_retain_snapshot(&snapshot)
        .expect("overlong bounded STRING retain value should be canonicalized");

    assert_eq!(
        harness.get_output("label"),
        Some(Value::String("TOO".into()))
    );
    let events = debug.drain_runtime_events();
    assert!(
        events
            .iter()
            .any(|event| format!("{event:?}").contains("RetainMigrationApplied")),
        "expected retain migration event, got {events:?}"
    );
}

#[test]
fn retain_subrange_rejects_out_of_range_value_on_load() {
    let source = r#"
VAR_GLOBAL RETAIN
    limited : INT(0..10) := INT#0;
END_VAR

PROGRAM Main
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("limited", Value::Int(100));

    let err = harness
        .runtime_mut()
        .apply_retain_snapshot(&snapshot)
        .expect_err("out-of-range subrange retain value must be rejected");
    assert!(
        err.to_string().contains("outside declared subrange 0..10"),
        "expected subrange retain rejection, got {err}"
    );
    assert_eq!(harness.get_output("limited"), Some(Value::Int(0)));
}

#[test]
fn retain_snapshot_migration_failure_does_not_partially_apply_earlier_values() {
    let source = r#"
VAR_GLOBAL RETAIN
    accepted_first : DINT := DINT#1;
    rejected_later : INT(0..10) := INT#2;
END_VAR

PROGRAM Main
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    harness.set_input("accepted_first", Value::DInt(70));
    harness.set_input("rejected_later", Value::Int(7));

    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("accepted_first", Value::DInt(111));
    snapshot.insert("rejected_later", Value::Int(100));

    let err = harness
        .runtime_mut()
        .apply_retain_snapshot(&snapshot)
        .expect_err("one invalid retained value must reject the complete snapshot");
    assert!(
        err.to_string().contains("outside declared subrange 0..10"),
        "expected subrange retain rejection, got {err}"
    );
    assert_eq!(
        harness.get_output("accepted_first"),
        Some(Value::DInt(70)),
        "a value validated before the failing entry must not leak from the rejected snapshot"
    );
    assert_eq!(
        harness.get_output("rejected_later"),
        Some(Value::Int(7)),
        "the invalid retained target must preserve its pre-load value"
    );
}

#[test]
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
