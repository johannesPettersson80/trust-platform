//! The deployment record (spec 34 §5.1.1): its version-1 layout, what it says about a program,
//! its identities, and that a reader refuses every malformed record with the reason.

use trust_runtime_core::bytecode::{
    BytecodeModule, BytecodeVersion, SectionData, SectionId, StorageDeclaration,
};
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{
    configuration_identity, retain_schema_identity, sha256, ApplicationIdentity, Capabilities,
    Contracts, DeploymentRecord, DeploymentRecordError, ImportKind, NumericContract,
    ProcessImageSummary, RequestedLimits, RequiredImport, SchedulingProfile, TaskSummary,
    TimingEvidence, RECORD_MAX_BYTES, RECORD_VERSION,
};
use trust_runtime_core::error::{RuntimeError, StableErrorCode};
use trust_runtime_core::vm::PreparationLimits;

const PROGRAM: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
const CLOCK: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/clock-v2.stbc");

const CONFIGURATION: [u8; 32] = [0x5A; 32];

fn record_of(program: &[u8]) -> DeploymentRecord {
    DeploymentRecord::for_program(program, PreparationLimits::default(), CONFIGURATION)
        .expect("the saved fixture is admitted under the hosted defaults")
}

fn import(kind: ImportKind, name: &str, contract_version: u16) -> RequiredImport {
    RequiredImport {
        kind,
        name: name.into(),
        contract_version,
    }
}

/// A record with every field set, for the layout tests.
fn sample() -> DeploymentRecord {
    DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: [0x11; 32],
            program_bytes: 0x0102_0304,
            bytecode_version: BytecodeVersion::SOURCE_FREE,
            bytecode_flags: 0x0000_0001,
            configuration_sha256: [0x22; 32],
            retain_schema_sha256: [0x33; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::MEASURED_QUALIFIED,
            capabilities: Capabilities::WALL_CLOCK,
        },
        limits: RequestedLimits::from_preparation(&PreparationLimits {
            max_call_depth: 7,
            ..PreparationLimits::default()
        }),
        process_image: ProcessImageSummary {
            input_bytes: 4,
            output_bytes: 2,
            marker_bytes: 8,
            bindings: 3,
        },
        tasks: vec![
            TaskSummary {
                interval_nanos: 10_000_000,
                priority: 1,
                event: false,
            },
            TaskSummary {
                interval_nanos: 0,
                priority: 5,
                event: true,
            },
        ],
        imports: vec![
            import(ImportKind::Function, "CURRENT_DT", 1),
            import(ImportKind::Function, "SQRT", 1),
            import(ImportKind::FunctionBlock, "TON", 1),
        ],
    }
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// The sections of an encoded record, as identifier, reserved field and payload.
fn sections(bytes: &[u8]) -> Vec<(u16, u16, Vec<u8>)> {
    let end = bytes.len() - 4;
    let mut at = 12;
    let mut found = Vec::new();
    while at < end {
        let length = u32_at(bytes, at + 4) as usize;
        found.push((
            u16_at(bytes, at),
            u16_at(bytes, at + 2),
            bytes[at + 8..at + 8 + length].to_vec(),
        ));
        at += 8 + length;
    }
    found
}

