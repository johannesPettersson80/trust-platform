//! The rules of a chip file (specification 34, Appendix H.1 and H.5). Each test edits a real
//! chip file and checks that the edit is refused with the problem of the broken rule, or that an
//! edit within the rules is accepted.

use trust_chips::{ChipFile, Family, Problem};

const F407: &str = include_str!("../../../chips/stm32f407.toml");
const C6: &str = include_str!("../../../chips/esp32c6.toml");
const U5: &str = include_str!("../../../chips/stm32u5a9.toml");

/// A TOML text as a table.
fn table(text: &str) -> toml::Table {
    text.parse().expect("TOML")
}

/// The table at a dotted path; `key#n` is entry `n` of an array of tables.
fn at<'a>(file: &'a mut toml::Table, path: &str) -> &'a mut toml::Table {
    path.split('.').fold(file, |table, key| {
        let (name, entry) = match key.split_once('#') {
            Some((name, index)) => (name, Some(index.parse::<usize>().expect("an index"))),
            None => (key, None),
        };
        let value = table.get_mut(name).expect("a key of the file");
        match entry {
            Some(index) => value.as_array_mut().expect("an array")[index]
                .as_table_mut()
                .expect("a table"),
            None => value.as_table_mut().expect("a table"),
        }
    })
}

/// Sets `key` in the table at `path`.
fn set(file: &mut toml::Table, path: &str, key: &str, value: impl Into<toml::Value>) {
    at(file, path).insert(key.to_owned(), value.into());
}

/// Removes `key` from the table at `path`.
fn remove(file: &mut toml::Table, path: &str, key: &str) {
    at(file, path).remove(key).expect("the key is there");
}

/// Appends a path to `sources.unchecked`.
fn uncheck(file: &mut toml::Table, path: &str) {
    at(file, "sources")
        .get_mut("unchecked")
        .and_then(toml::Value::as_array_mut)
        .expect("sources.unchecked")
        .push(path.into());
}

/// A chip file after `edit`, read and checked.
fn edited(
    name: &str,
    text: &str,
    edit: impl FnOnce(&mut toml::Table),
) -> Result<ChipFile, Problem> {
    let mut file = table(text);
    edit(&mut file);
    let text = toml::to_string(&file).expect("a table writes as TOML");
    ChipFile::parse(name, &text).map_err(|error| error.problem)
}

fn f407(edit: impl FnOnce(&mut toml::Table)) -> Result<ChipFile, Problem> {
    edited("stm32f407", F407, edit)
}

fn c6(edit: impl FnOnce(&mut toml::Table)) -> Result<ChipFile, Problem> {
    edited("esp32c6", C6, edit)
}

fn u5(edit: impl FnOnce(&mut toml::Table)) -> Result<ChipFile, Problem> {
    edited("stm32u5a9", U5, edit)
}

/// The TOML message of a refusal.
fn toml_message(result: Result<ChipFile, Problem>) -> String {
    match result {
        Err(Problem::Toml(message)) => message,
        other => panic!("expected a TOML refusal, got {other:?}"),
    }
}

#[test]
fn both_real_files_are_accepted_and_survive_a_rewrite() {
    assert_eq!(f407(|_| {}).err(), None);
    assert_eq!(c6(|_| {}).err(), None);
    assert_eq!(f407(|_| {}).ok(), ChipFile::parse("stm32f407", F407).ok());
}

