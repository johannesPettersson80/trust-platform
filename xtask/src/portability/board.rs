//! The bench boards that `xtask portability` inspects and verifies. A board's profile holds the
//! bench's own values: its wiring, the reference firmware's memory budget, its boot banner and
//! its module's flash. Its chip values come from the chip's file (specification 34, TARGET-03 and
//! Appendix H), so they are written once. The NUCLEO-F401RE's STM32F401 is no longer a supported
//! chip and has no file; its profile keeps the values its scope B evidence was taken with.
use anyhow::{bail, Context, Result};
use trust_chips::{Catalog, ChipFile};

/// A bench board, named on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Board {
    NucleoF401re,
    Stm32f407gDisc1,
    Esp32c6Devkitc1,
}

impl Board {
    pub(super) fn parse(name: &str) -> Result<Self> {
        match name {
            "nucleo-f401re" => Ok(Self::NucleoF401re),
            "stm32f407g-disc1" => Ok(Self::Stm32f407gDisc1),
            "esp32c6-devkitc-1" => Ok(Self::Esp32c6Devkitc1),
            other => bail!(
                "unknown board {other:?}; expected nucleo-f401re, stm32f407g-disc1 or esp32c6-devkitc-1"
            ),
        }
    }

    /// The supported chip on the board, whose file holds the board's chip values.
    pub(super) fn chip(self) -> Option<&'static str> {
        match self {
            Self::NucleoF401re => None,
            Self::Stm32f407gDisc1 => Some("stm32f407"),
            Self::Esp32c6Devkitc1 => Some("esp32c6"),
        }
    }

    pub(super) fn profile(self) -> Result<BoardProfile> {
        match self {
            Self::NucleoF401re => Ok(BoardProfile::nucleo_f401re()),
            Self::Stm32f407gDisc1 => BoardProfile::stm32f407g_disc1(),
            Self::Esp32c6Devkitc1 => BoardProfile::esp32c6_devkitc_1(),
        }
    }
}

/// A supported chip's file, from the catalog that `trust-chips` embeds.
pub(super) fn chip(name: &str) -> Result<ChipFile> {
    Catalog::embedded()?
        .chip(name)
        .cloned()
        .with_context(|| format!("no chip file chips/{name}.toml"))
}

/// The file of the board's chip.
fn chip_of(board: Board) -> Result<ChipFile> {
    chip(
        board
            .chip()
            .with_context(|| format!("{board:?} has no supported chip"))?,
    )
}

/// CPU cycles in one 10 ms scan at `cpu_hz`.
const fn cycles_per_scan(cpu_hz: u64) -> u64 {
    cpu_hz / 100
}

/// The values one board's commands need. The F401 values are the constants the commands used
/// before this profile existed. The F407 values are the DISC1 bench composition in spec 34, D.4,
/// and the C6 values the reference composition in spec 34, D.2; their chip values are read from
/// the chip files.
#[derive(Clone, Copy, Debug)]
pub(super) struct BoardProfile {
    pub board: Board,
    /// The command-line name, also written into every report.
    pub name: &'static str,
    pub cpu_hz: u64,
    /// Flash size in KiB, as IDENTITY reports it.
    pub flash_kib: u64,
    /// Cycle bound for one 10 ms scan.
    pub scan_cycle_bound: u64,
    /// Bounded heap arena, as the allocator enforces it.
    pub heap_bytes: u64,
    pub stack_window_bytes: u64,
    pub stack_headroom_bytes: u64,
    /// Non-record UART lines admitted in each boot; zero admits none.
    pub boot_banner_lines: u64,
    /// Whether the user button's pin reads high while the button is pressed. It does on the
    /// DISC1, where B1 connects PA0 to VDD. It does not on the F401's PC13 or the C6's GPIO9,
    /// which are pulled up and driven low by the button.
    pub button_pressed_high: bool,
    /// Space the linked image must fit: the upper executable region on F401 (sectors 4 to 7), the
    /// firmware sectors on F407 (4 to 11), the factory application partition on C6.
    pub image_capacity_bytes: u64,
    /// The separately installable application partition.
    pub application_bytes: u64,
}

impl BoardProfile {
    pub(super) fn nucleo_f401re() -> Self {
        Self {
            board: Board::NucleoF401re,
            name: "nucleo-f401re",
            cpu_hz: 84_000_000,
            flash_kib: 512,
            scan_cycle_bound: 840_000,
            heap_bytes: 72 * 1024,
            stack_window_bytes: 16 * 1024,
            stack_headroom_bytes: 2 * 1024,
            boot_banner_lines: 0,
            button_pressed_high: false,
            image_capacity_bytes: 448 * 1024,
            application_bytes: 16 * 1024,
        }
    }

