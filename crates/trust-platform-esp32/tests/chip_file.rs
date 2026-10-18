//! The ESP32-C6 data of this adapter is the chip's file, `chips/esp32c6.toml` (specification 34,
//! TARGET-03 and Appendix H): the chip ID, the clock, the partitions the firmware reads, and the
//! instruction set the firmware checks at boot. The firmware cannot read the file, so this host
//! test keeps the two equal.

use trust_chips::{Catalog, ChipFile, Family, Fpu};
use trust_platform_esp32::profile::{
    numeric_profile_ok, CHIP_ID, CPU_HZ, DEVICE_DESCRIPTOR, FACTORY_OFFSET, FLASH_KIB, STBC_BYTES,
    STBC_OFFSET,
};
use trust_runtime_core::deployment::DeviceDescriptor;

fn chip_file() -> ChipFile {
    Catalog::embedded()
        .expect("the chip files are valid")
        .chip("esp32c6")
        .cloned()
        .expect("the ESP32-C6 has a chip file")
}

/// The `misa` an RV32 core with these extensions reads: MXL 1 in bits 31:30, one bit per letter.
fn misa(isa: &str) -> u32 {
    let extensions = isa.strip_prefix("RV32").expect("an RV32 instruction set");
    extensions
        .bytes()
        .fold(1 << 30, |misa, letter| misa | (1 << (letter - b'A')))
}

#[test]
fn the_c6_profile_has_the_identity_clock_and_partitions_of_the_chip_file() {
    let file = chip_file();
    assert_eq!(file.identity.model, "ESP32-C6");
    assert_eq!(file.identity.family, Family::Esp32);
    assert_eq!(file.identity.device_id, u32::from(CHIP_ID));
    // `CpuClock::max()`: the firmware runs the HP core at its highest clock.
    assert_eq!(file.clock.firmware_hz, CPU_HZ);
    assert_eq!(file.clock.max_hz, CPU_HZ);
    assert_eq!(file.layout.firmware.start, FACTORY_OFFSET);
    assert_eq!(file.layout.application.start, STBC_OFFSET);
    assert_eq!(
        file.layout.application.bytes(),
        u64::try_from(STBC_BYTES).expect("16 KiB")
    );
}

#[test]
fn the_instruction_set_of_the_chip_file_is_the_one_the_firmware_checks_at_boot() {
    let file = chip_file();
    assert!(numeric_profile_ok(misa(&file.processor.isa)));
    assert_eq!(file.processor.fpu, Fpu::None);
    // A chip with the F extension would fail the firmware's check.
    assert!(!numeric_profile_ok(misa("RV32IMAFC")));
}

#[test]
fn the_bench_module_flash_holds_the_whole_layout() {
    // FLASH_KIB is the 8 MB of the owned DevKitC-1's module, not a chip value: the C6's flash is
    // outside the chip, and its size is read when truST is installed.
    let file = chip_file();
    assert!(file.memory.flash.is_none());
    let end = file
        .layout
        .regions()
        .into_iter()
        .map(|(_, region)| region.end())
        .max()
        .expect("a layout");
    assert!(end <= u64::from(FLASH_KIB) * 1024);
}

/// The device descriptor admission reads is the chip file's application region, and its maxima are
/// trust-runtime's hosted maxima because `chips/esp32c6.toml` names no maxima yet (specification
/// 34 §5.1.1: "Appendix H gains the keys for its maxima with the chip's firmware"). Catches a slot
/// that is not the chip's application region (for example the 16 KiB bench bundle) and maxima that
/// were invented instead of taken from the chip file or from trust-runtime.
#[test]
fn the_device_descriptor_matches_the_chip_file() {
    let file = chip_file();
    assert_eq!(
        DEVICE_DESCRIPTOR.application_slot_bytes,
        Some(file.layout.application.bytes())
    );
    // No maxima keys yet: the descriptor carries trust-runtime's hosted maxima, which the
    // chip file will replace when Appendix H gains them.
    assert_eq!(DEVICE_DESCRIPTOR.maxima, DeviceDescriptor::hosted().maxima);
}
