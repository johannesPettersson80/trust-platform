use trust_runtime::bytecode::{
    BytecodeModule, InterfaceMethod, PouKind, RefLocation, SectionData, SectionId, StringTable,
    TypeData, TypeEntry, TypeKind, TypeTable,
};
use trust_runtime::harness::{
    bytecode_bytes_from_source, bytecode_module_from_source, bytecode_module_from_source_with_path,
    TestHarness,
};

fn lookup_string(strings: &trust_runtime::bytecode::StringTable, idx: u32) -> &str {
    strings
        .entries
        .get(idx as usize)
        .map(|s| s.as_str())
        .unwrap_or("")
}

fn find_type<'a>(types: &'a TypeTable, strings: &'a StringTable, name: &str) -> &'a TypeEntry {
    let idx = strings
        .entries
        .iter()
        .position(|entry| entry.eq_ignore_ascii_case(name))
        .expect("string not found");
    types
        .entries
        .iter()
        .find(|entry| entry.name_idx == Some(idx as u32))
        .expect("type entry not found")
}

fn expect_primitive(types: &TypeTable, type_id: u32, prim_id: u16) {
    let entry = types
        .entries
        .get(type_id as usize)
        .expect("primitive type entry");
    match entry.data {
        TypeData::Primitive {
            prim_id: actual, ..
        } => assert_eq!(actual, prim_id),
        _ => panic!("expected primitive type"),
    }
}

fn expect_interface_methods(methods: &[InterfaceMethod], strings: &StringTable, expected: &[&str]) {
    assert_eq!(methods.len(), expected.len());
    for (idx, name) in expected.iter().enumerate() {
        let method = &methods[idx];
        assert_eq!(method.slot, idx as u32);
        assert_eq!(lookup_string(strings, method.name_idx), *name);
    }
}

fn walk_instructions(code: &[u8], mut on_op: impl FnMut(u8, &[u8])) {
    let mut i = 0usize;
    while i < code.len() {
        let opcode = code[i];
        i += 1;
        let operand_len = match opcode {
            0x02..=0x04 => 4,
            0x05 => 4,
            0x07 => 4,
            0x08 => 8,
            0x10 => 4,
            0x16 => 1,
            0x20..=0x22 => 4,
            0x30 => 4,
            0x60 => 4,
            0x70 => 4,
            _ => 0,
        };
        if i + operand_len > code.len() {
            break;
        }
        let operands = &code[i..i + operand_len];
        on_op(opcode, operands);
        i += operand_len;
    }
}

fn collect_opcodes(code: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    walk_instructions(code, |opcode, _| out.push(opcode));
    out
}

fn collect_ref_indices(code: &[u8]) -> Vec<u32> {
    let mut out = Vec::new();
    walk_instructions(code, |opcode, operands| {
        if matches!(opcode, 0x20..=0x22) && operands.len() == 4 {
            let idx = u32::from_le_bytes([operands[0], operands[1], operands[2], operands[3]]);
            out.push(idx);
        }
    });
    out
}

#[test]
fn encoder_roundtrip_validates() {
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

    let module = bytecode_module_from_source(source).unwrap();
    let bytes = module.encode().unwrap();
    let decoded = BytecodeModule::decode(&bytes).unwrap();
    decoded.validate().unwrap();
}