#[test]
fn an_unknown_key_is_refused_in_every_table() {
    let message = toml_message(f407(|file| {
        file.insert("colour".to_owned(), "blue".into());
    }));
    assert!(message.contains("unknown field `colour`"), "{message}");
    for path in [
        "identity",
        "identity.usb_bootloader",
        "processor",
        "clock",
        "memory",
        "memory.flash",
        "memory.flash.sectors#0",
        "memory.ram#2",
        "layout",
        "layout.vectors",
        "layout.checkpoint_b",
        "interfaces",
        "gpio",
        "gpio.ports#0",
        "gpio.reserved#5",
        "firmware",
        "sources",
    ] {
        let message = toml_message(f407(|file| set(file, path, "colour", "blue")));
        assert!(
            message.contains("unknown field `colour`"),
            "{path}: {message}"
        );
    }
    for path in [
        "memory.spi_flash",
        "layout.bootloader",
        "layout.partition_table",
    ] {
        let message = toml_message(c6(|file| set(file, path, "colour", "blue")));
        assert!(
            message.contains("unknown field `colour`"),
            "{path}: {message}"
        );
    }
}

#[test]
fn a_missing_key_a_wrong_type_or_a_word_outside_a_closed_list_is_refused() {
    let message = toml_message(f407(|file| remove(file, "identity", "device_id")));
    assert!(message.contains("missing field `device_id`"), "{message}");
    let message = toml_message(c6(|file| remove(file, "interfaces", "dac")));
    assert!(message.contains("missing field `dac`"), "{message}");
    toml_message(f407(|file| set(file, "clock", "max_hz", "fast")));
    toml_message(f407(|file| set(file, "clock", "max_hz", -1_i64)));
    toml_message(f407(|file| {
        set(file, "identity.usb_bootloader", "vid", 0x1_0000_i64);
    }));
    for (path, key, word) in [
        ("identity", "family", "stm32h7"),
        ("processor", "fpu", "half"),
        ("gpio.reserved#0", "reason", "led"),
    ] {
        let message = toml_message(f407(|file| set(file, path, key, word)));
        assert!(message.contains("unknown variant"), "{word}: {message}");
    }
    let message = toml_message(f407(|file| {
        set(file, "firmware", "components", vec!["engine", "printer"]);
    }));
    assert!(message.contains("unknown variant `printer`"), "{message}");
}

#[test]
fn the_name_is_the_files_name_in_small_letters_and_digits() {
    assert_eq!(
        ChipFile::parse("esp32c6", F407).map_err(|error| error.problem),
        Err(Problem::NameMismatch {
            found: "stm32f407".to_owned()
        })
    );
    for name in ["STM32F407", "stm32-f407", "stm32f407 "] {
        let result = edited(name, F407, |file| set(file, "identity", "name", name));
        assert_eq!(result, Err(Problem::BadName), "{name:?}");
    }
}

#[test]
fn texts_the_format_requires_are_not_empty() {
    for (path, key, field) in [
        ("identity", "model", "identity.model"),
        ("processor", "core", "processor.core"),
        ("processor", "isa", "processor.isa"),
        ("memory.ram#1", "name", "memory.ram.name"),
        ("firmware", "bin", "firmware.bin"),
        ("sources", "measured_on", "sources.measured_on"),
    ] {
        assert_eq!(
            f407(|file| set(file, path, key, " ")),
            Err(Problem::Empty { field }),
            "{field}"
        );
    }
    assert_eq!(
        f407(|file| set(file, "interfaces", "can", vec!["CAN1", ""])),
        Err(Problem::Empty {
            field: "interfaces.can"
        })
    );
}

#[test]
fn an_stm32f4_device_id_has_the_twelve_bits_of_dev_id() {
    for device_id in [0_i64, 0x1413] {
        assert_eq!(
            f407(|file| set(file, "identity", "device_id", device_id)),
            Err(Problem::DeviceId {
                device_id: u32::try_from(device_id).expect("a u32")
            })
        );
    }
    assert_eq!(
        f407(|file| set(file, "identity", "device_id", 0xFFF_i64)).err(),
        None
    );
    // An ESP32 chip ID has no such limit.
    assert_eq!(
        c6(|file| set(file, "identity", "device_id", 0x1_0000_i64)).err(),
        None
    );
}

