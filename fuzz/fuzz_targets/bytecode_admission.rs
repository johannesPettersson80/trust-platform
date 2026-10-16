#![no_main]

//! Structured admission fuzzing for source-free STBC (spec 12 §4.1 and §4.6; spec 34 LOAD-03
//! and LOAD-07). A mutated container gets a recomputed header CRC32, then goes through
//! decode, full validation and profile preparation. Each admitted resource runs on two
//! executors: the register path with Tier-1 at threshold one, and the shared stack path forced
//! by `debug_active`. Both must return the same result on every cycle.

use libfuzzer_sys::fuzz_target;
use trust_runtime_core::bytecode::{BytecodeVersion, ValidationLimits, HEADER_FLAG_CRC32};
use trust_runtime_core::crc32;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{DateTimeValue, Duration};
use trust_runtime_core::vm::register::RegisterExecutionConfiguration;
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;
/// Input cap, which is also the admission `max_artifact_bytes`.
const MAX_INPUT_BYTES: usize = 256 * KIB;
/// Admission requires exactly one resource, so index one returns an `unknown resource index`
/// error. That error is an accepted outcome, not a finding.
const MAX_RESOURCES: usize = 2;
const CYCLES: i64 = 3;
const HEADER_LEN: usize = 24;

fuzz_target!(|data: &[u8]| {
    let mut bytes = data[..data.len().min(MAX_INPUT_BYTES)].to_vec();
    repair_header_checksum(&mut bytes);
    let Ok(prepared) = PreparedModule::from_bytes(&bytes, admission_limits()) else {
        return;
    };
    let register_services = RegisterServices;
    let stack_services = StackServices;
    for resource in 0..MAX_RESOURCES {
        let (Ok(mut register), Ok(mut stack)) = (
            prepared.instantiate_with_services(resource, &register_services),
            prepared.instantiate_with_services(resource, &stack_services),
        ) else {
            continue;
        };
        for cycle in 0..CYCLES {
            register.clear_fault();
            stack.clear_fault();
            let now = Duration::from_millis(10 * (cycle + 1));
            let register_result = register.execute_cycle(now);
            let stack_result = stack.execute_cycle(now);
            assert_eq!(
                register_result, stack_result,
                "resource {resource}, cycle {cycle}: register and stack paths diverged"
            );
        }
    }
});

/// Explicit admission profile for fuzzing. Every bound is finite and small, so the exhaustion
/// paths are reachable within the input cap.
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

/// Recompute the header CRC32 that the decoder checks (spec 12 §4.1), so that a mutation reaches
/// validation instead of stopping at the checksum. This mirrors the crate-private
/// `header_checksum` in `bytecode/format/header.rs`: STBC 3.0 covers the whole file with bytes
/// 20..24 read as zero, and other versions cover everything from the section table onward.
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

/// Register path with Tier-1 selected at the first block hit, so every block can run specialized.
struct RegisterServices;

impl ExecutionServices for RegisterServices {
    fn register_configuration(&self) -> RegisterExecutionConfiguration {
        RegisterExecutionConfiguration {
            profile_enabled: true,
            tier1_enabled: true,
            tier1_hot_threshold: 1,
            ..RegisterExecutionConfiguration::default()
        }
    }
    fn debug_active(&self) -> bool {
        false
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// Shared stack path: `debug_active` forces the canonical interpreter.
struct StackServices;

impl ExecutionServices for StackServices {
    fn debug_active(&self) -> bool {
        true
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}