#[test]
fn encoder_emits_method_tables() {
    let source = r#"
CLASS Base
METHOD PUBLIC Foo : INT
Foo := INT#1;
END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
METHOD PUBLIC OVERRIDE Foo : INT
Foo := INT#2;
END_METHOD
METHOD PUBLIC Bar : INT
Bar := INT#3;
END_METHOD
END_CLASS

PROGRAM Main
VAR
    obj : Derived;
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

    let base = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Class && lookup_string(strings, entry.name_idx) == "Base"
        })
        .expect("Base class entry");
    let derived = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Class && lookup_string(strings, entry.name_idx) == "Derived"
        })
        .expect("Derived class entry");

    let derived_meta = derived.class_meta.as_ref().expect("Derived metadata");
    assert_eq!(derived_meta.parent_pou_id, Some(base.id));
    assert_eq!(derived_meta.methods.len(), 2);

    let foo_entry = derived_meta
        .methods
        .iter()
        .find(|entry| lookup_string(strings, entry.name_idx) == "Foo")
        .expect("Foo entry");
    let bar_entry = derived_meta
        .methods
        .iter()
        .find(|entry| lookup_string(strings, entry.name_idx) == "Bar")
        .expect("Bar entry");

    assert_eq!(foo_entry.vtable_slot, 0);
    assert_eq!(bar_entry.vtable_slot, 1);

    let derived_foo = pou_index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Method
                && entry.owner_pou_id == Some(derived.id)
                && lookup_string(strings, entry.name_idx) == "Foo"
        })
        .expect("Derived.Foo POU");
    assert_eq!(foo_entry.pou_id, derived_foo.id);
}

#[test]
fn encoder_emits_composite_types() {
    let source = r#"
TYPE
    MySubrange : INT(0..10);
    MyAlias : INT;
    MyArray : ARRAY[1..3] OF INT;
    MyStruct : STRUCT
        a : INT;
        b : BOOL;
    END_STRUCT;
    MyUnion : UNION
        u1 : INT;
        u2 : BOOL;
    END_UNION;
    MyEnum : (Red := 1, Green := 2, Blue := 3) INT;
    MyRef : REF_TO INT;
END_TYPE

PROGRAM Main
VAR
    sr : MySubrange;
    al : MyAlias;
    arr : MyArray;
    st : MyStruct;
    un : MyUnion;
    enum_val : MyEnum;
    rf : MyRef;
END_VAR
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

    let array_alias = find_type(types, strings, "MyArray");
    assert_eq!(array_alias.kind, TypeKind::Alias);
    let array_type_id = if let TypeData::Alias { target_type_id } = &array_alias.data {
        *target_type_id
    } else {
        panic!("expected alias type data");
    };
    let array = types
        .entries
        .get(array_type_id as usize)
        .expect("array target type");
    assert_eq!(array.kind, TypeKind::Array);
    if let TypeData::Array { elem_type_id, dims } = &array.data {
        assert_eq!(dims, &vec![(1, 3)]);
        expect_primitive(types, *elem_type_id, 7);
    } else {
        panic!("expected array type data");
    }

    let strukt = find_type(types, strings, "MyStruct");
    assert_eq!(strukt.kind, TypeKind::Struct);
    if let TypeData::Struct { fields } = &strukt.data {
        assert_eq!(fields.len(), 2);
        assert_eq!(lookup_string(strings, fields[0].name_idx), "a");
        assert_eq!(lookup_string(strings, fields[1].name_idx), "b");
        expect_primitive(types, fields[0].type_id, 7);
        expect_primitive(types, fields[1].type_id, 1);
    } else {
        panic!("expected struct type data");
    }

    let union = find_type(types, strings, "MyUnion");
    assert_eq!(union.kind, TypeKind::Union);
    if let TypeData::Union { fields } = &union.data {
        assert_eq!(fields.len(), 2);
        assert_eq!(lookup_string(strings, fields[0].name_idx), "u1");
        assert_eq!(lookup_string(strings, fields[1].name_idx), "u2");
        expect_primitive(types, fields[0].type_id, 7);
        expect_primitive(types, fields[1].type_id, 1);
    } else {
        panic!("expected union type data");
    }

    let enum_ty = find_type(types, strings, "MyEnum");
    assert_eq!(enum_ty.kind, TypeKind::Enum);
    if let TypeData::Enum {
        base_type_id,
        variants,
    } = &enum_ty.data
    {
        expect_primitive(types, *base_type_id, 7);
        assert_eq!(variants.len(), 3);
        assert_eq!(lookup_string(strings, variants[0].name_idx), "Red");
        assert_eq!(lookup_string(strings, variants[1].name_idx), "Green");
        assert_eq!(lookup_string(strings, variants[2].name_idx), "Blue");
        assert_eq!(variants[0].value, 1);
        assert_eq!(variants[1].value, 2);
        assert_eq!(variants[2].value, 3);
    } else {
        panic!("expected enum type data");
    }

    let alias = find_type(types, strings, "MyAlias");
    assert_eq!(alias.kind, TypeKind::Alias);
    if let TypeData::Alias { target_type_id } = &alias.data {
        expect_primitive(types, *target_type_id, 7);
    } else {
        panic!("expected alias type data");
    }

    let subrange_alias = find_type(types, strings, "MySubrange");
    assert_eq!(subrange_alias.kind, TypeKind::Alias);
    let subrange_type_id = if let TypeData::Alias { target_type_id } = &subrange_alias.data {
        *target_type_id
    } else {
        panic!("expected alias type data");
    };
    let subrange = types
        .entries
        .get(subrange_type_id as usize)
        .expect("subrange target type");
    assert_eq!(subrange.kind, TypeKind::Subrange);
    if let TypeData::Subrange {
        base_type_id,
        lower,
        upper,
    } = &subrange.data
    {
        expect_primitive(types, *base_type_id, 7);
        assert_eq!(*lower, 0);
        assert_eq!(*upper, 10);
    } else {
        panic!("expected subrange type data");
    }

    let reference_alias = find_type(types, strings, "MyRef");
    assert_eq!(reference_alias.kind, TypeKind::Alias);
    let reference_type_id = if let TypeData::Alias { target_type_id } = &reference_alias.data {
        *target_type_id
    } else {
        panic!("expected alias type data");
    };
    let reference = types
        .entries
        .get(reference_type_id as usize)
        .expect("reference target type");
    assert_eq!(reference.kind, TypeKind::Reference);
    if let TypeData::Reference { target_type_id } = &reference.data {
        expect_primitive(types, *target_type_id, 7);
    } else {
        panic!("expected reference type data");
    }
}

