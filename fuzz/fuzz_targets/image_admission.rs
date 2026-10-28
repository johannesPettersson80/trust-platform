#![no_main]

//! Admission fuzzing for program images (specification 44 §5 and §10.2). A mutated image gets its
//! declared length, its kernel interface version, the program library hash of its own CPU and its
//! header CRC repaired, so a mutation reaches the section table, signature, table, binding, memory
//! and stack checks instead of stopping at the header. The image then goes through admission on a
//! device of that CPU that trusts every signer. Every outcome is a refusal or an admitted image;
//! none may panic. The code section is machine code, which no fuzzed image runs.

use libfuzzer_sys::fuzz_target;
use trust_plc_abi::INTERFACE_VERSION;
use trust_runtime_core::crc32;
use trust_runtime_core::native::admission::{admit, Device};
use trust_runtime_core::native::image::{Target, HEADER_BYTES, SECTION_ENTRY_BYTES};
use trust_runtime_core::native::library::{library_hash, PROGRAM_LIBRARY};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
/// Input cap.
const MAX_INPUT_BYTES: usize = 256 * 1024;

fuzz_target!(|data: &[u8]| {
    let mut bytes = data[..data.len().min(MAX_INPUT_BYTES)].to_vec();
    let Some(target) = repair_header(&mut bytes) else {
        return;
    };
    let device = Device {
        target,
        program_memory: 16 * MIB,
        task_stack: 64 * KIB,
        max_statements: 1_000,
        trusted_keys: &[],
    };
    if let Ok(image) = admit(&bytes, &device) {
        // An admitted image's figure is what the device reports; reading it must not fail.
        let _ = image.memory_figure();
    }
});

/// Repairs the header fields a random image gets wrong (specification 44 §5.2): the length, the
/// kernel interface version, the program library hash of the image's CPU, and the CRC-32 of bytes
/// 12 to the end of the section table. An image of no known CPU is left out, since every device
/// refuses it at its first check.
fn repair_header(bytes: &mut [u8]) -> Option<Target> {
    if bytes.len() < HEADER_BYTES {
        return None;
    }
    let target = Target::from_raw(read_u32(bytes, 20))?;
    let length = u32::try_from(bytes.len()).ok()?;
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
    let interface = u16::try_from(INTERFACE_VERSION).ok()?;
    bytes[18..20].copy_from_slice(&interface.to_le_bytes());
    bytes[96..112].copy_from_slice(&library_hash(PROGRAM_LIBRARY, target));
    let count = usize::try_from(read_u32(bytes, 112)).ok()?;
    let table = usize::try_from(read_u32(bytes, 116)).ok()?;
    let end = count
        .checked_mul(SECTION_ENTRY_BYTES)
        .and_then(|size| size.checked_add(table))
        .filter(|end| *end <= bytes.len() && table >= HEADER_BYTES)?;
    let checksum = crc32::checksum(&bytes[12..end]);
    bytes[8..12].copy_from_slice(&checksum.to_le_bytes());
    Some(target)
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
