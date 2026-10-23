//! Bounded source-free decode, validation, and preparation mutation cases.

use trust_runtime_core::bytecode::{
    BytecodeError, BytecodeModule, BytecodeVersion, SectionData, SectionDiagnostic, SectionId,
    ValidationLimits, HEADER_FLAG_CRC32,
};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::vm::{PreparationLimits, PreparationUsage, PreparedModule};

const PROGRAM_V2: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
const NUMERIC_V2: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc");
const GPIO_V3: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

const VALIDATION_LIMITS: ValidationLimits = ValidationLimits {
    max_scratch_bytes: 4 * 1024 * 1024,
    max_work: 4 * 1024 * 1024,
};
const MUTATION_CASES_PER_SEED: usize = 256;
const BYTE_MUTATION_SEED: u32 = 0x0a07_2026;

const PREPARATION_LIMITS: PreparationLimits = PreparationLimits {
    max_artifact_bytes: 1024 * 1024,
    max_preparation_bytes: 8 * 1024 * 1024,
    max_preparation_work: 4 * 1024 * 1024,
    validation: VALIDATION_LIMITS,
    max_construction_values: 100_000,
    max_construction_bytes: 4 * 1024 * 1024,
    max_process_image_bytes: 1024 * 1024,
    max_work: 1_000_000,
    max_construction_work: 1_000_000,
    max_call_depth: 128,
};

