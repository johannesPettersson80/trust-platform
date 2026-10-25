//! Fixed ESP32-C6-DevKitC-1 profile: the D.2 constants and the pure decisions built on them.
use crate::cookie::{ResetCookie, WATCHDOG_PHASE};
use crate::program_flash::SLOT_BYTES;
use trust_runtime_core::deployment::{
    Capabilities, DeviceDescriptor, NativeTarget, NumericContract, ProgramVersion, RequestedLimits,
    SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::native::image::Target;
use trust_runtime_services::runner::DeviceStack;
use trust_runtime_services::transfer::TransferSession;

/// CPU clock from `CpuClock::max()` on ESP32-C6 (160 MHz); the PCCR rate matches it.
pub const CPU_HZ: u32 = 160_000_000;
/// Fixture sampling period in microseconds; the resource loop schedules the program's tasks.
pub const SCAN_PERIOD_US: u64 = 10_000;
/// CPU cycles in one scan period at [`CPU_HZ`] (10 ms).
pub const CYCLES_PER_SCAN: u32 = 1_600_000;
/// Chip ID from the running image header and the owned DevKitC-1.
pub const CHIP_ID: u16 = 13;
/// Chip revision as major·100 + minor (v0.2 on the owned board).
pub const CHIP_REVISION: u16 = 2;
/// Base MAC address of the owned DevKitC-1, in printed order.
pub const BASE_MAC: [u8; 6] = [0x20, 0x6e, 0xf1, 0x17, 0x04, 0xc0];
/// Flash size in KiB of the owned 8 MB DevKitC-1.
pub const FLASH_KIB: u16 = 8192;
/// Stage-0 MWDT0 interval, the same 4,000 ms as the STM32F407 firmware's watchdog.
pub const WATCHDOG_MS: u32 = 4_000;

/// Reset reason of an MWDT0 system reset (`CoreMwdt0`).
pub const RESET_WATCHDOG: u8 = 0x07;
/// Reset reason of a USB-Serial/JTAG reset (`CoreUsbUart`), which the bench uses to start a run.
pub const RESET_USB: u8 = 0x15;
/// Power-on reset (`ChipPowerOn`).
pub const RESET_POWER_ON: u8 = 0x01;

/// Factory application partition; its first 24 bytes are the running image header.
pub const FACTORY_OFFSET: u32 = 0x1_0000;
/// Separately installable application bundle partition (`stbc`).
pub const STBC_OFFSET: u32 = 0x20_0000;
/// Size of the `stbc` partition in bytes: the flash the chip allows for the installed application,
/// from [`STBC_OFFSET`] to the two checkpoint slots at the end of the 8 MB module.
pub const STBC_BYTES: usize = 6112 * 1024;

/// Checkpoint slot `checkpoint_a`: its flash address, the chip file's `[layout] checkpoint_a`
/// (specification 34 D.2 and §10.1). The slot follows the installed application.
pub const CHECKPOINT_A_OFFSET: u32 = 0x7F_8000;
/// Checkpoint slot `checkpoint_b`: its flash address, the chip file's `[layout] checkpoint_b`. It
/// follows slot A and ends the 8 MB module.
pub const CHECKPOINT_B_OFFSET: u32 = 0x7F_C000;
/// The bytes of each checkpoint slot: 16 KiB, four erase units (the chip file's `kib = 16`).
pub const CHECKPOINT_SLOT_BYTES: u32 = 16 * 1024;
/// The bytes one flash program writes at a time, and the alignment of every flash access the
/// checkpoint makes: the word `esp-storage` writes (specification 34 §10.1).
pub const FLASH_PROGRAM_BYTES: u32 = 4;
/// The bytes one flash erase clears: a 4 KiB sector (the chip file's `memory.spi_flash.erase_kib`).
pub const FLASH_ERASE_BYTES: u32 = 4 * 1024;
/// The most bytes one checkpoint step programs between two scans: one 256-byte flash page, so a
/// step takes a few milliseconds (specification 34 §10).
pub const CHECKPOINT_STEP_BYTES: usize = 256;
/// The most bytes one step of an install programs or reads back between two passes of the loop: one
/// 256-byte flash page, as a checkpoint step. An erase of one 4 KiB unit, up to about 0.4 s, is the
/// longest step (specification 34 §11.2 and D.2).
pub const PROGRAM_STEP_BYTES: usize = 256;

/// The device descriptor admission reads for the C6 (specification 34 §5.1.1, COMPAT-02 and
/// TARGET-03; specification 44 §10.2): the compiled program image as the only program format, its
/// scheduling profile and numeric contract, its timing evidence, no platform capability, no maximum
/// for the record's limits but the process image, which the image's checks bound, its event store's
/// default and maximum, its application slot, and what it offers compiled programs
/// ([`NATIVE_TARGET`]).
///
/// The event store's default and maximum are the `[events]` table of `chips/esp32c6.toml` (200 and
/// 200). The application slot is one program slot of the program store, not the whole
/// `application` region: an install writes one slot (specification 34 §5.1). The C6 has no wall
/// clock until slice T6, so it supplies no capability and admission refuses a program that calls
/// `CURRENT_DT`.
pub const DEVICE_DESCRIPTOR: DeviceDescriptor = DeviceDescriptor {
    program_versions: &[ProgramVersion::NATIVE_IMAGE],
    scheduling_profiles: &[SchedulingProfile::COOPERATIVE_RESOURCE_V1],
    numeric_contracts: &[NumericContract::V1],
    timing_evidence: TimingEvidence::UNQUALIFIED,
    capabilities: Capabilities::NONE,
    // The maxima of the chip file (`chips/esp32c6.toml`). An image requests the bytes of its process
    // images and 0 for every other limit (specification 44 §10.1), and its own checks bound it
    // (§10.2).
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
    // The `[events]` table of `chips/esp32c6.toml`: the C6's event store keeps 200 events by
    // default and admits at most 200 until a measurement of the heap a program leaves on the
    // bench allows more.
    event_store_default: 200,
    event_store_max: 200,
    // One of the two program slots of `chips/esp32c6.toml` (`application_a` and `application_b`, the
    // halves of the `application` region that the owner gave the program, decision O7): an install
    // writes one slot, so a program and its record can be no longer than that (specification 34 §5.1.1
    // step 8).
    application_slot_bytes: Some(SLOT_BYTES as u64),
    // Measured at boot, before the first program loads (`Platform::set_program_memory`).
    program_memory_bytes: None,
    native: Some(NATIVE_TARGET),
};

/// The C6's main stack (specification 34 MEM-03 and Appendix D.2; specification 44 §8.5).
///
/// - The window is 78 KiB of the 85,980-byte esp-hal stack region below 0x4086_E610, so that the
///   region keeps the 4 KiB margin above the window that the linker guard and the build require.
/// - The C1-25 bench measured the deepest path of the firmware before a program runs at 34,008 bytes
///   (the install of a program).
pub const MAIN_STACK: DeviceStack = DeviceStack {
    window_bytes: 78 * 1024,
    base_bytes: 34_008,
};

/// The bytes of the C6's program region (specification 44 §10.3, §12.3): RAM the C6 executes, which
/// the heap holds the programs' code in, an installed program's module copied in and the image of an
/// online change, whose module runs where it lies. 192 KiB holds the pasteurizer's installed module
/// (43,810 bytes with debug hooks) and its changed image (90,200 bytes), and later two changed images
/// at once, as a change of a changed program needs; a module the region has no room for runs where
/// it lies in its slot. The region is the linker's `.trust_code` section; it, the main heap's added
/// 64 KiB and the main stack's region share the RAM that the bytecode engine's RAM code took, and the
/// stack's region keeps more than the 82 KiB that its window and margin need (`stack_guard.x`, the
/// build report's `stack.region_bytes`).
pub const PROGRAM_CODE_BYTES: usize = 192 * 1024;

/// What the C6 offers compiled programs (specification 44 §10.2 and NX-10): the RV32IMAC module
/// target; as the task stack, the main stack's window less its headroom and the deepest path's
/// base, 79,872 − 2,048 − 34,008 = 43,816 bytes, which a scan's path, shallower than the install's
/// that the base measured, leaves to the module; and the default statement budget of 1,000,000.
pub const NATIVE_TARGET: NativeTarget = NativeTarget {
    target: Target::Rv32imac,
    task_stack_bytes: MAIN_STACK.task_stack_bytes() as u64,
    max_statements: 1_000_000,
};

/// The capacity of the C6's heap, in bytes: its three regions, the 32,832-byte main heap, the 64 KiB
/// of the boot loader's reclaimed RAM and the 16,320 bytes of LP SRAM (`HEAP_BYTES`,
/// `RECLAIMED_BYTES` and `LOW_POWER_BYTES` of the firmware's `memory.rs`; specification 34, Appendix
/// D.2). It is the reply capacity of the transfer session: a chip holds a reply as far as its heap
/// reaches (PORT-05). The host test `the_c6_session_holds_replies_up_to_its_heap` keeps it equal to
/// the three figures of `memory.rs`.
pub const HEAP_CAPACITY_BYTES: usize = 32_832 + 65_536 + 16_320;

/// The transfer session of boot number `boot` (specification 34, PORT-05). A transfer id carries the
/// boot number in its high 32 bits, so an id of an earlier boot never names a transfer of this one. The
/// boot number is the chip's boot counter, which it keeps in its flash (`boot_counter.rs`): it rises
/// by one at every boot, whatever its cause, power-on included, so an id never repeats across boots.
///
/// An upload takes at most one program slot, since a larger container can never be installed. A reply
/// is held up to the capacity of the heap, [`HEAP_CAPACITY_BYTES`], as a computer holds one: a chip
/// answers a reply longer than its 8,192-byte frame with a head and the client reads it with
/// `transfer.get` (PORT-05). The reply is written in parts of 4,096 bytes that the heap gives as the
/// writing reaches them, and they are freed when the reply is read, aborted or replaced
/// (Appendix D.2, long replies).
#[must_use]
pub fn transfer_session(boot: u32) -> TransferSession {
    TransferSession::new(boot, SLOT_BYTES as usize, HEAP_CAPACITY_BYTES)
}

/// BOOT button GPIO: a strapping pin that is read only after boot, active low.
pub const BUTTON_GPIO: u8 = 9;
/// Plain output GPIO with no board load; read back through its own pad.
pub const OUTPUT_GPIO: u8 = 18;

/// Largest number of bytes in one console record, shared by all of its fragments.
pub const RECORD_BYTES: usize = 512;
/// Drain deadline of one console record in microseconds of SYSTIMER time.
pub const RECORD_DEADLINE_US: u64 = 100_000;
/// The same 100 ms deadline in CPU cycles, for the emergency writer that reads PCCR only.
pub const RECORD_DEADLINE_CYCLES: u32 = 16_000_000;

/// One BOOT button observation. GPIO9 is active low with the internal pull-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputSnapshot {
    /// Actual GPIO9 input level.
    pub gpio9_high: bool,
    /// Pressing BOOT connects GPIO9 to ground.
    pub button_pressed: bool,
}
impl InputSnapshot {
    /// Normalize the physical level; this does not debounce or invent a press.
    pub const fn from_gpio9(high: bool) -> Self {
        Self {
            gpio9_high: high,
            button_pressed: !high,
        }
    }
}

