//! The deployment record (spec 34 §5.1.1): its version-3 layout, its identities, and that a reader
//! refuses every malformed record with the reason.

use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{
    configuration_identity, sha256, ApplicationIdentity, Capabilities, Configuration, Contracts,
    CrateSource, DeploymentRecord, DeploymentRecordError, ImportKind, NumericContract,
    ProcessImageSummary, ProgramVersion, RequestedLimits, RequiredImport, RustCrate,
    SchedulingProfile, TaskSummary, TimingEvidence, RECORD_MAX_BYTES, RECORD_VERSION,
};
use trust_runtime_core::error::{RuntimeError, StableErrorCode};

fn rust_crate(name: &str, version: &str, source: CrateSource, checksum: [u8; 32]) -> RustCrate {
    RustCrate {
        name: name.into(),
        version: version.into(),
        source,
        checksum,
    }
}

/// A git crate's checksum: the 20 bytes of its commit, then 12 zero bytes.
fn commit(byte: u8) -> [u8; 32] {
    let mut checksum = [0; 32];
    checksum[..20].fill(byte);
    checksum
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
            program_version: ProgramVersion::NATIVE_IMAGE,
            program_flags: 0x0000_0001,
            configuration_sha256: [0x22; 32],
            retain_schema_sha256: [0x33; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::MEASURED_QUALIFIED,
            capabilities: Capabilities::WALL_CLOCK,
        },
        // Each limit its position in the limits section, 1 to 11.
        limits: RequestedLimits {
            max_artifact_bytes: 1,
            max_preparation_bytes: 2,
            max_preparation_work: 3,
            max_validation_scratch_bytes: 4,
            max_validation_work: 5,
            max_construction_values: 6,
            max_construction_bytes: 7,
            max_process_image_bytes: 8,
            max_work: 9,
            max_construction_work: 10,
            max_call_depth: 11,
        },
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
        configuration: Configuration::default(),
        // A path crate, a registry crate and a git crate, in the order of their names.
        crates: vec![
            rust_crate("helper", "0.3.1", CrateSource::Path, [0; 32]),
            rust_crate("libm", "0.2.16", CrateSource::Registry, [0xAB; 32]),
            rust_crate("trust-ext", "1.0.0", CrateSource::Git, commit(0xCD)),
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
fn a_record_round_trips_through_the_version_3_layout() {
    let record = sample();
    let bytes = record.encode().unwrap();
    assert_eq!(bytes.len(), record.encoded_len());
    assert_eq!(&bytes[0..4], b"TRDR");
    assert_eq!(u16_at(&bytes, 4), 3, "version 3 (specification 34 §5.1.1)");
    assert_eq!(u16_at(&bytes, 6), 9, "nine sections");
    assert_eq!(u32_at(&bytes, 8) as usize, bytes.len());
    let end = bytes.len() - 4;
    assert_eq!(u32_at(&bytes, end), crc32::checksum(&bytes[..end]));

    let parts = sections(&bytes);
    let ids: Vec<u16> = parts.iter().map(|part| part.0).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert!(
        parts.iter().all(|part| part.1 == 0),
        "reserved fields are 0"
    );
    let lengths: Vec<usize> = parts.iter().map(|part| part.2.len()).collect();
    let names: usize = record.imports.iter().map(|import| import.name.len()).sum();
    // Section 7 is the empty configuration: its 42 bytes, the resource name's length byte
    // included. Its layout is pinned byte by byte in `deployment_configuration.rs`. Section 8 holds
    // the parameters of no driver: its two-byte count, 0. Section 9 is its count and reserved
    // field, then for each crate 36 bytes and its name and version: helper 0.3.1 (6 + 5), libm
    // 0.2.16 (4 + 6) and trust-ext 1.0.0 (9 + 5), 4 + 47 + 46 + 50 = 147.
    assert_eq!(
        lengths,
        [108, 8, 88, 16, 4 + 2 * 16, 4 + 3 * 4 + names, 42, 2, 147]
    );

    // Section 1: program SHA-256 and length, program version (44.1) and flags, the two
    // identities.
    let application = &parts[0].2;
    assert_eq!(&application[0..32], &[0x11; 32]);
    assert_eq!(u32_at(application, 32), 0x0102_0304);
    assert_eq!(u16_at(application, 36), 44);
    assert_eq!(u16_at(application, 38), 1);
    assert_eq!(u32_at(application, 40), 1);
    assert_eq!(&application[44..76], &[0x22; 32]);
    assert_eq!(&application[76..108], &[0x33; 32]);
    // Section 2: profile, numeric contract, timing evidence, reserved, capabilities.
    assert_eq!(parts[1].2, [1, 0, 1, 0, 1, 0, 1, 0]);
    // Section 3: the eleven limits as 64-bit values in their order, from max_artifact_bytes to
    // max_call_depth.
    let limits = &parts[2].2;
    for (index, value) in (1..=11_u64).enumerate() {
        let at = index * 8;
        assert_eq!(
            u64::from_le_bytes(limits[at..at + 8].try_into().unwrap()),
            value,
            "limit {value}"
        );
    }
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
    // Section 9: three crates, then each crate's source (2 path, 1 registry, 3 git), the lengths of
    // its name and version, a reserved 0, the checksum (none for the path crate, the archive's
    // SHA-256 for the registry crate, the commit and 12 zeros for the git crate), the name and the
    // version. Catches a crate list in another order, without its source, or a checksum moved.
    let mut expected = vec![3, 0, 0, 0];
    expected.extend_from_slice(&[2, 6, 5, 0]);
    expected.extend_from_slice(&[0; 32]);
    expected.extend_from_slice(b"helper0.3.1");
    expected.extend_from_slice(&[1, 4, 6, 0]);
    expected.extend_from_slice(&[0xAB; 32]);
    expected.extend_from_slice(b"libm0.2.16");
    expected.extend_from_slice(&[3, 9, 5, 0]);
    expected.extend_from_slice(&[0xCD; 20]);
    expected.extend_from_slice(&[0; 12]);
    expected.extend_from_slice(b"trust-ext1.0.0");
    assert_eq!(parts[8].2, expected);

    let decoded = DeploymentRecord::decode(&bytes).unwrap();
    assert_eq!(decoded, record);
    assert_eq!(decoded.encode().unwrap(), bytes);
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
fn every_changed_byte_and_every_shorter_length_is_refused() {
    let bytes = sample().encode().unwrap();
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
    version[4] = 4;
    assert_eq!(refused(&version), DeploymentRecordError::Version(4));
    assert_eq!(
        DeploymentRecordError::Version(4).to_string(),
        "record version 4 is unknown; this runtime reads versions 1 to 3"
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
fn the_sections_of_version_3_are_exactly_1_to_9_in_order() {
    assert_eq!(
        refused(&edited(|parts| parts[8].0 = 10)),
        DeploymentRecordError::UnknownSection(10)
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
        refused(&raw(4, 6, &parts)),
        DeploymentRecordError::Version(4)
    );
    // Version 2 has no section 9, and version 3 needs it.
    assert_eq!(
        refused(&raw(2, 9, &parts)),
        DeploymentRecordError::UnknownSection(9)
    );
    assert_eq!(
        refused(&raw(RECORD_VERSION, 8, &parts[..8])),
        DeploymentRecordError::MissingSection(9)
    );
}

/// A record of version 2, written before the crate list, still reads, with no crates; so does one
/// of version 1 (specification 34 §5.1.1: a runtime reads every version it knows). Catches a reader
/// that drops the older versions, and one that invents crates for them.
#[test]
fn a_version_2_record_reads_without_crates() {
    let mut record = sample();
    let parts = sections(&record.encode().unwrap());
    let version_2 = DeploymentRecord::decode(&raw(2, 8, &parts[..8])).expect("version 2 reads");
    record.crates.clear();
    assert_eq!(version_2, record);
    let version_1 = DeploymentRecord::decode(&raw(1, 7, &parts[..7])).expect("version 1 reads");
    assert!(version_1.crates.is_empty());
}

/// Section 9 refuses what specification 34 §5.1.1 refuses: a source other than 1, 2 or 3, a
/// reserved byte other than 0, an empty or non-UTF-8 name or version, a checksum byte its source
/// leaves zero that is not 0, crates out of their order, and a count the payload does not hold.
/// Catches a reader that takes a crate list a writer could never have written.
#[test]
fn section_9_refuses_a_malformed_crate_list() {
    // The second crate, libm, starts after the count (4) and the first crate (36 + 11 bytes).
    let libm = 4 + 47;
    let crate_section =
        |edit: &dyn Fn(&mut Vec<u8>)| refused(&edited(|parts| edit(&mut parts[8].2)));
    assert_eq!(
        crate_section(&|payload| payload[libm] = 4),
        DeploymentRecordError::Field("a crate source")
    );
    assert_eq!(
        crate_section(&|payload| payload[libm + 3] = 1),
        DeploymentRecordError::Reserved
    );
    assert_eq!(
        crate_section(&|payload| payload[2] = 1),
        DeploymentRecordError::Reserved
    );
    // libm's name, 4 bytes long, becomes 0 bytes long.
    assert_eq!(
        crate_section(&|payload| payload[libm + 1] = 0),
        DeploymentRecordError::Field("a crate name or version")
    );
    // libm's name is not UTF-8.
    assert_eq!(
        crate_section(&|payload| payload[libm + 36] = 0xFF),
        DeploymentRecordError::Field("a crate name or version")
    );
    // A checksum byte of the path crate helper, and the 21st byte of the git crate's.
    assert_eq!(
        crate_section(&|payload| payload[4 + 4] = 1),
        DeploymentRecordError::Field("a crate checksum")
    );
    let git = libm + 46;
    assert_eq!(
        crate_section(&|payload| payload[git + 4 + 20] = 1),
        DeploymentRecordError::Field("a crate checksum")
    );
    // libm becomes zibm, after trust-ext.
    assert_eq!(
        crate_section(&|payload| payload[libm + 36] = b'z'),
        DeploymentRecordError::Field("the crate order")
    );
    // Four crates counted, three written.
    assert_eq!(
        crate_section(&|payload| payload[0] = 4),
        DeploymentRecordError::SectionLength(9)
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

/// The writer refuses the crate lists section 9's reader refuses, so a build never writes a record
/// that no runtime reads. Catches a writer that sorts nothing, takes an empty or long name, or
/// writes a checksum its source cannot have.
#[test]
fn a_writer_refuses_a_crate_list_a_reader_would_refuse() {
    let with = |edit: &dyn Fn(&mut Vec<RustCrate>)| {
        let mut record = sample();
        edit(&mut record.crates);
        record.encode()
    };
    assert_eq!(
        with(&|crates| crates.swap(0, 1)),
        Err(DeploymentRecordError::Field("the crate order"))
    );
    assert_eq!(
        with(&|crates| crates.push(crates[2].clone())),
        Err(DeploymentRecordError::Field("the crate order"))
    );
    assert_eq!(
        with(&|crates| crates[0].name = "".into()),
        Err(DeploymentRecordError::Field("a crate name or version"))
    );
    assert_eq!(
        with(&|crates| crates[2].version = "1".repeat(256).into()),
        Err(DeploymentRecordError::Field("a crate name or version"))
    );
    assert_eq!(
        with(&|crates| crates[0].checksum[31] = 1),
        Err(DeploymentRecordError::Field("a crate checksum"))
    );
    assert_eq!(
        with(&|crates| crates[2].checksum[20] = 1),
        Err(DeploymentRecordError::Field("a crate checksum"))
    );
    // One name, two versions, and one name and version from two sources, are two crates.
    assert!(with(&|crates| {
        crates.insert(
            2,
            rust_crate("libm", "0.2.17", CrateSource::Registry, [1; 32]),
        );
        crates.insert(
            1,
            rust_crate("helper", "0.3.1", CrateSource::Git, commit(2)),
        );
    })
    .is_ok());
}
