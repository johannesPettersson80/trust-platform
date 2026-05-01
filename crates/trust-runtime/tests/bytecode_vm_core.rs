use std::time::{Duration as StdDuration, Instant};

use trust_runtime::bytecode::{
    BytecodeModule, PouKind, SectionData, SectionId, TypeData, TypeEntry, TypeKind,
};
use trust_runtime::error::RuntimeError;
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{bytecode_module_from_source, TestHarness};
use trust_runtime::value::Value;
use trust_runtime::Runtime;

fn vm_harness(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let bytes = trust_runtime::harness::bytecode_bytes_from_source(source).expect("build bytecode");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply bytecode");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    harness
}

fn main_pou_entry(module: &BytecodeModule) -> (u32, usize, usize) {
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings,
        _ => panic!("missing string table"),
    };
    let index = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index,
        _ => panic!("missing pou index"),
    };
    let main = index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == PouKind::Program
                && strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("MAIN")
        })
        .expect("main entry");
    (
        main.id,
        main.code_offset as usize,
        (main.code_offset + main.code_length) as usize,
    )
}

fn main_body_bytes(module: &BytecodeModule) -> Vec<u8> {
    let (_, start, end) = main_pou_entry(module);
    let code = match module.section(SectionId::PouBodies) {
        Some(SectionData::PouBodies(code)) => code,
        _ => panic!("missing POU_BODIES"),
    };
    code[start..end].to_vec()
}

fn replace_main_body(module: &mut BytecodeModule, new_body: &[u8]) {
    let (main_id, _, _) = main_pou_entry(module);
    let new_offset =
        if let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) {
            let offset = code.len() as u32;
            code.extend_from_slice(new_body);
            offset
        } else {
            panic!("missing POU_BODIES");
        };

    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        for entry in &mut index.entries {
            if entry.id == main_id {
                entry.code_offset = new_offset;
                entry.code_length = new_body.len() as u32;
            }
        }
    } else {
        panic!("missing POU_INDEX");
    }

    // Debug map offsets may no longer align after manual body patching.
    module.sections.retain(|section| {
        section.id != SectionId::DebugMap.as_raw()
            && section.id != SectionId::DebugStringTable.as_raw()
    });
}

fn vm_harness_from_module(source: &str, module: &BytecodeModule) -> TestHarness {
    let bytes = module.encode().expect("encode module");
    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply bytecode");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    harness
}

fn assert_invalid_bytecode_contains(errors: &[RuntimeError], needle: &str) {
    assert!(
        errors.iter().any(
            |err| matches!(err, RuntimeError::InvalidBytecode(message) if message.contains(needle))
        ),
        "expected InvalidBytecode containing '{needle}', got {errors:?}"
    );
}

fn assert_apply_invalid_bytecode_contains(module: &BytecodeModule, needle: &str) {
    let bytes = module.encode().expect("encode module");
    let mut runtime = Runtime::new();
    let err = runtime
        .apply_bytecode_bytes(&bytes, None)
        .expect_err("mutated module should fail during apply");
    match err {
        RuntimeError::InvalidBytecode(message) => {
            assert!(
                message.contains(needle),
                "expected InvalidBytecode containing '{needle}', got '{message}'"
            );
        }
        other => panic!("expected InvalidBytecode, got {other:?}"),
    }
}

fn mutate_first_const_payload_for_primitive(
    module: &mut BytecodeModule,
    primitive_id: u16,
    payload: Vec<u8>,
) {
    let type_table = match module.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(table)) => table,
        _ => panic!("missing TYPE_TABLE"),
    };
    let const_pool = match module.section(SectionId::ConstPool) {
        Some(SectionData::ConstPool(pool)) => pool,
        _ => panic!("missing CONST_POOL"),
    };

    let const_idx = const_pool
        .entries
        .iter()
        .position(|entry| {
            matches!(
                type_table.entries.get(entry.type_id as usize).map(|entry| &entry.data),
                Some(TypeData::Primitive { prim_id, .. }) if *prim_id == primitive_id
            )
        })
        .expect("expected const entry for primitive type");

    if let Some(SectionData::ConstPool(pool)) = module.section_mut(SectionId::ConstPool) {
        pool.entries[const_idx].payload = payload;
    } else {
        panic!("missing CONST_POOL");
    }
}

