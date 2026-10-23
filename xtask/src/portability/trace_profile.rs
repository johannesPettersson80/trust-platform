//! Per-board identity and boot-banner rules for the UART verifier. The record grammar, record
//! order and counts are shared by every board; only the rules in this module differ.
use super::board::{Board, BoardProfile};
use anyhow::{ensure, Result};

// F401 identity: the device's chip ID and UID words, and bit 29 of the reset word, which reports
// the independent watchdog reset.
const F401_WATCHDOG_RESET: u64 = 1 << 29;

// ESP32-C6 identity, measured on the owned DevKitC-1 (spec 34, D.2): chip ID 13, chip revision
// v0.2 as major * 100 + minor, and the base MAC split into its high 16 and low 32 bits.
const C6_CHIP_ID: u64 = 0x0d;
const C6_REVISION: u64 = 2;
const C6_MAC_HIGH: u64 = 0x206e;
const C6_MAC_LOW: u64 = 0xf117_04c0;
// misa: MXL in bits 31:30 reads RV32, I, M, A and C are set, and F and D are clear.
const MISA_MXL_RV32: u64 = 1;
const MISA_IMAC: u64 = (1 << 8) | (1 << 12) | (1 << 2) | 1;
const MISA_F_D: u64 = (1 << 5) | (1 << 3);
// The first boot starts from the bench USB-UART reset (0x15) or a power-on reset (0x01). The
// watchdog boot must report the MWDT0 system reset (0x07).
const C6_FIRST_BOOT_RESETS: [u64; 2] = [0x15, 0x01];
const C6_WATCHDOG_RESET: u64 = 0x07;

/// Checks one IDENTITY record. `boot` is 0 for the first boot and 1 for the watchdog boot.
pub(super) fn identity(board: &BoardProfile, id: &[u64; 8], boot: usize) -> Result<()> {
    match board.board {
        Board::NucleoF401re => {
            ensure!(
                id[0] == 0x10016433 && id[1..4] == [0x00410016, 0x30395119, 0x35363638],
                "wrong connected reference device"
            );
            ensure!(
                id[4] == board.flash_kib
                    && id[5] == board.cpu_hz
                    && id[6] & ((3 << 22) | (1 << 24)) == 0,
                "identity/profile mismatch"
            );
            ensure!(
                boot == 0 || id[7] & F401_WATCHDOG_RESET != 0,
                "missing independent watchdog reset evidence"
            );
        }
        Board::Esp32c6Devkitc1 => {
            ensure!(
                id[0] == C6_CHIP_ID
                    && id[1] == C6_REVISION
                    && id[2] == C6_MAC_HIGH
                    && id[3] == C6_MAC_LOW,
                "wrong connected reference device"
            );
            ensure!(
                id[4] == board.flash_kib && id[5] == board.cpu_hz,
                "identity/profile mismatch"
            );
            ensure!(
                id[6] >> 30 == MISA_MXL_RV32
                    && id[6] & MISA_IMAC == MISA_IMAC
                    && id[6] & MISA_F_D == 0,
                "misa is not RV32IMAC without F or D"
            );
            if boot == 0 {
                ensure!(
                    C6_FIRST_BOOT_RESETS.contains(&id[7]),
                    "first boot reset reason is neither the USB-UART nor the power-on reset"
                );
            } else {
                ensure!(id[7] == C6_WATCHDOG_RESET, "missing MWDT0 reset evidence");
            }
        }
    }
    Ok(())
}

/// Admits one non-record UART line. Banner lines are the ROM and bootloader output that precedes
/// each boot's first IDENTITY record: the first boot, and the watchdog boot between
/// `STATE,WATCHDOG_ARM` and its IDENTITY. `admitted` counts the banner lines already admitted in
/// the current boot.
pub(super) fn admit_banner(
    board: &BoardProfile,
    identities: usize,
    last_state: Option<&str>,
    admitted: u64,
    line: &str,
) -> Result<()> {
    let window = board.boot_banner_lines > 0
        && (identities == 0 || (identities == 1 && last_state == Some("WATCHDOG_ARM")));
    ensure!(window, "foreign UART record outside a boot banner window");
    ensure!(
        admitted < board.boot_banner_lines,
        "boot banner exceeds {} lines in one boot",
        board.boot_banner_lines
    );
    ensure!(
        line.bytes().all(|byte| (0x20..=0x7e).contains(&byte)),
        "boot banner contains a non-printable byte"
    );
    Ok(())
}
