//! The firmware that `build-firmware` links: one per supported chip, read from the chip's file
//! (specification 34, TARGET-03 and Appendix H), and the NUCLEO-F401RE composition of scope B,
//! which has no chip file because its STM32F401 is no longer a supported chip. There is no
//! firmware per board: a bench board's name is refused with its chip's.
use super::board::Board;
use anyhow::{bail, Result};
use trust_chips::{Catalog, ChipFile, Family};

/// The scope B composition, built under its bench name.
const NUCLEO_F401RE: &str = "nucleo-f401re";

/// What one `build-firmware` run builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Firmware {
    /// What the command was given, written into the build report: a chip, or `nucleo-f401re`.
    pub name: String,
    /// Firmware crate directory, relative to the repository root.
    pub firmware_dir: String,
    pub target: String,
    pub bin: String,
    /// Flag pairs, such as `["-C", "link-arg=-Tlinkall.x"]`, that the target rustflags must contain.
    pub required_rustflags: &'static [[&'static str; 2]],
}

impl Firmware {
    /// The firmware of a supported chip, or the F401 composition.
    pub(super) fn parse(name: &str) -> Result<Self> {
        if name == NUCLEO_F401RE {
            return Ok(Self::nucleo_f401re());
        }
        let catalog = Catalog::embedded()?;
        if let Some(chip) = catalog.chip(name) {
            return Ok(Self::for_chip(chip));
        }
        if let Some(chip) = Board::parse(name).ok().and_then(Board::chip) {
            bail!("{name} is a bench board, and firmware is built for its chip: build-firmware {chip}");
        }
        bail!(
            "unknown chip {name:?}; expected {}, or {NUCLEO_F401RE} for the scope B composition",
            catalog.names().join(", ")
        )
    }

    /// The chip's firmware: the crate, binary and Rust target its file names.
    fn for_chip(chip: &ChipFile) -> Self {
        Self {
            name: chip.identity.name.clone(),
            firmware_dir: chip.firmware.crate_dir.clone(),
            target: chip.processor.rust_target.clone(),
            bin: chip.firmware.bin.clone(),
            required_rustflags: required_rustflags(chip.identity.family),
        }
    }

    /// The F401 composition, with the values the command used before chip files existed.
    pub(super) fn nucleo_f401re() -> Self {
        Self {
            name: NUCLEO_F401RE.to_owned(),
            firmware_dir: "firmware/trust-nucleo-f401re".to_owned(),
            target: "thumbv7em-none-eabihf".to_owned(),
            bin: "trust-nucleo-f401re".to_owned(),
            required_rustflags: &[],
        }
    }
}

/// The linker scripts a family's firmware must link with. Without `profile.x` the STM32F4
/// firmware's linker assertions and `__firmware_load_end` are missing; `linkall.x` is esp-hal's
/// memory map and section placement.
fn required_rustflags(family: Family) -> &'static [[&'static str; 2]] {
    match family {
        Family::Stm32f4 => &[["-C", "link-arg=-Tprofile.x"]],
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
    fn c6_firmware_is_built_from_its_chip_file() {
        let firmware = Firmware::parse("esp32c6").unwrap();
        assert_eq!(firmware.name, "esp32c6");
        assert_eq!(firmware.firmware_dir, "firmware/trust-esp32c6-reference");
        assert_eq!(firmware.target, "riscv32imac-unknown-none-elf");
        assert_eq!(firmware.bin, "trust-esp32c6-reference");
        assert_eq!(
            firmware.required_rustflags,
            [["-C", "link-arg=-Tlinkall.x"]].as_slice()
        );
    }

    #[test]
    fn f401_firmware_keeps_the_values_the_command_used() {
        let firmware = Firmware::parse("nucleo-f401re").unwrap();
        assert_eq!(firmware, Firmware::nucleo_f401re());
        assert_eq!(firmware.firmware_dir, "firmware/trust-nucleo-f401re");
        assert_eq!(firmware.target, "thumbv7em-none-eabihf");
        assert_eq!(firmware.bin, "trust-nucleo-f401re");
        assert!(firmware.required_rustflags.is_empty());
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
        assert!(
            error.contains("stm32f407, esp32c6") && error.contains("nucleo-f401re"),
            "{error}"
        );
    }
}