fn patch_first_call_native_arg_count(module: &mut BytecodeModule, arg_count: u32) {
    fn opcode_operand_len(opcode: u8) -> Option<usize> {
        match opcode {
            0x00
            | 0x01
            | 0x06
            | 0x11
            | 0x12
            | 0x13
            | 0x14
            | 0x15
            | 0x25
            | 0x23
            | 0x24
            | 0x31
            | 0x32
            | 0x33
            | 0x61
            | 0x40..=0x4E
            | 0x50..=0x55 => Some(0),
            0x02..=0x05 | 0x07 | 0x10 | 0x20..=0x22 | 0x30 | 0x60 | 0x62 | 0x63 | 0x70 => Some(4),
            0x08 => Some(8),
            0x09 => Some(12),
            0x16 => Some(1),
            _ => None,
        }
    }

    let (_, start, end) = main_pou_entry(module);
    let code = match module.section_mut(SectionId::PouBodies) {
        Some(SectionData::PouBodies(code)) => code,
        _ => panic!("missing POU_BODIES"),
    };

    let mut patched = false;
    let mut pc = start;
    while pc < end {
        let opcode = code[pc];
        if opcode == 0x09 {
            if pc + 13 > end {
                panic!("truncated CALL_NATIVE payload");
            }
            code[pc + 9..pc + 13].copy_from_slice(&arg_count.to_le_bytes());
            patched = true;
            break;
        }
        pc += opcode_operand_len(opcode)
            .map(|len| 1 + len)
            .unwrap_or_else(|| panic!("invalid opcode in main body: 0x{opcode:02X}"));
    }

    assert!(patched, "expected at least one CALL_NATIVE in main body");
}

fn patch_first_opcode_u32_operand(module: &mut BytecodeModule, opcode: u8, operand: u32) {
    let (_, start, end) = main_pou_entry(module);
    let code = match module.section_mut(SectionId::PouBodies) {
        Some(SectionData::PouBodies(code)) => code,
        _ => panic!("missing POU_BODIES"),
    };

    let mut patched = false;
    let mut pc = start;
    while pc < end {
        let current = code[pc];
        let operand_len = match current {
            0x00
            | 0x01
            | 0x06
            | 0x11
            | 0x12
            | 0x13
            | 0x14
            | 0x15
            | 0x25
            | 0x23
            | 0x24
            | 0x31
            | 0x32
            | 0x33
            | 0x61
            | 0x40..=0x4E
            | 0x50..=0x55 => 0,
            0x02..=0x05 | 0x07 | 0x10 | 0x20..=0x22 | 0x30 | 0x60 | 0x62 | 0x63 | 0x70 => 4,
            0x08 => 8,
            0x09 => 12,
            0x16 => 1,
            _ => panic!("invalid opcode in main body: 0x{current:02X}"),
        };
        if current == opcode {
            if operand_len < 4 || pc + 5 > end {
                panic!("opcode 0x{opcode:02X} has no u32 operand");
            }
            code[pc + 1..pc + 5].copy_from_slice(&operand.to_le_bytes());
            patched = true;
            break;
        }
        pc += 1 + operand_len;
    }

    assert!(
        patched,
        "expected opcode 0x{opcode:02X} in main body for operand patch"
    );
}

fn first_opcode_u32_operand(module: &BytecodeModule, opcode: u8) -> u32 {
    let (_, start, end) = main_pou_entry(module);
    let code = match module.section(SectionId::PouBodies) {
        Some(SectionData::PouBodies(code)) => code,
        _ => panic!("missing POU_BODIES"),
    };

    let mut pc = start;
    while pc < end {
        let current = code[pc];
        let operand_len = match current {
            0x00
            | 0x01
            | 0x06
            | 0x11
            | 0x12
            | 0x13
            | 0x14
            | 0x15
            | 0x25
            | 0x23
            | 0x24
            | 0x31
            | 0x32
            | 0x33
            | 0x61
            | 0x40..=0x4E
            | 0x50..=0x55 => 0,
            0x02..=0x05 | 0x07 | 0x10 | 0x20..=0x22 | 0x30 | 0x60 | 0x62 | 0x63 | 0x70 => 4,
            0x08 => 8,
            0x09 => 12,
            0x16 => 1,
            _ => panic!("invalid opcode in main body: 0x{current:02X}"),
        };
        if current == opcode {
            if operand_len < 4 || pc + 5 > end {
                panic!("opcode 0x{opcode:02X} has no u32 operand");
            }
            return u32::from_le_bytes([code[pc + 1], code[pc + 2], code[pc + 3], code[pc + 4]]);
        }
        pc += 1 + operand_len;
    }
    panic!("expected opcode 0x{opcode:02X} in main body");
}

