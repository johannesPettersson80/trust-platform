mod bytecode_helpers;

use bytecode_helpers::{base_module, module_with_debug};
use trust_runtime::bytecode::{
    BytecodeError, BytecodeModule, BytecodeVersion, RefLocation, RetainInit, Section, SectionData,
    SectionId, VarMeta, SUPPORTED_MAJOR_VERSION,
};
use trust_runtime::Runtime;

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
    module.version = BytecodeVersion::new(SUPPORTED_MAJOR_VERSION + 1, 0);
    let bytes = module.encode().expect("encode");
    let err = BytecodeModule::decode(&bytes).unwrap_err();
    assert!(matches!(err, BytecodeError::UnsupportedVersion { .. }));
}

#[test]
fn unknown_optional_section_is_preserved_and_ignored_by_runtime_apply() {
    const UNKNOWN_SECTION_ID: u16 = 0x7FFF;
    const PAYLOAD: &[u8] = b"optional-v1-extension";

    let mut module =
        trust_runtime::harness::bytecode_module_from_source("PROGRAM Main\nEND_PROGRAM\n")
            .expect("compile runtime-resolvable bytecode module");
    module.sections.push(Section {
        id: UNKNOWN_SECTION_ID,
        flags: 0,
        data: SectionData::Raw(PAYLOAD.to_vec()),
    });

    let bytes = module.encode().expect("encode unknown optional section");
    let decoded = BytecodeModule::decode(&bytes).expect("decode unknown optional section");
    decoded
        .validate()
        .expect("unknown optional section must not invalidate STBC v1");
    assert!(decoded.sections.iter().any(|section| {
        section.id == UNKNOWN_SECTION_ID
            && section.flags == 0
            && matches!(&section.data, SectionData::Raw(raw) if raw == PAYLOAD)
    }));

    Runtime::new()
        .apply_bytecode_bytes(&bytes, None)
        .expect("runtime apply must ignore an unknown optional section");
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
