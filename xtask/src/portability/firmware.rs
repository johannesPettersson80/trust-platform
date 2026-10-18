//! The firmware that `build-firmware` links: one per supported chip, read from the chip's file
//! (specification 34, TARGET-03 and Appendix H). There is no firmware per board: a bench board's
//! name is refused with its chip's.
use super::board::Board;
use anyhow::{bail, Result};
use trust_chips::{Catalog, ChipFile, Family};

/// What one `build-firmware` run builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Firmware {
    /// The chip the command was given, written into the build report.
    pub name: String,
    /// The chip's family, which selects the chip's link-time guards.
    pub family: Family,
    /// Firmware crate directory, relative to the repository root.
    pub firmware_dir: String,
    pub target: String,
    pub bin: String,
    /// Flag pairs, such as `["-C", "link-arg=-Tlinkall.x"]`, that the target rustflags must contain.
    pub required_rustflags: &'static [[&'static str; 2]],
}

impl Firmware {
    /// The firmware of a supported chip.
    pub(super) fn parse(name: &str) -> Result<Self> {
        let catalog = Catalog::embedded()?;
        if let Some(chip) = catalog.chip(name) {
            return Ok(Self::for_chip(chip));
        }
        if let Ok(board) = Board::parse(name) {
            let chip = board.chip();
            if catalog.chip(chip).is_none() {
                bail!("{name} is a bench board whose chip {chip} has no firmware yet (S1b-9u)");
            }
            bail!("{name} is a bench board, and firmware is built for its chip: build-firmware {chip}");
        }
        bail!(
            "unknown chip {name:?}; expected {}",
            catalog.names().join(", ")
        )
    }

    /// The chip's firmware: the crate, binary and Rust target its file names.
    fn for_chip(chip: &ChipFile) -> Self {
        Self {
            name: chip.identity.name.clone(),
            family: chip.identity.family,
            firmware_dir: chip.firmware.crate_dir.clone(),
            target: chip.processor.rust_target.clone(),
            bin: chip.firmware.bin.clone(),
            required_rustflags: required_rustflags(chip.identity.family),
        }
    }
}

/// The linker scripts a family's firmware must link with. Without `profile.x` the STM32 firmware's
/// linker assertions and `__firmware_load_end` are missing; `linkall.x` is esp-hal's memory map
/// and section placement.
fn required_rustflags(family: Family) -> &'static [[&'static str; 2]] {
    match family {
        Family::Stm32f4 | Family::Stm32u5 => &[["-C", "link-arg=-Tprofile.x"]],
        Family::Esp32 => &[["-C", "link-arg=-Tlinkall.x"]],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f407_firmware_is_built_from_its_chip_file() {
        let firmware = Firmware::parse("stm32f407").unwrap();
        assert_eq!(firmware.name, "stm32f407");
        assert_eq!(firmware.firmware_dir, "firmware/trust-stm32f407");
        assert_eq!(firmware.target, "thumbv7em-none-eabihf");
        assert_eq!(firmware.bin, "trust-stm32f407");
        assert_eq!(
            firmware.required_rustflags,
            [["-C", "link-arg=-Tprofile.x"]].as_slice()
        );
    }

    #[test]
    fn the_stm32u5_links_with_the_profile_script_as_the_stm32f4_does() {
        // Specification 34, Appendix D.5: the STM32U5A9 firmware links with profile.x, which holds the
        // linker assertions and __firmware_load_end, as the STM32F4 firmware does. Catches a family
        // mapped to no script or to the ESP32's linkall.x.
        assert_eq!(
            required_rustflags(Family::Stm32u5),
            [["-C", "link-arg=-Tprofile.x"]].as_slice()
        );
        assert_eq!(
            required_rustflags(Family::Stm32u5),
            required_rustflags(Family::Stm32f4)
        );
    }

    #[test]
    fn c6_firmware_is_built_from_its_chip_file() {
        let firmware = Firmware::parse("esp32c6").unwrap();
        assert_eq!(firmware.name, "esp32c6");
        assert_eq!(firmware.firmware_dir, "firmware/trust-esp32c6");
        assert_eq!(firmware.target, "riscv32imac-unknown-none-elf");
        assert_eq!(firmware.bin, "trust-esp32c6");
        assert_eq!(
            firmware.required_rustflags,
            [["-C", "link-arg=-Tlinkall.x"]].as_slice()
        );
    }

    #[test]
    fn every_supported_chip_has_a_firmware_crate_with_its_rust_target() {
        let catalog = Catalog::embedded().unwrap();
        for chip in catalog.chips() {
            let firmware = Firmware::parse(&chip.identity.name).unwrap();
            let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(&firmware.firmware_dir);
            let manifest = std::fs::read_to_string(directory.join("Cargo.toml")).unwrap();
            assert!(
                manifest.contains(&format!("name = \"{}\"", firmware.bin)),
                "{}",
                firmware.name
            );
            let config = std::fs::read_to_string(directory.join(".cargo/config.toml")).unwrap();
            assert!(
                config.contains(&format!("target = \"{}\"", firmware.target)),
                "{}",
                firmware.name
            );
        }
    }

    #[test]
    fn the_usage_names_every_supported_chip() {
        for name in Catalog::embedded().unwrap().names() {
            assert!(crate::usage().contains(name), "{name}");
        }
    }

    #[test]
    fn a_bench_board_is_refused_with_its_chip_and_an_unknown_name_lists_the_chips() {
        let error = Firmware::parse("stm32f407g-disc1").unwrap_err().to_string();
        assert!(error.contains("build-firmware stm32f407"), "{error}");
        let error = Firmware::parse("esp32c6-devkitc-1")
            .unwrap_err()
            .to_string();
        assert!(error.contains("build-firmware esp32c6"), "{error}");
        let error = Firmware::parse("stm32f401").unwrap_err().to_string();
        assert!(error.contains("stm32f407, esp32c6"), "{error}");
        // The Riverdi board's chip joins the catalog with its firmware in S1b-9u.
        let error = Firmware::parse("riverdi-rvt50hqsnwc00-b")
            .unwrap_err()
            .to_string();
        assert!(error.contains("has no firmware yet (S1b-9u)"), "{error}");
        // The NUCLEO-F401RE composition was retired on 17 October 2026; its name is unknown.
        let error = Firmware::parse("nucleo-f401re").unwrap_err().to_string();
        assert!(
            error.contains("unknown chip") && error.contains("stm32f407, esp32c6"),
            "{error}"
        );
    }
}
