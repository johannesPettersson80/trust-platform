//! The supported chips' files and the catalog (specification 34, TARGET-03 and Appendix H).

use std::path::Path;

use trust_chips::{Catalog, ChipFile, Component, Family, Fpu, PinReason, Problem};

const F407: &str = include_str!("../../../chips/stm32f407.toml");

fn catalog() -> Catalog {
    Catalog::embedded().expect("the catalog's chip files are valid")
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
fn the_catalog_holds_every_chip_file_whose_firmware_crate_is_in_the_repository() {
    // Specification 34, Appendix H.6: the catalog lists the chips whose firmware crate is in the
    // repository, which `build-firmware` builds. A chip file whose crate is not there is read and
    // checked by its adapter's tests and is not in the catalog.
    // Catches a chip whose firmware exists but is missing from the catalog, a catalog entry whose
    // firmware is absent, and a chip file that no longer parses.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files: Vec<String> = std::fs::read_dir(root.join("chips"))
        .expect("the chips directory")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .filter_map(|path| {
            let name = path
                .file_stem()
                .expect("a file name")
                .to_string_lossy()
                .into_owned();
            let text = std::fs::read_to_string(&path).expect("a chip file");
            let chip =
                ChipFile::parse(&name, &text).expect("every file of chips/ is a valid chip file");
            root.join(&chip.firmware.crate_dir).is_dir().then_some(name)
        })
        .collect();
    files.sort();
    let catalog = catalog();
    let mut names = catalog.names();
    assert_eq!(names, ["stm32f407", "esp32c6", "stm32u5a9"]);
    names.sort_unstable();
    assert_eq!(names, files);
    assert_eq!(catalog.chips().len(), 3);
    assert!(catalog.chip("stm32f401").is_none());
    assert!(catalog.chip("stm32f407g-disc1").is_none());
    assert!(catalog.chip("riverdi-rvt50hqsnwc00-b").is_none());
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
    assert_eq!(chip.firmware.crate_dir, "firmware/trust-esp32c6");
    assert_eq!(chip.firmware.bin, "trust-esp32c6");
    // Appendix H.3: the components the C6 bench measured; the F407 holds the engine alone.
    assert_eq!(
        chip.firmware.components,
        [Component::Engine, Component::Control, Component::LiveValues]
    );
}

#[test]
fn the_stm32u5a9_file_describes_the_chip_of_appendix_d5() {
    // Specification 34, Appendix D.5 and H.2: the U5's identity, its clock, its flash in 512 runs of
    // 8 KiB, the layout of its two banks and the pins it keeps with their reasons, as the design
    // gives them. Catches a file that drifts from the design, such as a checkpoint slot moved.
    let chip = ChipFile::parse("stm32u5a9", include_str!("../../../chips/stm32u5a9.toml"))
        .expect("the STM32U5A9 chip file is valid");
    assert_eq!(chip.identity.model, "STM32U5A9");
    assert_eq!(chip.identity.family, Family::Stm32u5);
    assert_eq!(chip.identity.device_id, 0x481);
    assert_eq!(chip.processor.rust_target, "thumbv8m.main-none-eabihf");
    assert_eq!(chip.processor.fpu, Fpu::Single);
    assert_eq!(chip.clock.max_hz, 160_000_000);
    assert_eq!(chip.clock.firmware_hz, 160_000_000);
    let flash = chip
        .memory
        .flash
        .as_ref()
        .expect("flash in the address space");
    assert_eq!((flash.start, flash.kib), (0x0800_0000, 4096));
    let sectors: Vec<(u32, u32)> = flash
        .sectors
        .iter()
        .map(|run| (run.count, run.kib))
        .collect();
    assert_eq!(sectors, [(512, 8)]);
    assert!(chip.memory.spi_flash.is_none());
    assert_eq!(
        layout(&chip),
        [
            ("vectors", 0x0800_0000, 8),
            ("firmware", 0x0800_2000, 2040),
            ("application", 0x0820_0000, 2016),
            ("checkpoint_a", 0x083F_8000, 16),
            ("checkpoint_b", 0x083F_C000, 16),
        ]
    );
    assert_eq!(chip.interfaces.usb, ["OTG_HS"]);
    assert_eq!(chip.interfaces.ethernet, Vec::<String>::new());
    let usb_pins: Vec<&str> = chip
        .gpio
        .reserved
        .iter()
        .filter(|entry| entry.reason == PinReason::Usb)
        .map(|entry| entry.pin.as_str())
        .collect();
    assert_eq!(usb_pins, ["PA11", "PA12"]);
    assert_eq!(chip.gpio.reserved.len(), 5);
    assert_eq!(chip.firmware.crate_dir, "firmware/trust-stm32u5a9");
    assert_eq!(chip.firmware.bin, "trust-stm32u5a9");
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
    // Appendix H.3: every chip in the catalog holds the engine; the C6's bench measured the control requests
    // and Live Values, which the F407 and U5 firmware do not serve yet. Catches a chip offered for a
    // component its firmware lacks, or one left out for a component it holds.
    assert_eq!(
        catalog.chips_with(Component::Engine),
        ["stm32f407", "esp32c6", "stm32u5a9"]
    );
    assert_eq!(catalog.chips_with(Component::Control), ["esp32c6"]);
    assert_eq!(catalog.chips_with(Component::LiveValues), ["esp32c6"]);
    for component in Component::ALL.into_iter().skip(3) {
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
