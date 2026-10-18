//! The GPIO process image of the installed program over a chip's pins (specification 34, §7.2,
//! TARGET-01, STATE-01 and STATE-04). A chip claims the record's lines by pin name, latches each
//! output to its level, and gives the claimed pins to [`ProcessImage`] behind [`GpioPins`]. The image
//! reads the inputs into the flat input image and drives the outputs from the flat output image, each
//! line with its `io.toml` inversion. The claim stays with each chip, because the chips check their
//! pins and latch their outputs differently.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::deployment::{Configuration, GpioInput, GpioOutput};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::value::Value;

use crate::runner::engine::ProcessIo;
use crate::status::{IoDriverHealth, IoDriverStatus};

/// One line's place in the flat process image and its `io.toml` inversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageBit {
    /// Byte of the image, from the line's `io.toml` address.
    pub byte: usize,
    /// Bit within `byte`, 0 to 7.
    pub bit: u8,
    /// Whether the pin's level is inverted (`io.toml` `invert`).
    pub invert: bool,
}

impl ImageBit {
    /// The logical value of the pin's physical level `high`; an inverted line flips it.
    #[must_use]
    pub const fn value(self, high: bool) -> bool {
        high != self.invert
    }

    /// The pin's physical level for the logical value `value`; an inverted line flips it.
    #[must_use]
    pub const fn level(self, value: bool) -> bool {
        value != self.invert
    }
}

/// A chip's GPIO pins as the process image drives them (specification 34, TARGET-01). A claimed input
/// is read and a claimed output is driven, each through the handle the chip's claim returned.
pub trait GpioPins {
    /// A claimed input pin.
    type Input;
    /// A claimed output pin.
    type Output;

    /// Whether the input pin is high now.
    fn is_high(&self, input: &Self::Input) -> bool;

    /// Drives the output pin to the physical level `high`.
    fn drive(&mut self, output: &mut Self::Output, high: bool);
}

/// The installed program's process image over a chip's claimed pins: one entry per `io.toml` line,
/// in `io.toml` order, each with its bit of the flat image.
pub struct ProcessImage<P: GpioPins> {
    pins: P,
    inputs: Vec<(P::Input, ImageBit)>,
    outputs: Vec<(P::Output, ImageBit)>,
}

impl<P: GpioPins> ProcessImage<P> {
    /// The image over `pins`, with the claimed input and output lines and their bits of the image.
    #[must_use]
    pub fn new(
        pins: P,
        inputs: Vec<(P::Input, ImageBit)>,
        outputs: Vec<(P::Output, ImageBit)>,
    ) -> Self {
        Self {
            pins,
            inputs,
            outputs,
        }
    }
}

impl<P: GpioPins> ProcessIo for ProcessImage<P> {
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        for (pin, bit) in &self.inputs {
            let high = self.pins.is_high(pin);
            write_bit(image, *bit, bit.value(high))?;
        }
        Ok(())
    }

    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        for (pin, bit) in &mut self.outputs {
            let value = read_bit(image, *bit)?;
            self.pins.drive(pin, bit.level(value));
        }
        Ok(())
    }

    /// Every output is attempted, even when one image bit cannot be read, and the first failure is
    /// reported (specification 34, STATE-04).
    fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let mut first = None;
        for (pin, bit) in &mut self.outputs {
            match read_bit(image, *bit) {
                Ok(value) => self.pins.drive(pin, bit.level(value)),
                Err(error) => {
                    if first.is_none() {
                        first = Some(error);
                    }
                }
            }
        }
        first.map_or(Ok(()), Err)
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        vec![IoDriverStatus {
            name: "gpio".into(),
            health: IoDriverHealth::Ok,
        }]
    }
}

/// The record's `gpio` driver lines, in `io.toml` order (specification 34, TARGET-01). An enabled
/// driver of another name is refused with its name; `chip` names the chip in the refusal, because a
/// chip carries out only GPIO until its other drivers exist. A disabled driver is skipped.
///
/// # Errors
///
/// The refusal's text, for an enabled driver whose name is not `gpio`.
pub fn record_lines(
    config: &Configuration,
    chip: &str,
) -> Result<(Vec<GpioInput>, Vec<GpioOutput>), String> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for driver in &config.io_drivers {
        if !driver.enabled {
            continue;
        }
        if driver.name.as_str() != "gpio" {
            return Err(format!(
                "the installed application uses the {} driver, which the {chip} does not carry out yet",
                driver.name
            ));
        }
        inputs.extend(driver.inputs.iter().cloned());
        outputs.extend(driver.outputs.iter().cloned());
    }
    Ok((inputs, outputs))
}

