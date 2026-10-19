//! Per-board identity, clock-record and boot-banner rules for the UART verifier. The record
//! grammar, record order and counts are shared by every board; only the rules in this module
//! differ.
use super::board::{Board, BoardProfile};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::ops::RangeInclusive;

// STM32F4 identity: the chip's DBGMCU_IDCODE and UID words, and bit 29 of the reset word
// (RCC_CSR.IWDGRSTF), which reports the independent watchdog reset on the F407.
const STM32_WATCHDOG_RESET: u64 = 1 << 29;

/// The words of an STM32F4 IDENTITY record that name the connected chip.
struct Stm32Device {
    /// DBGMCU_IDCODE: the revision in the upper half and the device in the lower.
    idcode: u64,
    /// The three words of the 96-bit unique ID.
    uid: [u64; 3],
}

// The owned STM32F407G-DISC1, from the M0 record of 16 October 2026 (spec 34, D.4).
const F407_DEVICE: Stm32Device = Stm32Device {
    idcode: 0x100f6413,
    uid: [0x0034002d, 0x34314715, 0x35363630],
};

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
        Board::Stm32f407gDisc1 => stm32_identity(board, &F407_DEVICE, id, boot)?,
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
        // The U5 track reads its evidence over the control protocol, so it has no trace identity.
        Board::RiverdiRvt50hqsnwc00B => anyhow::bail!(
            "{}: the STM32U5A9 identity is read over the control protocol, not from a trace",
            board.name
        ),
    }
    Ok(())
}

/// The IDENTITY record of an STM32F4 board: the connected chip, the flash size and system clock
/// of the profile, the FPSCR with round-to-nearest and no flush-to-zero, and, on the watchdog
/// boot, the independent watchdog reset flag.
fn stm32_identity(
    board: &BoardProfile,
    device: &Stm32Device,
    id: &[u64; 8],
    boot: usize,
) -> Result<()> {
    ensure!(
        id[0] == device.idcode && id[1..4] == device.uid,
        "wrong connected reference device"
    );
    ensure!(
        id[4] == board.flash_kib && id[5] == board.cpu_hz && id[6] & ((3 << 22) | (1 << 24)) == 0,
        "identity/profile mismatch"
    );
    ensure!(
        boot == 0 || id[7] & STM32_WATCHDOG_RESET != 0,
        "missing independent watchdog reset evidence"
    );
    Ok(())
}

/// Whether the board's firmware prints a `CLOCK` record right after each IDENTITY record: the
/// first boot's and the watchdog boot's. Only the F407 does; it measures its crystal at start.
pub(super) fn reports_clock(board: &BoardProfile) -> bool {
    board.board == Board::Stm32f407gDisc1
}

/// The crystals the F407's oscillator supports, in Hz (DS8626: 4 to 26 MHz).
const CRYSTAL_HZ: RangeInclusive<u64> = 4_000_000..=26_000_000;

/// The notes of the clock record (`ClockNote::code` in the adapter's `crystal.rs`): the clock
/// source each goes with, and whether a crystal frequency was measured.
const CLOCK_NOTES: [(&str, &str, bool); 5] = [
    ("measured", "HSE", true),
    ("hse-not-ready", "HSI", false),
    ("no-capture", "HSI", false),
    ("out-of-range", "HSI", true),
    ("ambiguous", "HSI", true),
];