#[test]
fn vm_executes_program_with_stack_and_pc_progression() {
    let source = r#"
        PROGRAM Main
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;
    let mut harness = vm_harness(source);
    harness.assert_eq("count", 0i32);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected VM cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("count", 1i32);
}

#[test]
fn vm_opcode_positive_path_covers_arith_logical_branch_jump_load_store_ref() {
    let source = r#"
        PROGRAM Main
        VAR
            i: DINT := 0;
            acc: DINT := 0;
            gate: BOOL := FALSE;
        END_VAR
        WHILE i < 4 DO
            gate := (i < 2) AND TRUE;
            IF gate THEN
                acc := acc + i;
            END_IF;
            i := i + 1;
        END_WHILE;
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(body.contains(&0x02), "expected JUMP opcode in main body");
    assert!(
        body.contains(&0x03) || body.contains(&0x04),
        "expected JUMP_IF_TRUE/FALSE opcode in main body"
    );
    assert!(
        body.contains(&0x20),
        "expected LOAD_REF opcode in main body"
    );
    assert!(
        body.contains(&0x21),
        "expected STORE_REF opcode in main body"
    );
    assert!(body.contains(&0x40), "expected ADD opcode in main body");
    assert!(body.contains(&0x46), "expected AND opcode in main body");

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "opcode positive-path execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("i", 4i32);
    harness.assert_eq("acc", 1i32);
}

#[test]
fn vm_opcode_positive_path_covers_call_native_stdlib_dispatch() {
    let source = r#"
        PROGRAM Main
        VAR
            out_sel : INT := 0;
        END_VAR
        out_sel := SEL(G := TRUE, IN0 := INT#4, IN1 := INT#7);
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x09),
        "expected CALL_NATIVE opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "CALL_NATIVE stdlib dispatch failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_sel", 7i16);
}

#[test]
fn vm_opcode_positive_path_covers_call_native_oop_dispatch() {
    let source = r#"
        INTERFACE ICounter
        METHOD Inc : INT
        VAR_INPUT
            delta : INT;
        END_VAR
        END_METHOD
        END_INTERFACE

        CLASS Counter IMPLEMENTS ICounter
        VAR PUBLIC
            value : INT := INT#0;
        END_VAR
        METHOD PUBLIC Inc : INT
        VAR_INPUT
            delta : INT;
        END_VAR
        value := value + delta;
        Inc := value;
        END_METHOD
        END_CLASS

        FUNCTION_BLOCK ThisCounter
        VAR
            count : INT := INT#5;
        END_VAR
        VAR_OUTPUT
            value : INT;
        END_VAR
        value := THIS.count;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK BaseFb
        VAR PUBLIC
            count : INT := INT#10;
        END_VAR
        METHOD PUBLIC GetCount : INT
        GetCount := count;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK DerivedFb EXTENDS BaseFb
        VAR PUBLIC
            extra : INT := INT#3;
        END_VAR
        METHOD PUBLIC GetCount : INT
        GetCount := count + extra;
        END_METHOD
        METHOD PUBLIC GetSuper : INT
        GetSuper := SUPER.GetCount();
        END_METHOD
        END_FUNCTION_BLOCK

        PROGRAM Main
        VAR
            i : ICounter;
            c : Counter;
            fb_this : ThisCounter;
            fb_derived : DerivedFb;
            out_this : INT := INT#0;
            out_override : INT := INT#0;
            out_super : INT := INT#0;
            out_iface : INT := INT#0;
            out_direct : INT := INT#0;
        END_VAR
        i := c;
        fb_this(value => out_this);
        out_override := fb_derived.GetCount();
        out_super := fb_derived.GetSuper();
        out_iface := i.Inc(INT#1);
        out_direct := c.Inc(INT#2);
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x09),
        "expected CALL_NATIVE opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "CALL_NATIVE OOP dispatch failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_this", 5i16);
    harness.assert_eq("out_override", 13i16);
    harness.assert_eq("out_super", 10i16);
    harness.assert_eq("out_iface", 1i16);
    harness.assert_eq("out_direct", 3i16);
}

#[test]
fn vm_opcode_positive_path_covers_string_and_wstring_literals() {
    let source = r#"
        PROGRAM Main
        VAR
            s : STRING := '';
            ws : WSTRING := "";
            str_eq : BOOL := FALSE;
            wstr_lt : BOOL := FALSE;
        END_VAR
        s := 'AB';
        ws := "CD";
        str_eq := s = 'AB';
        wstr_lt := ws < "CE";
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x10),
        "expected LOAD_CONST opcode for string/wstring literals"
    );
    assert!(body.contains(&0x50), "expected EQ opcode in main body");
    assert!(body.contains(&0x52), "expected LT opcode in main body");

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "string/wstring literal execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("s", Value::String("AB".into()));
    harness.assert_eq("ws", Value::WString("CD".to_string()));
    harness.assert_eq("str_eq", true);
    harness.assert_eq("wstr_lt", true);
}