#[test]
fn the_firmware_clock_and_the_crystals_lie_within_the_chips_limits() {
    for firmware_hz in [168_000_001_i64, 0] {
        assert_eq!(
            f407(|file| set(file, "clock", "firmware_hz", firmware_hz)),
            Err(Problem::FirmwareClock {
                firmware_hz: u32::try_from(firmware_hz).expect("a u32"),
                max_hz: 168_000_000
            })
        );
    }
    assert_eq!(
        f407(|file| set(file, "clock", "firmware_hz", 84_000_000_i64)).err(),
        None
    );
    for crystal_min_hz in [27_000_000_i64, 0] {
        assert_eq!(
            f407(|file| set(file, "clock", "crystal_min_hz", crystal_min_hz)),
            Err(Problem::CrystalRange {
                min_hz: u32::try_from(crystal_min_hz).expect("a u32"),
                max_hz: 26_000_000
            })
        );
    }
}

#[test]
fn the_rust_target_is_of_the_familys_architecture() {
    let riscv = "riscv32imac-unknown-none-elf";
    assert_eq!(
        f407(|file| set(file, "processor", "rust_target", riscv)),
        Err(Problem::TargetFamily {
            target: riscv.to_owned()
        })
    );
    let arm = "thumbv7em-none-eabihf";
    assert_eq!(
        c6(|file| set(file, "processor", "rust_target", arm)),
        Err(Problem::TargetFamily {
            target: arm.to_owned()
        })
    );
}

#[test]
fn the_rust_target_agrees_with_the_fpu_and_the_instruction_set() {
    let hard_float = || Problem::TargetFpu {
        target: "thumbv7em-none-eabihf".to_owned(),
    };
    assert_eq!(
        f407(|file| set(file, "processor", "fpu", "none")),
        Err(hard_float())
    );
    let soft_float = "thumbv7em-none-eabi";
    assert_eq!(
        f407(|file| set(file, "processor", "rust_target", soft_float)),
        Err(Problem::TargetFpu {
            target: soft_float.to_owned()
        })
    );
    let without_fpu = f407(|file| {
        set(file, "processor", "rust_target", soft_float);
        set(file, "processor", "fpu", "none");
    });
    assert_eq!(without_fpu.err(), None);
    let imac = "riscv32imac-unknown-none-elf";
    assert_eq!(
        c6(|file| set(file, "processor", "fpu", "single")),
        Err(Problem::TargetFpu {
            target: imac.to_owned()
        })
    );
    assert_eq!(
        c6(|file| set(file, "processor", "isa", "RV32IMC")),
        Err(Problem::TargetIsa {
            target: imac.to_owned()
        })
    );
    let with_fpu = c6(|file| {
        set(
            file,
            "processor",
            "rust_target",
            "riscv32imafc-unknown-none-elf",
        );
        set(file, "processor", "isa", "RV32IMAFC");
        set(file, "processor", "fpu", "single");
    });
    assert_eq!(with_fpu.err(), None);
}

#[test]
fn each_family_describes_its_flash_in_its_own_table() {
    let spi = || toml::Value::Table(table("erase_kib = 4"));
    let mapped = || {
        toml::Value::Table(table(
            "start = 0\nkib = 4096\nsectors = [{ count = 1024, kib = 4 }]",
        ))
    };
    let stm32 = Err(Problem::FlashKind {
        family: Family::Stm32f4,
    });
    assert_eq!(f407(|file| set(file, "memory", "spi_flash", spi())), stm32);
    let moved = f407(|file| {
        remove(file, "memory", "flash");
        set(file, "memory", "spi_flash", spi());
    });
    assert_eq!(moved, stm32);
    let esp32 = Err(Problem::FlashKind {
        family: Family::Esp32,
    });
    assert_eq!(c6(|file| remove(file, "memory", "spi_flash")), esp32);
    assert_eq!(c6(|file| set(file, "memory", "flash", mapped())), esp32);
}

