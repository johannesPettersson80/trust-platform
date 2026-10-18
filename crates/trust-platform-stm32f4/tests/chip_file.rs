//! The STM32F407 data of this adapter is the chip's file, `chips/stm32f407.toml` (specification
//! 34, TARGET-03 and Appendix H): the chip table in `chip.rs` and the crystal range of
//! `crystal.rs`. The firmware cannot read the file, so this host test keeps the two equal.

use trust_chips::{Catalog, ChipFile, Family, PinReason};
use trust_platform_stm32f4::chip::{FLASH_START, STM32F407};
use trust_platform_stm32f4::crystal::{MAX_CRYSTAL_HZ, MIN_CRYSTAL_HZ};
use trust_platform_stm32f4::pins::Reserved;

fn chip_file() -> ChipFile {
    Catalog::embedded()
        .expect("the chip files are valid")
        .chip("stm32f407")
        .cloned()
        .expect("the STM32F407 has a chip file")
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

#[test]
fn the_f407_chip_table_has_the_identity_clock_and_memory_of_the_chip_file() {
    let file = chip_file();
    assert_eq!(file.identity.model, STM32F407.name);
    assert_eq!(file.identity.family, Family::Stm32f4);
    assert_eq!(file.identity.device_id, u32::from(STM32F407.device_id));
    assert_eq!(file.clock.firmware_hz, STM32F407.sysclk_hz);
    let flash = file
        .memory
        .flash
        .as_ref()
        .expect("the F407's flash is in its address space");
    assert_eq!(flash.start, FLASH_START);
    assert_eq!(flash.kib, u32::from(STM32F407.flash_kib));
    let sectors: Vec<(u32, u32)> = flash
        .sectors
        .iter()
        .map(|run| (run.count, run.kib))
        .collect();
    let table: Vec<(u32, u32)> = STM32F407
        .sectors
        .iter()
        .map(|run| (u32::from(run.count), u32::from(run.kib)))
        .collect();
    assert_eq!(sectors, table);
    let ram: Vec<(&str, u32, u32, bool)> = file
        .memory
        .ram
        .iter()
        .map(|region| (region.name.as_str(), region.start, region.kib, region.dma))
        .collect();
    let table: Vec<(&str, u32, u32, bool)> = STM32F407
        .ram
        .iter()
        .map(|region| (region.name, region.start, u32::from(region.kib), region.dma))
        .collect();
    assert_eq!(ram, table);
}

#[test]
fn the_f407_chip_table_has_the_ports_and_kept_pins_of_the_chip_file() {
    let file = chip_file();
    let ports: Vec<(String, u32)> = file
        .gpio
        .ports
        .as_ref()
        .expect("the F407's pins are named by port")
        .iter()
        .map(|entry| (entry.port.clone(), entry.pins))
        .collect();
    let table: Vec<(String, u32)> = STM32F407
        .ports
        .iter()
        .map(|spec| (spec.port.letter().to_string(), u32::from(spec.pins)))
        .collect();
    assert_eq!(ports, table);
    let kept: Vec<(String, PinReason)> = file
        .gpio
        .reserved
        .iter()
        .map(|entry| (entry.pin.clone(), entry.reason))
        .collect();
    let table: Vec<(String, PinReason)> = STM32F407
        .reserved
        .iter()
        .map(|entry| (entry.pin.to_string(), reason(entry.reason)))
        .collect();
    assert_eq!(kept, table);
    // The console's pins are among the pins kept for the console.
    for pin in [STM32F407.console.tx, STM32F407.console.rx] {
        assert!(
            kept.contains(&(pin.to_string(), PinReason::Console)),
            "{pin}"
        );
    }
}

#[test]
fn the_crystal_measurement_accepts_the_crystals_of_the_chip_file() {
    let file = chip_file();
    assert_eq!(file.clock.crystal_min_hz, MIN_CRYSTAL_HZ);
    assert_eq!(file.clock.crystal_max_hz, MAX_CRYSTAL_HZ);
}