#[test]
fn vm_opcode_positive_path_covers_dynamic_reference_and_nested_chains() {
    let source = r#"
        TYPE
            Inner : STRUCT
                arr : ARRAY[0..2] OF INT;
            END_STRUCT;
            Outer : STRUCT
                inner : Inner;
            END_STRUCT;
        END_TYPE

        PROGRAM Main
        VAR
            o : Outer;
            idx : INT := INT#1;
            value_cell : INT := INT#4;
            r : REF_TO INT;
            out_ref : INT := INT#0;
            out_nested : INT := INT#0;
        END_VAR
        r := REF(value_cell);
        r^ := r^ + INT#5;
        out_ref := r^;
        out_nested := REF(o)^.inner.arr[idx];
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x30),
        "expected REF_FIELD opcode in main body"
    );
    assert!(
        body.contains(&0x31),
        "expected REF_INDEX opcode in main body"
    );
    assert!(
        body.contains(&0x32),
        "expected LOAD_DYNAMIC opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "dynamic reference opcode execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_ref", 9i16);
    harness.assert_eq("out_nested", 0i16);
}

#[test]
fn vm_opcode_positive_path_covers_nested_field_chain_assignments() {
    let source = r#"
        TYPE
            Inner : STRUCT
                value : INT;
            END_STRUCT;
            Outer : STRUCT
                inner : Inner;
            END_STRUCT;
        END_TYPE

        PROGRAM Main
        VAR
            outer : Outer;
            out_value : INT := INT#0;
        END_VAR
        outer.inner.value := INT#9;
        out_value := outer.inner.value;
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x30),
        "expected REF_FIELD opcode in main body"
    );
    assert!(
        body.contains(&0x32),
        "expected LOAD_DYNAMIC opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "nested field-chain assignment execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_value", 9i16);
}

#[test]
fn vm_opcode_positive_path_covers_nested_field_index_assignments() {
    let source = r#"
        TYPE
            Item : STRUCT
                value : INT;
            END_STRUCT;
            Outer : STRUCT
                arr : ARRAY[0..2] OF INT;
            END_STRUCT;
        END_TYPE

        PROGRAM Main
        VAR
            outer : Outer;
            items : ARRAY[0..1] OF Item;
            idx : INT := INT#1;
            out_field_index : INT := INT#0;
            out_index_field : INT := INT#0;
        END_VAR
        outer.arr[idx] := INT#9;
        items[idx].value := INT#7;
        out_field_index := outer.arr[idx];
        out_index_field := items[idx].value;
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x30),
        "expected REF_FIELD opcode in main body"
    );
    assert!(
        body.contains(&0x31),
        "expected REF_INDEX opcode in main body"
    );
    assert!(
        body.contains(&0x32),
        "expected LOAD_DYNAMIC opcode in main body"
    );
    assert!(
        body.contains(&0x33),
        "expected STORE_DYNAMIC opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "nested field/index assignment execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_field_index", 9i16);
    harness.assert_eq("out_index_field", 7i16);
}