/// Checks one CLOCK record, `B1,CLOCK,<HSE|HSI>,<measured Hz>,<crystal Hz>,<SYSCLK Hz>,<note>`
/// (spec 34, D.4), and returns its fields for the report. `fields` is the whole record.
///
/// The crystal is measured against the HSI and rounded to a whole MHz; the PLL runs from it
/// (source HSE) or, when there is none that can be used, from the HSI. The system clock is the
/// profile's either way.
pub(super) fn clock(board: &BoardProfile, fields: &[&str]) -> Result<Value> {
    ensure!(
        reports_clock(board),
        "{} prints no clock record",
        board.name
    );
    ensure!(
        fields.len() == 7,
        "malformed clock record: {} fields, expected 7",
        fields.len()
    );
    let number = |text: &str| {
        text.parse::<u64>()
            .context("invalid unsigned decimal in clock record")
    };
    let (source, note) = (fields[2], fields[6]);
    let (measured_hz, crystal_hz, sysclk_hz) =
        (number(fields[3])?, number(fields[4])?, number(fields[5])?);
    ensure!(
        matches!(source, "HSE" | "HSI"),
        "unknown clock source {source:?}"
    );
    let &(_, note_source, measures) = CLOCK_NOTES
        .iter()
        .find(|(name, ..)| *name == note)
        .with_context(|| format!("unknown clock note {note:?}"))?;
    ensure!(
        source == note_source,
        "clock source {source} does not go with the note {note}"
    );
    ensure!(
        (measured_hz != 0) == measures,
        "measured crystal of {measured_hz} Hz does not go with the note {note}"
    );
    ensure!(
        sysclk_hz == board.cpu_hz,
        "system clock of {sysclk_hz} Hz differs from the profile's {} Hz",
        board.cpu_hz
    );
    if source == "HSE" {
        ensure!(
            CRYSTAL_HZ.contains(&crystal_hz),
            "crystal of {crystal_hz} Hz is outside 4 to 26 MHz"
        );
        let rounded = measured_hz.saturating_add(500_000) / 1_000_000 * 1_000_000;
        ensure!(
            crystal_hz == rounded,
            "crystal of {crystal_hz} Hz is not the measured {measured_hz} Hz rounded to a whole MHz"
        );
    } else {
        ensure!(
            crystal_hz == 0,
            "the PLL runs from the HSI, so it has no crystal figure; got {crystal_hz} Hz"
        );
    }
    Ok(json!({
        "source": source, "measured_hz": measured_hz, "crystal_hz": crystal_hz,
        "sysclk_hz": sysclk_hz, "note": note,
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn disc1() -> BoardProfile {
        BoardProfile::stm32f407g_disc1().unwrap()
    }

    fn c6() -> BoardProfile {
        BoardProfile::esp32c6_devkitc_1().unwrap()
    }

    /// The error text for a clock record on the DISC1, with its causes.
    fn refused(record: &str) -> String {
        let fields: Vec<&str> = record.split(',').collect();
        format!("{:#}", clock(&disc1(), &fields).unwrap_err())
    }

    fn accepted(record: &str) -> Value {
        let fields: Vec<&str> = record.split(',').collect();
        clock(&disc1(), &fields).unwrap()
    }

    #[test]
    fn the_pinned_chip_values_are_those_of_the_chip_files() {
        let f407 = crate::portability::board::chip("stm32f407").unwrap();
        // DEV_ID, bits 11:0 of the DISC1's DBGMCU_IDCODE.
        assert_eq!(
            F407_DEVICE.idcode & 0xfff,
            u64::from(f407.identity.device_id)
        );
        let crystals = u64::from(f407.clock.crystal_min_hz)..=u64::from(f407.clock.crystal_max_hz);
        assert_eq!(CRYSTAL_HZ, crystals);
        let c6 = crate::portability::board::chip("esp32c6").unwrap();
        assert_eq!(C6_CHIP_ID, u64::from(c6.identity.device_id));
        // misa: RV32 with I, M, A and C, and neither F nor D, so no FPU.
        assert_eq!(c6.processor.isa, "RV32IMAC");
        assert_eq!(c6.processor.fpu, trust_chips::Fpu::None);
    }

    #[test]
    fn only_the_f407_prints_a_clock_record() {
        assert!(reports_clock(&disc1()));
        assert!(!reports_clock(&c6()));
        let record: Vec<&str> = "B1,CLOCK,HSE,8000000,8000000,168000000,measured"
            .split(',')
            .collect();
        let error = clock(&c6(), &record).unwrap_err().to_string();
        assert!(error.contains("prints no clock record"), "{error}");
    }

    #[test]
    fn a_crystal_record_reports_its_fields() {
        let report = accepted("B1,CLOCK,HSE,7998123,8000000,168000000,measured");
        assert_eq!(
            report,
            json!({"source":"HSE","measured_hz":7_998_123,"crystal_hz":8_000_000,
                "sysclk_hz":168_000_000,"note":"measured"})
        );
        // The crystals the oscillator supports, at the ends of the range and as measured against
        // an HSI that is a little fast or slow.
        for (measured, crystal) in [
            (3_960_000, 4_000_000),
            (4_000_000, 4_000_000),
            (25_950_000, 26_000_000),
            (26_100_000, 26_000_000),
            (8_499_999, 8_000_000),
            (8_500_000, 9_000_000),
        ] {
            let record = format!("B1,CLOCK,HSE,{measured},{crystal},168000000,measured");
            assert_eq!(accepted(&record)["crystal_hz"], crystal, "{record}");
        }
    }

    #[test]
    fn every_fallback_to_the_hsi_is_a_valid_start_that_says_why() {
        for record in [
            "B1,CLOCK,HSI,0,0,168000000,hse-not-ready",
            "B1,CLOCK,HSI,0,0,168000000,no-capture",
            "B1,CLOCK,HSI,2999000,0,168000000,out-of-range",
            "B1,CLOCK,HSI,8300000,0,168000000,ambiguous",
        ] {
            let report = accepted(record);
            assert_eq!(report["source"], "HSI", "{record}");
            assert_eq!(report["crystal_hz"], 0, "{record}");
        }
    }

    #[test]
    fn the_record_has_seven_fields_and_decimal_numbers() {
        let good = "B1,CLOCK,HSE,7998123,8000000,168000000,measured";
        assert!(refused(&format!("{good},extra")).contains("malformed clock record"));
        assert!(refused("B1,CLOCK,HSE,7998123,8000000,168000000").contains("malformed"));
        assert!(refused("B1,CLOCK").contains("malformed"));
        for bad in ["", "-1", "8e6", "0x7a1200", "8000000.0", " 8000000"] {
            let record = format!("B1,CLOCK,HSE,{bad},8000000,168000000,measured");
            assert!(
                refused(&record).contains("invalid unsigned decimal"),
                "{record}"
            );
        }
        assert!(
            refused("B1,CLOCK,HSE,7998123,18446744073709551616,168000000,measured")
                .contains("invalid unsigned decimal")
        );
    }

    #[test]
    fn the_source_must_be_hse_or_hsi_and_go_with_the_note() {
        assert!(refused("B1,CLOCK,PLL,7998123,8000000,168000000,measured")
            .contains("unknown clock source"));
        assert!(refused("B1,CLOCK,hse,7998123,8000000,168000000,measured")
            .contains("unknown clock source"));
        assert!(
            refused("B1,CLOCK,HSE,7998123,8000000,168000000,locked").contains("unknown clock note")
        );
        // A crystal in use is a measured one, and a note of a failure means the HSI.
        assert!(refused("B1,CLOCK,HSI,7998123,8000000,168000000,measured")
            .contains("does not go with the note"));
        assert!(
            refused("B1,CLOCK,HSE,0,0,168000000,no-capture").contains("does not go with the note")
        );
        assert!(refused("B1,CLOCK,HSE,8300000,8000000,168000000,ambiguous")
            .contains("does not go with the note"));
    }

    #[test]
    fn the_system_clock_is_the_profiles_whichever_clock_feeds_the_pll() {
        for record in [
            "B1,CLOCK,HSE,7998123,8000000,84000000,measured",
            "B1,CLOCK,HSE,7998123,8000000,0,measured",
            "B1,CLOCK,HSE,7998123,8000000,168000001,measured",
            "B1,CLOCK,HSI,0,0,84000000,no-capture",
        ] {
            assert!(
                refused(record).contains("differs from the profile"),
                "{record}"
            );
        }
    }

    #[test]
    fn a_crystal_figure_is_in_the_oscillator_range_and_the_measurement_rounded() {
        // An HSE with no crystal figure, and figures outside 4 to 26 MHz.
        for crystal in ["0", "3000000", "3999999", "26000001", "27000000"] {
            let record = format!("B1,CLOCK,HSE,7998123,{crystal},168000000,measured");
            assert!(refused(&record).contains("outside 4 to 26 MHz"), "{record}");
        }
        // The figure is the measurement to the nearest whole MHz, not another crystal.
        for crystal in ["7000000", "9000000", "8000001", "26000000"] {
            let record = format!("B1,CLOCK,HSE,7998123,{crystal},168000000,measured");
            assert!(
                refused(&record).contains("rounded to a whole MHz"),
                "{record}"
            );
        }
        // A measurement of nothing cannot have named a crystal.
        assert!(refused("B1,CLOCK,HSE,0,8000000,168000000,measured")
            .contains("does not go with the note"));
        // The largest measurement does not overflow the rounding.
        assert!(
            refused("B1,CLOCK,HSE,18446744073709551615,26000000,168000000,measured")
                .contains("rounded to a whole MHz")
        );
    }

    #[test]
    fn the_hsi_has_no_crystal_figure_and_a_measurement_matches_its_note() {
        assert!(
            refused("B1,CLOCK,HSI,0,8000000,168000000,no-capture").contains("no crystal figure")
        );
        assert!(refused("B1,CLOCK,HSI,8300000,8000000,168000000,ambiguous")
            .contains("no crystal figure"));
        // Nothing measured goes with a crystal that did not start or a capture that failed, and
        // something measured with a figure that was refused.
        assert!(refused("B1,CLOCK,HSI,5000,0,168000000,no-capture")
            .contains("does not go with the note"));
        assert!(refused("B1,CLOCK,HSI,5000,0,168000000,hse-not-ready")
            .contains("does not go with the note"));
        assert!(
            refused("B1,CLOCK,HSI,0,0,168000000,ambiguous").contains("does not go with the note")
        );
        assert!(refused("B1,CLOCK,HSI,0,0,168000000,out-of-range")
            .contains("does not go with the note"));
    }

    #[test]
    fn the_clock_notes_are_the_adapters_five_codes() {
        let sources: Vec<_> = CLOCK_NOTES.iter().map(|&(_, source, _)| source).collect();
        assert_eq!(sources, ["HSE", "HSI", "HSI", "HSI", "HSI"]);
        let notes: Vec<_> = CLOCK_NOTES.iter().map(|&(note, ..)| note).collect();
        assert_eq!(
            notes,
            [
                "measured",
                "hse-not-ready",
                "no-capture",
                "out-of-range",
                "ambiguous"
            ]
        );
        // `ClockNote::code` in crates/trust-platform-stm32f4/src/crystal.rs prints these codes.
        let adapter = include_str!("../../../crates/trust-platform-stm32f4/src/crystal.rs");
        for note in notes {
            assert!(adapter.contains(&format!("=> \"{note}\"")), "{note}");
        }
        assert!(adapter.contains("Self::Hse => \"HSE\""));
        assert!(adapter.contains("Self::Hsi => \"HSI\""));
    }

    #[test]
    fn the_f407_is_told_apart_from_another_chip_by_its_chip_words() {
        let words = |idcode: &str, uid: [&str; 3], flash: &str, hz: &str| -> [u64; 8] {
            let hex = |text: &str| u64::from_str_radix(text, 16).unwrap();
            [
                hex(idcode),
                hex(uid[0]),
                hex(uid[1]),
                hex(uid[2]),
                flash.parse().unwrap(),
                hz.parse().unwrap(),
                0,
                0,
            ]
        };
        // Another chip's words: the STM32F401 of the retired NUCLEO-F401RE.
        let other = words(
            "10016433",
            ["00410016", "30395119", "35363638"],
            "512",
            "84000000",
        );
        let f407 = words(
            "100f6413",
            ["0034002d", "34314715", "35363630"],
            "1024",
            "168000000",
        );
        identity(&disc1(), &f407, 0).unwrap();
        let error = identity(&disc1(), &other, 0).unwrap_err().to_string();
        assert!(
            error.contains("wrong connected reference device"),
            "{error}"
        );
        // The watchdog boot needs the independent watchdog flag.
        identity(
            &disc1(),
            &[
                f407[0],
                f407[1],
                f407[2],
                f407[3],
                1024,
                168_000_000,
                0,
                1 << 29,
            ],
            1,
        )
        .unwrap();
        assert!(identity(&disc1(), &f407, 1).is_err());
    }
}