#[test]
fn the_sectors_add_up_to_the_flash_below_4_gib() {
    assert_eq!(
        f407(|file| set(file, "memory.flash", "kib", 2048_i64)),
        Err(Problem::SectorSum {
            sum_kib: 1024,
            flash_kib: 2048
        })
    );
    assert_eq!(
        f407(|file| set(file, "memory.flash.sectors#2", "count", 6_i64)),
        Err(Problem::SectorSum {
            sum_kib: 896,
            flash_kib: 1024
        })
    );
    assert_eq!(
        f407(|file| set(file, "memory.flash", "start", 0xFFF8_0000_i64)),
        Err(Problem::AddressRange {
            what: "memory.flash".to_owned()
        })
    );
}

#[test]
fn no_memory_region_or_erase_unit_is_empty() {
    let zero = |what: &str| {
        Err(Problem::ZeroSize {
            what: what.to_owned(),
        })
    };
    let none: Vec<toml::Value> = Vec::new();
    assert_eq!(
        f407(|file| set(file, "memory.flash.sectors#0", "count", 0_i64)),
        zero("memory.flash.sectors")
    );
    assert_eq!(
        f407(|file| set(file, "memory.flash", "sectors", none.clone())),
        zero("memory.flash")
    );
    assert_eq!(
        f407(|file| set(file, "memory", "ram", none)),
        zero("memory.ram")
    );
    assert_eq!(
        f407(|file| set(file, "memory.ram#2", "kib", 0_i64)),
        zero("memory.ram CCM")
    );
    assert_eq!(
        f407(|file| set(file, "layout.application", "kib", 0_i64)),
        zero("layout.application")
    );
    assert_eq!(
        c6(|file| set(file, "memory.spi_flash", "erase_kib", 0_i64)),
        zero("memory.spi_flash.erase_kib")
    );
    assert_eq!(
        c6(|file| set(file, "gpio", "pins", 0_i64)),
        zero("gpio.pins")
    );
}

#[test]
fn ram_regions_are_distinct_below_4_gib_and_apart_from_the_flash() {
    assert_eq!(
        f407(|file| set(file, "memory.ram#2", "start", 0x2001_0000_i64)),
        Err(Problem::RamOverlap {
            first: "SRAM1".to_owned(),
            second: "CCM".to_owned()
        })
    );
    assert_eq!(
        f407(|file| set(file, "memory.ram#1", "name", "SRAM1")),
        Err(Problem::Duplicate {
            list: "memory.ram",
            entry: "SRAM1".to_owned()
        })
    );
    assert_eq!(
        f407(|file| set(file, "memory.ram#2", "start", 0x0800_0000_i64)),
        Err(Problem::FlashOverlapsRam {
            ram: "CCM".to_owned()
        })
    );
    assert_eq!(
        f407(|file| set(file, "memory.ram#2", "start", 0xFFFF_8000_i64)),
        Err(Problem::AddressRange {
            what: "memory.ram CCM".to_owned()
        })
    );
    // A region that ends at the last byte of the address space lies below 4 GiB.
    assert_eq!(
        f407(|file| set(file, "memory.ram#2", "start", 0xFFFF_0000_i64)).err(),
        None
    );
}

#[test]
fn the_layout_has_the_regions_of_its_family_and_no_others() {
    let region = |text: &str| toml::Value::Table(table(text));
    assert_eq!(
        f407(|file| remove(file, "layout", "vectors")),
        Err(Problem::MissingRegion { region: "vectors" })
    );
    assert_eq!(
        f407(|file| set(
            file,
            "layout",
            "bootloader",
            region("start = 0x08000000\nkib = 16")
        )),
        Err(Problem::UnexpectedRegion {
            region: "bootloader"
        })
    );
    assert_eq!(
        c6(|file| remove(file, "layout", "partition_table")),
        Err(Problem::MissingRegion {
            region: "partition_table"
        })
    );
    assert_eq!(
        c6(|file| set(file, "layout", "vectors", region("start = 0\nkib = 4"))),
        Err(Problem::UnexpectedRegion { region: "vectors" })
    );
}