/// What the boot reset reason and the leftover reset cookie allow the firmware to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootDecision {
    /// A USB or power-on reset starts a new run; any leftover cookie is discarded.
    StartRun,
    /// An MWDT0 reset with the watchdog cookie completes the intentional watchdog phase.
    CompleteWatchdogRun,
    /// Any other reset, or an MWDT0 reset without the watchdog cookie, is a failed run.
    UnexpectedReset,
}

/// Apply the D.2 reset-cookie rule to the boot reset reason and the stored cookie.
pub const fn boot_decision(reason: u8, cookie: Option<ResetCookie>) -> BootDecision {
    match reason {
        RESET_WATCHDOG => match cookie {
            Some(ResetCookie {
                phase: WATCHDOG_PHASE,
            }) => BootDecision::CompleteWatchdogRun,
            _ => BootDecision::UnexpectedReset,
        },
        RESET_USB | RESET_POWER_ON => BootDecision::StartRun,
        _ => BootDecision::UnexpectedReset,
    }
}

const MISA_A: u32 = 1 << 0;
const MISA_C: u32 = 1 << 2;
const MISA_D: u32 = 1 << 3;
const MISA_F: u32 = 1 << 5;
const MISA_I: u32 = 1 << 8;
const MISA_M: u32 = 1 << 12;

