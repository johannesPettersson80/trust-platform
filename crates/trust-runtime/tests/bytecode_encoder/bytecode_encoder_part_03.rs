use super::*;

#[test]
fn encoder_validates_enum_constant_payloads() {
    let source = r#"
TYPE
    E_Mode : (Idle := 0, Run := 1);
END_TYPE

PROGRAM Main
VAR
    state : E_Mode := E_Mode#Idle;
    is_idle : BOOL;
END_VAR

is_idle := state = E_Mode#Idle;
END_PROGRAM
"#;

    let module = bytecode_module_from_source(source).unwrap();
    module.validate().unwrap();
}

#[test]
fn encoder_emits_debug_map() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
counter := counter + 2;
END_PROGRAM
"#;

    let path = "/tmp/main.st";
    let module = bytecode_module_from_source_with_path(source, path).unwrap();
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(table)) => table,
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let debug_strings = match module.section(SectionId::DebugStringTable) {
        Some(SectionData::DebugStringTable(table)) => table,
        other => panic!("expected DEBUG_STRING_TABLE, got {other:?}"),
    };
    let pou_index = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index,
        other => panic!("expected POU_INDEX, got {other:?}"),
    };
    let debug_map = match module.section(SectionId::DebugMap) {
        Some(SectionData::DebugMap(map)) => map,
        other => panic!("expected DEBUG_MAP, got {other:?}"),
    };

    assert_eq!(debug_map.entries.len(), 2);
    let entry = &debug_map.entries[0];
    let program = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Program && lookup_string(strings, entry.name_idx) == "Main"
        })
        .expect("program entry");
    assert_eq!(program.code_length, 32);
    assert_eq!(entry.pou_id, program.id);
    assert_eq!(lookup_string(debug_strings, entry.file_idx), path);
    assert_eq!(entry.line, 6);
    assert_eq!(entry.column, 1);
    assert_eq!(entry.kind, 0);
    assert_eq!(entry.code_offset, 0);

    let second = &debug_map.entries[1];
    assert_eq!(second.pou_id, program.id);
    assert_eq!(second.line, 7);
    assert_eq!(second.column, 1);
    assert_eq!(second.code_offset, 16);
}

#[test]
fn encoder_emits_param_defaults() {
    let source = r#"
FUNCTION Add : INT
VAR_INPUT
    x : INT := INT#5;
END_VAR
Add := x + 1;
END_FUNCTION

PROGRAM Main
END_PROGRAM
"#;

    let module = bytecode_module_from_source(source).unwrap();
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(table)) => table,
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let pou_index = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index,
        other => panic!("expected POU_INDEX, got {other:?}"),
    };
    let const_pool = match module.section(SectionId::ConstPool) {
        Some(SectionData::ConstPool(pool)) => pool,
        other => panic!("expected CONST_POOL, got {other:?}"),
    };

    let add = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Function && lookup_string(strings, entry.name_idx) == "Add"
        })
        .expect("Add function");
    let param = add.params.first().expect("Add param");
    let idx = param.default_const_idx.expect("default const idx");
    assert!((idx as usize) < const_pool.entries.len());
}

#[test]
fn encoder_emits_var_meta_and_retain_init() {
    let source = r#"
PROGRAM Main
END_PROGRAM

CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL RETAIN
    g_count : INT := INT#7;
END_VAR
TASK T (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Main WITH T : Main;
END_RESOURCE
END_CONFIGURATION
"#;

    let module = bytecode_module_from_source(source).unwrap();
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(table)) => table,
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let var_meta = match module.section(SectionId::VarMeta) {
        Some(SectionData::VarMeta(meta)) => meta,
        other => panic!("expected VAR_META, got {other:?}"),
    };
    let retain_init = match module.section(SectionId::RetainInit) {
        Some(SectionData::RetainInit(retain)) => retain,
        other => panic!("expected RETAIN_INIT, got {other:?}"),
    };

    let entry = var_meta
        .entries
        .iter()
        .find(|entry| lookup_string(strings, entry.name_idx) == "g_count")
        .expect("g_count meta");
    assert_eq!(entry.retain, 1);
    assert!(entry.init_const_idx.is_some());
    assert!(retain_init
        .entries
        .iter()
        .any(|retain| retain.ref_idx == entry.ref_idx));
}

#[test]
fn encoder_emits_program_field_var_meta_for_validation() {
    let source = r#"
PROGRAM Main
VAR
    target : DINT := DINT#0;
    flag : BOOL := FALSE;
END_VAR
target := DINT#1;
flag := TRUE;
END_PROGRAM
"#;

    let module = bytecode_module_from_source(source).unwrap();
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(table)) => table,
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let types = match module.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(table)) => table,
        other => panic!("expected TYPE_TABLE, got {other:?}"),
    };
    let var_meta = match module.section(SectionId::VarMeta) {
        Some(SectionData::VarMeta(meta)) => meta,
        other => panic!("expected VAR_META, got {other:?}"),
    };

    let target = var_meta
        .entries
        .iter()
        .find(|entry| lookup_string(strings, entry.name_idx) == "Main.target")
        .expect("Main.target meta");
    let flag = var_meta
        .entries
        .iter()
        .find(|entry| lookup_string(strings, entry.name_idx) == "Main.flag")
        .expect("Main.flag meta");

    assert_ne!(target.ref_idx, flag.ref_idx);
    assert_eq!(primitive_id(types, target.type_id), Some(8));
    assert_eq!(primitive_id(types, flag.type_id), Some(1));
}

fn primitive_id(types: &trust_runtime::bytecode::TypeTable, type_id: u32) -> Option<u16> {
    match &types.entries.get(type_id as usize)?.data {
        TypeData::Primitive { prim_id, .. } => Some(*prim_id),
        _ => None,
    }
}