/// A record of `version` with `count` in its header and these sections, its length and CRC
/// computed as a writer computes them.
fn raw(version: u16, count: u16, sections: &[(u16, u16, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = b"TRDR".to_vec();
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    for (id, reserved, payload) in sections {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&reserved.to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(payload);
    }
    let length = u32::try_from(bytes.len() + 4).unwrap();
    bytes[8..12].copy_from_slice(&length.to_le_bytes());
    let crc = crc32::checksum(&bytes);
    bytes.extend_from_slice(&crc.to_le_bytes());
    bytes
}

/// The record's sections with `edit` applied to them, rewritten with a correct header and CRC.
fn edited(edit: impl FnOnce(&mut Vec<(u16, u16, Vec<u8>)>)) -> Vec<u8> {
    let mut parts = sections(&sample().encode().unwrap());
    edit(&mut parts);
    raw(RECORD_VERSION, u16::try_from(parts.len()).unwrap(), &parts)
}

fn refused(bytes: &[u8]) -> DeploymentRecordError {
    DeploymentRecord::decode(bytes).expect_err("a malformed record is refused")
}

#[test]
fn a_record_round_trips_through_the_version_1_layout() {
    let record = sample();
    let bytes = record.encode().unwrap();
    assert_eq!(bytes.len(), record.encoded_len());
    assert_eq!(&bytes[0..4], b"TRDR");
    assert_eq!(u16_at(&bytes, 4), 1);
    assert_eq!(u16_at(&bytes, 6), 6, "six sections");
    assert_eq!(u32_at(&bytes, 8) as usize, bytes.len());
    let end = bytes.len() - 4;
    assert_eq!(u32_at(&bytes, end), crc32::checksum(&bytes[..end]));

    let parts = sections(&bytes);
    let ids: Vec<u16> = parts.iter().map(|part| part.0).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5, 6]);
    assert!(
        parts.iter().all(|part| part.1 == 0),
        "reserved fields are 0"
    );
    let lengths: Vec<usize> = parts.iter().map(|part| part.2.len()).collect();
    let names: usize = record.imports.iter().map(|import| import.name.len()).sum();
    assert_eq!(lengths, [108, 8, 88, 16, 4 + 2 * 16, 4 + 3 * 4 + names]);

    // Section 1: program SHA-256 and length, STBC version and flags, the two identities.
    let application = &parts[0].2;
    assert_eq!(&application[0..32], &[0x11; 32]);
    assert_eq!(u32_at(application, 32), 0x0102_0304);
    assert_eq!(u16_at(application, 36), 3);
    assert_eq!(u16_at(application, 38), 0);
    assert_eq!(u32_at(application, 40), 1);
    assert_eq!(&application[44..76], &[0x22; 32]);
    assert_eq!(&application[76..108], &[0x33; 32]);
    // Section 2: profile, numeric contract, timing evidence, reserved, capabilities.
    assert_eq!(parts[1].2, [1, 0, 1, 0, 1, 0, 1, 0]);
    // Section 3: the limits in `PreparationLimits` order; call depth is the last.
    let limits = &parts[2].2;
    assert_eq!(
        u64::from_le_bytes(limits[80..88].try_into().unwrap()),
        7,
        "max_call_depth"
    );
    assert_eq!(
        u64::from_le_bytes(limits[0..8].try_into().unwrap()),
        u64::try_from(PreparationLimits::default().max_artifact_bytes).unwrap(),
        "max_artifact_bytes"
    );
    // Section 4: the image sizes and the binding count.
    assert_eq!(parts[3].2, [4, 0, 0, 0, 2, 0, 0, 0, 8, 0, 0, 0, 3, 0, 0, 0]);
    // Section 5: two tasks; the event task has flag bit 0.
    let tasks = &parts[4].2;
    assert_eq!(u16_at(tasks, 0), 2);
    assert_eq!(u32_at(tasks, 4 + 16 + 12), 1);
    // Section 6: kind, name length, contract version, name.
    let imports = &parts[5].2;
    assert_eq!(u16_at(imports, 0), 3);
    assert_eq!(&imports[4..8], &[1, 10, 1, 0]);
    assert_eq!(&imports[8..18], b"CURRENT_DT");

    let decoded = DeploymentRecord::decode(&bytes).unwrap();
    assert_eq!(decoded, record);
    assert_eq!(decoded.encode().unwrap(), bytes);
}