#[test]
fn vm_opcode_positive_path_covers_string_and_wstring_index_reads() {
    let source = r#"
        PROGRAM Main
        VAR
            idx : INT := INT#2;
            text_value : STRING[8] := 'ABCD';
            wide_value : WSTRING[8] := "WXYZ";
            out_char : CHAR;
            out_wchar : WCHAR;
        END_VAR

        out_char := text_value[idx];
        out_wchar := wide_value[idx + INT#1];
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x31),
        "expected REF_INDEX opcode in main body"
    );
    assert!(
        body.contains(&0x32),
        "expected LOAD_DYNAMIC opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "string index execution failed: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("out_char"), Some(Value::Char(b'B')));
    assert_eq!(
        harness.get_output("out_wchar"),
        Some(Value::WChar('Y' as u16))
    );
}

#[test]
fn vm_opcode_positive_path_covers_non_ascii_string_and_wstring_index_reads() {
    let source = r#"
        PROGRAM Main
        VAR
            text_value : STRING[8] := 'ÄBC';
            wide_value : WSTRING[8] := "ÄBC";
            out_char : CHAR;
            out_wchar : WCHAR;
        END_VAR

        out_char := text_value[INT#1];
        out_wchar := wide_value[INT#1];
        END_PROGRAM
    "#;

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "non-ascii string index execution failed: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("out_char"), Some(Value::Char(0xC4)));
    assert_eq!(harness.get_output("out_wchar"), Some(Value::WChar(0x00C4)));
}

#[test]
fn vm_opcode_positive_path_covers_sizeof_type_and_storage_operands() {
    let source = r#"
        PROGRAM Main
        VAR
            out_size_type_int : DINT := DINT#0;
            sized : STRING[5];
            out_size_var_s : DINT := DINT#0;
        END_VAR

        out_size_type_int := SIZEOF(INT);
        out_size_var_s := SIZEOF(sized);
        END_PROGRAM
    "#;
    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x60),
        "expected SIZEOF_TYPE opcode in main body"
    );
    assert!(
        !body.contains(&0x61),
        "did not expect legacy SIZEOF_VALUE opcode in main body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "SIZEOF opcode execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_size_type_int", 2i32);
    harness.assert_eq("out_size_var_s", 5i32);
}

#[test]
fn vm_enforces_execution_deadline() {
    let source = r#"
        PROGRAM Main
        WHILE TRUE DO
        END_WHILE;
        END_PROGRAM
    "#;
    let mut harness = vm_harness(source);
    harness
        .runtime_mut()
        .set_execution_deadline(Instant::now().checked_sub(StdDuration::from_millis(1)));
    let cycle = harness.cycle();
    assert!(
        cycle
            .errors
            .iter()
            .any(|err| matches!(err, RuntimeError::ExecutionTimeout)),
        "expected ExecutionTimeout, got {:?}",
        cycle.errors
    );
}

#[test]
fn vm_rejects_invalid_call_native_symbol_index() {
    let source = r#"
        PROGRAM Main
        VAR
            keep : INT := 0;
        END_VAR
        keep := keep + INT#1;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x09);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(&255_u32.to_le_bytes());
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.push(0x06);
    replace_main_body(&mut module, &body);

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for native symbol");
}

#[test]
fn vm_rejects_invalid_call_native_method_missing_receiver_payload() {
    let source = r#"
        CLASS Counter
        METHOD PUBLIC Next : INT
        Next := INT#1;
        END_METHOD
        END_CLASS

        PROGRAM Main
        VAR
            c : Counter;
            out_next : INT := INT#0;
        END_VAR
        out_next := c.Next();
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    patch_first_call_native_arg_count(&mut module, 0);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(
        &cycle.errors,
        "vm invalid CALL_NATIVE payload: arg_count smaller than native receiver arity",
    );
}

#[test]
fn vm_validator_rejects_invalid_ref_field_string_index() {
    let source = r#"
        TYPE
            Box : STRUCT
                value : INT;
            END_STRUCT;
        END_TYPE

        PROGRAM Main
        VAR
            b : Box;
            out_value : INT := INT#0;
        END_VAR
        b.value := INT#7;
        out_value := REF(b)^.value;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    patch_first_opcode_u32_operand(&mut module, 0x30, 255);

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for string");
}

#[test]
fn vm_rejects_load_dynamic_with_non_reference_operand() {
    let source = r#"
        PROGRAM Main
        VAR
            x : INT := INT#1;
        END_VAR
        x := x;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x20);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.push(0x32);
    body.push(0x06);
    replace_main_body(&mut module, &body);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert!(
        cycle
            .errors
            .iter()
            .any(|err| matches!(err, RuntimeError::TypeMismatch)),
        "expected TypeMismatch for LOAD_DYNAMIC on non-reference, got {:?}",
        cycle.errors
    );
}

