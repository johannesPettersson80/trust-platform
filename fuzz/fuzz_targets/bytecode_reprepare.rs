#![no_main]

//! Preparation determinism and codec fixed-point fuzzing for source-free STBC (spec 12 §4.1;
//! spec 34 LOAD-07). The input gets the same CRC repair and admission profile as
//! `bytecode_admission`. Two preparations of the same bytes must agree: both Ok with the same
//! preparation usage, or both the same error. An admitted input must then satisfy
//! encode -> decode -> encode = encode. A canonical input, one whose encoding equals its bytes,
//! is a special case of that fixed point.

use libfuzzer_sys::fuzz_target;
use trust_runtime_core::bytecode::{
    BytecodeModule, BytecodeVersion, ValidationLimits, HEADER_FLAG_CRC32,
};
use trust_runtime_core::crc32;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;
/// Input cap, which is also the admission `max_artifact_bytes`.
const MAX_INPUT_BYTES: usize = 256 * KIB;
const HEADER_LEN: usize = 24;

fuzz_target!(|data: &[u8]| {
    let mut bytes = data[..data.len().min(MAX_INPUT_BYTES)].to_vec();
    repair_header_checksum(&mut bytes);
    let first = PreparedModule::from_bytes(&bytes, admission_limits());
    let second = PreparedModule::from_bytes(&bytes, admission_limits());
    // `PreparedModule` has no equality, and its Debug output is not stable: its VM tables are
    // std `HashMap`s, whose iteration order differs between instances. The public, deterministic
    // result is the preparation usage (and the error, when there is one).
    match (&first, &second) {
        (Ok(a), Ok(b)) => assert_eq!(
            a.preparation_usage(),
            b.preparation_usage(),
            "preparation usage differs between two calls on the same bytes"
        ),
        (Err(a), Err(b)) => assert_eq!(a, b, "preparation error differs between two calls"),
        _ => panic!("preparation is Ok on one call and Err on the other"),
    }
    if first.is_err() {
        return;
    }
    let decoded = BytecodeModule::decode(&bytes).expect("an admitted artifact decodes");
    let encoded = decoded
        .encode()
        .expect("a decoded admitted artifact encodes");
    let reencoded = BytecodeModule::decode(&encoded)
        .expect("an encoded admitted artifact decodes")
        .encode()
        .expect("a re-decoded admitted artifact encodes");
    assert_eq!(
        encoded, reencoded,
        "encode -> decode -> encode is not a fixed point"
    );
});

/// Explicit admission profile for fuzzing, identical to `bytecode_admission`.
fn admission_limits() -> PreparationLimits {
    PreparationLimits {
        max_artifact_bytes: MAX_INPUT_BYTES,
        max_preparation_bytes: 16 * MIB,
        max_preparation_work: 1_000_000,
        validation: ValidationLimits {
            max_scratch_bytes: 16 * MIB,
            max_work: 1_000_000,
        },
        max_construction_values: 10_000,
        max_construction_bytes: 4 * MIB,
        max_process_image_bytes: 64 * KIB,
        max_work: 100_000,
        max_construction_work: 1_000_000,
        max_call_depth: 64,
    }
}

/// Recompute the header CRC32 that the decoder checks (spec 12 §4.1), identical to
/// `bytecode_admission`. It mirrors the crate-private `header_checksum` in
/// `bytecode/format/header.rs`: STBC 3.0 covers the whole file with bytes 20..24 read as zero,
/// and other versions cover everything from the section table onward.
fn repair_header_checksum(bytes: &mut [u8]) {
    if bytes.len() < HEADER_LEN {
        return;
    }
    let version = BytecodeVersion::new(
        u16::from_le_bytes([bytes[4], bytes[5]]),
        u16::from_le_bytes([bytes[6], bytes[7]]),
    );
    let flags = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let section_table_off =
        u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
    if flags & HEADER_FLAG_CRC32 == 0 || section_table_off > bytes.len() {
        return;
    }
    let checksum = if version == BytecodeVersion::SOURCE_FREE {
        let zero = [0u8; 4];
        crc32::checksum_parts(&[&bytes[..20], &zero[..], &bytes[24..]])
    } else {
        crc32::checksum(&bytes[section_table_off..])
    };
    bytes[20..24].copy_from_slice(&checksum.to_le_bytes());
}