#[test]
fn a_record_describes_the_program_it_was_built_for() {
    let limits = PreparationLimits {
        max_work: 123_456,
        ..PreparationLimits::default()
    };
    let record = DeploymentRecord::for_program(PROGRAM, limits, CONFIGURATION).unwrap();
    let module = BytecodeModule::decode(PROGRAM).unwrap();
    assert_eq!(
        record.application,
        ApplicationIdentity {
            program_sha256: sha256(PROGRAM),
            program_bytes: u32::try_from(PROGRAM.len()).unwrap(),
            bytecode_version: BytecodeVersion::SOURCE_FREE_V2,
            bytecode_flags: module.flags,
            configuration_sha256: CONFIGURATION,
            retain_schema_sha256: retain_schema_identity(&module).unwrap(),
        }
    );
    assert_eq!(
        record.contracts,
        Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::NONE,
        }
    );
    assert_eq!(record.limits, RequestedLimits::from_preparation(&limits));
    let Some(SectionData::ResourceMeta(meta)) = module.section(SectionId::ResourceMeta) else {
        panic!("the fixture has resource metadata");
    };
    let resource = &meta.resources[0];
    let bindings = match module.section(SectionId::IoMap) {
        Some(SectionData::IoMap(map)) => map.bindings.len(),
        _ => 0,
    };
    assert_eq!(
        record.process_image,
        ProcessImageSummary {
            input_bytes: resource.inputs_size,
            output_bytes: resource.outputs_size,
            marker_bytes: resource.memory_size,
            bindings: u32::try_from(bindings).unwrap(),
        }
    );
    // `TASK Periodic (INTERVAL := T#25ms, PRIORITY := 0)`.
    assert_eq!(
        record.tasks,
        [TaskSummary {
            interval_nanos: 25_000_000,
            priority: 0,
            event: false,
        }]
    );
    // `DINT_TO_INT(...)` and the `TON` instance; the user FB `Counter` is not a built-in.
    assert_eq!(
        record.imports,
        [
            import(ImportKind::Function, "DINT_TO_INT", 1),
            import(ImportKind::FunctionBlock, "TON", 1),
        ]
    );
    let bytes = record.encode().unwrap();
    assert_eq!(DeploymentRecord::decode(&bytes).unwrap(), record);
}

#[test]
fn a_program_reading_the_wall_clock_requires_it() {
    let record = record_of(CLOCK);
    assert_eq!(record.contracts.capabilities, Capabilities::WALL_CLOCK);
    assert_eq!(
        record.imports,
        [
            import(ImportKind::Function, "CURRENT_DT", 1),
            import(ImportKind::Function, "TIME", 1),
        ]
    );
}

#[test]
fn a_program_the_runtime_cannot_prepare_gets_no_record() {
    assert!(matches!(
        DeploymentRecord::for_program(
            b"not a program",
            PreparationLimits::default(),
            CONFIGURATION
        ),
        Err(RuntimeError::Bytecode { .. } | RuntimeError::BytecodeCause(_))
    ));
    let small = PreparationLimits {
        max_artifact_bytes: PROGRAM.len() - 1,
        ..PreparationLimits::default()
    };
    assert_eq!(
        DeploymentRecord::for_program(PROGRAM, small, CONFIGURATION),
        Err(RuntimeError::PreparationLimit)
    );
}

#[test]
fn a_checkpoint_carries_the_first_16_bytes_of_both_identities() {
    let record = sample();
    let identity = record.checkpoint_identity();
    assert_eq!(identity.application, [0x11; 16]);
    assert_eq!(identity.schema, [0x33; 16]);
}

#[test]
fn the_identities_are_sha_256_over_their_defined_bytes() {
    // FIPS 180-4, the one-block message "abc".
    assert_eq!(
        sha256(b"abc"),
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
    );
    // The configuration identity frames both files: name, zero, then presence, length, bytes.
    let mut framed = Vec::new();
    framed.extend_from_slice(b"runtime.toml\0\x01");
    framed.extend_from_slice(&3u64.to_le_bytes());
    framed.extend_from_slice(b"a=1");
    framed.extend_from_slice(b"io.toml\0\x00");
    assert_eq!(configuration_identity(Some(b"a=1"), None), sha256(&framed));
    assert_ne!(
        configuration_identity(None, None),
        configuration_identity(Some(b""), None),
        "a missing file differs from an empty one"
    );
    assert_ne!(
        configuration_identity(Some(b"x"), None),
        configuration_identity(None, Some(b"x")),
        "the same text in the other file differs"
    );
}