    pub(super) fn stm32f407g_disc1() -> Result<Self> {
        let chip = chip_of(Board::Stm32f407gDisc1)?;
        let flash = chip
            .memory
            .flash
            .as_ref()
            .context("the STM32F407's chip file has no memory.flash")?;
        let cpu_hz = u64::from(chip.clock.firmware_hz);
        Ok(Self {
            board: Board::Stm32f407gDisc1,
            name: "stm32f407g-disc1",
            cpu_hz,
            flash_kib: u64::from(flash.kib),
            scan_cycle_bound: cycles_per_scan(cpu_hz),
            heap_bytes: 72 * 1024,
            stack_window_bytes: 16 * 1024,
            stack_headroom_bytes: 2 * 1024,
            boot_banner_lines: 0,
            button_pressed_high: true,
            image_capacity_bytes: chip.layout.firmware.bytes(),
            application_bytes: chip.layout.application.bytes(),
        })
    }

    pub(super) fn esp32c6_devkitc_1() -> Result<Self> {
        let chip = chip_of(Board::Esp32c6Devkitc1)?;
        let cpu_hz = u64::from(chip.clock.firmware_hz);
        Ok(Self {
            board: Board::Esp32c6Devkitc1,
            name: "esp32c6-devkitc-1",
            cpu_hz,
            // The 8 MB of SPI flash in the DevKitC-1's module, as its image header reports it;
            // the C6 has no flash of its own.
            flash_kib: 8192,
            scan_cycle_bound: cycles_per_scan(cpu_hz),
            heap_bytes: 72 * 1024,
            stack_window_bytes: 16 * 1024,
            stack_headroom_bytes: 2 * 1024,
            boot_banner_lines: 64,
            button_pressed_high: false,
            image_capacity_bytes: chip.layout.firmware.bytes(),
            application_bytes: chip.layout.application.bytes(),
        })
    }

