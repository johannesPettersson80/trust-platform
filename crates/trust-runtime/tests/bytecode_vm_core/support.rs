use std::time::{Duration as StdDuration, Instant};

use trust_runtime::bytecode::{
    BytecodeModule, PouKind, RefEntry, RefLocation, RefSegment, SectionData, SectionId, TypeData,
    TypeEntry, TypeKind,
};
use trust_runtime::error::RuntimeError;
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{
    bytecode_bytes_from_source, bytecode_module_from_source, TestHarness,
};
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