#[test]
fn the_retain_schema_follows_the_retained_declarations_only() {
    let module = BytecodeModule::decode(PROGRAM).unwrap();
    let identity = retain_schema_identity(&module).unwrap();
    assert_eq!(retain_schema_identity(&module).unwrap(), identity);
    assert_ne!(
        retain_schema_identity(&BytecodeModule::decode(CLOCK).unwrap()).unwrap(),
        identity
    );
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("the fixture has a storage layout");
    };
    // `activations` is the fixture's one RETAIN variable; `counter` is not retained.
    let retained = layout
        .entries
        .iter()
        .position(|declaration| declaration.retain == 1)
        .expect("a RETAIN variable");
    let ordinary = layout
        .entries
        .iter()
        .position(|declaration| declaration.retain == 0 && declaration.type_id == Some(0))
        .expect("a variable that is not retained");
    let edit = |index: usize, change: fn(&mut StorageDeclaration)| {
        let mut changed = module.clone();
        let Some(SectionData::StorageLayout(layout)) =
            changed.section_mut(SectionId::StorageLayout)
        else {
            panic!("the fixture has a storage layout");
        };
        change(&mut layout.entries[index]);
        retain_schema_identity(&changed).unwrap()
    };
    assert_ne!(
        edit(retained, |declaration| declaration.retain = 3),
        identity,
        "RETAIN and PERSISTENT are different schemas"
    );
    assert_ne!(
        edit(retained, |declaration| declaration.retain = 0),
        identity,
        "a variable that is no longer retained leaves the schema"
    );
    assert_eq!(
        edit(ordinary, |declaration| declaration.type_id = Some(1)),
        identity,
        "a variable that is not retained is not part of the schema"
    );
}

#[test]
fn every_changed_byte_and_every_shorter_length_is_refused() {
    let bytes = record_of(PROGRAM).encode().unwrap();
    let mut accepted = Vec::new();
    for index in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[index] ^= 0xFF;
        if DeploymentRecord::decode(&mutated).is_ok() {
            accepted.push(index);
        }
    }
    assert_eq!(accepted, Vec::<usize>::new(), "mutated bytes that decoded");
    let accepted: Vec<usize> = (0..bytes.len())
        .filter(|length| DeploymentRecord::decode(&bytes[..*length]).is_ok())
        .collect();
    assert_eq!(
        accepted,
        Vec::<usize>::new(),
        "truncated lengths that decoded"
    );
}

#[test]
fn the_header_names_what_is_wrong_with_it() {
    let bytes = sample().encode().unwrap();
    assert_eq!(refused(&bytes[..3]), DeploymentRecordError::Truncated);
    let mut magic = bytes.clone();
    magic[0] = b'X';
    assert_eq!(refused(&magic), DeploymentRecordError::Magic);
    let mut version = bytes.clone();
    version[4] = 2;
    assert_eq!(refused(&version), DeploymentRecordError::Version(2));
    assert_eq!(
        DeploymentRecordError::Version(2).to_string(),
        "record version 2 is unknown; this runtime reads version 1"
    );
    let mut longer = bytes.clone();
    longer.push(0);
    assert_eq!(refused(&longer), DeploymentRecordError::Length);
    assert_eq!(
        refused(&vec![0; RECORD_MAX_BYTES + 1]),
        DeploymentRecordError::TooLarge
    );
    let mut length = bytes.clone();
    length[8..12].copy_from_slice(&u32::try_from(RECORD_MAX_BYTES + 1).unwrap().to_le_bytes());
    assert_eq!(refused(&length), DeploymentRecordError::TooLarge);
    let mut payload = bytes.clone();
    payload[20] ^= 1;
    assert_eq!(refused(&payload), DeploymentRecordError::Checksum);
}

#[test]
fn the_sections_of_version_1_are_exactly_1_to_6_in_order() {
    assert_eq!(
        refused(&edited(|parts| parts[5].0 = 7)),
        DeploymentRecordError::UnknownSection(7)
    );
    assert_eq!(
        refused(&edited(|parts| parts[0].0 = 0)),
        DeploymentRecordError::UnknownSection(0)
    );
    assert_eq!(
        refused(&edited(|parts| parts.swap(1, 2))),
        DeploymentRecordError::SectionOrder(2)
    );
    assert_eq!(
        refused(&edited(|parts| {
            let copy = parts[1].clone();
            parts.insert(2, copy);
        })),
        DeploymentRecordError::SectionOrder(2)
    );
    assert_eq!(
        refused(&edited(|parts| {
            parts.remove(3);
        })),
        DeploymentRecordError::MissingSection(4)
    );
    assert_eq!(
        refused(&edited(|parts| parts[2].2.push(0))),
        DeploymentRecordError::SectionLength(3)
    );
    assert_eq!(
        refused(&edited(|parts| {
            parts[0].2.pop();
        })),
        DeploymentRecordError::SectionLength(1)
    );
    assert_eq!(
        refused(&edited(|parts| parts[3].1 = 1)),
        DeploymentRecordError::Reserved
    );
    // A header that counts fewer sections than the record holds.
    let parts = sections(&sample().encode().unwrap());
    assert_eq!(
        refused(&raw(RECORD_VERSION, 5, &parts)),
        DeploymentRecordError::Field("the section count")
    );
    // A later version is refused as a whole, whatever its sections.
    assert_eq!(
        refused(&raw(2, 6, &parts)),
        DeploymentRecordError::Version(2)
    );
}

