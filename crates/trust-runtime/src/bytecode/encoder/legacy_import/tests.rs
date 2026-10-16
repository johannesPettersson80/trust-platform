use super::*;
use crate::{
    harness::{CompileSession, SourceFile},
    value::{Duration, Value},
};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn context(source: &str) -> crate::harness::LoweredApplication {
    crate::harness::lower_application_with_abi(
        &[SourceFile::new(source)],
        false,
        crate::value::StorageAbi::LegacyTemporal32,
    )
    .unwrap()
    .prepare_authoring(&[])
    .unwrap()
}

fn legacy(source: &str) -> BytecodeModule {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
        .unwrap()
}

#[test]
fn legacy_import_rejects_context_compiled_for_current_temporal_storage() {
    let source = "VAR_GLOBAL width : DINT := SIZEOF(TIME); END_VAR PROGRAM Main END_PROGRAM";
    let old = legacy(source);
    let before = old.clone();
    let current = crate::harness::lower_application(&[SourceFile::new(source)], false)
        .unwrap()
        .prepare_authoring(&[])
        .unwrap();
    let error = build_module_from_legacy_context(&old, &current).unwrap_err();
    assert_eq!(error, invalid("expected legacy temporal storage ABI"));
    assert_eq!(old, before);

    let imported = build_module_from_legacy_context(&old, &context(source)).unwrap();
    assert_eq!(
        imported.storage_abi().unwrap(),
        crate::value::StorageAbi::LegacyTemporal32
    );
    let prepared =
        PreparedModule::from_bytes(&imported.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let state = prepared.instantiate(0).unwrap();
    assert_eq!(state.storage().get_global("width"), Some(&Value::DInt(4)));
    assert_eq!(old, before);
}

fn import(source: &str, old: &BytecodeModule) -> BytecodeModule {
    let new = build_module_from_legacy_context(old, &context(source)).unwrap();
    for id in [
        SectionId::StringTable,
        SectionId::TypeTable,
        SectionId::ConstPool,
        SectionId::RefTable,
    ] {
        match (old.section(id).unwrap(), new.section(id).unwrap()) {
            (SectionData::StringTable(a), SectionData::StringTable(b)) => {
                assert!(b.entries.starts_with(&a.entries))
            }
            (SectionData::TypeTable(a), SectionData::TypeTable(b)) => {
                assert!(b.entries.starts_with(&a.entries))
            }
            (SectionData::ConstPool(a), SectionData::ConstPool(b)) => {
                assert!(b.entries.starts_with(&a.entries))
            }
            (SectionData::RefTable(a), SectionData::RefTable(b)) => {
                assert!(b.entries.starts_with(&a.entries))
            }
            _ => panic!("wrong section"),
        }
    }
    let (Some(SectionData::PouBodies(a)), Some(SectionData::PouBodies(b))) = (
        old.section(SectionId::PouBodies),
        new.section(SectionId::PouBodies),
    ) else {
        panic!("bodies");
    };
    assert!(b.starts_with(a));
    let (Some(SectionData::PouIndex(a)), Some(SectionData::PouIndex(b))) = (
        old.section(SectionId::PouIndex),
        new.section(SectionId::PouIndex),
    ) else {
        panic!("index");
    };
    for (old, new) in a.entries.iter().zip(&b.entries) {
        assert_eq!(
            (
                old.id,
                old.name_idx,
                old.code_offset,
                old.code_length,
                old.local_ref_start,
                old.local_ref_count
            ),
            (
                new.id,
                new.name_idx,
                new.code_offset,
                new.code_length,
                new.local_ref_start,
                new.local_ref_count
            )
        );
    }
    new.validate().unwrap();
    new
}

fn matching_legacy_executes(source: &str, expected: i16) {
    let supplied = legacy(source);
    let original = supplied.encode().unwrap();
    let enriched = import(source, &supplied);
    assert_eq!(
        supplied.encode().unwrap(),
        original,
        "import leaves supplied bytes intact"
    );
    let prepared =
        PreparedModule::from_bytes(&enriched.encode().unwrap(), Default::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(
        state.storage().get_global("marker"),
        Some(&Value::Int(expected))
    );
    let mut hosted = CompileSession::from_source(source)
        .build_runtime_from_bytecode(&supplied, None)
        .unwrap();
    hosted.set_current_time(Duration::from_millis(10));
    hosted.execute_cycle().unwrap();
    assert_eq!(
        hosted.storage().get_global("marker"),
        Some(&Value::Int(expected))
    );
}

#[test]
fn repeated_anonymous_arrays_bind_original_global_and_configured_member_types() {
    let source = r#"
VAR_GLOBAL marker : INT; ga : ARRAY[0..1] OF INT; gb : ARRAY[0..1] OF INT; END_VAR
PROGRAM Main
VAR a : ARRAY[0..1] OF INT; b : ARRAY[0..1] OF INT; END_VAR
a[0] := 11; b[1] := 22; ga[0] := 31; gb[1] := 32;
marker := a[0] + b[1] + ga[0] + gb[1];
END_PROGRAM
CONFIGURATION C RESOURCE R ON PLC PROGRAM plant : Main; END_RESOURCE END_CONFIGURATION
"#;
    matching_legacy_executes(source, 96);
    let supplied = legacy(source);
    assert!(build_module_from_legacy_context(
        &supplied,
        &context(&source.replace("b : ARRAY[0..1]", "b : ARRAY[0..2]"))
    )
    .is_err());
}

#[test]
fn repeated_bounded_text_and_pointer_declarations_keep_legacy_type_identity() {
    for declarations in [
        "a : STRING[8]; b : STRING[8];",
        "a : WSTRING[8]; b : WSTRING[8];",
        "a : POINTER TO INT; b : POINTER TO INT;",
    ] {
        let source = format!("VAR_GLOBAL marker : INT; {declarations} END_VAR PROGRAM Main VAR {declarations} END_VAR marker := 7; END_PROGRAM");
        matching_legacy_executes(&source, 7);
        let supplied = legacy(&source);
        let before = supplied.encode().unwrap();
        let incompatible = source
            .replace("[8]", "[9]")
            .replace("POINTER TO INT", "POINTER TO BOOL");
        assert!(build_module_from_legacy_context(&supplied, &context(&incompatible)).is_err());
        assert_eq!(supplied.encode().unwrap(), before);
    }
}

#[test]
fn anonymous_function_signature_and_frame_types_are_seeded_before_collection() {
    let source = r#"
VAR_GLOBAL marker : INT; END_VAR
FUNCTION Echo : STRING[8]
VAR_INPUT text : STRING[8]; END_VAR
VAR_IN_OUT shared : ARRAY[0..1] OF INT; END_VAR
VAR_OUTPUT copied : STRING[8]; END_VAR
VAR first : STRING[8]; second : STRING[8]; END_VAR
first := text; second := first; copied := second; shared[0] := shared[0] + 1; Echo := second;
END_FUNCTION
PROGRAM Main VAR data : ARRAY[0..1] OF INT; copied : STRING[8]; END_VAR
marker := LEN(Echo(text := 'abc', shared := data, copied => copied)) + data[0] + LEN(copied);
END_PROGRAM
"#;
    matching_legacy_executes(source, 7);
}

#[test]
fn same_named_methods_bind_anonymous_types_through_their_actual_owner() {
    let source = r#"
VAR_GLOBAL marker : INT; END_VAR
FUNCTION_BLOCK Left
METHOD PUBLIC Echo : STRING[8]
VAR_INPUT text : STRING[8]; END_VAR
VAR_OUTPUT copied : STRING[8]; END_VAR
VAR first : STRING[8]; second : STRING[8]; END_VAR
first := text; second := first; copied := second; Echo := second;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Right
METHOD PUBLIC Echo : STRING[8]
VAR first : STRING[8]; second : STRING[8]; END_VAR
first := 'right'; second := first; Echo := second;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main VAR l : Left; r : Right; copied : STRING[8]; END_VAR
marker := LEN(l.Echo(text := 'left', copied => copied)) + LEN(r.Echo()) + LEN(copied);
END_PROGRAM
"#;
    matching_legacy_executes(source, 13);
}

#[test]
fn anonymous_block_and_program_temps_do_not_reuse_instance_parameter_slots() {
    let source = r#"
VAR_GLOBAL marker : INT; END_VAR
FUNCTION_BLOCK Worker
VAR_INPUT input : INT; END_VAR
VAR_OUTPUT output : INT; END_VAR
VAR_TEMP first : ARRAY[0..1] OF INT; second : ARRAY[0..1] OF INT; END_VAR
first[0] := input; second[1] := first[0]; output := second[1];
END_FUNCTION_BLOCK
PROGRAM Main
VAR worker : Worker; END_VAR
VAR_TEMP first : ARRAY[0..1] OF INT; second : ARRAY[0..1] OF INT; END_VAR
first[0] := 12; second[1] := first[0]; worker(input := second[1]); marker := worker.output;
END_PROGRAM
"#;
    matching_legacy_executes(source, 12);
}

#[test]
fn changed_supplied_instructions_execute_instead_of_recompiled_source() {
    let source = "PROGRAM Main VAR a : INT; b : INT; END_VAR a := INT#1; b := INT#2; END_PROGRAM";
    let mut old = legacy(source);
    let Some(SectionData::ConstPool(constants)) = old.section(SectionId::ConstPool) else {
        panic!("constants");
    };
    let two = constants
        .entries
        .iter()
        .position(|entry| entry.payload == 2i16.to_le_bytes())
        .unwrap() as u32;
    let Some(SectionData::PouIndex(index)) = old.section(SectionId::PouIndex) else {
        panic!("index");
    };
    let offset = index
        .entries
        .iter()
        .find(|entry| entry.kind == PouKind::Program)
        .unwrap()
        .code_offset as usize;
    let Some(SectionData::PouBodies(bytes)) = old.section_mut(SectionId::PouBodies) else {
        panic!("bodies");
    };
    assert_eq!(bytes[offset], 0x10, "LOAD_CONST at the first assignment");
    bytes[offset + 1..offset + 5].copy_from_slice(&two.to_le_bytes());
    old.validate().unwrap();
    let new = import(source, &old);
    let prepared =
        PreparedModule::from_bytes(&new.encode().unwrap(), PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Instance(root)) = state.storage().get_global("Main") else {
        panic!("root");
    };
    assert_eq!(
        state.storage().get_instance_var(*root, "a"),
        Some(&Value::Int(2))
    );
}

#[test]
fn legacy_import_preserves_distinct_bodies_for_multiple_instances_of_one_program_type() {
    let supplied_source = r#"
VAR_GLOBAL two : INT := INT#2; END_VAR
PROGRAM OriginalA
    VAR value : INT; END_VAR
    value := INT#1;
END_PROGRAM
PROGRAM OriginalB
    VAR value : INT; END_VAR
    value := INT#1;
END_PROGRAM
CONFIGURATION Config
    RESOURCE CPU ON PLC
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM A WITH Fast : OriginalA;
        PROGRAM B WITH Fast : OriginalB;
    END_RESOURCE
END_CONFIGURATION
"#;
    let context_source = r#"
VAR_GLOBAL two : INT := INT#2; END_VAR
PROGRAM Main
    VAR value : INT; END_VAR
    value := INT#1;
END_PROGRAM
CONFIGURATION Config
    RESOURCE CPU ON PLC
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM A WITH Fast : Main;
        PROGRAM B WITH Fast : Main;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert!(
        CompileSession::from_source(context_source)
            .build_bytecode_module()
            .is_err(),
        "source-free authoring retains its one-instance-per-type restriction"
    );
    assert!(
        CompileSession::from_source(context_source)
            .build_bytecode_module_for_version(BytecodeVersion::LEGACY)
            .is_err(),
        "the legacy source producer retains its one-instance-per-type restriction"
    );
    let mut supplied = legacy(supplied_source);
    let Some(SectionData::StringTable(strings)) = supplied.section(SectionId::StringTable) else {
        panic!("strings");
    };
    let Some(SectionData::PouIndex(index)) = supplied.section(SectionId::PouIndex) else {
        panic!("POU index");
    };
    let program_ids = index
        .entries
        .iter()
        .filter(|entry| entry.kind == PouKind::Program)
        .filter_map(|entry| {
            let name = strings.entries.get(entry.name_idx as usize)?;
            Some((name.clone(), entry.id, entry.code_offset as usize))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        program_ids.len(),
        2,
        "the legacy producer stores each instance"
    );
    let a = program_ids.iter().find(|(name, _, _)| name == "A").unwrap();
    let b = program_ids.iter().find(|(name, _, _)| name == "B").unwrap();
    let Some(SectionData::ConstPool(constants)) = supplied.section(SectionId::ConstPool) else {
        panic!("constants");
    };
    let two = constants
        .entries
        .iter()
        .position(|entry| entry.payload == 2i16.to_le_bytes())
        .expect("global initializer supplies the replacement constant") as u32;
    let Some(SectionData::PouBodies(bodies)) = supplied.section_mut(SectionId::PouBodies) else {
        panic!("bodies");
    };
    let b_body = b.2;
    assert_eq!(bodies[b_body], 0x10, "LOAD_CONST begins the body");
    bodies[b_body + 1..b_body + 5].copy_from_slice(&two.to_le_bytes());
    supplied.validate().unwrap();
    let supplied_bytes = supplied.encode().unwrap();

    let mut runtime = CompileSession::from_source(context_source)
        .build_runtime_from_bytecode(&supplied, None)
        .unwrap();
    assert_eq!(supplied.encode().unwrap(), supplied_bytes);
    let imported = runtime.construction_artifact().unwrap();
    let (
        Some(SectionData::PouBodies(supplied_bodies)),
        Some(SectionData::PouBodies(imported_bodies)),
    ) = (
        supplied.section(SectionId::PouBodies),
        imported.section(SectionId::PouBodies),
    )
    else {
        panic!("program bodies");
    };
    assert!(imported_bodies.starts_with(supplied_bodies));
    runtime.set_current_time(Duration::from_millis(10));
    runtime.execute_cycle().unwrap();
    for (name, expected) in [("A", 1), ("B", 2)] {
        let Some(Value::Instance(instance)) = runtime.storage().get_global(name) else {
            panic!("configured instance {name} must remain independently installed");
        };
        assert_eq!(
            runtime.storage().get_instance_var(*instance, "value"),
            Some(&Value::Int(expected)),
            "instance {name} must execute its own supplied POU body"
        );
    }
    assert_ne!(a.1, b.1);

    // The admitted construction archive is the durable source-free form of
    // this contextual import. Reload it through the public artifact boundary
    // with the same context; the import-only repeated-type allowance must not
    // turn into source-authoring permission or replace either supplied body.
    let exported = runtime.construction_artifact().unwrap().clone();
    let reload_context = format!("TYPE ContextOnly : REAL; END_TYPE\n{context_source}");
    let mut reloaded = CompileSession::from_source(&reload_context)
        .build_runtime_from_bytecode(&exported, None)
        .unwrap();
    assert!(
        reloaded.registry().lookup("ContextOnly").is_none(),
        "uncertified source types must not enter the admitted registry"
    );
    reloaded.set_current_time(Duration::from_millis(10));
    reloaded.execute_cycle().unwrap();
    for (name, expected) in [("A", 1), ("B", 2)] {
        let Some(Value::Instance(instance)) = reloaded.storage().get_global(name) else {
            panic!("reloaded configured instance {name} must remain installed");
        };
        assert_eq!(
            reloaded.storage().get_instance_var(*instance, "value"),
            Some(&Value::Int(expected)),
            "reloading the construction archive must preserve supplied body for {name}"
        );
    }
}

#[test]
fn configured_instance_globals_inheritance_statics_and_loop_scratch_keep_identities() {
    let source = r#"
FUNCTION_BLOCK Base VAR inherited : INT := 3; END_VAR END_FUNCTION_BLOCK
FUNCTION_BLOCK Child EXTENDS Base VAR member : INT := 4; END_VAR END_FUNCTION_BLOCK
VAR_GLOBAL first : Child; END_VAR
FUNCTION Next : INT
VAR_INPUT delta : INT := 2; END_VAR
VAR_STAT count : INT := 5; END_VAR
count := count + delta; Next := count;
END_FUNCTION
PROGRAM Main
VAR local : Child; timer : TON; total : INT; i : INT; END_VAR
FOR i := 1 TO 2 DO total := total + Next(); END_FOR;
timer(IN := TRUE, PT := T#10ms);
END_PROGRAM
CONFIGURATION C RESOURCE R ON PLC
TASK Scan(INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Main;
END_RESOURCE END_CONFIGURATION
"#;
    let old = legacy(source);
    let new = import(source, &old);
    let prepared =
        PreparedModule::from_bytes(&new.encode().unwrap(), PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Instance(root)) = state.storage().get_global("Plant") else {
        panic!("root");
    };
    assert_eq!(
        state.storage().get_instance_var(*root, "total"),
        Some(&Value::Int(16))
    );
    assert!(state.storage().get_global("first").is_some());
}

#[test]
fn mismatched_context_is_rejected_without_replacing_supplied_tables() {
    let source = "PROGRAM Main VAR value : INT; END_VAR value := 1; END_PROGRAM";
    let old = legacy(source);
    let before = old.clone();
    let incompatible = context("PROGRAM Main VAR value : BOOL; END_VAR value := TRUE; END_PROGRAM");
    assert!(build_module_from_legacy_context(&old, &incompatible).is_err());
    assert_eq!(old, before);
}

#[test]
fn bounded_string_io_enrichment_preserves_address_and_reference() {
    let source = "PROGRAM Main VAR text AT %QB0 : STRING[8] := 'abc'; END_VAR END_PROGRAM";
    let old = legacy(source);
    let new = import(source, &old);
    let (Some(SectionData::IoMap(a)), Some(SectionData::IoMap(b))) =
        (old.section(SectionId::IoMap), new.section(SectionId::IoMap))
    else {
        panic!("I/O");
    };
    assert_eq!(a.bindings.len(), 1);
    assert_eq!(a.bindings[0].address_str_idx, b.bindings[0].address_str_idx);
    assert_eq!(a.bindings[0].ref_idx, b.bindings[0].ref_idx);
    let Some(SectionData::TypeTable(types)) = new.section(SectionId::TypeTable) else {
        panic!("types");
    };
    assert!(matches!(
        types.entries[b.bindings[0].type_id.unwrap() as usize].data,
        TypeData::Primitive {
            prim_id: 24,
            max_length: 8
        }
    ));
    let prepared =
        PreparedModule::from_bytes(&new.encode().unwrap(), PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let mut baseline = CompileSession::from_source(source)
        .build_runtime_from_bytecode(&old, None)
        .unwrap();
    baseline.set_current_time(Duration::from_millis(10));
    baseline.execute_cycle().unwrap();
    assert_eq!(state.outputs(), baseline.io().outputs());
    assert_eq!(state.outputs().len(), 8);
    assert_eq!(&state.outputs()[..3], b"abc");
    assert!(build_module_from_legacy_context(
        &old,
        &context("PROGRAM Main VAR text AT %QB0 : STRING[9]; END_VAR END_PROGRAM")
    )
    .is_err());
}