/// The logical safe value of the output at `address` (specification 34, TARGET-08): its
/// `[[io.safe_state]]` entry when that is a boolean, and false otherwise, a de-energized output. A
/// chip latches each output to this value before the output is enabled, and its terminal fault path
/// drives the output to it before the chip resets.
#[must_use]
pub fn safe_value(config: &Configuration, address: &IoAddress) -> bool {
    config
        .safe_state
        .iter()
        .find(|entry| entry.address == *address)
        .is_some_and(|entry| matches!(entry.value, Value::Bool(true)))
}

/// Reads the bit of the flat output image that `bit` drives.
fn read_bit(image: &[u8], bit: ImageBit) -> Result<bool, RuntimeError> {
    if bit.bit > 7 {
        return Err(invalid("gpio bit index must be between 0 and 7"));
    }
    let Some(value) = image.get(bit.byte) else {
        return Err(invalid("gpio output mapping outside the image"));
    };
    Ok(value & (1 << bit.bit) != 0)
}

/// Writes `value` to the bit of the flat input image that `bit` fills.
fn write_bit(image: &mut [u8], bit: ImageBit, value: bool) -> Result<(), RuntimeError> {
    if bit.bit > 7 {
        return Err(invalid("gpio bit index must be between 0 and 7"));
    }
    let Some(target) = image.get_mut(bit.byte) else {
        return Err(invalid("gpio input mapping outside the image"));
    };
    if value {
        *target |= 1 << bit.bit;
    } else {
        *target &= !(1 << bit.bit);
    }
    Ok(())
}

