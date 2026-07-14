mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime::bytecode::{
    BytecodeError, BytecodeModule, BytecodeVersion, SectionId, SUPPORTED_MAJOR_VERSION,
};

fn section_payload_offset(bytes: &[u8], section_id: SectionId) -> usize {
    let section_count = u16::from_le_bytes(bytes[14..16].try_into().unwrap()) as usize;
    let section_table_off = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;

    for index in 0..section_count {
        let entry = section_table_off + index * 12;
        let id = u16::from_le_bytes(bytes[entry..entry + 2].try_into().unwrap());
        if id == section_id.as_raw() {
            return u32::from_le_bytes(bytes[entry + 4..entry + 8].try_into().unwrap()) as usize;
        }
    }

    panic!("section {section_id:?} missing from encoded module");
}

#[test]
fn header_validation() {
    let module = base_module();
    let mut bytes = module.encode().expect("encode");
    bytes[0] = 0x00;
    let err = BytecodeModule::decode(&bytes).unwrap_err();
    assert!(matches!(err, BytecodeError::InvalidMagic));
}

#[test]
fn section_table_validation() {
    let mut module = base_module();
    module.flags = 0;
    let bytes = module.encode().expect("encode");

    // Out of bounds offset for first section entry.
    let mut out_of_bounds = bytes.clone();
    let bad_offset = (out_of_bounds.len() as u32 + 4).to_le_bytes();
    out_of_bounds[28..32].copy_from_slice(&bad_offset);
    let err = BytecodeModule::decode(&out_of_bounds).unwrap_err();
    assert!(matches!(err, BytecodeError::SectionOutOfBounds));

    // Overlapping offsets between first and second section entries.
    let mut overlap = bytes.clone();
    let first_offset = &bytes[28..32];
    overlap[40..44].copy_from_slice(first_offset);
    let err = BytecodeModule::decode(&overlap).unwrap_err();
    assert!(matches!(err, BytecodeError::SectionOverlap));
}

#[test]
fn checksum_validation() {
    let module = base_module();
    let mut bytes = module.encode().expect("encode");
    let section_table_off = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    bytes[section_table_off] ^= 0xFF;
    let err = BytecodeModule::decode(&bytes).unwrap_err();
    assert!(matches!(err, BytecodeError::InvalidChecksum { .. }));
}

#[test]
fn version_gate() {
    let mut module = base_module();
    module.version = BytecodeVersion::new(SUPPORTED_MAJOR_VERSION + 1, 0);
    let bytes = module.encode().expect("encode");
    let err = BytecodeModule::decode(&bytes).unwrap_err();
    assert!(matches!(err, BytecodeError::UnsupportedVersion { .. }));
}

#[test]
fn string_table_count_must_fit_section_before_allocation() {
    let mut module = base_module();
    module.flags = 0;
    let mut bytes = module.encode().expect("encode");
    let payload = section_payload_offset(&bytes, SectionId::StringTable);
    bytes[payload..payload + 4].copy_from_slice(&4096u32.to_le_bytes());

    let err = BytecodeModule::decode(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            BytecodeError::InvalidSection(ref message)
                if message == "STRING_TABLE count exceeds section bounds"
        ),
        "unexpected decoder result: {err:?}"
    );
}

#[test]
fn type_table_count_must_fit_offset_table_before_allocation() {
    let mut module = base_module();
    module.flags = 0;
    let mut bytes = module.encode().expect("encode");
    let payload = section_payload_offset(&bytes, SectionId::TypeTable);
    bytes[payload..payload + 4].copy_from_slice(&4096u32.to_le_bytes());

    let err = BytecodeModule::decode(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            BytecodeError::InvalidSection(ref message)
                if message == "TYPE_TABLE count exceeds section bounds"
        ),
        "unexpected decoder result: {err:?}"
    );
}
