#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use bytecode_helpers::base_module;
use trust_runtime_core::bytecode::{
    BytecodeError, BytecodeModule, BytecodeVersion, RejectionReason, Section, SectionData,
    SectionId, BYTECODE_MAX_CONSTRUCTION_RECORDS, BYTECODE_MAX_LOCALS_PER_POU,
    BYTECODE_MAX_PARAMETERS_PER_POU, BYTECODE_MAX_REFERENCES,
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

// Overwrites the little-endian u32 at this word index of a raw payload.
fn set_word(payload: &mut [u8], word: usize, value: u32) {
    payload[word * 4..word * 4 + 4].copy_from_slice(&value.to_le_bytes());
}

fn decode_section_at(
    version: BytecodeVersion,
    section_id: SectionId,
    payload: Vec<u8>,
) -> Result<BytecodeModule, BytecodeError> {
    let mut module = BytecodeModule::new(version);
    module.sections = vec![Section {
        id: section_id.as_raw(),
        flags: 0,
        data: SectionData::Raw(payload),
    }];
    let bytes = module.encode().expect("encode raw section");
    BytecodeModule::decode(&bytes)
}

fn record_count(section: Option<&SectionData>) -> Option<usize> {
    match section? {
        SectionData::StorageLayout(layout) => Some(layout.entries.len()),
        SectionData::ConstructionRoots(roots) => Some(roots.entries.len()),
        SectionData::Initializers(index) => Some(index.entries.len()),
        SectionData::AccessBindings(bindings) => Some(bindings.entries.len()),
        _ => None,
    }
}

fn assert_rejected_at(
    version: BytecodeVersion,
    section_id: SectionId,
    payload: Vec<u8>,
    expected: &str,
) {
    let err = decode_section_at(version, section_id, payload).unwrap_err();
    assert!(
        matches!(err, BytecodeError::InvalidSection(ref message) if message == expected),
        "unexpected decoder result for {section_id:?}: {err:?}"
    );
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
fn allocation_pressure_from_declared_counts_fails_before_reservation() {
    for section_id in [
        SectionId::StringTable,
        SectionId::ConstPool,
        SectionId::ResourceMeta,
    ] {
        let error = decode_raw_section(section_id, u32::MAX.to_le_bytes().to_vec());
        assert!(
            matches!(error, BytecodeError::InvalidSection(ref message) if message.contains("count exceeds section bounds")),
            "{section_id:?} must reject impossible allocation count, got {error:?}"
        );
    }
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
        pou_index_entry(0, BYTECODE_MAX_PARAMETERS_PER_POU as u32),
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

    let mut programs = resource_with_task_prefix(4096);
    append_u32(&mut programs, 0);
    assert_count_rejected(
        SectionId::ResourceMeta,
        programs,
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

#[test]
fn ref_table_count_admits_the_exact_limit_and_refuses_one_more() {
    // Site: decode/section_decode.rs `read_ref_table_count`, then decode.rs `read_bounded_count`.
    // A 16-byte record is location, flags, reserved, owner, offset and a zero segment count.
    let mut payload = vec![0u8; 4 + 16 * BYTECODE_MAX_REFERENCES];
    set_word(&mut payload, 0, BYTECODE_MAX_REFERENCES as u32);
    let decoded = decode_section_at(BytecodeVersion::new(1, 1), SectionId::RefTable, payload)
        .expect("65,536 references are admitted");
    let Some(SectionData::RefTable(table)) = decoded.section(SectionId::RefTable) else {
        panic!("REF_TABLE missing from decoded module");
    };
    assert_eq!(table.entries.len(), BYTECODE_MAX_REFERENCES);

    // The count limit is checked before the byte bound, so no records follow.
    assert_count_rejected(
        SectionId::RefTable,
        65_537u32.to_le_bytes().to_vec(),
        "REF_TABLE entries exceed fixed resource limit",
    );
}

#[test]
fn pou_local_and_parameter_counts_admit_the_exact_limit_and_refuse_one_more() {
    // Sites: decode/section_decode.rs:145-146 (local count), :157-162 (parameter count,
    // limit checked before the byte bound). Word 0 is the section count; word 7 of the
    // one-entry payload is local_ref_count.
    let mut locals = pou_index_entry(0, 0);
    set_word(&mut locals, 7, BYTECODE_MAX_LOCALS_PER_POU as u32);
    let decoded = decode_section_at(BytecodeVersion::new(1, 1), SectionId::PouIndex, locals)
        .expect("65,536 POU local references are admitted");
    let Some(SectionData::PouIndex(index)) = decoded.section(SectionId::PouIndex) else {
        panic!("POU_INDEX missing from decoded module");
    };
    assert_eq!(
        index.entries[0].local_ref_count,
        BYTECODE_MAX_LOCALS_PER_POU as u32
    );

    let mut over_locals = pou_index_entry(0, 0);
    set_word(&mut over_locals, 7, BYTECODE_MAX_LOCALS_PER_POU as u32 + 1);
    assert_eq!(
        decode_section_at(BytecodeVersion::new(1, 1), SectionId::PouIndex, over_locals)
            .unwrap_err(),
        BytecodeError::from(RejectionReason::PouIndexLocalReferenceCountExceedsFixedResourceLimit,)
    );

    // A 1.1 parameter record is 16 bytes: name, type, direction, flags, reserved, default.
    let mut params = pou_index_entry(0, BYTECODE_MAX_PARAMETERS_PER_POU as u32);
    params.extend(std::iter::repeat_n(
        0u8,
        16 * BYTECODE_MAX_PARAMETERS_PER_POU,
    ));
    let decoded = decode_section_at(BytecodeVersion::new(1, 1), SectionId::PouIndex, params)
        .expect("1,024 POU parameters are admitted");
    let Some(SectionData::PouIndex(index)) = decoded.section(SectionId::PouIndex) else {
        panic!("POU_INDEX missing from decoded module");
    };
    assert_eq!(
        index.entries[0].params.len(),
        BYTECODE_MAX_PARAMETERS_PER_POU
    );

    // The parameter limit is checked before the byte bound, so no records follow.
    assert_count_rejected(
        SectionId::PouIndex,
        pou_index_entry(0, BYTECODE_MAX_PARAMETERS_PER_POU as u32 + 1),
        "POU_INDEX parameter count exceeds fixed resource limit",
    );
}

#[test]
fn construction_record_counts_admit_the_exact_limit_and_refuse_one_more() {
    // Site: decode/construction.rs:24 (limit, then byte bound). Zero is a valid
    // discriminant for every enumerated field of these four records, so the admitted
    // payload decodes completely.
    for (section_id, width) in [
        (SectionId::StorageLayout, 40),
        (SectionId::ConstructionRoots, 24),
        (SectionId::Initializers, 60),
        (SectionId::AccessBindings, 20),
    ] {
        let mut payload = vec![0u8; 4 + width * BYTECODE_MAX_CONSTRUCTION_RECORDS];
        set_word(&mut payload, 0, BYTECODE_MAX_CONSTRUCTION_RECORDS as u32);
        let decoded = decode_section_at(BytecodeVersion::SOURCE_FREE, section_id, payload)
            .expect("65,536 construction records are admitted");
        assert_eq!(
            record_count(decoded.section(section_id)),
            Some(BYTECODE_MAX_CONSTRUCTION_RECORDS)
        );

        assert_rejected_at(
            BytecodeVersion::SOURCE_FREE,
            section_id,
            65_537u32.to_le_bytes().to_vec(),
            "construction metadata count exceeds fixed resource limit",
        );
    }
}