    /// The largest stack peak the measured window accepts: the window less its headroom.
    pub(super) fn stack_peak_bound(&self) -> u64 {
        self.stack_window_bytes - self.stack_headroom_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARDS: [Board; 3] = [
        Board::NucleoF401re,
        Board::Stm32f407gDisc1,
        Board::Esp32c6Devkitc1,
    ];

    #[test]
    fn board_names_select_their_profiles_and_unknown_names_list_every_board() {
        assert_eq!(Board::parse("nucleo-f401re").unwrap(), Board::NucleoF401re);
        assert_eq!(
            Board::parse("stm32f407g-disc1").unwrap(),
            Board::Stm32f407gDisc1
        );
        assert_eq!(
            Board::parse("esp32c6-devkitc-1").unwrap(),
            Board::Esp32c6Devkitc1
        );
        for board in BOARDS {
            assert_eq!(board.profile().unwrap().board, board);
        }
        let error = Board::parse("esp32c6").unwrap_err().to_string();
        assert!(
            error.contains("nucleo-f401re")
                && error.contains("stm32f407g-disc1")
                && error.contains("esp32c6-devkitc-1")
        );
        // The board name is the profile's name, so a report names what the command was given.
        for name in ["nucleo-f401re", "stm32f407g-disc1", "esp32c6-devkitc-1"] {
            assert_eq!(Board::parse(name).unwrap().profile().unwrap().name, name);
        }
    }

    #[test]
    fn each_bench_board_names_its_supported_chip() {
        assert_eq!(Board::NucleoF401re.chip(), None);
        assert_eq!(Board::Stm32f407gDisc1.chip(), Some("stm32f407"));
        assert_eq!(Board::Esp32c6Devkitc1.chip(), Some("esp32c6"));
        for board in BOARDS {
            if let Some(name) = board.chip() {
                assert_eq!(chip(name).unwrap().identity.name, name);
            }
        }
        assert!(chip("stm32f401").is_err());
    }

    #[test]
    fn f407_profile_carries_the_disc1_bench_values() {
        let disc1 = Board::Stm32f407gDisc1.profile().unwrap();
        assert_eq!(disc1.name, "stm32f407g-disc1");
        assert_eq!(disc1.cpu_hz, 168_000_000);
        assert_eq!(disc1.flash_kib, 1024);
        assert_eq!(disc1.scan_cycle_bound, 1_680_000);
        assert_eq!(disc1.heap_bytes, 72 * 1024);
        assert_eq!(disc1.stack_window_bytes, 16 * 1024);
        assert_eq!(disc1.stack_peak_bound(), 14_336);
        assert_eq!(disc1.boot_banner_lines, 0);
        // Sectors 4 to 11 (64 KiB and seven of 128 KiB) from 0x0801_0000 to the end of flash.
        assert_eq!(disc1.image_capacity_bytes, 0x0810_0000 - 0x0801_0000);
        assert_eq!(disc1.image_capacity_bytes, 960 * 1024);
        assert_eq!(disc1.application_bytes, 16 * 1024);
        assert!(disc1.button_pressed_high);
    }

    #[test]
    fn bench_profiles_take_their_chip_values_from_the_chip_files() {
        let f407 = chip("stm32f407").unwrap();
        let disc1 = Board::Stm32f407gDisc1.profile().unwrap();
        assert_eq!(disc1.cpu_hz, u64::from(f407.clock.firmware_hz));
        assert_eq!(
            disc1.flash_kib,
            u64::from(f407.memory.flash.as_ref().unwrap().kib)
        );
        assert_eq!(disc1.image_capacity_bytes, f407.layout.firmware.bytes());
        assert_eq!(disc1.application_bytes, f407.layout.application.bytes());
        let c6_chip = chip("esp32c6").unwrap();
        let c6 = Board::Esp32c6Devkitc1.profile().unwrap();
        assert_eq!(c6.cpu_hz, u64::from(c6_chip.clock.firmware_hz));
        assert_eq!(c6.image_capacity_bytes, c6_chip.layout.firmware.bytes());
        assert_eq!(c6.application_bytes, c6_chip.layout.application.bytes());
        // The C6's flash is the module's: the bench module holds the chip's whole layout.
        let end = c6_chip
            .layout
            .regions()
            .into_iter()
            .map(|(_, region)| region.end())
            .max()
            .unwrap();
        assert!(c6_chip.memory.flash.is_none());
        assert!(end <= c6.flash_kib * 1024);
    }

    #[test]
    fn only_the_disc1_button_reads_high_while_pressed() {
        assert!(!Board::NucleoF401re.profile().unwrap().button_pressed_high);
        assert!(
            !Board::Esp32c6Devkitc1
                .profile()
                .unwrap()
                .button_pressed_high
        );
        assert!(
            Board::Stm32f407gDisc1
                .profile()
                .unwrap()
                .button_pressed_high
        );
    }

    #[test]
    fn every_scan_cycle_bound_is_ten_milliseconds_of_cpu_cycles() {
        for board in BOARDS {
            let profile = board.profile().unwrap();
            assert_eq!(profile.scan_cycle_bound, profile.cpu_hz / 100, "{board:?}");
        }
    }

    #[test]
    fn c6_profile_carries_the_reference_composition_values() {
        let c6 = Board::Esp32c6Devkitc1.profile().unwrap();
        assert_eq!(c6.name, "esp32c6-devkitc-1");
        assert_eq!(c6.cpu_hz, 160_000_000);
        assert_eq!(c6.flash_kib, 8192);
        assert_eq!(c6.scan_cycle_bound, 1_600_000);
        assert_eq!(c6.heap_bytes, 72 * 1024);
        assert_eq!(c6.stack_peak_bound(), 14_336);
        assert_eq!(c6.boot_banner_lines, 64);
        assert_eq!(c6.image_capacity_bytes, 0x1F0000);
        assert_eq!(c6.application_bytes, 16 * 1024);
    }

    #[test]
    fn c6_partition_table_matches_the_profile_and_uses_custom_types() {
        // Spec 34 D.2 flash layout, as the chip file records it. Custom types keep espflash's
        // table conversion and the bootloader away from truST's data partitions.
        let csv = include_str!("../../../firmware/trust-esp32c6-reference/partitions.csv");
        let rows: Vec<Vec<&str>> = csv
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
            .map(|line| line.split(',').map(str::trim).collect())
            .collect();
        let number = |text: &str| u64::from_str_radix(text.trim_start_matches("0x"), 16).unwrap();
        let layout = chip("esp32c6").unwrap().layout;
        let span = |region: trust_chips::Region| (u64::from(region.start), region.bytes());
        let expected = [
            ("factory", "app", "factory", span(layout.firmware)),
            ("stbc", "0x40", "0x00", span(layout.application)),
            ("ckpt_a", "0x41", "0x00", span(layout.checkpoint_a)),
            ("ckpt_b", "0x41", "0x01", span(layout.checkpoint_b)),
        ];
        assert_eq!(rows.len(), expected.len());
        for (row, (name, kind, subtype, (offset, size))) in rows.iter().zip(expected) {
            assert_eq!(
                (row[0], row[1], row[2], number(row[3]), number(row[4])),
                (name, kind, subtype, offset, size)
            );
        }
        for pair in rows.windows(2) {
            assert!(number(pair[0][3]) + number(pair[0][4]) <= number(pair[1][3]));
        }
        // The factory partition and the application partition are the profile's capacities.
        let c6 = Board::Esp32c6Devkitc1.profile().unwrap();
        assert_eq!(number(rows[0][4]), c6.image_capacity_bytes);
        assert_eq!(number(rows[1][4]), c6.application_bytes);
    }

    #[test]
    fn f401_profile_keeps_the_values_the_commands_used() {
        let f401 = Board::NucleoF401re.profile().unwrap();
        assert_eq!(f401.cpu_hz, 84_000_000);
        assert_eq!(f401.flash_kib, 512);
        assert_eq!(f401.scan_cycle_bound, 840_000);
        assert_eq!(f401.stack_peak_bound(), 14_336);
        assert_eq!(f401.boot_banner_lines, 0);
        assert_eq!(f401.image_capacity_bytes, 448 * 1024);
        assert_eq!(f401.application_bytes, 16 * 1024);
    }
}
