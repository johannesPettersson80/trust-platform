//! The supported chips' files and the catalog (specification 34, TARGET-03 and Appendix H).

use std::path::Path;

use trust_chips::{Catalog, ChipFile, Component, Family, Fpu, PinReason, Problem};

const F407: &str = include_str!("../../../chips/stm32f407.toml");

fn catalog() -> Catalog {
    Catalog::embedded().expect("every file of chips/ is a valid chip file")
}

fn chip(name: &str) -> ChipFile {
    catalog().chip(name).cloned().expect("a supported chip")
}

/// Each layout region as its key, its start and its size in KiB.
fn layout(chip: &ChipFile) -> Vec<(&'static str, u32, u32)> {
    chip.layout
        .regions()
        .into_iter()
        .map(|(key, region)| (key, region.start, region.kib))
        .collect()
}

#[test]
fn the_catalog_holds_every_file_of_the_chips_directory() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../chips");
    let mut files: Vec<String> = std::fs::read_dir(directory)
        .expect("the chips directory")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .map(|path| {
            path.file_stem()
                .expect("a file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    files.sort();
    let catalog = catalog();
    let mut names = catalog.names();
    assert_eq!(names, ["stm32f407", "esp32c6"]);
    names.sort_unstable();
    assert_eq!(names, files);
    assert_eq!(catalog.chips().len(), 2);
    assert!(catalog.chip("stm32f401").is_none());
    assert!(catalog.chip("stm32f407g-disc1").is_none());
}

#[test]
fn the_stm32f407_file_describes_the_chip_of_appendix_d4() {
    let chip = chip("stm32f407");
    assert_eq!(chip.identity.model, "STM32F407");
    assert_eq!(chip.identity.family, Family::Stm32f4);
    assert_eq!(chip.identity.device_id, 0x413);
    assert_eq!(chip.processor.rust_target, "thumbv7em-none-eabihf");
    assert_eq!(chip.processor.fpu, Fpu::Single);
    assert_eq!(chip.clock.max_hz, 168_000_000);
    assert_eq!(chip.clock.firmware_hz, 168_000_000);
    let flash = chip
        .memory
        .flash
        .as_ref()
        .expect("flash in the address space");
    assert_eq!((flash.start, flash.kib), (0x0800_0000, 1024));
    assert!(chip.memory.spi_flash.is_none());
    // 192 KiB of RAM, of which DMA reaches the 128 KiB outside the CCM.
    let total: u32 = chip.memory.ram.iter().map(|region| region.kib).sum();
    let reached: u32 = chip
        .memory
        .ram
        .iter()
        .filter(|region| region.dma)
        .map(|region| region.kib)
        .sum();
    assert_eq!((total, reached), (192, 128));
    assert_eq!(
        layout(&chip),
        [
            ("vectors", 0x0800_0000, 16),
            ("firmware", 0x0801_0000, 704),
            ("application", 0x080C_0000, 256),
            ("checkpoint_a", 0x0800_8000, 16),
            ("checkpoint_b", 0x0800_C000, 16),
        ]
    );
    assert_eq!(chip.interfaces.usb, ["OTG_FS", "OTG_HS"]);
    assert_eq!(chip.interfaces.ethernet, ["ETH"]);
    assert_eq!(chip.interfaces.wifi, Vec::<String>::new());
    assert_eq!(chip.interfaces.bluetooth, Vec::<String>::new());
    assert_eq!(chip.gpio.reserved.len(), 6);
    assert_eq!(chip.firmware.crate_dir, "firmware/trust-stm32f407");
    assert_eq!(chip.firmware.bin, "trust-stm32f407");
    assert_eq!(chip.firmware.components, [Component::Engine]);
}

#[test]
fn the_esp32c6_file_describes_the_chip_of_appendix_d2() {
    let chip = chip("esp32c6");
    assert_eq!(chip.identity.model, "ESP32-C6");
    assert_eq!(chip.identity.family, Family::Esp32);
    assert_eq!(chip.identity.device_id, 13);
    assert_eq!(chip.processor.isa, "RV32IMAC");
    assert_eq!(chip.processor.rust_target, "riscv32imac-unknown-none-elf");
    assert_eq!(chip.processor.fpu, Fpu::None);
    assert_eq!(chip.clock.firmware_hz, 160_000_000);
    // The flash is the module's, so the file gives its erase unit and no size.
    assert!(chip.memory.flash.is_none());
    assert_eq!(chip.memory.spi_flash.map(|flash| flash.erase_kib), Some(4));
    assert_eq!(
        layout(&chip),
        [
            ("bootloader", 0x0, 32),
            ("partition_table", 0x8000, 4),
            ("firmware", 0x1_0000, 1984),
            ("application", 0x20_0000, 6112),
            ("checkpoint_a", 0x7F_8000, 16),
            ("checkpoint_b", 0x7F_C000, 16),
        ]
    );
    assert_eq!(chip.interfaces.ethernet, Vec::<String>::new());
    assert_eq!(chip.interfaces.wifi, ["WIFI"]);
    assert_eq!(chip.interfaces.bluetooth, ["BLE"]);
    let usb_pins: Vec<&str> = chip
        .gpio
        .reserved
        .iter()
        .filter(|entry| entry.reason == PinReason::Usb)
        .map(|entry| entry.pin.as_str())
        .collect();
    assert_eq!(usb_pins, ["GPIO12", "GPIO13"]);
    assert_eq!(chip.firmware.crate_dir, "firmware/trust-esp32c6-reference");
    assert_eq!(chip.firmware.components, [Component::Engine]);
}

#[test]
fn both_files_record_what_was_measured_on_the_owners_chips() {
    for name in ["stm32f407", "esp32c6"] {
        let sources = chip(name).sources;
        assert!(
            sources
                .measured
                .iter()
                .any(|path| path == "identity.device_id"),
            "{name}"
        );
        assert!(sources.measured_on.is_some(), "{name}");
        assert!(!sources.documents.is_empty(), "{name}");
        // The data-sheet USB IDs of the boot loaders are not yet in the repository's records.
        assert!(
            sources
                .unchecked
                .iter()
                .any(|path| path == "identity.usb_bootloader"),
            "{name}"
        );
    }
}

#[test]
fn a_component_names_the_chips_whose_firmware_holds_it() {
    let catalog = catalog();
    // Today's firmware runs the M2E fixture and holds the engine alone.
    assert_eq!(
        catalog.chips_with(Component::Engine),
        ["stm32f407", "esp32c6"]
    );
    for component in Component::ALL.into_iter().skip(1) {
        assert!(
            catalog.chips_with(component).is_empty(),
            "{}",
            component.name()
        );
    }
}

#[test]
fn the_component_words_are_the_closed_list_of_appendix_h3() {
    let words: Vec<&str> = Component::ALL.into_iter().map(Component::name).collect();
    assert_eq!(
        words,
        [
            "engine",
            "control",
            "live-values",
            "debug",
            "deploy",
            "access",
            "discovery",
            "communication",
            "web-hmi",
            "modbus-rtu",
            "ethercat",
            "opcua",
            "ads",
            "cloud",
            "observability",
            "openot",
            "simulation",
        ]
    );
    // Each word reads back as its component in a chip file.
    for component in Component::ALL.into_iter().skip(1) {
        let text = F407.replace(
            "components = [\"engine\"]",
            &format!("components = [\"engine\", \"{}\"]", component.name()),
        );
        let chip = ChipFile::parse("stm32f407", &text).expect("a valid file");
        assert_eq!(chip.firmware.components, [Component::Engine, component]);
    }
}

#[test]
fn a_chip_file_gives_the_event_store_default_and_maximum() {
    // Specification 34, Appendix H.2 and H.5 rule 11: `[events]` gives the newest events the
    // store keeps when the project sets none and the most a project may set, with 1 <=
    // default_capacity <= max_capacity <= 1,000,000. The C6 keeps 200 and admits at most 200
    // until a measurement of the heap a program leaves on the bench allows more. Catches a
    // missing or swapped pair, and a range the rule refuses.
    let file = include_str!("../../../chips/esp32c6.toml");
    let chip = chip("esp32c6");
    assert_eq!(chip.events.default_capacity, 200);
    assert_eq!(chip.events.max_capacity, 200);

    // A default above the maximum.
    let above = file.replace(
        "default_capacity = 200\nmax_capacity = 200",
        "default_capacity = 201\nmax_capacity = 200",
    );
    assert_eq!(
        ChipFile::parse("esp32c6", &above).unwrap_err().problem,
        Problem::EventStoreRange {
            default_capacity: 201,
            max_capacity: 200,
        }
    );
    // A zero.
    let zero = file.replace(
        "default_capacity = 200\nmax_capacity = 200",
        "default_capacity = 0\nmax_capacity = 200",
    );
    assert_eq!(
        ChipFile::parse("esp32c6", &zero).unwrap_err().problem,
        Problem::EventStoreRange {
            default_capacity: 0,
            max_capacity: 200,
        }
    );
    // A maximum above the limit.
    let above_limit = file.replace(
        "default_capacity = 200\nmax_capacity = 200",
        "default_capacity = 200\nmax_capacity = 1_000_001",
    );
    assert_eq!(
        ChipFile::parse("esp32c6", &above_limit)
            .unwrap_err()
            .problem,
        Problem::EventStoreRange {
            default_capacity: 200,
            max_capacity: 1_000_001,
        }
    );
    // A missing key, as H.1 refuses it for every key.
    let missing = file.replace("default_capacity = 200\n", "");
    assert!(matches!(
        ChipFile::parse("esp32c6", &missing).unwrap_err().problem,
        Problem::Toml(_)
    ));
}

#[test]
fn two_files_of_one_name_are_refused() {
    let error = Catalog::from_files(&[("stm32f407", F407), ("stm32f407", F407)]).unwrap_err();
    assert_eq!(
        error.problem,
        Problem::Duplicate {
            list: "chips",
            entry: "stm32f407".to_owned()
        }
    );
    assert!(Catalog::from_files(&[("stm32f407", F407)]).is_ok());
}

#[test]
fn a_refusal_names_the_file() {
    let error = ChipFile::parse("esp32c6", F407).unwrap_err();
    assert_eq!(error.chip, "esp32c6");
    assert_eq!(
        error.to_string(),
        "chips/esp32c6.toml: identity.name is \"stm32f407\", not the file's name"
    );
}
