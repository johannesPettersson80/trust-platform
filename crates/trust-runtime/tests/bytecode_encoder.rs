use trust_runtime::bytecode::{
    BytecodeModule, InterfaceMethod, PouKind, RefLocation, SectionData, SectionId, StringTable,
    TypeData, TypeEntry, TypeKind, TypeTable,
};
use trust_runtime::harness::{
    bytecode_bytes_from_source, bytecode_module_from_source, TestHarness,
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
        let operand_len = trust_runtime_core::vm::register_test_support::opcode_operand_len(opcode)
            .unwrap_or_else(|| panic!("unknown producer opcode 0x{opcode:02X}"));
        assert!(
            i + operand_len <= code.len(),
            "truncated producer instruction"
        );
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

#[path = "bytecode_encoder/bytecode_encoder_part_01.rs"]
mod bytecode_encoder_part_01;
#[path = "bytecode_encoder/bytecode_encoder_part_02.rs"]
mod bytecode_encoder_part_02;
#[path = "bytecode_encoder/bytecode_encoder_part_03.rs"]
mod bytecode_encoder_part_03;
#[path = "bytecode_encoder/bytecode_encoder_part_04.rs"]
mod bytecode_encoder_part_04;
#[path = "bytecode_encoder/bytecode_encoder_part_05.rs"]
mod bytecode_encoder_part_05;