#[test]
fn a_layout_region_lies_inside_the_flash_on_erase_unit_boundaries() {
    // Past the end of the flash, and before its start.
    assert_eq!(
        f407(|file| set(file, "layout.firmware", "kib", 976_i64)),
        Err(Problem::RegionOutsideFlash { region: "firmware" })
    );
    assert_eq!(
        f407(|file| set(file, "layout.application", "start", 0x07FF_C000_i64)),
        Err(Problem::RegionOutsideFlash {
            region: "application"
        })
    );
    // Half of a 16 KiB sector, and the firmware ending in the middle of a 128 KiB sector.
    assert_eq!(
        f407(|file| set(file, "layout.application", "kib", 8_i64)),
        Err(Problem::RegionNotAligned {
            region: "application"
        })
    );
    assert_eq!(
        f407(|file| set(file, "layout.firmware", "kib", 896_i64)),
        Err(Problem::RegionNotAligned { region: "firmware" })
    );
    assert_eq!(
        f407(|file| set(file, "layout.firmware", "kib", 704_i64)).err(),
        None
    );
    // SPI flash is erased in 4 KiB units from offset 0.
    assert_eq!(
        c6(|file| set(file, "layout.application", "start", 0x20_0800_i64)),
        Err(Problem::RegionNotAligned {
            region: "application"
        })
    );
}

#[test]
fn layout_regions_do_not_overlap_and_the_chip_starts_from_the_first() {
    assert_eq!(
        f407(|file| set(file, "layout.checkpoint_b", "start", 0x0800_8000_i64)),
        Err(Problem::RegionsOverlap {
            first: "checkpoint_a",
            second: "checkpoint_b"
        })
    );
    assert_eq!(
        c6(|file| set(file, "layout.application", "start", 0x1F_0000_i64)),
        Err(Problem::RegionsOverlap {
            first: "firmware",
            second: "application"
        })
    );
    // The vectors must start at the first byte of flash; sector 1, now unused, is a bound address
    // where they are refused.
    let moved = f407(|file| set(file, "layout.vectors", "start", 0x0800_4000_i64));
    assert_eq!(moved, Err(Problem::BootRegion { region: "vectors" }));
    let late = c6(|file| {
        set(file, "layout.bootloader", "start", 0x1000_i64);
        set(file, "layout.bootloader", "kib", 28_i64);
    });
    assert_eq!(
        late,
        Err(Problem::BootRegion {
            region: "bootloader"
        })
    );
}

/// The program slots lie inside `application` and apart from one another and from the checkpoint
/// slots (specification 34 Appendix H.2). The refusal names the two regions that overlap. Catches a
/// check that compares a slot with `application` only, which lets the two slots share flash, and a
/// slot that could erase a checkpoint unit.
#[test]
fn the_chip_file_refuses_overlapping_slots() {
    let region = |text: &str| toml::Value::Table(table(text));
    // application_b starts 0xC000 below application_a's end (0x4F_C000), inside application_a.
    assert_eq!(
        c6(|file| set(
            file,
            "layout",
            "application_b",
            region("start = 0x4F_0000\nkib = 3056")
        )),
        Err(Problem::RegionsOverlap {
            first: "application_b",
            second: "application_a"
        })
    );
    assert_eq!(
        c6(|file| set(
            file,
            "layout",
            "application_a",
            region("start = 0x4F_0000\nkib = 3056")
        )),
        Err(Problem::RegionsOverlap {
            first: "application_b",
            second: "application_a"
        })
    );
    // From 0x7F_0000, 3,056 KiB reach past checkpoint_a, which begins at 0x7F_8000.
    assert_eq!(
        c6(|file| set(
            file,
            "layout",
            "application_b",
            region("start = 0x7F_0000\nkib = 3056")
        )),
        Err(Problem::RegionsOverlap {
            first: "checkpoint_a",
            second: "application_b"
        })
    );
    assert_eq!(
        c6(|file| set(
            file,
            "layout",
            "application_a",
            region("start = 0x7F_0000\nkib = 3056")
        )),
        Err(Problem::RegionsOverlap {
            first: "checkpoint_a",
            second: "application_a"
        })
    );
}