#[test]
fn vm_validator_rejects_invalid_sizeof_type_index() {
    let source = r#"
        PROGRAM Main
        VAR
            out_size : DINT := DINT#0;
        END_VAR
        out_size := SIZEOF(INT);
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    patch_first_opcode_u32_operand(&mut module, 0x60, 255);

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for type");
}

#[test]
fn vm_rejects_legacy_sizeof_value_opcode_with_empty_stack() {
    let source = r#"
        PROGRAM Main
        VAR
            out_size : DINT := DINT#0;
        END_VAR
        out_size := DINT#0;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let body = vec![
        0x61, // SIZEOF_VALUE without operand -> stack underflow
        0x06, // RET
    ];
    replace_main_body(&mut module, &body);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(&cycle.errors, "vm operand stack underflow");
}

#[test]
fn vm_rejects_sizeof_type_with_excessive_non_cyclic_alias_depth() {
    let source = r#"
        PROGRAM Main
        VAR
            out_size : DINT := DINT#0;
        END_VAR
        out_size := SIZEOF(INT);
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let int_type_idx = first_opcode_u32_operand(&module, 0x60);
    let alias_start = match module.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(table)) => table.entries.len() as u32,
        _ => panic!("missing TYPE_TABLE"),
    };
    let alias_depth = 129u32;
    if let Some(SectionData::TypeTable(table)) = module.section_mut(SectionId::TypeTable) {
        for i in 0..alias_depth {
            let target_type_id = if i + 1 < alias_depth {
                alias_start + i + 1
            } else {
                int_type_idx
            };
            table.entries.push(TypeEntry {
                kind: TypeKind::Alias,
                name_idx: None,
                data: TypeData::Alias { target_type_id },
            });
        }
    } else {
        panic!("missing TYPE_TABLE");
    }
    patch_first_opcode_u32_operand(&mut module, 0x60, alias_start);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(&cycle.errors, "SIZEOF type nesting exceeds max depth");
}

#[test]
fn vm_lowering_supports_exit_and_continue_in_loop_stmt_paths() {
    let source = r#"
        PROGRAM Main
        VAR
            i : INT := INT#0;
            w : INT := INT#0;
            r : INT := INT#0;
            sum_for : INT := INT#0;
            sum_while : INT := INT#0;
            sum_repeat : INT := INT#0;
        END_VAR

        FOR i := INT#0 TO INT#4 BY INT#1 DO
            IF i = INT#1 THEN
                CONTINUE;
            END_IF;
            IF i = INT#3 THEN
                EXIT;
            END_IF;
            sum_for := sum_for + i;
        END_FOR;

        WHILE w < INT#5 DO
            w := w + INT#1;
            IF w = INT#2 THEN
                CONTINUE;
            END_IF;
            IF w = INT#4 THEN
                EXIT;
            END_IF;
            sum_while := sum_while + w;
        END_WHILE;

        REPEAT
            r := r + INT#1;
            IF r = INT#2 THEN
                CONTINUE;
            END_IF;
            IF r = INT#4 THEN
                EXIT;
            END_IF;
            sum_repeat := sum_repeat + r;
        UNTIL r >= INT#6 END_REPEAT;
        END_PROGRAM
    "#;

    let module = bytecode_module_from_source(source).expect("compile bytecode module");
    let body = main_body_bytes(&module);
    assert!(
        body.contains(&0x02),
        "expected JMP opcode in main loop body"
    );
    assert!(
        body.contains(&0x03),
        "expected JMP_IF_TRUE opcode in main loop body"
    );
    assert!(
        body.contains(&0x04),
        "expected JMP_IF_FALSE opcode in main loop body"
    );

    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "vm execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("sum_for", 2i16);
    harness.assert_eq("sum_while", 4i16);
    harness.assert_eq("sum_repeat", 4i16);
}

