#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::{base_module, module_with_debug};
use trust_runtime_core::bytecode::{
    BytecodeError, BytecodeModule, BytecodeVersion, RefLocation, RetainInit, Section, SectionData,
    SectionId, VarMeta,
};

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
fn duplicate_standard_section_ids_are_rejected() {
    let mut module = module_with_debug();
    module.flags = 0;
    module.sections.extend([
        Section {
            id: SectionId::VarMeta.as_raw(),
            flags: 0,
            data: SectionData::VarMeta(VarMeta::default()),
        },
        Section {
            id: SectionId::RetainInit.as_raw(),
            flags: 0,
            data: SectionData::RetainInit(RetainInit::default()),
        },
    ]);

    for section in module.sections.clone() {
        let mut duplicated = module.clone();
        duplicated.sections.push(section.clone());
        let bytes = duplicated.encode().expect("encode duplicate section");
        let err = BytecodeModule::decode(&bytes).unwrap_err();
        let expected = format!("duplicate standardized section id 0x{:04X}", section.id);
        assert!(
            matches!(
                err,
                BytecodeError::InvalidSection(ref message) if message == &expected
            ),
            "unexpected decoder result for section 0x{:04X}: {err:?}",
            section.id
        );
    }
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
    module.version = BytecodeVersion::new(BytecodeVersion::SOURCE_FREE.major + 1, 0);
    let bytes = module.encode().expect("encode");
    let err = BytecodeModule::decode(&bytes).unwrap_err();
    assert!(matches!(err, BytecodeError::UnsupportedVersion { .. }));
}

#[test]
fn truncated_reference_owner_field_is_rejected() {
    let mut payload = 1_u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&[RefLocation::Instance as u8, 0, 0, 0]);

    let err = decode_raw_section(SectionId::RefTable, payload);

    assert!(
        matches!(err, BytecodeError::InvalidSection(ref message) if message == "REF_TABLE count exceeds section bounds"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn unsupported_reference_location_tag_is_rejected() {
    let mut payload = 1_u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&[0xFF, 0, 0, 0]);
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());

    let err = decode_raw_section(SectionId::RefTable, payload);

    assert!(
        matches!(err, BytecodeError::InvalidSection(ref message) if message == "invalid ref location"),
        "unexpected error: {err:?}"
    );
}

fn decode_raw_section(section_id: SectionId, payload: Vec<u8>) -> BytecodeError {
    let mut module = BytecodeModule::new(BytecodeVersion::new(1, 1));
    module.flags = 0;
    module.sections = vec![Section {
        id: section_id.as_raw(),
        flags: 0,
        data: SectionData::Raw(payload),
    }];
    let bytes = module.encode().expect("encode raw section");
    BytecodeModule::decode(&bytes).expect_err("malformed raw section must be rejected")
}

#[test]
fn section_end_beyond_32_bit_address_space_is_rejected() {
    let mut module = base_module();
    module.flags = 0;
    let mut bytes = module.encode().expect("encode");
    bytes[28..32].copy_from_slice(&0xFFFF_FFFCu32.to_le_bytes());
    bytes[32..36].copy_from_slice(&8u32.to_le_bytes());
    assert_eq!(
        BytecodeModule::decode(&bytes),
        Err(BytecodeError::SectionOutOfBounds)
    );
}

#[test]
fn section_payloads_cannot_alias_header_or_section_table() {
    for offset in [0, 24] {
        let mut module = base_module();
        module.flags = 0;
        let mut bytes = module.encode().unwrap();
        bytes[28..32].copy_from_slice(&u32::try_from(offset).unwrap().to_le_bytes());
        assert_eq!(
            BytecodeModule::decode(&bytes),
            Err(BytecodeError::SectionOverlap)
        );
    }
}

#[test]
fn declared_header_alignment_and_table_boundary_are_checked() {
    let bytes = base_module().encode().unwrap();
    let mut unaligned = bytes.clone();
    unaligned[12..14].copy_from_slice(&25u16.to_le_bytes());
    assert_eq!(
        BytecodeModule::decode(&unaligned),
        Err(BytecodeError::SectionAlignment)
    );
    let mut overlap = bytes;
    overlap[12..14].copy_from_slice(&28u16.to_le_bytes());
    assert!(matches!(
        BytecodeModule::decode(&overlap),
        Err(BytecodeError::InvalidHeader(_))
    ));
}

#[test]
fn encoder_rejects_section_count_before_serializing_payloads() {
    use trust_runtime_core::bytecode::{Section, SectionData};
    let mut module = BytecodeModule::new(BytecodeVersion::new(1, 1));
    module
        .sections
        .resize_with(u16::MAX as usize + 1, || Section {
            id: 0x8000,
            flags: 0,
            data: SectionData::Raw(Vec::new()),
        });
    assert_eq!(
        module.encode().unwrap_err(),
        BytecodeError::InvalidHeader("section count overflow".into())
    );
}
