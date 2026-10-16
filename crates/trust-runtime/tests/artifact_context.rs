#![cfg(feature = "full")]
use trust_runtime::{
    bytecode::{
        build_module_from_runtime, build_module_from_runtime_with_sources, BytecodeVersion,
        SectionData, SectionId,
    },
    harness::{CompileSession, SourceFile},
    value::Value,
};
const CONTEXT: &str =
    "VAR_GLOBAL marker : INT := 7; END_VAR PROGRAM Main marker := marker + 1; END_PROGRAM";

#[test]
fn legacy_context_installs_supplied_bodies_and_compiles_only_missing_construction() {
    let supplied = CompileSession::from_source(CONTEXT.replace("marker + 1", "marker + 2"))
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .unwrap();
    let context = CompileSession::from_source(CONTEXT);
    let mut runtime = context
        .build_runtime_from_bytecode(&supplied, None)
        .unwrap();
    let installed = build_module_from_runtime(&runtime).unwrap();
    let Some(SectionData::PouBodies(before)) = supplied.section(SectionId::PouBodies) else {
        panic!("body")
    };
    let Some(SectionData::PouBodies(after)) = installed.section(SectionId::PouBodies) else {
        panic!("body")
    };
    assert!(
        after.starts_with(before),
        "supplied ordinary instruction bytes are unchanged"
    );
    assert_eq!(runtime.storage().get_global("marker"), Some(&Value::Int(7)));
    assert!(build_module_from_runtime_with_sources(&runtime, &[CONTEXT]).is_err());
    assert!(runtime
        .metadata_snapshot()
        .resolve_breakpoint_location(CONTEXT, 0, 1, 1)
        .is_none());
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("marker"), Some(&Value::Int(9)));
    assert!(CompileSession::from_sources(vec![])
        .build_runtime_from_bytecode(&supplied, None)
        .is_err());
}

#[test]
fn source_free_body_mismatch_executes_supplied_code_without_false_source_association() {
    let supplied = CompileSession::from_source(CONTEXT.replace("marker + 1", "marker + 2"))
        .build_bytecode_module()
        .unwrap();
    // Plain export deliberately omits opt-in source/debug metadata. All other
    // artifact fields must still match the supplied application exactly.
    let mut expected_plain = supplied.clone();
    expected_plain.sections.retain(|section| {
        section.id != SectionId::DebugMap.as_raw()
            && section.id != SectionId::DebugStringTable.as_raw()
    });
    let mut runtime = CompileSession::from_source(CONTEXT)
        .build_runtime_from_bytecode(&supplied, None)
        .unwrap();
    assert_eq!(build_module_from_runtime(&runtime).unwrap(), expected_plain);
    assert!(build_module_from_runtime_with_sources(&runtime, &[CONTEXT]).is_err());
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("marker"), Some(&Value::Int(9)));
    assert_eq!(build_module_from_runtime(&runtime).unwrap(), expected_plain);
    runtime.write_global("marker", Value::Int(42)).unwrap();
    assert_eq!(build_module_from_runtime(&runtime).unwrap(), expected_plain);
    let mut source_free = CompileSession::from_sources(vec![])
        .build_runtime_from_bytecode(&supplied, None)
        .unwrap();
    source_free.execute_cycle().unwrap();
    assert_eq!(
        source_free.storage().get_global("marker"),
        Some(&Value::Int(9))
    );
    assert_eq!(
        build_module_from_runtime(&source_free).unwrap(),
        expected_plain
    );
}

#[test]
fn matching_source_positions_allow_path_relabel_and_failed_context_leaves_owner_unchanged() {
    let producer =
        CompileSession::from_sources(vec![SourceFile::with_path("old/main.st", CONTEXT)]);
    let supplied = producer.build_bytecode_module().unwrap();
    let context = CompileSession::from_sources(vec![SourceFile::with_path("new/main.st", CONTEXT)]);
    let runtime = context
        .build_runtime_from_bytecode(&supplied, None)
        .unwrap();
    build_module_from_runtime_with_sources(&runtime, &[CONTEXT])
        .expect("certified path-only source association");
    let legacy = producer
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .unwrap();
    let matching_legacy = context.build_runtime_from_bytecode(&legacy, None).unwrap();
    build_module_from_runtime_with_sources(&matching_legacy, &[CONTEXT])
        .expect("matching legacy source/path association");
    assert!(matching_legacy
        .metadata_snapshot()
        .statement_locations(0)
        .is_some());

    let mut installed = producer.build_runtime().unwrap();
    installed.write_global("marker", Value::Int(42)).unwrap();
    let before = build_module_from_runtime(&installed).unwrap();
    let result =
        CompileSession::from_source("VAR_GLOBAL marker : BOOL; END_VAR PROGRAM Main END_PROGRAM")
            .build_runtime_from_bytecode(&legacy, None);
    assert!(result.is_err());
    assert_eq!(
        installed.storage().get_global("marker"),
        Some(&Value::Int(42))
    );
    assert_eq!(build_module_from_runtime(&installed).unwrap(), before);
}

#[test]
fn unique_artifact_source_labels_can_swap_without_reassigning_indexed_positions() {
    let function =
        "FUNCTION AddOne : INT VAR_INPUT n : INT; END_VAR AddOne := n + INT#1; END_FUNCTION";
    let program = "PROGRAM Main VAR count : INT := 2; END_VAR count := AddOne(count); END_PROGRAM";
    let source = CompileSession::from_sources(vec![
        SourceFile::with_path("a.st", function),
        SourceFile::with_path("b.st", program),
    ]);
    for version in [
        BytecodeVersion::LEGACY,
        BytecodeVersion::SOURCE_FREE_V2,
        BytecodeVersion::SOURCE_FREE,
    ] {
        let mut supplied = source.build_bytecode_module_for_version(version).unwrap();
        let Some(SectionData::DebugStringTable(labels)) =
            supplied.section_mut(SectionId::DebugStringTable)
        else {
            panic!("source labels")
        };
        for label in &mut labels.entries {
            *label = match label.as_str() {
                "a.st" => "b.st".into(),
                "b.st" => "a.st".into(),
                other => panic!("unexpected label {other}"),
            };
        }
        let mut runtime = source.build_runtime_from_bytecode(&supplied, None).unwrap();
        build_module_from_runtime_with_sources(&runtime, &[function, program]).unwrap();
        runtime.execute_cycle().unwrap();
        let Some(Value::Instance(main)) = runtime.storage().get_global("Main") else {
            panic!("program root")
        };
        assert_eq!(
            runtime.storage().get_instance_var(*main, "count"),
            Some(&Value::Int(3))
        );
        assert!(runtime.metadata_snapshot().statement_locations(0).is_some());
        assert!(runtime.metadata_snapshot().statement_locations(1).is_some());
    }
}
