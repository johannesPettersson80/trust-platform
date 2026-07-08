use super::*;

#[test]
fn encoder_emits_local_refs_for_functions_and_methods() {
    let source = r#"
FUNCTION AddOne : INT
VAR_INPUT
    x : INT;
END_VAR
VAR
    y : INT;
END_VAR
y := x + 1;
AddOne := y;
END_FUNCTION

CLASS Counter
METHOD PUBLIC Inc : INT
VAR
    temp : INT;
END_VAR
temp := 1;
Inc := temp;
END_METHOD
END_CLASS

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
    let ref_table = match module.section(SectionId::RefTable) {
        Some(SectionData::RefTable(table)) => table,
        other => panic!("expected REF_TABLE, got {other:?}"),
    };
    let bodies = match module.section(SectionId::PouBodies) {
        Some(SectionData::PouBodies(bodies)) => bodies,
        other => panic!("expected POU_BODIES, got {other:?}"),
    };

    let function = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Function && lookup_string(strings, entry.name_idx) == "AddOne"
        })
        .expect("AddOne function");
    assert_eq!(function.local_ref_count, 3);
    let start = function.local_ref_start as usize;
    let end = start + function.local_ref_count as usize;
    let func_locals = &ref_table.entries[start..end];
    assert!(func_locals
        .iter()
        .all(|entry| entry.location == RefLocation::Local));
    assert_eq!(func_locals[0].offset, 0);
    assert_eq!(func_locals[1].offset, 1);
    assert_eq!(func_locals[2].offset, 2);

    let code_start = function.code_offset as usize;
    let code_end = code_start + function.code_length as usize;
    let func_code = &bodies[code_start..code_end];
    let refs = collect_ref_indices(func_code);
    assert!(refs.iter().all(|idx| {
        *idx >= function.local_ref_start
            && *idx < function.local_ref_start + function.local_ref_count
    }));

    let method = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Method && lookup_string(strings, entry.name_idx) == "Inc"
        })
        .expect("Inc method");
    assert_eq!(method.local_ref_count, 2);
    let start = method.local_ref_start as usize;
    let end = start + method.local_ref_count as usize;
    let method_locals = &ref_table.entries[start..end];
    assert!(method_locals
        .iter()
        .all(|entry| entry.location == RefLocation::Local));
}

#[test]
fn encoder_preserves_label_only_statement_as_explicit_nop() {
    let source = r#"
PROGRAM Main
L1: ;
END_PROGRAM
"#;

    let module = bytecode_module_from_source(source).unwrap();
    module.validate().unwrap();

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
    let main = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Program && lookup_string(strings, entry.name_idx) == "Main"
        })
        .expect("Main program entry");
    let start = main.code_offset as usize;
    let end = start + main.code_length as usize;
    assert_eq!(&bodies[start..end], &[0x00]);
}

#[test]
fn encoder_emits_control_flow_jumps() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
    total : INT := 0;
    idx : INT := 0;
END_VAR

IF counter < 10 THEN
    counter := counter + 1;
ELSIF counter = 10 THEN
    counter := counter + 2;
ELSE
    counter := counter + 3;
END_IF;

CASE counter OF
    1: counter := counter + 1;
    2..3: counter := counter + 2;
ELSE
    counter := counter + 3;
END_CASE;

WHILE counter < 5 DO
    counter := counter + 1;
END_WHILE;

REPEAT
    counter := counter + 1;
UNTIL counter > 10
END_REPEAT;

FOR idx := 1 TO 3 BY 1 DO
    total := total + idx;
END_FOR;
END_PROGRAM
"#;

    let module = bytecode_module_from_source(source).unwrap();
    module.validate().unwrap();

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

    let program = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Program && lookup_string(strings, entry.name_idx) == "Main"
        })
        .expect("program entry");
    assert_eq!(program.local_ref_count, 2);

    let code_start = program.code_offset as usize;
    let code_end = code_start + program.code_length as usize;
    let code = &bodies[code_start..code_end];
    let opcodes = collect_opcodes(code);
    assert!(opcodes.contains(&0x02));
    assert!(opcodes.contains(&0x04));
    assert!(opcodes.contains(&0x03));
    assert!(opcodes.contains(&0x11));
    assert!(opcodes.contains(&0x01));
}

#[test]
fn encoder_emits_if_with_string_literal_elsif_condition() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR

IF counter < 1 THEN
    counter := counter + 1;
ELSIF "A" = "B" THEN
    counter := counter + 2;
END_IF;
END_PROGRAM
"#;

    let module = bytecode_module_from_source(source).unwrap();
    module.validate().unwrap();

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

    let program = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Program && lookup_string(strings, entry.name_idx) == "Main"
        })
        .expect("program entry");

    let code_start = program.code_offset as usize;
    let code_end = code_start + program.code_length as usize;
    let code = &bodies[code_start..code_end];
    assert_ne!(code, &[0x00]);
    let opcodes = collect_opcodes(code);
    assert!(
        opcodes.contains(&0x10),
        "expected LOAD_CONST opcode for string literals"
    );
    assert!(
        opcodes.contains(&0x50),
        "expected EQ opcode for string literal comparison"
    );

    let _ = module.section(SectionId::DebugMap);
}
