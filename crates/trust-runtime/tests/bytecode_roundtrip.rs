#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::module_with_debug;
use trust_runtime::bytecode::{BytecodeError, BytecodeModule};

#[test]
fn roundtrip() {
    let module = module_with_debug();
    let bytes = module.encode().expect("encode");
    let decoded = BytecodeModule::decode(&bytes).expect("decode");
    assert_eq!(decoded, module);
}

#[test]
fn corruption_rejected() {
    let mut module = module_with_debug();
    module.flags = 0;
    let mut bytes = module.encode().expect("encode");
    // Corrupt the first string length to be too large.
    let section_table_off = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let first_entry = section_table_off;
    let payload_off =
        u32::from_le_bytes(bytes[first_entry + 4..first_entry + 8].try_into().unwrap()) as usize;
    // Overwrite the first string length field.
    bytes[payload_off + 4..payload_off + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    let err = BytecodeModule::decode(&bytes).unwrap_err();
    assert!(matches!(
        err,
        BytecodeError::UnexpectedEof | BytecodeError::InvalidSection(_)
    ));
}
