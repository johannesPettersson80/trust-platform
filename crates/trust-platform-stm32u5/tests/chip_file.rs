//! The STM32U5A9 data of this adapter is the chip's file, `chips/stm32u5a9.toml` (specification 34,
//! TARGET-03 and Appendix H): the chip table in `chip.rs` and the clock of `clock.rs`. The firmware
//! cannot read the file, so this host test keeps the two equal, and checks the design's values
//! (Appendix D.5) written here as literals.

use trust_chips::{ChipFile, Family, PinReason};
use trust_platform_stm32::pins::{Pin, PinError, Port, Reserved};
use trust_platform_stm32u5::chip::{CHIP, STM32U5A9};
use trust_platform_stm32u5::clock::{FLASH_WAIT_STATES, HSE_HZ, SYSCLK_HZ};

fn chip_file() -> ChipFile {
    // The file joins the catalog with its firmware crate in S1b-9u, so it is parsed here as the
    // catalog parses a file.
    ChipFile::parse("stm32u5a9", include_str!("../../../chips/stm32u5a9.toml"))
        .expect("the STM32U5A9 chip file is valid")
}

/// The chip file's word for why the firmware keeps a pin.
fn reason(reason: Reserved) -> PinReason {
    match reason {
        Reserved::Console => PinReason::Console,
        Reserved::Debug => PinReason::Debug,
        Reserved::Oscillator => PinReason::Oscillator,
        Reserved::Usb => PinReason::Usb,
    }
}

/// The pins the firmware keeps, with the reason the design gives for each (Appendix D.5, Pins).
const KEPT: [(&str, Reserved); 5] = [
    ("PA11", Reserved::Usb),
    ("PA12", Reserved::Usb),
    ("PA13", Reserved::Debug),
    ("PA14", Reserved::Debug),
    ("PH0", Reserved::Oscillator),
];

#[test]
fn the_u5_chip_table_has_the_identity_and_memory_of_the_chip_file() {
    // Specification 34, Appendix D.5 (Chip, Identity) and H.2: DEV_ID 0x481, 4096 KiB of flash in
    // 512 runs of 8 KiB from 0x0800_0000, and 2512 KiB of SRAM beside the 2 KiB backup SRAM (DS13543
    // Table 2). Catches a table and a file that agree with each other but not with the design.
    let file = chip_file();
    assert_eq!(file.identity.model, STM32U5A9.name);
    assert_eq!(file.identity.family, Family::Stm32u5);
    assert_eq!(STM32U5A9.device_id, 0x481);
    assert_eq!(file.identity.device_id, u32::from(STM32U5A9.device_id));
    assert_eq!(STM32U5A9.flash_kib, 4096);
    let flash = file
        .memory
        .flash
        .as_ref()
        .expect("the U5's flash is in its address space");
    assert_eq!(flash.start, 0x0800_0000);
    assert_eq!(flash.kib, u32::from(STM32U5A9.flash_kib));
    assert!(file.memory.spi_flash.is_none());
    let sectors: Vec<(u32, u32)> = flash
        .sectors
        .iter()
        .map(|run| (run.count, run.kib))
        .collect();
    let table: Vec<(u32, u32)> = STM32U5A9
        .sectors
        .iter()
        .map(|run| (u32::from(run.count), u32::from(run.kib)))
        .collect();
    assert_eq!(sectors, table);
    assert_eq!(table, [(512, 8)]);
    let ram: Vec<(&str, u32, u32, bool)> = file
        .memory
        .ram
        .iter()
        .map(|region| (region.name.as_str(), region.start, region.kib, region.dma))
        .collect();
    let table: Vec<(&str, u32, u32, bool)> = STM32U5A9
        .ram
        .iter()
        .map(|region| (region.name, region.start, u32::from(region.kib), region.dma))
        .collect();
    assert_eq!(ram, table);
    let sram: u32 = STM32U5A9
        .ram
        .iter()
        .filter(|region| region.name != "BKPSRAM")
        .map(|region| u32::from(region.kib))
        .sum();
    assert_eq!(sram, 2512);
}

