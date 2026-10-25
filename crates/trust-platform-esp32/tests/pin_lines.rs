//! The GPIO line rules of the ESP32-C6 adapter (design §1.2 and specification 34, TARGET-01): a
//! line's pin is checked against the chip's file, and an inverted line reads the opposite level.
//! Both are pure, so a host test pins them without esp-hal.

use trust_chips::Catalog;
use trust_platform_esp32::process_io::{
    check_pin, GpioPin, InputLine, OutputLine, PinClaims, PinError, SafeLevels, CHIP_GPIO_PINS,
    CHIP_GPIO_RESERVED,
};

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
    // `chips/esp32c6.toml` has 31 pins (GPIO0 to GPIO30) and reserves GPIO12 and GPIO13 for USB and
    // GPIO24 to GPIO30 for the SPI flash.
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
    // A flash pin: a line there would take the flash the chip runs from. Catches a firmware that
    // lets a project drive the SPI flash's clock.
    let flash_pin = check_pin("GPIO29", pins, &reserved).expect_err("GPIO29 is the flash clock");
    assert_eq!(flash_pin.to_string(), "GPIO29 is reserved by the firmware");
    // The firmware cannot read the chip file, so its compiled-in count and reserved list are the
    // chip file's `[gpio]`. Catches a firmware that counts the wrong number of pins or forgets a
    // reserved pin.
    assert_eq!(CHIP_GPIO_PINS, pins);
    assert_eq!(
        CHIP_GPIO_RESERVED.map(GpioPin::from_number).as_slice(),
        reserved.as_slice()
    );
    // A pin two lines name is refused with its name.
    assert_eq!(
        PinError::Claimed(GpioPin::from_number(18)).to_string(),
        "GPIO18 is already claimed by another line"
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
        safe: false,
    };
    assert!(output.level(false));
    assert!(!output.level(true));
}

#[test]
fn the_terminal_fault_path_drives_each_output_to_its_safe_level() {
    // Specification 34, TARGET-08: each output goes to its `[[io.safe_state]]` value, and an output
    // without one is de-energized; an inverted line drives the opposite level. GPIO2 is safe true,
    // GPIO5 safe false, GPIO7 safe true on an inverted line and GPIO9 has no entry, so the fault path
    // drives GPIO2 high and GPIO5, GPIO7 and GPIO9 low: masks 1 << 2 and (1 << 5) | (1 << 7) | (1 << 9).
    // A pin recorded again takes its newer level. Catches every output driven low (the M2E rule this
    // replaces), an inversion that is ignored, and a pin that was never an output being driven.
    let line = |pin: &str, invert: bool, safe: bool| OutputLine {
        byte: 0,
        bit: 0,
        pin: GpioPin::parse(pin).expect("a pin name"),
        invert,
        initial: false,
        safe,
    };
    let levels = SafeLevels::new();
    for output in [
        line("GPIO2", false, true),
        line("GPIO5", false, false),
        line("GPIO7", true, true),
        line("GPIO9", false, false),
    ] {
        levels.record(output.pin, output.safe_level());
    }
    assert_eq!(levels.masks(), (1 << 2, (1 << 5) | (1 << 7) | (1 << 9)));
    levels.record(GpioPin::parse("GPIO5").expect("a pin name"), true);
    assert_eq!(
        levels.masks(),
        ((1 << 2) | (1 << 5), (1 << 7) | (1 << 9)),
        "a pin recorded again takes its newer level"
    );
    assert_eq!(
        SafeLevels::new().masks(),
        (0, 0),
        "no output, no pin driven"
    );
}

#[test]
fn a_forgotten_output_is_no_longer_driven_by_the_terminal_fault_path() {
    // Specification 34 STATE-08 and TARGET-08: the fault path drives the outputs of the program that
    // runs. An output whose line was dropped with its program is forgotten, and an output the next
    // program claims is recorded again, with its own safe level. GPIO2 (safe high) and GPIO5 (safe
    // low) are recorded; forgetting GPIO2 leaves only GPIO5 to drive low; recording GPIO2 again with
    // a low safe level drives both low; forgetting a pin that was never recorded changes nothing.
    // Catches a fault path that keeps driving the pins of a program that is gone, and a forget that
    // clears the level of the wrong pin.
    let pin = |name: &str| GpioPin::parse(name).expect("a pin name");
    let levels = SafeLevels::new();
    levels.record(pin("GPIO2"), true);
    levels.record(pin("GPIO5"), false);
    assert_eq!(levels.masks(), (1 << 2, 1 << 5));
    levels.forget(pin("GPIO2"));
    assert_eq!(levels.masks(), (0, 1 << 5));
    levels.forget(pin("GPIO7"));
    assert_eq!(levels.masks(), (0, 1 << 5));
    levels.record(pin("GPIO2"), false);
    assert_eq!(levels.masks(), (0, (1 << 2) | (1 << 5)));
}

#[test]
fn a_claimed_pin_is_free_again_when_its_line_is_dropped() {
    // Specification 34 STATE-08 and TARGET-01: a pin belongs to one line at a time. GPIO18 is claimed
    // and held; a second claim of it is refused with `GPIO18 is already claimed by another line`
    // while GPIO19 is free; dropping the line frees GPIO18 and only GPIO18, and the program loaded
    // next claims it again. Catches a claim that is never given back (the second program could claim
    // no pin the first one used), a release that frees every pin, and a claim that refuses a pin
    // another line does not hold.
    let pin = |name: &str| GpioPin::parse(name).expect("a pin name");
    let claims = PinClaims::default();
    let line = claims.claim(pin("GPIO18")).expect("a free pin is claimed");
    let other = claims.claim(pin("GPIO19")).expect("another pin is free");
    assert!(claims.holds(pin("GPIO18")));
    let refused = claims
        .claim(pin("GPIO18"))
        .expect_err("a held pin is refused");
    assert_eq!(
        refused.to_string(),
        "GPIO18 is already claimed by another line"
    );

    drop(line);
    assert!(
        !claims.holds(pin("GPIO18")),
        "the dropped line freed its pin"
    );
    assert!(claims.holds(pin("GPIO19")), "and only its pin");
    let again = claims
        .claim(pin("GPIO18"))
        .expect("the program loaded next claims the pin again");
    assert!(claims.holds(pin("GPIO18")));
    drop((again, other));
    assert!(!claims.holds(pin("GPIO18")));
    assert!(!claims.holds(pin("GPIO19")));
}