#[test]
fn a_field_outside_its_range_is_refused() {
    // Contracts: the reserved byte.
    assert_eq!(
        refused(&edited(|parts| parts[1].2[5] = 1)),
        DeploymentRecordError::Reserved
    );
    // Tasks: a count that does not fit the payload, a reserved field, an undefined flag.
    assert_eq!(
        refused(&edited(|parts| parts[4].2[0] = 3)),
        DeploymentRecordError::SectionLength(5)
    );
    assert_eq!(
        refused(&edited(|parts| parts[4].2[2] = 1)),
        DeploymentRecordError::Reserved
    );
    assert_eq!(
        refused(&edited(|parts| parts[4].2[4 + 12] = 2)),
        DeploymentRecordError::Field("a task flag")
    );
    // Imports: kind, name and order.
    assert_eq!(
        refused(&edited(|parts| parts[5].2[4] = 3)),
        DeploymentRecordError::Field("an import kind")
    );
    assert_eq!(
        refused(&edited(|parts| parts[5].2[8] = 0xFF)),
        DeploymentRecordError::Field("an import name")
    );
    assert_eq!(
        refused(&edited(|parts| {
            // CURRENT_DT becomes TURRENT_DT, after SQRT.
            parts[5].2[8] = b'T';
        })),
        DeploymentRecordError::Field("the import order")
    );
    // The first name's length becomes 0.
    assert_eq!(
        refused(&edited(|parts| parts[5].2[5] = 0)),
        DeploymentRecordError::Field("an import name")
    );
    assert_eq!(
        RuntimeError::DeploymentRecordInvalid(DeploymentRecordError::Checksum).stable_code(),
        StableErrorCode::RuntimeDeploymentRecordInvalid
    );
    assert_eq!(
        RuntimeError::DeploymentRecordInvalid(DeploymentRecordError::Field("an import kind"))
            .to_string(),
        "invalid deployment record: an import kind is outside its range"
    );
}

#[test]
fn a_writer_refuses_what_a_reader_would_refuse() {
    let mut unordered = sample();
    unordered.imports.swap(0, 1);
    assert_eq!(
        unordered.encode(),
        Err(DeploymentRecordError::Field("the import order"))
    );
    let mut repeated = sample();
    repeated
        .imports
        .push(import(ImportKind::FunctionBlock, "TON", 1));
    assert_eq!(
        repeated.encode(),
        Err(DeploymentRecordError::Field("the import order"))
    );
    let mut nameless = sample();
    nameless.imports = vec![import(ImportKind::Function, "", 1)];
    assert_eq!(
        nameless.encode(),
        Err(DeploymentRecordError::Field("an import name"))
    );
    let mut long_name = sample();
    long_name.imports = vec![import(ImportKind::Function, &"A".repeat(256), 1)];
    assert_eq!(
        long_name.encode(),
        Err(DeploymentRecordError::Field("an import name"))
    );
    let mut many_tasks = sample();
    many_tasks.tasks = vec![many_tasks.tasks[0]; usize::from(u16::MAX) + 1];
    assert_eq!(
        many_tasks.encode(),
        Err(DeploymentRecordError::Field("the task count"))
    );
    let mut large = sample();
    large.imports = (0..4_000)
        .map(|index| {
            import(
                ImportKind::Function,
                &format!("IMPORT_{index:05}_WITH_A_LONG_NAME"),
                1,
            )
        })
        .collect();
    assert_eq!(large.encode(), Err(DeploymentRecordError::TooLarge));
}