#[test]
fn encoder_emits_interface_methods() {
    let source = r#"
INTERFACE IBase
METHOD Foo : INT
END_METHOD
END_INTERFACE

INTERFACE IDerived EXTENDS IBase
METHOD Bar : INT
END_METHOD
END_INTERFACE

CLASS Impl IMPLEMENTS IDerived
METHOD PUBLIC Foo : INT
Foo := INT#1;
END_METHOD
METHOD PUBLIC Bar : INT
Bar := INT#2;
END_METHOD
END_CLASS

PROGRAM Main
VAR
    i : IDerived;
    b : IBase;
    c : Impl;
END_VAR
i := c;
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

    let base = find_type(types, strings, "IBase");
    assert_eq!(base.kind, TypeKind::Interface);
    if let TypeData::Interface { methods } = &base.data {
        expect_interface_methods(methods, strings, &["Foo"]);
    } else {
        panic!("expected interface type data");
    }

    let derived = find_type(types, strings, "IDerived");
    assert_eq!(derived.kind, TypeKind::Interface);
    if let TypeData::Interface { methods } = &derived.data {
        expect_interface_methods(methods, strings, &["Foo", "Bar"]);
    } else {
        panic!("expected interface type data");
    }
}

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
fn encoder_does_not_emit_partial_if_with_unsupported_elsif() {
    let source = r#"
FUNCTION IsReady : BOOL
IsReady := TRUE;
END_FUNCTION

PROGRAM Main
VAR
    counter : INT := 0;
END_VAR

IF counter < 1 THEN
    counter := counter + 1;
ELSIF IsReady() THEN
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
    assert_eq!(code, &[0x00]);

    let _ = module.section(SectionId::DebugMap);
}

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