/// A driver error with the message `message`.
fn invalid(message: &str) -> RuntimeError {
    RuntimeError::InvalidConfig(message.into())
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;
    use alloc::vec;
    use core::cell::RefCell;

    use smol_str::SmolStr;
    use trust_runtime_core::deployment::{Configuration, GpioInput, IoDriver};
    use trust_runtime_core::io_address::IoAddress;

    use super::*;

    /// The levels a test's pins read and the level each output was last driven to. An input handle is
    /// an index into `levels`, an output handle an index into the shared `driven` table, which the
    /// test keeps a clone of to read what the image drove.
    struct FakePins {
        levels: Vec<bool>,
        driven: Rc<RefCell<Vec<Option<bool>>>>,
    }

    impl GpioPins for FakePins {
        type Input = usize;
        type Output = usize;

        fn is_high(&self, input: &usize) -> bool {
            self.levels[*input]
        }

        fn drive(&mut self, output: &mut usize, high: bool) {
            self.driven.borrow_mut()[*output] = Some(high);
        }
    }

    /// The message of the invalid-configuration refusal `result` holds; any other result fails the test.
    fn refusal_of(result: Result<(), RuntimeError>) -> SmolStr {
        match result {
            Err(RuntimeError::InvalidConfig(message)) => message,
            other => panic!("expected an invalid-configuration refusal, got {other:?}"),
        }
    }

    /// Pins with the given input levels and `outputs` outputs that have not been driven yet, and the
    /// shared table of what the outputs were driven to.
    fn pins(levels: Vec<bool>, outputs: usize) -> (FakePins, Rc<RefCell<Vec<Option<bool>>>>) {
        let driven = Rc::new(RefCell::new(vec![None; outputs]));
        let pins = FakePins {
            levels,
            driven: Rc::clone(&driven),
        };
        (pins, driven)
    }

    /// Specification 34, TARGET-08: an output's safe value is its `[[io.safe_state]]` entry when that is
    /// a boolean, and false otherwise. `%QX0.1` is listed true, `%QX0.2` false and `%QX0.3` with a
    /// number; `%QX0.4` is not listed. Catches a lookup that matches another address, a non-boolean
    /// entry read as true, and an unlisted output that is not de-energized.
    #[test]
    fn an_outputs_safe_value_is_its_boolean_safe_state_entry_or_false() {
        use trust_runtime_core::deployment::SafeStateEntry;
        use trust_runtime_core::value::Value;
        let address = |text: &str| IoAddress::parse(text).expect("an output address");
        let config = Configuration {
            safe_state: vec![
                SafeStateEntry {
                    address: address("%QX0.1"),
                    value: Value::Bool(true),
                },
                SafeStateEntry {
                    address: address("%QX0.2"),
                    value: Value::Bool(false),
                },
                SafeStateEntry {
                    address: address("%QX0.3"),
                    value: Value::Int(1),
                },
            ],
            ..Configuration::default()
        };
        assert!(safe_value(&config, &address("%QX0.1")));
        assert!(!safe_value(&config, &address("%QX0.2")));
        assert!(
            !safe_value(&config, &address("%QX0.3")),
            "a non-boolean entry is not true"
        );
        assert!(
            !safe_value(&config, &address("%QX0.4")),
            "an unlisted output is de-energized"
        );
    }

    /// An inverted input reads the opposite of its pin: a high pin on an inverted line clears its bit,
    /// and a low pin sets it (`io.toml` `invert`). Catches an inversion that is ignored, and one that
    /// is applied to the pin twice.
    #[test]
    fn an_inverted_input_line_reads_the_opposite_of_its_pin() {
        let line = ImageBit {
            byte: 0,
            bit: 3,
            invert: true,
        };
        let (high_pins, _) = pins(vec![true], 0);
        let mut high = ProcessImage::new(high_pins, vec![(0, line)], Vec::new());
        let mut image = [0xFF_u8];
        high.read_inputs(&mut image).expect("the image holds bit 3");
        assert_eq!(image, [0xF7], "a high pin on an inverted line clears bit 3");

        let (low_pins, _) = pins(vec![false], 0);
        let mut low = ProcessImage::new(low_pins, vec![(0, line)], Vec::new());
        let mut image = [0x00_u8];
        low.read_inputs(&mut image).expect("the image holds bit 3");
        assert_eq!(image, [0x08], "a low pin on an inverted line sets bit 3");
    }

    /// An input line writes only its own bit and leaves the other bits of its byte as they are. Catches
    /// an input that clears or rewrites the whole byte.
    #[test]
    fn an_input_line_writes_only_its_own_bit() {
        let line = ImageBit {
            byte: 0,
            bit: 0,
            invert: false,
        };
        let (pins, _) = pins(vec![true], 0);
        let mut image_of = ProcessImage::new(pins, vec![(0, line)], Vec::new());
        let mut image = [0xAA_u8];
        image_of
            .read_inputs(&mut image)
            .expect("the image holds bit 0");
        assert_eq!(image, [0xAB], "a high pin sets bit 0 and keeps bits 1 to 7");
    }

    /// An output drives its pin to its image bit: a set bit drives high and a clear bit drives low.
    /// Catches an output that reads a neighbouring bit or the wrong byte.
    #[test]
    fn an_output_line_drives_its_pin_to_its_image_bit() {
        let line = ImageBit {
            byte: 0,
            bit: 2,
            invert: false,
        };
        let (set_pins, set_driven) = pins(Vec::new(), 1);
        let mut set = ProcessImage::new(set_pins, Vec::new(), vec![(0, line)]);
        set.write_outputs(&[0x04]).expect("the image holds bit 2");
        assert_eq!(*set_driven.borrow(), vec![Some(true)]);

        let (clear_pins, clear_driven) = pins(Vec::new(), 1);
        let mut clear = ProcessImage::new(clear_pins, Vec::new(), vec![(0, line)]);
        clear.write_outputs(&[0xFB]).expect("the image holds bit 2");
        assert_eq!(*clear_driven.borrow(), vec![Some(false)]);
    }

    /// An inverted output drives the opposite of its image bit: a set bit drives its pin low. Catches an
    /// inversion that is ignored on an output.
    #[test]
    fn an_inverted_output_line_drives_the_opposite_of_its_image_bit() {
        let line = ImageBit {
            byte: 0,
            bit: 0,
            invert: true,
        };
        let (pins, driven) = pins(Vec::new(), 1);
        let mut image_of = ProcessImage::new(pins, Vec::new(), vec![(0, line)]);
        image_of
            .write_outputs(&[0x01])
            .expect("the image holds bit 0");
        assert_eq!(*driven.borrow(), vec![Some(false)]);
    }

    /// STATE-04: a safe write attempts every output even when one image bit cannot be read, and reports
    /// the first failure. Catches a write that stops at the unreadable line, which would leave the last
    /// output undriven, and one that reports the last failure instead of the first.
    #[test]
    fn a_safe_write_drives_every_output_and_reports_the_first_unreadable_bit() {
        let (pins, driven) = pins(Vec::new(), 3);
        let mut image_of = ProcessImage::new(
            pins,
            Vec::new(),
            vec![
                (
                    0,
                    ImageBit {
                        byte: 0,
                        bit: 0,
                        invert: false,
                    },
                ),
                (
                    1,
                    ImageBit {
                        byte: 5,
                        bit: 0,
                        invert: false,
                    },
                ),
                (
                    2,
                    ImageBit {
                        byte: 0,
                        bit: 1,
                        invert: false,
                    },
                ),
            ],
        );
        let result = image_of.write_safe_outputs(&[0b0000_0010]);
        assert_eq!(
            *driven.borrow(),
            vec![Some(false), None, Some(true)],
            "the readable outputs are driven, and the unreadable one is not"
        );
        let message = refusal_of(result);
        assert_eq!(
            message.as_str(),
            "gpio output mapping outside the image",
            "the first failure is the unreadable bit's refusal"
        );
    }

    /// A bit index above 7 is refused with its message, and nothing is driven. Catches a shift that
    /// silently moves the bit into another byte.
    #[test]
    fn a_bit_index_above_seven_is_refused_with_its_message() {
        let (pins, driven) = pins(Vec::new(), 1);
        let mut image_of = ProcessImage::new(
            pins,
            Vec::new(),
            vec![(
                0,
                ImageBit {
                    byte: 0,
                    bit: 8,
                    invert: false,
                },
            )],
        );
        let result = image_of.write_outputs(&[0xFF]);
        assert_eq!(
            *driven.borrow(),
            vec![None],
            "the refused line drives nothing"
        );
        let message = refusal_of(result);
        assert_eq!(message.as_str(), "gpio bit index must be between 0 and 7");
    }

    /// The process image is one `gpio` driver, and it reports itself healthy. Catches a driver under
    /// another name, which `status` would list wrongly.
    #[test]
    fn the_process_image_reports_one_healthy_gpio_driver() {
        let (pins, _) = pins(Vec::new(), 0);
        let image_of = ProcessImage::new(pins, Vec::new(), Vec::new());
        let drivers = image_of.driver_health();
        assert_eq!(drivers.len(), 1);
        assert_eq!(drivers[0].name.as_str(), "gpio");
        assert_eq!(drivers[0].health, IoDriverHealth::Ok);
    }

    /// The record's enabled `gpio` driver lines are taken, and a disabled driver of another name is
    /// skipped. Catches a disabled driver that is refused, and lines dropped from the enabled one.
    #[test]
    fn a_disabled_driver_of_another_name_is_skipped_and_the_gpio_lines_are_taken() {
        let mut config = Configuration::default();
        config.io_drivers.push(IoDriver {
            name: "modbus".into(),
            enabled: false,
            inputs: Vec::new(),
            outputs: Vec::new(),
        });
        config.io_drivers.push(IoDriver {
            name: "gpio".into(),
            enabled: true,
            inputs: vec![GpioInput {
                address: IoAddress::parse("%IX0.1").expect("a valid input address"),
                line: "GPIO18".into(),
                invert: false,
                debounce_ms: 0,
            }],
            outputs: Vec::new(),
        });
        let (inputs, outputs) = record_lines(&config, "test-chip").expect("gpio is carried out");
        assert_eq!(inputs.len(), 1, "the enabled gpio driver's one input line");
        assert_eq!(inputs[0].line.as_str(), "GPIO18");
        assert_eq!(outputs, Vec::new());
    }

    /// An enabled driver that is not `gpio` is refused with its name and the chip's name, which is the
    /// text `status` reports (specification 34, TARGET-01). Catches a refusal that names the wrong
    /// driver or chip, and one that lets the driver through.
    #[test]
    fn an_enabled_driver_of_another_name_is_refused_with_its_name_and_the_chip() {
        let mut config = Configuration::default();
        config.io_drivers.push(IoDriver {
            name: "modbus".into(),
            enabled: true,
            inputs: Vec::new(),
            outputs: Vec::new(),
        });
        let refusal = record_lines(&config, "test-chip").expect_err("modbus is not carried out");
        assert_eq!(
            refusal,
            "the installed application uses the modbus driver, which the test-chip does not carry out yet"
        );
    }
}
