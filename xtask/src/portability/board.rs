//! The reference boards that `xtask portability` builds, inspects and verifies. Each board's
//! values are written once here, and the commands read them instead of carrying constants.
use anyhow::{bail, Result};

/// A reference board, named on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Board {
    NucleoF401re,
    Esp32c6Devkitc1,
}

impl Board {
    pub(super) fn parse(name: &str) -> Result<Self> {
        match name {
            "nucleo-f401re" => Ok(Self::NucleoF401re),
            "esp32c6-devkitc-1" => Ok(Self::Esp32c6Devkitc1),
            other => bail!("unknown board {other:?}; expected nucleo-f401re or esp32c6-devkitc-1"),
        }
    }

    pub(super) fn profile(self) -> BoardProfile {
        match self {
            Self::NucleoF401re => BoardProfile::nucleo_f401re(),
            Self::Esp32c6Devkitc1 => BoardProfile::esp32c6_devkitc_1(),
        }
    }
}

/// The values one board's commands need. The F401 values are the constants the commands used
/// before this profile existed. The C6 values are the reference composition in spec 34, D.2.
#[derive(Clone, Copy, Debug)]
pub(super) struct BoardProfile {
    pub board: Board,
    /// The command-line name, also written into every report.
    pub name: &'static str,
    /// Firmware crate directory, relative to the repository root.
    pub firmware_dir: &'static str,
    pub target: &'static str,
    pub bin: &'static str,
    /// Flag pairs, such as `["-C", "link-arg=-Tlinkall.x"]`, that the target rustflags must contain.
    pub required_rustflags: &'static [[&'static str; 2]],
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
    /// Space the linked image must fit: the upper executable region on F401, the factory
    /// application partition on C6.
    pub image_capacity_bytes: u64,
    /// The separately installable application partition.
    pub application_bytes: u64,
}

impl BoardProfile {
    pub(super) fn nucleo_f401re() -> Self {
        Self {
            board: Board::NucleoF401re,
            name: "nucleo-f401re",
            firmware_dir: "firmware/trust-nucleo-f401re",
            target: "thumbv7em-none-eabihf",
            bin: "trust-nucleo-f401re",
            required_rustflags: &[],
            cpu_hz: 84_000_000,
            flash_kib: 512,
            scan_cycle_bound: 840_000,
            heap_bytes: 72 * 1024,
            stack_window_bytes: 16 * 1024,
            stack_headroom_bytes: 2 * 1024,
            boot_banner_lines: 0,
            image_capacity_bytes: 448 * 1024,
            application_bytes: 16 * 1024,
        }
    }

    pub(super) fn esp32c6_devkitc_1() -> Self {
        Self {
            board: Board::Esp32c6Devkitc1,
            name: "esp32c6-devkitc-1",
            firmware_dir: "firmware/trust-esp32c6-reference",
            target: "riscv32imac-unknown-none-elf",
            bin: "trust-esp32c6-reference",
            required_rustflags: &[["-C", "link-arg=-Tlinkall.x"]],
            cpu_hz: 160_000_000,
            flash_kib: 8192,
            scan_cycle_bound: 1_600_000,
            heap_bytes: 72 * 1024,
            stack_window_bytes: 16 * 1024,
            stack_headroom_bytes: 2 * 1024,
            boot_banner_lines: 64,
            image_capacity_bytes: 1984 * 1024,
            application_bytes: 16 * 1024,
        }
    }

    /// The largest stack peak the measured window accepts: the window less its headroom.
    pub(super) fn stack_peak_bound(&self) -> u64 {
        self.stack_window_bytes - self.stack_headroom_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_names_select_their_profiles_and_unknown_names_list_both() {
        assert_eq!(Board::parse("nucleo-f401re").unwrap(), Board::NucleoF401re);
        assert_eq!(
            Board::parse("esp32c6-devkitc-1").unwrap(),
            Board::Esp32c6Devkitc1
        );
        let error = Board::parse("esp32c6").unwrap_err().to_string();
        assert!(error.contains("nucleo-f401re") && error.contains("esp32c6-devkitc-1"));
    }

    #[test]
    fn c6_profile_carries_the_reference_composition_values() {
        let c6 = Board::Esp32c6Devkitc1.profile();
        assert_eq!(c6.name, "esp32c6-devkitc-1");
        assert_eq!(c6.firmware_dir, "firmware/trust-esp32c6-reference");
        assert_eq!(c6.target, "riscv32imac-unknown-none-elf");
        assert_eq!(c6.bin, "trust-esp32c6-reference");
        assert_eq!(c6.cpu_hz, 160_000_000);
        assert_eq!(c6.flash_kib, 8192);
        assert_eq!(c6.scan_cycle_bound, 1_600_000);
        assert_eq!(c6.heap_bytes, 72 * 1024);
        assert_eq!(c6.stack_peak_bound(), 14_336);
        assert_eq!(c6.boot_banner_lines, 64);
        assert_eq!(c6.image_capacity_bytes, 0x1F0000);
        assert_eq!(c6.application_bytes, 16 * 1024);
        assert_eq!(
            c6.required_rustflags,
            [["-C", "link-arg=-Tlinkall.x"]].as_slice()
        );
    }

    #[test]
    fn c6_partition_table_matches_the_profile_and_uses_custom_types() {
        // Spec 34 D.2 flash layout. Custom types keep espflash's table conversion and the
        // bootloader away from truST's data partitions.
        let csv = include_str!("../../../firmware/trust-esp32c6-reference/partitions.csv");
        let rows: Vec<Vec<&str>> = csv
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
            .map(|line| line.split(',').map(str::trim).collect())
            .collect();
        let number = |text: &str| u64::from_str_radix(text.trim_start_matches("0x"), 16).unwrap();
        let c6 = Board::Esp32c6Devkitc1.profile();
        let expected = [
            (
                "factory",
                "app",
                "factory",
                0x1_0000,
                c6.image_capacity_bytes,
            ),
            ("stbc", "0x40", "0x00", 0x20_0000, c6.application_bytes),
            ("ckpt_a", "0x41", "0x00", 0x20_4000, 0x4000),
            ("ckpt_b", "0x41", "0x01", 0x20_8000, 0x4000),
        ];
        assert_eq!(rows.len(), expected.len());
        for (row, (name, kind, subtype, offset, size)) in rows.iter().zip(expected) {
            assert_eq!(
                (row[0], row[1], row[2], number(row[3]), number(row[4])),
                (name, kind, subtype, offset, size)
            );
        }
        for pair in rows.windows(2) {
            assert!(number(pair[0][3]) + number(pair[0][4]) <= number(pair[1][3]));
        }
    }

    #[test]
    fn f401_profile_keeps_the_values_the_commands_used() {
        let f401 = Board::NucleoF401re.profile();
        assert_eq!(f401.firmware_dir, "firmware/trust-nucleo-f401re");
        assert_eq!(f401.target, "thumbv7em-none-eabihf");
        assert_eq!(f401.bin, "trust-nucleo-f401re");
        assert_eq!(f401.cpu_hz, 84_000_000);
        assert_eq!(f401.flash_kib, 512);
        assert_eq!(f401.scan_cycle_bound, 840_000);
        assert_eq!(f401.stack_peak_bound(), 14_336);
        assert_eq!(f401.boot_banner_lines, 0);
        assert!(f401.required_rustflags.is_empty());
    }
}