#[test]
fn vm_lowering_rejects_unsupported_c5_edge_case_stmt_paths() {
    let source = r#"
        PROGRAM Main
        VAR
            x : INT := INT#0;
        END_VAR
        JMP L1;
        x := INT#1;
        L1: x := x + INT#2;
        END_PROGRAM
    "#;
    let err =
        bytecode_module_from_source(source).expect_err("expected deterministic lowering error");
    let message = err.to_string();
    assert!(
        message.contains("unsupported C5 edge-case lowering path"),
        "expected deterministic C5 lowering rejection, got: {message}"
    );
}

#[test]
fn vm_rejects_invalid_string_const_utf8_payload() {
    let source = r#"
        PROGRAM Main
        VAR
            s : STRING := '';
        END_VAR
        s := 'A';
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    mutate_first_const_payload_for_primitive(&mut module, 24, vec![0xFF]);

    assert_apply_invalid_bytecode_contains(&module, "invalid STRING const UTF-8");
}

#[test]
fn vm_rejects_invalid_wstring_const_utf16_payload() {
    let source = r#"
        PROGRAM Main
        VAR
            ws : WSTRING := "";
        END_VAR
        ws := "A";
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    mutate_first_const_payload_for_primitive(&mut module, 25, vec![0x41]);

    assert_apply_invalid_bytecode_contains(&module, "invalid WSTRING const payload length");
}

#[test]
fn vm_enforces_instruction_budget() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x02);
    body.extend_from_slice(&(-5_i32).to_le_bytes());
    replace_main_body(&mut module, &body);

    let mut harness = vm_harness_from_module(source, &module);
    harness.runtime_mut().set_execution_deadline(None);
    let cycle = harness.cycle();
    assert!(
        cycle
            .errors
            .iter()
            .any(|err| matches!(err, RuntimeError::ExecutionTimeout)),
        "expected ExecutionTimeout from instruction budget, got {:?}",
        cycle.errors
    );
}

#[test]
fn vm_validator_rejects_invalid_ref_index_operand() {
    let source = r#"
        PROGRAM Main
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile bytecode module");
    let (main_offset, main_length) = {
        let strings = match module.section(SectionId::StringTable) {
            Some(SectionData::StringTable(strings)) => strings,
            _ => panic!("missing string table"),
        };
        let index = match module.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            _ => panic!("missing pou index"),
        };
        let main = index
            .entries
            .iter()
            .find(|entry| {
                entry.kind == PouKind::Program
                    && strings.entries[entry.name_idx as usize].eq_ignore_ascii_case("MAIN")
            })
            .expect("main entry");
        (main.code_offset as usize, main.code_length as usize)
    };
    if main_length < 5 {
        panic!("main body too short for patch");
    }
    if let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) {
        code[main_offset] = 0x20;
        code[main_offset + 1..main_offset + 5].copy_from_slice(&255_u32.to_le_bytes());
    } else {
        panic!("missing POU_BODIES");
    }

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for ref");
}

#[test]
fn vm_validator_rejects_invalid_const_index_operand() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x10);
    body.extend_from_slice(&255_u32.to_le_bytes());
    body.push(0x06);
    replace_main_body(&mut module, &body);

    assert_apply_invalid_bytecode_contains(&module, "invalid index 255 for const");
}

#[test]
fn vm_validator_rejects_duplicate_pou_ids() {
    let source = r#"
        FUNCTION Helper : DINT
            Helper := DINT#1;
        END_FUNCTION

        PROGRAM Main
        VAR
            value : DINT := DINT#0;
        END_VAR
            value := Helper();
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        assert!(index.entries.len() >= 2, "expected multiple POU entries");
        let duplicate_id = index.entries[0].id;
        index.entries[1].id = duplicate_id;
    } else {
        panic!("missing POU_INDEX");
    }

    assert_apply_invalid_bytecode_contains(&module, "duplicate POU id");
}

#[test]
fn vm_rejects_invalid_opcode() {
    let source = r#"
        PROGRAM Main
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile bytecode module");
    let (_, main_offset, _) = main_pou_entry(&module);
    if let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) {
        code[main_offset] = 0xFF;
    } else {
        panic!("missing POU_BODIES");
    }

    assert_apply_invalid_bytecode_contains(&module, "invalid opcode 0xFF");
}

