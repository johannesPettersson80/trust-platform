use super::*;

#[test]
fn encoder_emits_dynamic_instance_access() {
    let source = r#"
FUNCTION_BLOCK Counter
VAR
    value : INT := 0;
END_VAR
value := value + 1;
END_FUNCTION_BLOCK

CLASS Box
VAR
    count : INT := 0;
END_VAR
METHOD PUBLIC Inc : INT
count := count + 1;
Inc := count;
END_METHOD
END_CLASS

PROGRAM Main
VAR
    fb : Counter;
    obj : Box;
END_VAR
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
    let bodies = match module.section(SectionId::PouBodies) {
        Some(SectionData::PouBodies(bodies)) => bodies,
        other => panic!("expected POU_BODIES, got {other:?}"),
    };

    let fb_entry = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::FunctionBlock
                && lookup_string(strings, entry.name_idx) == "Counter"
        })
        .expect("Counter FB");
    let fb_code = &bodies
        [fb_entry.code_offset as usize..(fb_entry.code_offset + fb_entry.code_length) as usize];
    let fb_ops = collect_opcodes(fb_code);
    assert!(fb_ops.contains(&0x23));
    assert!(fb_ops.contains(&0x30));
    assert!(fb_ops.contains(&0x32));
    assert!(fb_ops.contains(&0x33));

    let method_entry = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Method && lookup_string(strings, entry.name_idx) == "Inc"
        })
        .expect("Inc method");
    let method_code = &bodies[method_entry.code_offset as usize
        ..(method_entry.code_offset + method_entry.code_length) as usize];
    let method_ops = collect_opcodes(method_code);
    assert!(method_ops.contains(&0x23));
    assert!(method_ops.contains(&0x30));
    assert!(method_ops.contains(&0x32));
    assert!(method_ops.contains(&0x33));
}

#[test]
fn encoder_bytes_roundtrip_from_source() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM

CONFIGURATION C
RESOURCE R ON CPU
TASK T (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Main WITH T : Main;
END_RESOURCE
END_CONFIGURATION
"#;

    let bytes = bytecode_bytes_from_source(source).unwrap();
    let module = BytecodeModule::decode(&bytes).unwrap();
    module.validate().unwrap();

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    runtime.apply_bytecode_module(&module, None).unwrap();
    assert_eq!(runtime.tasks().len(), 1);
    assert_eq!(runtime.tasks()[0].name, "T");
}

#[test]
fn encoder_emits_io_map() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
END_PROGRAM

CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL
    input AT %IX0.0 : BOOL;
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
    let io_map = match module.section(SectionId::IoMap) {
        Some(SectionData::IoMap(map)) => map,
        other => panic!("expected IO_MAP, got {other:?}"),
    };
    let ref_table = match module.section(SectionId::RefTable) {
        Some(SectionData::RefTable(table)) => table,
        other => panic!("expected REF_TABLE, got {other:?}"),
    };

    let binding = io_map.bindings.first().expect("IO binding");
    let addr = lookup_string(strings, binding.address_str_idx);
    assert_eq!(addr, "%IX0.0");

    let ref_entry = ref_table
        .entries
        .get(binding.ref_idx as usize)
        .expect("ref entry");
    assert_eq!(ref_entry.location, RefLocation::Global);
}

#[test]
fn encoder_resource_meta_sizes_follow_io_bindings() {
    let source = r#"
PROGRAM Main
VAR_EXTERNAL
    DI0 : BOOL;
    DO0 : BOOL;
END_VAR
DO0 := DI0;
END_PROGRAM

CONFIGURATION C
VAR_GLOBAL
    DI0 AT %IX0.0 : BOOL;
    DO0 AT %QX0.0 : BOOL;
END_VAR
RESOURCE R ON CPU
TASK T (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Main WITH T : Main;
END_RESOURCE
END_CONFIGURATION
"#;

    let module = bytecode_module_from_source(source).unwrap();
    let resource_meta = match module.section(SectionId::ResourceMeta) {
        Some(SectionData::ResourceMeta(meta)) => meta,
        other => panic!("expected RESOURCE_META, got {other:?}"),
    };
    let resource = resource_meta.resources.first().expect("resource entry");
    assert!(resource.inputs_size >= 1);
    assert!(resource.outputs_size >= 1);
}

#[test]
fn encoder_accepts_case_insensitive_names_in_call_heavy_if_blocks() {
    let source = r#"
TYPE CONSTANTS_PHYS :
STRUCT
    T0 : REAL := -273.15;
END_STRUCT
END_TYPE

VAR_GLOBAL
    PHYS : CONSTANTS_PHYS;
END_VAR

FUNCTION EXP10 : REAL
VAR_INPUT
    X : REAL;
END_VAR
EXP10 := EXP(X * 2.30258509299405);
END_FUNCTION

FUNCTION DEW_TEMP : REAL
VAR_INPUT
    RH : REAL;
    T : REAL;
END_VAR
VAR CONSTANT
    a : REAL := 7.5;
    b : REAL := 237.3;
END_VAR
VAR
    V : REAL;
    SaturationTerm : REAL;
END_VAR
IF rh > 0.0 THEN
    SaturationTerm := EXP10((a * T) / (b + T));
    V := LOG(RH * 0.01 * SaturationTerm);
    DEW_TEMP := b * V / (a - V);
ELSE
    DEW_TEMP := phys.T0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    out : REAL := REAL#0.0;
END_VAR
out := DEW_TEMP(RH := REAL#50.0, T := REAL#20.0);
END_PROGRAM
"#;

    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    module.validate().expect("validate module");
}
