//! The bench boards whose linked firmware `xtask portability inspect` checks. A board's profile
//! holds the bench's own values: the reference firmware's stack budget. Its chip values, the space
//! the image must fit and the application's, come from the chip's file (specification 34, TARGET-03
//! and Appendix H), so they are written once.
use anyhow::{bail, Context, Result};
use trust_chips::{Catalog, ChipFile};

/// A bench board, named on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Board {
    Esp32c6Devkitc1,
    RiverdiRvt50hqsnwc00B,
}

impl Board {
    pub(super) fn parse(name: &str) -> Result<Self> {
        match name {
            "esp32c6-devkitc-1" => Ok(Self::Esp32c6Devkitc1),
            "riverdi-rvt50hqsnwc00-b" => Ok(Self::RiverdiRvt50hqsnwc00B),
            other => {
                bail!("unknown board {other:?}; expected esp32c6-devkitc-1 or riverdi-rvt50hqsnwc00-b")
            }
        }
    }

    /// The supported chip on the board, whose file holds the board's chip values.
    pub(super) fn chip(self) -> &'static str {
        match self {
            Self::Esp32c6Devkitc1 => "esp32c6",
            Self::RiverdiRvt50hqsnwc00B => "stm32u5a9",
        }
    }

    pub(super) fn profile(self) -> Result<BoardProfile> {
        match self {
            Self::Esp32c6Devkitc1 => BoardProfile::esp32c6_devkitc_1(),
            Self::RiverdiRvt50hqsnwc00B => BoardProfile::riverdi_rvt50hqsnwc00_b(),
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

/// The file of the board's chip, from the catalog that `trust-chips` embeds.
fn chip_of(board: Board) -> Result<ChipFile> {
    chip(board.chip())
}

/// The values one board's commands need. The C6 values are the reference composition in spec 34,
/// D.2, and the Riverdi values the STM32U5A9 bench in spec 34, D.5; their chip values are read from
/// the chip files.
#[derive(Clone, Copy, Debug)]
pub(super) struct BoardProfile {
    pub board: Board,
    /// The command-line name, also written into every report.
    pub name: &'static str,
    pub stack_window_bytes: u64,
    pub stack_headroom_bytes: u64,
    /// Space the linked image must fit: the factory application partition on the C6, the firmware
    /// region of the chip's layout on the STM32U5A9.
    pub image_capacity_bytes: u64,
    /// The separately installable application partition.
    pub application_bytes: u64,
}

impl BoardProfile {
    pub(super) fn esp32c6_devkitc_1() -> Result<Self> {
        let chip = chip_of(Board::Esp32c6Devkitc1)?;
        Ok(Self {
            board: Board::Esp32c6Devkitc1,
            name: "esp32c6-devkitc-1",
            // Specification 34, D.2: 78 KiB of the 85,980-byte stack region, the main stack whose
            // call levels the C6's profile gives (MEM-03; the C1-25 bench, 21 October 2026).
            stack_window_bytes: 78 * 1024,
            stack_headroom_bytes: 2 * 1024,
            image_capacity_bytes: chip.layout.firmware.bytes(),
            application_bytes: chip.layout.application.bytes(),
        })
    }

    pub(super) fn riverdi_rvt50hqsnwc00_b() -> Result<Self> {
        let chip = chip_of(Board::RiverdiRvt50hqsnwc00B)?;
        Ok(Self {
            board: Board::RiverdiRvt50hqsnwc00B,
            name: "riverdi-rvt50hqsnwc00-b",
            // The 78 KiB main stack at the top of SRAM5, with the 2 KiB headroom of the stack rule
            // (design section 4.3, spec 34 D.5; the C6's window until this chip's bench, 21 October
            // 2026).
            stack_window_bytes: 78 * 1024,
            stack_headroom_bytes: 2 * 1024,
            image_capacity_bytes: chip.layout.firmware.bytes(),
            application_bytes: chip.layout.application.bytes(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARDS: [Board; 2] = [Board::Esp32c6Devkitc1, Board::RiverdiRvt50hqsnwc00B];

    #[test]
    fn board_names_select_their_profiles_and_unknown_names_list_every_board() {
        assert_eq!(
            Board::parse("esp32c6-devkitc-1").unwrap(),
            Board::Esp32c6Devkitc1
        );
        assert_eq!(
            Board::parse("riverdi-rvt50hqsnwc00-b").unwrap(),
            Board::RiverdiRvt50hqsnwc00B
        );
        for board in BOARDS {
            assert_eq!(board.profile().unwrap().board, board);
        }
        let error = Board::parse("esp32c6").unwrap_err().to_string();
        assert!(error.contains("esp32c6-devkitc-1") && error.contains("riverdi-rvt50hqsnwc00-b"));
        // The retired NUCLEO-F401RE and STM32F407G-DISC1 are unknown boards.
        for retired in ["nucleo-f401re", "stm32f407g-disc1"] {
            let error = Board::parse(retired).unwrap_err().to_string();
            assert!(error.contains("unknown board"), "{error}");
        }
        // The board name is the profile's name, so a report names what the command was given.
        for name in ["esp32c6-devkitc-1", "riverdi-rvt50hqsnwc00-b"] {
            assert_eq!(Board::parse(name).unwrap().profile().unwrap().name, name);
        }
    }

    #[test]
    fn the_riverdi_profile_carries_the_stm32u5a9_bench_values() {
        // Design section 6.1 (xtask): the application of 2016 KiB, the firmware image of 2040 KiB,
        // and the 78 KiB main stack with its 2 KiB headroom. Catches a profile that takes the C6's
        // values.
        let riverdi = Board::RiverdiRvt50hqsnwc00B.profile().unwrap();
        assert_eq!(riverdi.name, "riverdi-rvt50hqsnwc00-b");
        assert_eq!(riverdi.image_capacity_bytes, 2040 * 1024);
        assert_eq!(riverdi.application_bytes, 2016 * 1024);
        assert_eq!(riverdi.stack_window_bytes, 78 * 1024);
        assert_eq!(riverdi.stack_headroom_bytes, 2 * 1024);
    }

    #[test]
    fn each_bench_board_names_its_supported_chip() {
        assert_eq!(Board::Esp32c6Devkitc1.chip(), "esp32c6");
        assert_eq!(Board::RiverdiRvt50hqsnwc00B.chip(), "stm32u5a9");
        for board in BOARDS {
            assert_eq!(chip_of(board).unwrap().identity.name, board.chip());
        }
        assert!(chip("stm32f401").is_err());
        assert!(chip("stm32f407").is_err());
    }

    #[test]
    fn bench_profiles_take_their_chip_values_from_the_chip_files() {
        for board in BOARDS {
            let chip = chip_of(board).unwrap();
            let profile = board.profile().unwrap();
            assert_eq!(
                profile.image_capacity_bytes,
                chip.layout.firmware.bytes(),
                "{board:?}"
            );
            assert_eq!(
                profile.application_bytes,
                chip.layout.application.bytes(),
                "{board:?}"
            );
        }
    }

    #[test]
    fn c6_profile_carries_the_reference_composition_values() {
        let c6 = Board::Esp32c6Devkitc1.profile().unwrap();
        assert_eq!(c6.name, "esp32c6-devkitc-1");
        // Specification 34 D.2: a 78 KiB window with its 2 KiB headroom.
        assert_eq!(c6.stack_window_bytes, 78 * 1024);
        assert_eq!(c6.stack_headroom_bytes, 2 * 1024);
        assert_eq!(c6.image_capacity_bytes, 0x1F0000);
        assert_eq!(c6.application_bytes, 6112 * 1024);
    }

    #[test]
    fn c6_partition_table_matches_the_profile_and_uses_custom_types() {
        // Spec 34 D.2 flash layout, as the chip file records it. Custom types keep espflash's
        // table conversion and the bootloader away from truST's data partitions.
        let csv = include_str!("../../../firmware/trust-esp32c6/partitions.csv");
        let rows: Vec<Vec<&str>> = csv
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
            .map(|line| line.split(',').map(str::trim).collect())
            .collect();
        let number = |text: &str| u64::from_str_radix(text.trim_start_matches("0x"), 16).unwrap();
        let layout = chip("esp32c6").unwrap().layout;
        let span = |region: trust_chips::Region| (u64::from(region.start), region.bytes());
        // The boot counter's two sectors (Appendix D.2) are data partitions of their own, so that no
        // tool writes there: the chip file's `boot_counter_a` and `boot_counter_b`, in address order.
        let expected = [
            (
                "bootcnt_a",
                "0x42",
                "0x00",
                span(layout.boot_counter_a.unwrap()),
            ),
            (
                "bootcnt_b",
                "0x42",
                "0x01",
                span(layout.boot_counter_b.unwrap()),
            ),
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
        let size_of = |name: &str| {
            let row = rows.iter().find(|row| row[0] == name).unwrap();
            number(row[4])
        };
        assert_eq!(size_of("factory"), c6.image_capacity_bytes);
        assert_eq!(size_of("stbc"), c6.application_bytes);
    }
}