#[test]
fn vm_rejects_malformed_operands() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    replace_main_body(&mut module, &[0x20]);

    assert_apply_invalid_bytecode_contains(&module, "unexpected end of input");
}

#[test]
fn vm_rejects_invalid_jump_target() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x02);
    body.extend_from_slice(&(4_096_i32).to_le_bytes());
    body.push(0x06);
    replace_main_body(&mut module, &body);

    assert_apply_invalid_bytecode_contains(&module, "invalid jump target");
}

#[test]
fn vm_validator_rejects_unsupported_call_method_opcode() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let mut body = Vec::new();
    body.push(0x07);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.push(0x06);
    replace_main_body(&mut module, &body);

    assert_apply_invalid_bytecode_contains(&module, "unsupported runtime opcode CALL_METHOD");
}

#[test]
fn vm_rejects_stack_underflow() {
    let source = r#"
        PROGRAM Main
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    replace_main_body(&mut module, &[0x12, 0x06]);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(&cycle.errors, "vm operand stack underflow");
}

#[test]
fn vm_rejects_stack_overflow() {
    let source = r#"
        PROGRAM Main
        VAR
            keep: DINT := 1;
        END_VAR
        keep := keep + 0;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let const_idx = match module.section(SectionId::ConstPool) {
        Some(SectionData::ConstPool(pool)) if !pool.entries.is_empty() => 0_u32,
        Some(_) => panic!("expected const pool entries for overflow fixture"),
        _ => panic!("missing CONST_POOL"),
    };

    let mut body = Vec::new();
    body.push(0x10);
    body.extend_from_slice(&const_idx.to_le_bytes());
    body.push(0x11);
    body.push(0x02);
    body.extend_from_slice(&(-6_i32).to_le_bytes());
    replace_main_body(&mut module, &body);

    let mut harness = vm_harness_from_module(source, &module);
    let cycle = harness.cycle();
    assert_invalid_bytecode_contains(&cycle.errors, "vm operand stack overflow");
}

#[test]
fn vm_call_stack_handles_call_and_return() {
    let source = r#"
        FUNCTION Foo : DINT
        Foo := 1;
        END_FUNCTION

        PROGRAM Main
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;
    let mut module = bytecode_module_from_source(source).expect("compile module");
    let (main_id, foo_id) = {
        let strings = match module.section(SectionId::StringTable) {
            Some(SectionData::StringTable(strings)) => strings,
            _ => panic!("missing string table"),
        };
        let index = match module.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            _ => panic!("missing pou index"),
        };
        let mut main_id = None;
        let mut foo_id = None;
        for entry in &index.entries {
            let name = &strings.entries[entry.name_idx as usize];
            if name.eq_ignore_ascii_case("MAIN") {
                main_id = Some(entry.id);
            }
            if name.eq_ignore_ascii_case("FOO") {
                foo_id = Some(entry.id);
            }
        }
        (main_id.expect("Main POU id"), foo_id.expect("Foo POU id"))
    };

    let main_body = {
        let mut bytes = Vec::new();
        bytes.push(0x05);
        bytes.extend_from_slice(&foo_id.to_le_bytes());
        bytes.push(0x00);
        bytes.push(0x06);
        bytes
    };
    let foo_body = vec![0x06];

    let (main_offset, foo_offset) =
        if let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) {
            let main_offset = code.len() as u32;
            code.extend_from_slice(&main_body);
            let foo_offset = code.len() as u32;
            code.extend_from_slice(&foo_body);
            (main_offset, foo_offset)
        } else {
            panic!("missing POU_BODIES");
        };
    if let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) {
        for entry in &mut index.entries {
            if entry.id == main_id {
                entry.code_offset = main_offset;
                entry.code_length = main_body.len() as u32;
            } else if entry.id == foo_id {
                entry.code_offset = foo_offset;
                entry.code_length = foo_body.len() as u32;
            }
        }
    }
    module.sections.retain(|section| {
        section.id != SectionId::DebugMap.as_raw()
            && section.id != SectionId::DebugStringTable.as_raw()
    });

    let bytes = module.encode().expect("encode module");
    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply bytecode");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");

    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "call/ret VM execution should succeed, got {:?}",
        cycle.errors
    );
}