#[test]
fn the_u5_clock_of_the_table_and_the_file_is_160_mhz_within_the_crystal_range() {
    // Specification 34, Appendix D.5 (Clock) and H.2: SYSCLK 160 MHz, the board's 16 MHz HSE, and the
    // oscillator's 4 to 48 MHz crystals (DS13543 §5.3.8, Table 70, and p. 247). Catches a clock the
    // file and the module disagree on, or a board HSE outside the chip's oscillator range.
    let file = chip_file();
    assert_eq!(STM32U5A9.sysclk_hz, 160_000_000);
    assert_eq!(SYSCLK_HZ, STM32U5A9.sysclk_hz);
    assert_eq!(file.clock.firmware_hz, SYSCLK_HZ);
    assert_eq!(file.clock.max_hz, SYSCLK_HZ);
    assert_eq!(file.clock.crystal_min_hz, 4_000_000);
    assert_eq!(file.clock.crystal_max_hz, 48_000_000);
    assert_eq!(HSE_HZ, 16_000_000);
    assert!(file.clock.crystal_min_hz <= HSE_HZ && HSE_HZ <= file.clock.crystal_max_hz);
    assert_eq!(FLASH_WAIT_STATES, 4);
}

#[test]
fn the_u5_ports_and_kept_pins_of_the_table_are_those_of_the_chip_file() {
    // Specification 34, Appendix D.5 (Pins): ports A to I with 16 pins and port J with 12 (PJ0 to
    // PJ11), and the five pins the firmware keeps, with their reasons in the file's order.
    let file = chip_file();
    let ports: Vec<(String, u32)> = file
        .gpio
        .ports
        .as_ref()
        .expect("the U5's pins are named by port")
        .iter()
        .map(|entry| (entry.port.clone(), entry.pins))
        .collect();
    let table: Vec<(String, u32)> = STM32U5A9
        .ports
        .iter()
        .map(|spec| (spec.port.letter().to_string(), u32::from(spec.pins)))
        .collect();
    assert_eq!(ports, table);
    assert_eq!(table.last(), Some(&("J".to_string(), 12)));
    let kept: Vec<(String, PinReason)> = file
        .gpio
        .reserved
        .iter()
        .map(|entry| (entry.pin.clone(), entry.reason))
        .collect();
    let table: Vec<(String, PinReason)> = STM32U5A9
        .reserved
        .iter()
        .map(|entry| (entry.pin.to_string(), reason(entry.reason)))
        .collect();
    assert_eq!(kept, table);
    assert_eq!(kept.len(), KEPT.len());
}

#[test]
fn every_pin_name_of_ports_a_to_j_round_trips_and_the_pins_past_a_port_are_refused() {
    // Specification 34, Appendix D.5 (Pins): a name of port A to J and a number 0 to 15 round-trips
    // through its printed name; a number past the port's last pin is refused as no such pin (PJ12),
    // and a kept pin is refused with its reason. Catches a pin count one too high or too low, and a
    // port or pin the chip lacks being accepted.
    for letter in 'A'..='J' {
        let count = if letter == 'J' { 12 } else { 16 };
        for number in 0..16_u8 {
            let name = format!("P{letter}{number}");
            let pin = Pin::parse(&name).expect("a pin name of the grammar");
            assert_eq!(pin.to_string(), name, "the name round-trips");
            let expected = match KEPT.iter().find(|(kept, _)| *kept == name) {
                Some(&(_, why)) => Err(PinError::Reserved(pin, why)),
                None if number < count => Ok(pin),
                None => Err(PinError::NoSuchPin(pin)),
            };
            assert_eq!(CHIP.pin(&name), expected, "{name}");
        }
    }
    assert_eq!(
        CHIP.pin("PK0"),
        Err(PinError::NoSuchPort(Port::K)),
        "the U5 has no port K"
    );
}

#[test]
fn each_kept_pin_is_refused_with_its_reason_and_the_free_pins_stay_usable() {
    // Specification 34, Appendix D.5 (Pins): PA11 and PA12 are kept for USB, PA13 and PA14 for SWD,
    // PH0 for the HSE. PH1 is free in bypass mode, and PC14 and PC15 are not kept in this slice (the
    // design's notes to chips/stm32u5a9.toml). Catches a kept pin given to a project, a reason
    // swapped for another, or a free pin kept.
    for (name, why) in KEPT {
        assert_eq!(
            CHIP.pin(name),
            Err(PinError::Reserved(
                Pin::parse(name).expect("a pin name"),
                why
            )),
            "{name}"
        );
    }
    for name in ["PH1", "PC14", "PC15", "PA10"] {
        assert!(CHIP.pin(name).is_ok(), "{name} is free");
    }
}
