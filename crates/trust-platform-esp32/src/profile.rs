//! Fixed ESP32-C6-DevKitC-1 profile: the D.2 constants and the pure decisions built on them.
use crate::cookie::{ResetCookie, WATCHDOG_PHASE};

/// CPU clock from `CpuClock::max()` on ESP32-C6 (160 MHz); the PCCR rate matches it.
pub const CPU_HZ: u32 = 160_000_000;
/// Fixture sampling period in microseconds; shared runtime scheduling remains engine-owned.
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
/// Stage-0 MWDT0 interval, the same 4,000 ms as the F401 profile.
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
/// Size of the `stbc` partition in bytes.
pub const STBC_BYTES: usize = 16 * 1024;

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
        assert_eq!(STBC_BYTES, 16 * 1024);
        assert_eq!((BUTTON_GPIO, OUTPUT_GPIO), (9, 18));
    }
}
