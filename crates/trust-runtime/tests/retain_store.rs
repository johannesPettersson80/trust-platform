use std::env;

use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_runtime::retain::{FileRetainStore, RetainStore};
use trust_runtime::value::{ArrayValue, EnumValue, StructValue, Value};
use trust_runtime::RetainSnapshot;

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut path = env::temp_dir();
    let pid = std::process::id();
    path.push(format!("trust_runtime_retain_{pid}_{name}.bin"));
    path
}

#[test]
fn retain_store_roundtrip() {
    let mut registry = TypeRegistry::new();
    let phase_type = registry.register_enum(
        "Phase",
        TypeId::INT,
        vec![("Idle".into(), 0), ("Running".into(), 1)],
    );
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("Flag", Value::Bool(true));
    snapshot.insert("Count", Value::Int(42));
    snapshot.insert(
        "Array",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(vec![Value::Int(1), Value::Int(2)], vec![(1, 2)])
                .expect("valid raw retain array"),
        )),
    );
    snapshot.insert(
        "Struct",
        Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
            SmolStr::new("MyStruct"),
            [(SmolStr::new("FieldA"), Value::DInt(100))]
                .into_iter()
                .collect(),
        ))),
    );
    snapshot.insert(
        "Enum",
        Value::Enum(Box::new(
            EnumValue::new(&registry, phase_type, "Running").expect("valid retain enum"),
        )),
    );

    let path = temp_path("roundtrip");
    let store = FileRetainStore::new(&path);
    store.store(&snapshot).expect("store retain snapshot");

    let loaded = store.load().expect("load retain snapshot");
    assert_eq!(snapshot, loaded);

    let _ = std::fs::remove_file(path);
}

#[test]
fn retain_store_missing_file_returns_default() {
    let path = temp_path("missing");
    let _ = std::fs::remove_file(&path);
    let store = FileRetainStore::new(&path);
    let snapshot = store.load().expect("load missing retain snapshot");
    assert!(snapshot.values().is_empty());
}