/// Whether `misa` describes the measured C6 numeric profile: RV32 (MXL = 1) with I, M, A and C,
/// and without the F or D floating-point extensions, so REAL and LREAL use software arithmetic.
pub const fn numeric_profile_ok(misa: u32) -> bool {
    let required = MISA_I | MISA_M | MISA_A | MISA_C;
    (misa >> 30) == 1 && (misa & required) == required && (misa & (MISA_F | MISA_D)) == 0
}

/// The platform the C6 reports in `status` (specification 11, `status`): its chip name.
pub const PLATFORM: &str = "esp32c6";

/// The C6's stable runtime ID (specification 11, `status`): its chip name and its USB serial
/// number, the base MAC address in the order and case the USB-Serial/JTAG controller reports it,
/// for example `esp32c6-20:6E:F1:17:04:C0`. It needs no stored file, stays the same across resets
/// and differs between two chips.
#[must_use]
pub fn runtime_id(mac: [u8; 6]) -> smol_str::SmolStr {
    let [a, b, c, d, e, f] = mac;
    smol_str::format_smolstr!("{PLATFORM}-{a:02X}:{b:02X}:{c:02X}:{d:02X}:{e:02X}:{f:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cookie::RUNNING_PHASE;

    #[test]
    fn boot_button_is_active_low() {
        assert_eq!(
            InputSnapshot::from_gpio9(true),
            InputSnapshot {
                gpio9_high: true,
                button_pressed: false
            }
        );
        assert_eq!(
            InputSnapshot::from_gpio9(false),
            InputSnapshot {
                gpio9_high: false,
                button_pressed: true
            }
        );
    }

    #[test]
    fn measured_c6_isa_passes_and_fp_or_wrong_mxl_fails() {
        assert!(numeric_profile_ok(0x4090_3105));
        assert!(!numeric_profile_ok(0x4090_3125)); // F set
        assert!(!numeric_profile_ok(0x4090_310D)); // D set
        assert!(!numeric_profile_ok(0x8090_3105)); // MXL = 2
        assert!(!numeric_profile_ok(0x0090_3105)); // MXL = 0
        assert!(!numeric_profile_ok(0x4090_3005)); // I missing
        assert!(!numeric_profile_ok(0x4090_2105)); // M missing
        assert!(!numeric_profile_ok(0x4090_3104)); // A missing
        assert!(!numeric_profile_ok(0x4090_3101)); // C missing
    }

    #[test]
    fn boot_decision_covers_every_reset_and_cookie_branch() {
        let watchdog = Some(ResetCookie {
            phase: WATCHDOG_PHASE,
        });
        let running = Some(ResetCookie {
            phase: RUNNING_PHASE,
        });
        let other = Some(ResetCookie { phase: 0x1234 });
        assert_eq!(
            boot_decision(RESET_WATCHDOG, watchdog),
            BootDecision::CompleteWatchdogRun
        );
        assert_eq!(
            boot_decision(RESET_WATCHDOG, running),
            BootDecision::UnexpectedReset
        );
        assert_eq!(
            boot_decision(RESET_WATCHDOG, other),
            BootDecision::UnexpectedReset
        );
        assert_eq!(
            boot_decision(RESET_WATCHDOG, None),
            BootDecision::UnexpectedReset
        );
        for cookie in [None, watchdog, running, other] {
            assert_eq!(boot_decision(RESET_USB, cookie), BootDecision::StartRun);
            assert_eq!(
                boot_decision(RESET_POWER_ON, cookie),
                BootDecision::StartRun
            );
        }
        for reason in [0x00, 0x03, 0x0F, 0x12, 0x14, 0x16, 0x18, 0xff] {
            for cookie in [None, watchdog, running] {
                assert_eq!(boot_decision(reason, cookie), BootDecision::UnexpectedReset);
            }
        }
    }

    #[test]
    fn constants_match_the_measured_profile() {
        assert_eq!(u64::from(CYCLES_PER_SCAN) * 100, u64::from(CPU_HZ));
        assert_eq!(u64::from(RECORD_DEADLINE_CYCLES) * 10, u64::from(CPU_HZ));
        assert_eq!(SCAN_PERIOD_US, 10_000);
        assert_eq!(RECORD_DEADLINE_US, 100_000);
        assert_eq!(RECORD_BYTES, 512);
        assert_eq!((CHIP_ID, CHIP_REVISION, FLASH_KIB), (13, 2, 8192));
        assert_eq!(BASE_MAC, [0x20, 0x6e, 0xf1, 0x17, 0x04, 0xc0]);
        assert_eq!(WATCHDOG_MS, 4_000);
        assert_eq!(
            (RESET_WATCHDOG, RESET_USB, RESET_POWER_ON),
            (0x07, 0x15, 0x01)
        );
        assert_eq!((FACTORY_OFFSET, STBC_OFFSET), (0x1_0000, 0x20_0000));
        assert_eq!(STBC_BYTES, 6112 * 1024);
        assert_eq!((BUTTON_GPIO, OUTPUT_GPIO), (9, 18));
    }
}
