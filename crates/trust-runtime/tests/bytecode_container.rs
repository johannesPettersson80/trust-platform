mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime::bytecode::{
    BytecodeError, BytecodeModule, BytecodeVersion, Section, SectionData, SectionId,
    SUPPORTED_MAJOR_VERSION,
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

fn decode_raw_section(section_id: SectionId, payload: Vec<u8>) -> BytecodeError {
    let mut module = BytecodeModule::new(BytecodeVersion::new(1, 1));
    module.flags = 0;
    module.sections = vec![Section {
        id: section_id.as_raw(),
        flags: 0,
        data: SectionData::Raw(payload),
    }];
    let bytes = module.encode().expect("encode raw section");
    BytecodeModule::decode(&bytes).unwrap_err()
}

fn assert_count_rejected(section_id: SectionId, payload: Vec<u8>, expected: &str) {
    let err = decode_raw_section(section_id, payload);
    assert!(
        matches!(
            err,
            BytecodeError::InvalidSection(ref message) if message == expected
        ),
        "unexpected decoder result for {section_id:?}: {err:?}"
    );
}

fn append_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn type_table_entry(kind: u8, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    append_u32(&mut payload, 1);
    append_u32(&mut payload, 8);
    payload.extend_from_slice(&[kind, 0, 0, 0]);
    append_u32(&mut payload, u32::MAX);
    payload.extend_from_slice(data);
    payload
}

fn pou_index_entry(kind: u8, param_count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    append_u32(&mut payload, 1);
    append_u32(&mut payload, 1);
    append_u32(&mut payload, 0);
    payload.extend_from_slice(&[kind, 0, 0, 0]);
    for _ in 0..6 {
        append_u32(&mut payload, 0);
    }
    append_u32(&mut payload, param_count);
    payload
}

fn resource_with_task_prefix(program_count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    append_u32(&mut payload, 1);
    for _ in 0..4 {
        append_u32(&mut payload, 0);
    }
    append_u32(&mut payload, 1);
    append_u32(&mut payload, 0);
    append_u32(&mut payload, 0);
    payload.extend_from_slice(&0i64.to_le_bytes());
    append_u32(&mut payload, u32::MAX);
    append_u32(&mut payload, program_count);
    payload
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

#[test]
fn fixed_section_entry_counts_must_fit_before_allocation() {
    let cases = [
        (
            SectionId::DebugStringTable,
            "DEBUG_STRING_TABLE count exceeds section bounds",
        ),
        (
            SectionId::ConstPool,
            "CONST_POOL count exceeds section bounds",
        ),
        (
            SectionId::RefTable,
            "REF_TABLE count exceeds section bounds",
        ),
        (
            SectionId::PouIndex,
            "POU_INDEX count exceeds section bounds",
        ),
        (
            SectionId::ResourceMeta,
            "RESOURCE_META count exceeds section bounds",
        ),
        (SectionId::IoMap, "IO_MAP count exceeds section bounds"),
        (
            SectionId::DebugMap,
            "DEBUG_MAP count exceeds section bounds",
        ),
        (SectionId::VarMeta, "VAR_META count exceeds section bounds"),
        (
            SectionId::RetainInit,
            "RETAIN_INIT count exceeds section bounds",
        ),
    ];

    for (section_id, expected) in cases {
        assert_count_rejected(section_id, 4096u32.to_le_bytes().to_vec(), expected);
    }
}

#[test]
fn nested_type_counts_must_fit_before_allocation() {
    let mut array = Vec::new();
    append_u32(&mut array, 0);
    append_u32(&mut array, 4096);
    let cases = [
        (
            type_table_entry(1, &array),
            "TYPE_TABLE array dimension count exceeds section bounds",
        ),
        (
            type_table_entry(2, &4096u32.to_le_bytes()),
            "TYPE_TABLE struct field count exceeds section bounds",
        ),
        (
            {
                let mut data = Vec::new();
                append_u32(&mut data, 0);
                append_u32(&mut data, 4096);
                type_table_entry(3, &data)
            },
            "TYPE_TABLE enum variant count exceeds section bounds",
        ),
        (
            type_table_entry(7, &4096u32.to_le_bytes()),
            "TYPE_TABLE union field count exceeds section bounds",
        ),
        (
            type_table_entry(10, &4096u32.to_le_bytes()),
            "TYPE_TABLE interface method count exceeds section bounds",
        ),
    ];

    for (payload, expected) in cases {
        assert_count_rejected(SectionId::TypeTable, payload, expected);
    }
}

#[test]
fn nested_reference_counts_must_fit_before_allocation() {
    let mut segments = Vec::new();
    append_u32(&mut segments, 1);
    segments.extend_from_slice(&[0, 0, 0, 0]);
    append_u32(&mut segments, 0);
    append_u32(&mut segments, 0);
    append_u32(&mut segments, 4096);
    assert_count_rejected(
        SectionId::RefTable,
        segments,
        "REF_TABLE segment count exceeds section bounds",
    );

    let mut indices = Vec::new();
    append_u32(&mut indices, 1);
    indices.extend_from_slice(&[0, 0, 0, 0]);
    append_u32(&mut indices, 0);
    append_u32(&mut indices, 0);
    append_u32(&mut indices, 1);
    indices.extend_from_slice(&[0, 0, 0, 0]);
    append_u32(&mut indices, 4096);
    assert_count_rejected(
        SectionId::RefTable,
        indices,
        "REF_TABLE index count exceeds section bounds",
    );
}

#[test]
fn nested_pou_counts_must_fit_before_allocation() {
    assert_count_rejected(
        SectionId::PouIndex,
        pou_index_entry(0, 4096),
        "POU_INDEX parameter count exceeds section bounds",
    );

    let mut interfaces = pou_index_entry(3, 0);
    append_u32(&mut interfaces, u32::MAX);
    append_u32(&mut interfaces, 4096);
    assert_count_rejected(
        SectionId::PouIndex,
        interfaces,
        "POU_INDEX interface count exceeds section bounds",
    );

    let mut slots = pou_index_entry(3, 0);
    append_u32(&mut slots, u32::MAX);
    append_u32(&mut slots, 1);
    append_u32(&mut slots, 0);
    append_u32(&mut slots, 4096);
    assert_count_rejected(
        SectionId::PouIndex,
        slots,
        "POU_INDEX vtable slot count exceeds section bounds",
    );

    let mut methods = pou_index_entry(3, 0);
    append_u32(&mut methods, u32::MAX);
    append_u32(&mut methods, 0);
    append_u32(&mut methods, 4096);
    assert_count_rejected(
        SectionId::PouIndex,
        methods,
        "POU_INDEX method count exceeds section bounds",
    );
}

#[test]
fn nested_resource_counts_must_fit_before_allocation() {
    let mut tasks = Vec::new();
    append_u32(&mut tasks, 1);
    for _ in 0..4 {
        append_u32(&mut tasks, 0);
    }
    append_u32(&mut tasks, 4096);
    assert_count_rejected(
        SectionId::ResourceMeta,
        tasks,
        "RESOURCE_META task count exceeds section bounds",
    );

    assert_count_rejected(
        SectionId::ResourceMeta,
        resource_with_task_prefix(4096),
        "RESOURCE_META program count exceeds section bounds",
    );

    let mut fb_refs = resource_with_task_prefix(0);
    append_u32(&mut fb_refs, 4096);
    assert_count_rejected(
        SectionId::ResourceMeta,
        fb_refs,
        "RESOURCE_META function-block reference count exceeds section bounds",
    );
}