/// The program slots are named both or neither, have one size, and lie inside `application` on
/// erase-unit boundaries (specification 34 §5.1 and Appendix H.2: "two application slots of equal
/// size, which the chip file names"). Catches a file that gives the program store one slot, two
/// slots of different sizes, a slot below `application`, an empty slot, and a slot that starts
/// off an erase unit, whose erase would clear the bytes before it.
#[test]
fn the_program_slots_are_named_in_pairs_of_one_size_inside_application() {
    assert_eq!(
        c6(|file| remove(file, "layout", "application_b")),
        Err(Problem::MissingRegion {
            region: "application_b"
        })
    );
    assert_eq!(
        c6(|file| remove(file, "layout", "application_a")),
        Err(Problem::MissingRegion {
            region: "application_a"
        })
    );
    // 3,000 KiB is 3,072,000 bytes: slot B then ends at 0x7E_A000, inside application.
    assert_eq!(
        c6(|file| set(file, "layout.application_b", "kib", 3000_i64)),
        Err(Problem::UnequalSlots {
            first: "application_a",
            second: "application_b"
        })
    );
    // Slot A starts 0x1_0000 below application, which begins at 0x20_0000.
    assert_eq!(
        c6(|file| set(file, "layout.application_a", "start", 0x1F_0000_i64)),
        Err(Problem::OutsideApplication {
            slot: "application_a"
        })
    );
    assert_eq!(
        c6(|file| set(file, "layout.application_b", "kib", 0_i64)),
        Err(Problem::ZeroSize {
            what: "layout.application_b".to_owned()
        })
    );
    // 0x20_0800 is 2 KiB into an erase unit.
    assert_eq!(
        c6(|file| set(file, "layout.application_a", "start", 0x20_0800_i64)),
        Err(Problem::RegionNotAligned {
            region: "application_a"
        })
    );
}

#[test]
fn each_interface_list_names_each_peripheral_once() {
    assert_eq!(
        f407(|file| set(
            file,
            "interfaces",
            "uart",
            vec!["USART1", "USART2", "USART1"]
        )),
        Err(Problem::Duplicate {
            list: "interfaces.uart",
            entry: "USART1".to_owned()
        })
    );
}

#[test]
fn each_family_counts_its_pins_in_its_own_key() {
    let stm32 = Err(Problem::GpioKind {
        family: Family::Stm32f4,
    });
    assert_eq!(f407(|file| set(file, "gpio", "pins", 140_i64)), stm32);
    assert_eq!(f407(|file| remove(file, "gpio", "ports")), stm32);
    let port_a = toml::Value::Array(vec![toml::Value::Table(table("port = \"A\"\npins = 16"))]);
    assert_eq!(
        c6(|file| set(file, "gpio", "ports", port_a)),
        Err(Problem::GpioKind {
            family: Family::Esp32
        })
    );
}

#[test]
fn ports_are_letters_a_to_k_with_1_to_16_pins_listed_once() {
    for port in ["L", "AB", "a", ""] {
        assert_eq!(
            f407(|file| set(file, "gpio.ports#8", "port", port)),
            Err(Problem::BadPort {
                port: port.to_owned()
            }),
            "{port:?}"
        );
    }
    for pins in [0_i64, 17] {
        assert_eq!(
            f407(|file| set(file, "gpio.ports#8", "pins", pins)),
            Err(Problem::BadPort {
                port: "I".to_owned()
            }),
            "{pins}"
        );
    }
    assert_eq!(
        f407(|file| set(file, "gpio.ports#8", "port", "A")),
        Err(Problem::Duplicate {
            list: "gpio.ports",
            entry: "A".to_owned()
        })
    );
}

