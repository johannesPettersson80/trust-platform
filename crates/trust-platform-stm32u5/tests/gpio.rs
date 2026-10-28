//! The STM32U5A9's pins by name (specification 34, TARGET-01 and D.5): the claim against the chip table,
//! the set-up order of an output, and the record that the emergency path drives to the safe levels.

use trust_platform_stm32u5::chip::CHIP;
use trust_platform_stm32u5::gpio::{claim_pin, output_steps, record_bits, safe_levels, Step};
use trust_platform_stm32u5::pins::{Claims, Pin, PinError, PinNameError, Port, Pull, Reserved};

fn pin(name: &str) -> Pin {
    Pin::parse(name).expect("a pin name of the grammar")
}

#[test]
fn a_reserved_or_absent_pin_is_refused_with_its_reason() {
    // Specification 34, D.5 (Pins): PA11 and PA12 are kept for USB, PA13 and PA14 for SWD, PH0 for the
    // HSE in bypass mode. The chip has ports A to J, PJ0 to PJ11 and no port K. A pin is claimed once.
    // Catches a kept pin given to a line, a reason swapped for another, a pin past a port's last
    // accepted, a missing port accepted, and one pin claimed by two lines.
    let mut claims = Claims::new();
    assert_eq!(
        claim_pin(&CHIP, &mut claims, "PA11"),
        Err(PinError::Reserved(pin("PA11"), Reserved::Usb))
    );
    assert_eq!(
        claim_pin(&CHIP, &mut claims, "PA13"),
        Err(PinError::Reserved(pin("PA13"), Reserved::Debug))
    );
    assert_eq!(
        claim_pin(&CHIP, &mut claims, "PH0"),
        Err(PinError::Reserved(pin("PH0"), Reserved::Oscillator))
    );
    assert_eq!(
        claim_pin(&CHIP, &mut claims, "PJ12"),
        Err(PinError::NoSuchPin(pin("PJ12")))
    );
    assert_eq!(
        claim_pin(&CHIP, &mut claims, "PK0"),
        Err(PinError::NoSuchPort(Port::K))
    );
    assert_eq!(
        claim_pin(&CHIP, &mut claims, "PD16"),
        Err(PinError::Name(PinNameError::Range))
    );
    assert_eq!(claim_pin(&CHIP, &mut claims, "PD12"), Ok(pin("PD12")));
    assert_eq!(
        claim_pin(&CHIP, &mut claims, "PD12"),
        Err(PinError::AlreadyClaimed(pin("PD12")))
    );
}

#[test]
fn an_output_is_latched_to_its_safe_level_before_it_becomes_an_output() {
    // Specification 34, D.4 and TARGET-01: an output's latch takes its safe level before its mode becomes
    // output, so the pin never shows another level, and its mode is set last. Port D, pin 12, with the
    // safe level low: latch low, push-pull, low speed, no pull, then output mode. Catches a mode set
    // before the latch, a latch to the opposite level, a pull left on an output, and a latch that is
    // not the first step.
    let pd12 = pin("PD12");
    assert_eq!(
        output_steps(pd12, false),
        [
            Step::Latch {
                pin: pd12,
                high: false
            },
            Step::PushPull(pd12),
            Step::LowSpeed(pd12),
            Step::Pull(pd12, Pull::Floating),
            Step::Output(pd12),
        ]
    );
    // The same pin with a high safe level latches high first.
    assert_eq!(
        output_steps(pd12, true)[0],
        Step::Latch {
            pin: pd12,
            high: true
        }
    );
}

#[test]
fn the_emergency_path_drives_every_configured_output_to_its_safe_value() {
    // Specification 34, D.5 (outputs take their safe level on a fault). Port D has two outputs: PD12 with
    // a safe level of low and PD3 with a safe level of high. Each output sets its bit in the port's record
    // (bit n for pin n), and a high safe level also sets bit 16 + n. PD12 gives 0x0000_1000; PD3 gives
    // 0x0000_0008 and 0x0008_0000. The record is 0x0008_1008, from which the high pins are PD3 (0x0008)
    // and the low pins are PD12 (0x1000). Port A has PA0 with a safe level of high: record 0x0001_0001,
    // high pin 0x0001. An empty record drives nothing. Catches a swapped level, a pin left out, an
    // output of another port picked up, and a pin in both masks.
    let port_d = record_bits(pin("PD12"), false) | record_bits(pin("PD3"), true);
    assert_eq!(port_d, 0x0008_1008);
    assert_eq!(safe_levels(port_d), (0x0008, 0x1000));

    let port_a = record_bits(pin("PA0"), true);
    assert_eq!(port_a, 0x0001_0001);
    assert_eq!(safe_levels(port_a), (0x0001, 0x0000));

    assert_eq!(safe_levels(0), (0, 0));
}