const SAVED_SOURCE_FREE_STBC: [(&str, &[u8]); 3] = [
    ("program-v2", PROGRAM_V2),
    ("numeric-v2", NUMERIC_V2),
    ("gpio-v3", GPIO_V3),
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Check {
    NotReached,
    Passed,
    Rejected(BytecodeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    decode: Check,
    validation: Check,
    preparation: Result<PreparationUsage, RuntimeError>,
}

fn observe(bytes: &[u8]) -> Observation {
    let (decode, validation) = match BytecodeModule::decode_with_limits(
        bytes,
        PREPARATION_LIMITS.max_preparation_bytes,
        PREPARATION_LIMITS.max_preparation_work,
    ) {
        Ok((module, _stats)) => {
            let validation = match module.validated_source_free(VALIDATION_LIMITS) {
                Ok(token) => {
                    let _stats = token.stats();
                    Check::Passed
                }
                Err(error) => Check::Rejected(error),
            };
            (Check::Passed, validation)
        }
        Err(error) => (Check::Rejected(error), Check::NotReached),
    };

    let preparation = PreparedModule::from_bytes(bytes, PREPARATION_LIMITS)
        .map(|prepared| prepared.preparation_usage());

    Observation {
        decode,
        validation,
        preparation,
    }
}

fn next_mutation_value(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

fn recompute_source_free_crc(bytes: &mut [u8]) {
    let flags = u32::from_le_bytes(bytes[8..12].try_into().expect("seed header flags"));
    assert_ne!(
        flags & HEADER_FLAG_CRC32,
        0,
        "source-free fixture has CRC enabled"
    );
    let section_table_offset = usize::try_from(u32::from_le_bytes(
        bytes[16..20].try_into().expect("seed section-table offset"),
    ))
    .expect("section-table offset fits target");
    let covered = bytes
        .get(section_table_offset..)
        .expect("original section-table offset is in range");
    let checksum = trust_runtime_core::crc32::checksum(covered);
    bytes[20..24].copy_from_slice(&checksum.to_le_bytes());
}

#[test]
fn saved_source_free_seeds_roundtrip_and_prepare_deterministically() {
    for (name, bytes) in SAVED_SOURCE_FREE_STBC {
        let decoded = BytecodeModule::decode(bytes).expect("reviewed saved STBC seed decodes");
        assert!(
            matches!(
                decoded.version,
                BytecodeVersion::SOURCE_FREE_V2 | BytecodeVersion::SOURCE_FREE
            ),
            "{name} is an explicit source-free format fixture"
        );
        let token = decoded
            .validated_source_free(VALIDATION_LIMITS)
            .unwrap_or_else(|error| panic!("{name} validates under the fixed bound: {error:?}"));
        assert!(
            token.stats().work > 0,
            "{name} exercises bounded validation"
        );
        assert_eq!(
            decoded.encode().unwrap().as_slice(),
            bytes,
            "{name} wire round-trip"
        );

        let first = observe(bytes);
        assert_eq!(first.decode, Check::Passed, "{name} decode");
        assert_eq!(first.validation, Check::Passed, "{name} validation");
        assert!(first.preparation.is_ok(), "{name} preparation: {first:?}");
        assert_eq!(
            observe(bytes),
            first,
            "{name} repeated preparation is deterministic"
        );
        assert_eq!(
            observe(&decoded.encode().unwrap()),
            first,
            "{name} round-trip preparation"
        );
    }
}

#[test]
fn raw_header_corruptions_are_bounded_and_rejected_deterministically() {
    for (name, seed) in SAVED_SOURCE_FREE_STBC {
        let mut damaged = seed.to_vec();
        damaged[0] ^= 0xff;

        let first = observe(&damaged);
        assert_eq!(first.decode, Check::Rejected(BytecodeError::InvalidMagic));
        assert!(
            matches!(first.decode, Check::Rejected(_)),
            "{name} raw magic mutation must stop at decode"
        );
        assert_eq!(
            first.validation,
            Check::NotReached,
            "{name} validation stage"
        );
        assert!(
            first.preparation.is_err(),
            "{name} must not prepare damaged bytes"
        );
        assert_eq!(
            observe(&damaged),
            first,
            "{name} repeated rejection is deterministic"
        );
    }
}

#[test]
fn reencoded_instruction_and_index_mutations_reach_full_validation() {
    for (name, seed) in SAVED_SOURCE_FREE_STBC {
        let module = BytecodeModule::decode(seed).expect("saved STBC seed decodes");
        let body_offset = {
            let Some(SectionData::PouIndex(index)) = module.section(SectionId::PouIndex) else {
                panic!("{name} has a POU_INDEX")
            };
            index
                .entries
                .iter()
                .find(|entry| entry.code_length != 0)
                .map(|entry| usize::try_from(entry.code_offset).expect("bounded body offset"))
                .unwrap_or_else(|| panic!("{name} has at least one non-empty POU body"))
        };
        let mut bad_opcode = module.clone();
        let Some(SectionData::PouBodies(body)) = bad_opcode.section_mut(SectionId::PouBodies)
        else {
            panic!("{name} has POU_BODIES")
        };
        *body
            .get_mut(body_offset)
            .expect("POU body offset is in range") = 0xff;
        let bad_opcode = bad_opcode
            .encode()
            .expect("encoder recomputes container integrity");
        let opcode_result = observe(&bad_opcode);
        assert_eq!(
            opcode_result.decode,
            Check::Passed,
            "{name} framing after opcode mutation"
        );
        assert!(
            matches!(opcode_result.validation, Check::Rejected(_)),
            "{name} unknown opcode is rejected by full validation"
        );
        assert_eq!(
            opcode_result.validation,
            Check::Rejected(BytecodeError::InvalidOpcode(0xff)),
            "{name} reports the stable invalid-opcode variant"
        );
        assert!(
            opcode_result.preparation.is_err(),
            "{name} invalid opcode cannot prepare"
        );
        assert_eq!(
            observe(&bad_opcode),
            opcode_result,
            "{name} repeated opcode rejection"
        );
    }
}

#[test]
fn reencoded_duplicate_pou_id_reaches_full_validation() {
    let mut module = BytecodeModule::decode(GPIO_V3).expect("saved GPIO STBC seed decodes");
    let Some(SectionData::PouIndex(index)) = module.section_mut(SectionId::PouIndex) else {
        panic!("GPIO seed has a POU_INDEX")
    };
    assert!(
        index.entries.len() >= 2,
        "GPIO seed supports duplicate-id mutation"
    );
    let duplicate_pou = index.entries[0].id;
    index.entries[1].id = duplicate_pou;

    let duplicate_id = module
        .encode()
        .expect("encoder recomputes container integrity");
    let duplicate_result = observe(&duplicate_id);
    assert_eq!(
        duplicate_result.decode,
        Check::Passed,
        "container framing remains valid"
    );
    assert_eq!(
        duplicate_result.validation,
        Check::Rejected(BytecodeError::section_diagnostic(
            SectionDiagnostic::DuplicatePou(duplicate_pou),
        )),
        "duplicate POU id is the owning validation error"
    );
    assert!(
        duplicate_result.preparation.is_err(),
        "duplicate id cannot prepare"
    );
    assert_eq!(
        observe(&duplicate_id),
        duplicate_result,
        "repeated duplicate-id rejection is deterministic"
    );
}

#[test]
fn fixed_seed_wire_mutations_reach_bounded_decode_validate_and_prepare() {
    for (name, seed_bytes) in SAVED_SOURCE_FREE_STBC {
        let mut state = BYTE_MUTATION_SEED;
        let original_table_offset = usize::try_from(u32::from_le_bytes(
            seed_bytes[16..20]
                .try_into()
                .expect("saved seed section-table offset"),
        ))
        .expect("section-table offset fits target");
        assert!(
            original_table_offset < seed_bytes.len(),
            "{name} section table is bounded"
        );

        let mut decode_rejected = 0usize;
        let mut validation_rejected = 0usize;
        let mut validated = 0usize;
        let mut prepared = 0usize;
        let mut preparation_rejected = 0usize;

        for case_index in 0..MUTATION_CASES_PER_SEED {
            let mut mutated = seed_bytes.to_vec();
            let span = mutated.len() - original_table_offset;
            let offset = original_table_offset + (next_mutation_value(&mut state) as usize % span);
            let mut xor = (next_mutation_value(&mut state) >> 24) as u8;
            if xor == 0 {
                xor = 1;
            }
            mutated[offset] ^= xor;
            recompute_source_free_crc(&mut mutated);

            let first = observe(&mutated);
            assert_eq!(
                observe(&mutated),
                first,
                "{name} mutation case={case_index} offset={offset} xor={xor:#04x} is deterministic"
            );
            match (&first.decode, &first.validation, &first.preparation) {
                (Check::Rejected(_), Check::NotReached, Err(_)) => decode_rejected += 1,
                (Check::Passed, Check::Rejected(_), Err(_)) => validation_rejected += 1,
                (Check::Passed, Check::Passed, Ok(usage)) => {
                    validated += 1;
                    prepared += 1;
                    assert!(
                        usage.bytes <= PREPARATION_LIMITS.max_preparation_bytes,
                        "{name} case={case_index} preparation bytes remain within the explicit cap"
                    );
                    assert!(
                        usage.work <= PREPARATION_LIMITS.max_preparation_work,
                        "{name} case={case_index} preparation work remains within the explicit cap"
                    );
                }
                (Check::Passed, Check::Passed, Err(error)) => {
                    // Admission additionally resolves imports, execution metadata,
                    // profile capabilities and cumulative preparation demand.
                    // An internal execution/lifetime fault is not an admission result.
                    assert!(
                        matches!(
                            error,
                            RuntimeError::PreparationLimit
                                | RuntimeError::ProfileUnsupported(_)
                                | RuntimeError::Overflow
                                | RuntimeError::Bytecode { .. }
                                | RuntimeError::PreparationDiagnostic(_)
                        ),
                        "{name} case={case_index} offset={offset} xor={xor:#04x} unexpected admission fault: {error:?}"
                    );
                    validated += 1;
                    preparation_rejected += 1;
                }
                outcome => panic!(
                    "{name} mutation case={case_index} offset={offset} xor={xor:#04x} reached inconsistent pipeline state: {outcome:?}"
                ),
            }
        }

        assert!(
            validation_rejected > 0,
            "{name} fixed-seed corpus reaches semantic validation rejection"
        );
        assert!(
            prepared > 0,
            "{name} fixed-seed corpus retains at least one admissible mutation"
        );
        println!(
            "seed={name} fixed_seed={BYTE_MUTATION_SEED:#010x} cases={MUTATION_CASES_PER_SEED} decode_rejected={decode_rejected} validation_rejected={validation_rejected} validated={validated} prepared={prepared} preparation_rejected={preparation_rejected}"
        );
        assert_eq!(
            decode_rejected + validation_rejected + preparation_rejected + prepared,
            MUTATION_CASES_PER_SEED,
            "{name} every bounded mutation reaches one classified outcome"
        );
    }
}
