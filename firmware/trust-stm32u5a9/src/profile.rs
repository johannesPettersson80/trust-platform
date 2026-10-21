//! The fixed STM32U5A9 profile: the installed application's region and the device descriptor that
//! admission reads (specification 34, §5.1.1, COMPAT-02 and TARGET-03; design §4.4).
use trust_platform_stm32u5::memory::MAIN_STACK as MAIN_STACK_REGION;
use trust_runtime_core::bytecode::BytecodeVersion;
use trust_runtime_core::deployment::{
    Capabilities, DeviceDescriptor, NumericContract, RequestedLimits, SchedulingProfile,
    TimingEvidence,
};
use trust_runtime_services::runner::engine::DeviceStack;

/// First byte of the installed application: bank 2 (`[layout] application` of chips/stm32u5a9.toml).
pub const APPLICATION_BASE: usize = 0x0820_0000;
/// Size of the application region, 2016 KiB: all of bank 2 but the checkpoint log (design §4.4).
pub const APPLICATION_BYTES: usize = 2016 * 1024;
/// The first checkpoint slot, which the application region must end before (`checkpoint_a`).
const CHECKPOINT_A: usize = 0x083F_8000;
const _: () = assert!(APPLICATION_BASE + APPLICATION_BYTES <= CHECKPOINT_A);

/// The STM32U5A9's main stack (specification 34 MEM-03 and Appendix D.5; owner, 21 October 2026,
/// "Stack decides"): the 78 KiB at the top of SRAM5. Until this chip's bench measures its own, the
/// base and the level are the ESP32-C6's measured 34,008 and 6,144 bytes (D.2), so the window holds
/// the C6's (79,872 − 2,048 − 34,008) / 6,144 = 7 call levels.
pub const MAIN_STACK: DeviceStack = DeviceStack {
    window_bytes: (MAIN_STACK_REGION.end - MAIN_STACK_REGION.start) as usize,
    base_bytes: 34_008,
    level_bytes: 6_144,
};
const _: () = assert!(MAIN_STACK.call_depth() == 7);

/// The bytes of the heap the STM32U5A9 keeps free beside a program and its event store: its replies
/// and the Live Values and services of the program (specification 34 MEM-12, Appendix D.5).
pub const PROGRAM_MEMORY_RESERVE_BYTES: usize = 8 * 1024;

/// The device descriptor admission reads for the STM32U5A9 (specification 34 §5.1.1, COMPAT-02 and
/// TARGET-03): the STBC versions it prepares, its scheduling profile and numeric contract, its
/// timing evidence (none qualified yet), no platform capability, the hosted maxima of the
/// preparation limits, its event store and its application slot. The chip has no wall clock until
/// slice T6, so it supplies no capability and admission refuses a program that calls `CURRENT_DT`.
pub const DEVICE_DESCRIPTOR: DeviceDescriptor = DeviceDescriptor {
    bytecode_versions: &[
        BytecodeVersion::SOURCE_FREE_V2,
        BytecodeVersion::SOURCE_FREE,
    ],
    scheduling_profiles: &[SchedulingProfile::COOPERATIVE_RESOURCE_V1],
    numeric_contracts: &[NumericContract::V1],
    timing_evidence: TimingEvidence::UNQUALIFIED,
    capabilities: Capabilities::NONE,
    // The hosted maxima of `DeviceDescriptor::hosted`: the 64 MiB artifact and 1,024 call levels,
    // with no maximum for the other limits. On a 32-bit target admission clamps them.
    maxima: RequestedLimits {
        max_artifact_bytes: 64 * 1024 * 1024,
        max_preparation_bytes: u64::MAX,
        max_preparation_work: u64::MAX,
        max_validation_scratch_bytes: u64::MAX,
        max_validation_work: u64::MAX,
        max_construction_values: u64::MAX,
        max_construction_bytes: u64::MAX,
        max_process_image_bytes: u64::MAX,
        max_work: u64::MAX,
        max_construction_work: u64::MAX,
        max_call_depth: 1_024,
    },
    // The `[events]` table of chips/stm32u5a9.toml: the store keeps 1,000 events by default and
    // admits at most 1,000.
    event_store_default: 1_000,
    event_store_max: 1_000,
    application_slot_bytes: Some(APPLICATION_BYTES as u64),
    // Measured at boot, before the program loads (`Platform::set_program_memory`).
    program_memory_bytes: None,
};
