//! The GPIO line rules of the ESP32-C6 adapter (design §1.2 and specification 34, TARGET-01): a
//! line's pin is checked against the chip's file, and an inverted line reads the opposite level.
//! Both are pure, so a host test pins them without esp-hal.

use trust_chips::Catalog;
use trust_platform_esp32::process_io::{check_pin, GpioPin, InputLine, OutputLine};

/// The C6's GPIO count and reserved pins, read from `chips/esp32c6.toml`.
fn chip_pins() -> (u32, Vec<GpioPin>) {
    let file = Catalog::embedded()
        .expect("the chip files are valid")
        .chip("esp32c6")
        .cloned()
        .expect("the ESP32-C6 has a chip file");
    let pins = file.gpio.pins.expect("the esp32 family counts its pins");
    let reserved = file
        .gpio
        .reserved
        .iter()
        .map(|entry| GpioPin::parse(&entry.pin).expect("a reserved pin name is a GPIO name"))
        .collect();
    (pins, reserved)
}

/// `GPIO18` is a pin of the chip and free; a pin the file does not list, or lists as reserved, is
/// refused with its name (specification 34, TARGET-01 and Appendix H `gpio.reserved`).
#[test]
fn pin_names_are_checked_against_the_chip_file() {
    let (pins, reserved) = chip_pins();
    // `chips/esp32c6.toml` has 31 pins (GPIO0 to GPIO30) and reserves GPIO12 and GPIO13 for USB.
    assert_eq!(pins, 31);
    assert_eq!(
        check_pin("GPIO18", pins, &reserved).map(GpioPin::number),
        Ok(18)
    );
    let not_listed = check_pin("GPIO31", pins, &reserved).expect_err("GPIO31 is beyond the chip");
    assert_eq!(not_listed.to_string(), "GPIO31 is not a pin of this chip");
    let reserved_pin = check_pin("GPIO12", pins, &reserved).expect_err("GPIO12 is reserved");
    assert_eq!(
        reserved_pin.to_string(),
        "GPIO12 is reserved by the firmware"
    );
}

/// An inverted line reads the opposite of the pin's physical level (design §1.2 `invert`).
#[test]
fn an_inverted_input_reads_the_opposite_level() {
    let inverted = InputLine {
        byte: 0,
        bit: 0,
        pin: GpioPin::parse("GPIO9").expect("GPIO9 is a pin name"),
        invert: true,
    };
    assert!(!inverted.level(true));
    assert!(inverted.level(false));
    let plain = InputLine {
        invert: false,
        ..inverted
    };
    assert!(plain.level(true));
    assert!(!plain.level(false));
    // An output line inverts the same way: an inverted output drives the opposite of its value.
    let output = OutputLine {
        byte: 0,
        bit: 0,
        pin: GpioPin::parse("GPIO18").expect("GPIO18 is a pin name"),
        invert: true,
        initial: false,
    };
    assert!(output.level(false));
    assert!(!output.level(true));
}
