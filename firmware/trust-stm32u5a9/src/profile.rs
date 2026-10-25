//! The fixed STM32U5A9 profile: the installed application's region and the device descriptor that
//! admission reads (specification 34, §5.1.1, COMPAT-02 and TARGET-03; specification 44 §10.2;
//! design §4.4).
use trust_platform_stm32u5::memory::MAIN_STACK as MAIN_STACK_REGION;
use trust_runtime_core::deployment::{
    Capabilities, DeviceDescriptor, NativeTarget, NumericContract, ProgramVersion, RequestedLimits,
    SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::native::image::Target;
use trust_runtime_services::runner::DeviceStack;

/// First byte of the installed application: bank 2 (`[layout] application` of chips/stm32u5a9.toml).
pub const APPLICATION_BASE: usize = 0x0820_0000;
/// Size of the application region, 2016 KiB: all of bank 2 but the checkpoint log (design §4.4).
pub const APPLICATION_BYTES: usize = 2016 * 1024;
/// The first checkpoint slot, which the application region must end before (`checkpoint_a`).
const CHECKPOINT_A: usize = 0x083F_8000;
const _: () = assert!(APPLICATION_BASE + APPLICATION_BYTES <= CHECKPOINT_A);

/// The STM32U5A9's main stack (specification 34 MEM-03 and Appendix D.5; specification 44 §8.5): the
/// 78 KiB at the top of SRAM5. Until this chip's bench measures its own, the base of the firmware's
/// deepest path is the ESP32-C6's measured 34,008 bytes (D.2).
pub const MAIN_STACK: DeviceStack = DeviceStack {
    window_bytes: (MAIN_STACK_REGION.end - MAIN_STACK_REGION.start) as usize,
    base_bytes: 34_008,
};

/// What the STM32U5A9 offers compiled programs (specification 44 §10.2 and NX-10): the Cortex-M33
/// module target, whose module runs where its image lies in the internal flash (§10.3); as the task
/// stack, the main stack's window less its headroom and the firmware's deepest path,
/// 79,872 − 2,048 − 34,008 = 43,816 bytes; and the default statement budget of 1,000,000.
pub const NATIVE_TARGET: NativeTarget = NativeTarget {
    target: Target::ThumbV8mMainHf,
    task_stack_bytes: MAIN_STACK.task_stack_bytes() as u64,
    max_statements: 1_000_000,
};

/// The bytes of the heap the STM32U5A9 keeps free beside a program and its event store: its replies
/// and the Live Values and services of the program (specification 34 MEM-12, Appendix D.5).
pub const PROGRAM_MEMORY_RESERVE_BYTES: usize = 8 * 1024;

/// The device descriptor admission reads for the STM32U5A9 (specification 34 §5.1.1, COMPAT-02 and
/// TARGET-03; specification 44 §10.2): the compiled program image as the only program format, its
/// scheduling profile and numeric contract, its timing evidence (none qualified yet), no platform
/// capability, the maxima of the chip file, its event store, its application slot and what it offers
/// compiled programs ([`NATIVE_TARGET`]). The chip has no wall clock until slice T6, so it supplies no
/// capability and admission refuses a program that calls `CURRENT_DT`.
pub const DEVICE_DESCRIPTOR: DeviceDescriptor = DeviceDescriptor {
    program_versions: &[ProgramVersion::NATIVE_IMAGE],
    scheduling_profiles: &[SchedulingProfile::COOPERATIVE_RESOURCE_V1],
    numeric_contracts: &[NumericContract::V1],
    timing_evidence: TimingEvidence::UNQUALIFIED,
    capabilities: Capabilities::NONE,
    // The maxima of the chip file. An image requests the bytes of its process images and 0 for every
    // other limit (specification 44 §10.1), and its own checks bound it (§10.2).
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
    native: Some(NATIVE_TARGET),
};