#[test]
fn a_reserved_pin_is_a_pin_of_the_chip_listed_once() {
    for pin in ["PI12", "PJ0", "PA16", "pa2", "PA02", "PA", "GPIO2"] {
        assert_eq!(
            f407(|file| set(file, "gpio.reserved#0", "pin", pin)),
            Err(Problem::BadPin {
                pin: pin.to_owned()
            }),
            "{pin}"
        );
    }
    assert_eq!(
        f407(|file| set(file, "gpio.reserved#0", "pin", "PI11")).err(),
        None
    );
    for pin in ["GPIO31", "GPIO012", "12", "PA2", "gpio12"] {
        assert_eq!(
            c6(|file| set(file, "gpio.reserved#0", "pin", pin)),
            Err(Problem::BadPin {
                pin: pin.to_owned()
            }),
            "{pin}"
        );
    }
    assert_eq!(
        c6(|file| set(file, "gpio.reserved#0", "pin", "GPIO0")).err(),
        None
    );
    assert_eq!(
        f407(|file| set(file, "gpio.reserved#1", "pin", "PA2")),
        Err(Problem::Duplicate {
            list: "gpio.reserved",
            entry: "PA2".to_owned()
        })
    );
    assert_eq!(
        c6(|file| set(file, "gpio.reserved#1", "pin", "GPIO12")),
        Err(Problem::Duplicate {
            list: "gpio.reserved",
            entry: "GPIO12".to_owned()
        })
    );
}

#[test]
fn the_firmware_is_a_crate_under_firmware_that_holds_the_engine() {
    for path in [
        "crates/trust-runtime",
        "firmware/",
        "firmware/../crates/trust-runtime",
        "/firmware/trust-stm32f407",
        "firmware\\trust-stm32f407",
        "firmware//trust-stm32f407",
    ] {
        assert_eq!(
            f407(|file| set(file, "firmware", "crate", path)),
            Err(Problem::FirmwareCrate {
                path: path.to_owned()
            }),
            "{path}"
        );
    }
    let nested = "firmware/stm32/trust-stm32f407";
    assert_eq!(
        f407(|file| set(file, "firmware", "crate", nested)).err(),
        None
    );
    for components in [vec!["control"], Vec::new()] {
        assert_eq!(
            f407(|file| set(file, "firmware", "components", components)),
            Err(Problem::NoEngine)
        );
    }
    assert_eq!(
        f407(|file| set(
            file,
            "firmware",
            "components",
            vec!["engine", "control", "engine"]
        )),
        Err(Problem::Duplicate {
            list: "firmware.components",
            entry: "engine".to_owned()
        })
    );
}

#[test]
fn the_sources_name_their_documents_and_where_values_were_measured() {
    let none: Vec<&str> = Vec::new();
    assert_eq!(
        f407(|file| set(file, "sources", "documents", none.clone())),
        Err(Problem::NoDocuments)
    );
    assert_eq!(
        f407(|file| set(file, "sources", "documents", vec!["DS8626", "DS8626"])),
        Err(Problem::Duplicate {
            list: "sources.documents",
            entry: "DS8626".to_owned()
        })
    );
    assert_eq!(
        f407(|file| remove(file, "sources", "measured_on")),
        Err(Problem::MeasuredOn)
    );
    assert_eq!(
        f407(|file| set(file, "sources", "measured", none.clone())),
        Err(Problem::MeasuredOn)
    );
    let unmeasured = f407(|file| {
        set(file, "sources", "measured", none);
        remove(file, "sources", "measured_on");
    });
    assert_eq!(unmeasured.err(), None);
}

