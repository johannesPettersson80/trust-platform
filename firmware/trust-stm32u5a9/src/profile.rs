//! The fixed STM32U5A9 profile: the installed application's region, the device descriptor that
//! admission reads and the transfer session of a boot (specification 34, §5.1.1, COMPAT-02, TARGET-03
//! and PORT-05; specification 44 §10.2; design §4.4).
use trust_platform_stm32u5::memory::{HEAP_A, MAIN_STACK as MAIN_STACK_REGION};
use trust_runtime_core::deployment::{
    Capabilities, DeviceDescriptor, NativeTarget, NumericContract, ProgramVersion, RequestedLimits,
    SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::native::image::Target;
use trust_runtime_services::runner::DeviceStack;
use trust_runtime_services::transfer::TransferSession;

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

/// The capacity of the STM32U5A9's heap, in bytes: its one region, region A, the 1,048,576 bytes
/// (1 MiB) of SRAM1, SRAM2 and the first 192 KiB of SRAM3 (`HEAP_A` of the adapter's memory map, which
/// the firmware's `memory.rs` gives the allocator whole and asserts to be 1 MiB; specification 34,
/// Appendix D.5). Heap region B, the 754 KiB of SRAM5 below the main stack, is not given to the
/// allocator, so it is not counted. It is the reply capacity of the transfer session: a chip holds a
/// reply as far as its heap reaches (PORT-05).
pub const HEAP_CAPACITY_BYTES: usize = (HEAP_A.end - HEAP_A.start) as usize;

/// The transfer session of boot number `boot` (specification 34, PORT-05). A transfer id carries the
/// boot number in its high 32 bits, so that an id of an earlier boot never names a transfer of this
/// one. The boot number is the chip's boot counter in the backup register BKP14R
/// (`ResetControl::note_boot`), the count its reset record carries: it rises by one at every boot, and
/// the board's CR1220 cell keeps it across a loss of main power (Appendix D.5). A power-on with the
/// cell flat or missing finds the backup registers cleared and counts from 1 again, so an id of a boot
/// before that power-on can repeat; PORT-05 asks for a counter kept in the flash, as the ESP32-C6's is.
///
/// An upload takes at most the application slot, [`APPLICATION_BYTES`], since a larger container can
/// never be installed. A reply is held up to the capacity of the heap, [`HEAP_CAPACITY_BYTES`], as a
/// computer holds one: the chip answers a reply longer than its 8,192-byte frame with a head and the
/// client reads it with `transfer.get` (PORT-05). The reply is written in parts of 4,096 bytes that the
/// heap gives as the writing reaches them, and they are freed when the reply is read, aborted or
/// replaced.
#[must_use]
pub fn transfer_session(boot: u32) -> TransferSession {
    TransferSession::new(boot, APPLICATION_BYTES, HEAP_CAPACITY_BYTES)
}

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
