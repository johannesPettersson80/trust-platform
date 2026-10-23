use super::TemporalStorage;
use crate::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use crate::types::Type;

#[test]
fn changing_temporal_storage_rechecks_relative_layout_without_changing_source() {
    let source =
        "TYPE Packet : STRUCT stamp AT %B0 : TIME; tail AT %B4 : INT; END_STRUCT; END_TYPE";
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.into());
    let overlap = |db: &Database| {
        db.diagnostics(file).iter().any(|diagnostic| {
            diagnostic.is_error()
                && diagnostic
                    .message
                    .contains("relative structure fields overlap")
        })
    };
    assert_eq!(db.temporal_storage(), TemporalStorage::Uniform64);
    assert!(overlap(&db), "new TIME occupies bytes zero through seven");
    db.set_temporal_storage(TemporalStorage::Legacy32);
    assert!(!overlap(&db), "old TIME occupies bytes zero through three");
    assert_eq!(db.source_text(file).as_str(), source);
    db.set_temporal_storage(TemporalStorage::Uniform64);
    assert!(
        overlap(&db),
        "cached analysis must observe the selected ABI again"
    );
    assert_eq!(db.source_text(file).as_str(), source);
}

#[test]
fn temporal_sizeof_array_bounds_follow_the_selected_cached_analysis_abi() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        "TYPE Bytes : ARRAY[0..SIZEOF(TIME)-1] OF BYTE; END_TYPE".into(),
    );
    for (abi, upper) in [
        (TemporalStorage::Uniform64, 7),
        (TemporalStorage::Legacy32, 3),
        (TemporalStorage::Uniform64, 7),
    ] {
        db.set_temporal_storage(abi);
        let diagnostics = db.diagnostics(file);
        assert!(
            !diagnostics.iter().any(|diagnostic| diagnostic.is_error()),
            "{diagnostics:?}"
        );
        let symbols = db.file_symbols(file);
        let symbol = symbols
            .get(symbols.lookup("Bytes").expect("type declaration"))
            .expect("declared symbol");
        let resolved = symbols.resolve_alias_type(symbol.type_id);
        let Some(Type::Array { dimensions, .. }) = symbols.type_by_id(resolved) else {
            panic!("expected array type")
        };
        assert_eq!(dimensions.as_slice(), &[(0, upper)]);
    }
}

#[test]
fn constant_sizeof_located_structure_includes_gaps_and_temporal_extent() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, "TYPE Packet : STRUCT marker AT %B0 : BYTE; stamp AT %B8 : TIME; END_STRUCT; Bytes : ARRAY[0..SIZEOF(Packet)-1] OF BYTE; END_TYPE".into());
    for (abi, upper) in [
        (TemporalStorage::Uniform64, 15),
        (TemporalStorage::Legacy32, 4),
    ] {
        db.set_temporal_storage(abi);
        let diagnostics = db.diagnostics(file);
        assert!(
            !diagnostics.iter().any(|diagnostic| diagnostic.is_error()),
            "{diagnostics:?}"
        );
        let symbols = db.file_symbols(file);
        let symbol = symbols
            .get(symbols.lookup("Bytes").expect("type declaration"))
            .expect("declared symbol");
        let resolved = symbols.resolve_alias_type(symbol.type_id);
        let Some(Type::Array { dimensions, .. }) = symbols.type_by_id(resolved) else {
            panic!("expected array type")
        };
        assert_eq!(dimensions.as_slice(), &[(0, upper)]);
    }
}

#[test]
fn legacy_sizeof_quirk_cannot_hide_a_nested_physical_overlap() {
    let mut db = Database::new();
    db.set_temporal_storage(TemporalStorage::Legacy32);
    let file = FileId(0);
    db.set_source_text(file, "TYPE Inner : STRUCT marker AT %B0 : BYTE; stamp AT %B8 : TIME; END_STRUCT; Outer : STRUCT inner AT %B0 : Inner; tail AT %B5 : BYTE; END_STRUCT; END_TYPE".into());
    assert!(
        db.diagnostics(file).iter().any(|diagnostic| {
            diagnostic.is_error()
                && diagnostic
                    .message
                    .contains("relative structure fields overlap")
        }),
        "Inner physically spans twelve bytes, even though legacy SIZEOF reports five"
    );
}