#[test]
fn a_source_path_names_a_key_of_the_file_outside_sources() {
    for path in [
        "identity.colour",
        "sources.documents",
        "memory.ram.colour",
        "identity..name",
        "",
    ] {
        assert_eq!(
            f407(|file| uncheck(file, path)),
            Err(Problem::UnknownSourcePath {
                path: path.to_owned()
            }),
            "{path:?}"
        );
    }
    // A key in every entry of an array of tables, a key of an inline table, and a whole table.
    for path in ["memory.ram.dma", "layout.vectors.start", "gpio"] {
        assert_eq!(f407(|file| uncheck(file, path)).err(), None, "{path}");
    }
}

#[test]
fn a_source_path_is_listed_once_and_not_inside_another() {
    // Measured and unchecked at once; a table whose keys are listed; a key of a listed table.
    for path in ["identity.device_id", "interfaces", "memory.flash"] {
        assert_eq!(
            f407(|file| uncheck(file, path)),
            Err(Problem::RepeatedSourcePath {
                path: path.to_owned()
            }),
            "{path}"
        );
    }
}

#[test]
fn an_stm32u5_file_describes_its_flash_in_memory_flash_and_keeps_its_vectors() {
    // Appendix H.5 rules 5 and 7: an STM32 chip describes its flash in memory.flash and not in
    // spi_flash, and its layout has the vector table and no bootloader. Catches an STM32U5 file that
    // names an SPI flash, that drops its vectors, or that takes the ESP32's bootloader region.
    let spi = || toml::Value::Table(table("erase_kib = 4"));
    assert_eq!(
        u5(|file| set(file, "memory", "spi_flash", spi())),
        Err(Problem::FlashKind {
            family: Family::Stm32u5
        })
    );
    assert_eq!(
        u5(|file| remove(file, "layout", "vectors")),
        Err(Problem::MissingRegion { region: "vectors" })
    );
    assert_eq!(
        u5(|file| set(
            file,
            "layout",
            "bootloader",
            toml::Value::Table(table("start = 0\nkib = 4"))
        )),
        Err(Problem::UnexpectedRegion {
            region: "bootloader"
        })
    );
}

#[test]
fn an_stm32u5_device_id_has_twelve_bits_and_its_pins_are_named_by_port() {
    // Appendix H.5 rules 2 and 8: the STM32 families share the twelve bits of DEV_ID and name their
    // pins by port, not by a count. Catches an STM32U5 device ID that is not bounded, or pins given as
    // a count.
    for device_id in [0_i64, 0x1000] {
        assert_eq!(
            u5(|file| set(file, "identity", "device_id", device_id)),
            Err(Problem::DeviceId {
                device_id: u32::try_from(device_id).expect("a u32")
            })
        );
    }
    assert_eq!(
        u5(|file| set(file, "identity", "device_id", 0xFFF_i64)).err(),
        None
    );
    assert_eq!(
        u5(|file| set(file, "gpio", "pins", 156_i64)),
        Err(Problem::GpioKind {
            family: Family::Stm32u5
        })
    );
}

#[test]
fn an_stm32u5_target_is_the_thumb_hard_float_target_of_its_chip() {
    // Appendix H.5 rule 4: the U5's target is a thumb target with hardware floating point, because the
    // chip has a single-precision FPU. A RISC-V target and a soft-float target are refused. Catches
    // an STM32U5 file that takes another family's target or disagrees with its FPU.
    let riscv = "riscv32imac-unknown-none-elf";
    assert_eq!(
        u5(|file| set(file, "processor", "rust_target", riscv)),
        Err(Problem::TargetFamily {
            target: riscv.to_owned()
        })
    );
    let soft = "thumbv8m.main-none-eabi";
    assert_eq!(
        u5(|file| set(file, "processor", "rust_target", soft)),
        Err(Problem::TargetFpu {
            target: soft.to_owned()
        })
    );
    assert_eq!(u5(|_| {}).err(), None);
}
