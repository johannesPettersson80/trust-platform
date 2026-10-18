//! The GPIO process image of the installed program (design §1.2 and specification 34, §7.2,
//! TARGET-01 and STATE-04): one input or output line for each `io.toml` `gpio` line, named by a
//! pin (`GPIO18`) and a bit of the flat process image, with its invert and, for an output, its
//! initial value.
//!
//! The lines are plain data the installed-application record fills (a later slice); this module
//! checks the pin against the chip's file and inverts a level. `GpioIo` drives the esp-hal pins
//! behind the crate's riscv32 gate, so the pin check and the inversion run in `tests/pin_lines.rs`
//! without hardware.

use core::fmt;

/// A pin number of the esp32 GPIO family, printed `GPIO0` to `GPIO255`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpioPin(u8);

impl GpioPin {
    /// Reads a pin name `GPIO` followed by a decimal number without leading zeros, for example
    /// `GPIO18`, the form `io.toml` names a pin (specification 34, TARGET-01 and Appendix H).
    ///
    /// # Errors
    ///
    /// The `PinNameError` of a name that is not in that form.
    pub fn parse(name: &str) -> Result<Self, PinNameError> {
        let digits = name.strip_prefix("GPIO").ok_or(PinNameError::Syntax)?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(PinNameError::Syntax);
        }
        if digits.len() > 1 && digits.starts_with('0') {
            return Err(PinNameError::Number);
        }
        let number = digits.parse::<u8>().map_err(|_| PinNameError::Range)?;
        Ok(Self(number))
    }

    /// The pin number, 0 to 255.
    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }

    /// The pin with number `number`, for a number the caller already knows is a pin of the chip
    /// (the reserved list of a chip file, for example).
    #[must_use]
    pub const fn from_number(number: u8) -> Self {
        Self(number)
    }
}

impl fmt::Display for GpioPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GPIO{}", self.0)
    }
}

/// Why a text is not a `GPIO<n>` pin name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinNameError {
    /// The text is not `GPIO` followed by digits.
    Syntax,
    /// The pin number has a leading zero.
    Number,
    /// The pin number is above 255.
    Range,
}

impl fmt::Display for PinNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Syntax => "a GPIO pin name is GPIO and a decimal number, for example GPIO18",
            Self::Number => "the GPIO number must be decimal, without leading zeros",
            Self::Range => "the GPIO number must be 0 to 255",
        })
    }
}

/// Why a pin cannot be used as a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinError {
    /// The text is not a pin name.
    Name(PinNameError),
    /// The chip's file does not list the pin.
    NoSuchPin(GpioPin),
    /// The chip's file reserves the pin for the firmware.
    Reserved(GpioPin),
    /// Another line already claimed the pin.
    Claimed(GpioPin),
}

impl fmt::Display for PinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(error) => fmt::Display::fmt(error, f),
            Self::NoSuchPin(pin) => write!(f, "{pin} is not a pin of this chip"),
            Self::Reserved(pin) => write!(f, "{pin} is reserved by the firmware"),
            Self::Claimed(pin) => write!(f, "{pin} is already claimed by another line"),
        }
    }
}

/// Checks `name` against a chip's GPIO count and reserved pins: the chip has the pin (`number` is
/// below `pins`), and the firmware does not keep it (specification 34, TARGET-01 and Appendix H).
///
/// # Errors
///
/// The `PinError` of a text that is not a pin name, of a pin the chip does not list, or of a pin
/// the chip's file reserves.
pub fn check_pin(name: &str, pins: u32, reserved: &[GpioPin]) -> Result<GpioPin, PinError> {
    let pin = GpioPin::parse(name).map_err(PinError::Name)?;
    if u32::from(pin.number()) >= pins {
        return Err(PinError::NoSuchPin(pin));
    }
    if reserved.contains(&pin) {
        return Err(PinError::Reserved(pin));
    }
    Ok(pin)
}

/// The C6's GPIO count, from `chips/esp32c6.toml` `[gpio] pins`: GPIO0 to GPIO30. The firmware
/// cannot read the chip file, so the host test `pin_lines.rs` keeps this equal to it.
pub const CHIP_GPIO_PINS: u32 = 31;

/// The pins the C6's firmware keeps, from `chips/esp32c6.toml` `[gpio] reserved`: GPIO12 and
/// GPIO13 carry the USB-Serial/JTAG data lines. The host test `pin_lines.rs` keeps this equal to
/// the chip file.
pub const CHIP_GPIO_RESERVED: [u8; 2] = [12, 13];

/// One input line: the bit of the flat input image it fills, and the pin it samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputLine {
    /// Byte of the input image, from the line's `io.toml` address `%IX<byte>.<bit>`.
    pub byte: usize,
    /// Bit within `byte`, 0 to 7.
    pub bit: u8,
    /// The `GPIO<n>` pin.
    pub pin: GpioPin,
    /// Whether the pin's level is inverted (`io.toml` `invert`).
    pub invert: bool,
}

impl InputLine {
    /// The logical input value of the pin's physical level `high`; an inverted line flips it.
    #[must_use]
    pub const fn level(self, high: bool) -> bool {
        high != self.invert
    }
}

/// One output line: the bit of the flat output image it drives, its pin and invert, and the
/// initial level it is driven to before the first scan (specification 34, STATE-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputLine {
    /// Byte of the output image, from the line's `io.toml` address `%QX<byte>.<bit>`.
    pub byte: usize,
    /// Bit within `byte`, 0 to 7.
    pub bit: u8,
    /// The `GPIO<n>` pin.
    pub pin: GpioPin,
    /// Whether the pin's level is inverted (`io.toml` `invert`).
    pub invert: bool,
    /// The initial logical output value driven at boot (`io.toml` `initial`).
    pub initial: bool,
}

impl OutputLine {
    /// The pin's physical level for the logical output value `value`; an inverted line flips it.
    #[must_use]
    pub const fn level(self, value: bool) -> bool {
        value != self.invert
    }
}

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
mod gpio {
    use alloc::vec;
    use alloc::vec::Vec;

    use esp_hal::gpio::{AnyPin, Flex, Input, InputConfig, Level};
    use trust_runtime_core::deployment::{GpioInput, GpioOutput};
    use trust_runtime_core::error::RuntimeError;
    use trust_runtime_services::runner::engine::ProcessIo;
    use trust_runtime_services::status::{IoDriverHealth, IoDriverStatus};

    use super::{
        check_pin, GpioPin, InputLine, OutputLine, PinError, CHIP_GPIO_PINS, CHIP_GPIO_RESERVED,
    };

    /// The chip's GPIO bank, one entry per pin, taken once from the HAL's `Peripherals`. A line
    /// claims its pin by name, so no board's pins are built in (specification 34, ARCH-10 and
    /// TARGET-01).
    pub struct GpioBank {
        pins: [Option<AnyPin<'static>>; CHIP_GPIO_PINS as usize],
    }

    impl GpioBank {
        /// The bank of every pin of the chip, in pin order.
        #[must_use]
        pub fn new(pins: [Option<AnyPin<'static>>; CHIP_GPIO_PINS as usize]) -> Self {
            Self { pins }
        }

        /// The pin `pin`, or `None` when another line already claimed it.
        fn take(&mut self, pin: GpioPin) -> Option<AnyPin<'static>> {
            self.pins.get_mut(usize::from(pin.number()))?.take()
        }
    }

    /// The claimed GPIO lines of the installed application: the esp-hal input pins with their
    /// image lines, and the output pins with theirs, in `io.toml` order.
    pub type ClaimedLines = (
        Vec<(Input<'static>, InputLine)>,
        Vec<(Flex<'static>, OutputLine)>,
    );

    /// Claims the record's GPIO lines by pin name, in `io.toml` order: each pin is checked against
    /// the chip's file and taken from the bank, and an output is driven to its initial level
    /// before its driver is enabled (specification 34, STATE-01 and TARGET-01).
    ///
    /// # Errors
    ///
    /// The [`PinError`] of a line that is not a pin name, a pin the chip lacks or keeps, or one
    /// pin named by two lines.
    pub fn claim_lines(
        bank: &mut GpioBank,
        inputs: &[GpioInput],
        outputs: &[GpioOutput],
    ) -> Result<ClaimedLines, PinError> {
        let reserved = CHIP_GPIO_RESERVED.map(GpioPin::from_number);
        let mut input_lines = Vec::with_capacity(inputs.len());
        for input in inputs {
            let pin = check_pin(&input.line, CHIP_GPIO_PINS, &reserved)?;
            let any = bank.take(pin).ok_or(PinError::Claimed(pin))?;
            input_lines.push((
                Input::new(any, InputConfig::default()),
                InputLine {
                    byte: input.address.byte as usize,
                    bit: input.address.bit,
                    pin,
                    invert: input.invert,
                },
            ));
        }
        let mut output_lines = Vec::with_capacity(outputs.len());
        for output in outputs {
            let pin = check_pin(&output.line, CHIP_GPIO_PINS, &reserved)?;
            let any = bank.take(pin).ok_or(PinError::Claimed(pin))?;
            let line = OutputLine {
                byte: output.address.byte as usize,
                bit: output.address.bit,
                pin,
                invert: output.invert,
                initial: output.initial,
            };
            let mut handle = Flex::new(any);
            handle.set_level(level(line.level(line.initial)));
            handle.set_output_enable(true);
            // The terminal fault path drives this pin to its safe level (design §1.1, R4).
            crate::hardware::emergency::register_output(pin);
            output_lines.push((handle, line));
        }
        Ok((input_lines, output_lines))
    }

    /// The installed program's GPIO process image over esp-hal pins, one pin per configured line
    /// and in the same order.
    pub struct GpioIo {
        inputs: Vec<(Input<'static>, InputLine)>,
        outputs: Vec<(Flex<'static>, OutputLine)>,
    }

    impl GpioIo {
        /// Binds each configured line to its esp-hal pin, in `io.toml` order, and drives every
        /// output to its initial level before it becomes an output (specification 34, STATE-01).
        /// The caller has already configured each pin.
        #[must_use]
        pub fn new(
            inputs: Vec<(Input<'static>, InputLine)>,
            outputs: Vec<(Flex<'static>, OutputLine)>,
        ) -> Self {
            let mut this = Self { inputs, outputs };
            for (pin, line) in &mut this.outputs {
                pin.set_level(level(line.level(line.initial)));
            }
            this
        }
    }

    impl ProcessIo for GpioIo {
        fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
            for (pin, line) in &self.inputs {
                write_bit(image, line.byte, line.bit, line.level(pin.is_high()))?;
            }
            Ok(())
        }

        fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
            for (pin, line) in &mut self.outputs {
                let value = line.level(read_bit(image, line.byte, line.bit)?);
                pin.set_level(level(value));
            }
            Ok(())
        }

        fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
            // Specification 34, STATE-04: every output is attempted even when one fails, and the
            // first failure is reported. A GPIO write cannot fail; an unreadable image bit is a
            // mapping fault.
            let mut first = None;
            for (pin, line) in &mut self.outputs {
                match read_bit(image, line.byte, line.bit) {
                    Ok(raw) => pin.set_level(level(line.level(raw))),
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

    /// The pin level of a logical value.
    fn level(value: bool) -> Level {
        if value {
            Level::High
        } else {
            Level::Low
        }
    }

    /// Reads bit `bit` of byte `byte` in `image`.
    fn read_bit(image: &[u8], byte: usize, bit: u8) -> Result<bool, RuntimeError> {
        if bit > 7 {
            return Err(invalid("gpio bit index must be between 0 and 7"));
        }
        let Some(value) = image.get(byte) else {
            return Err(invalid("gpio output mapping outside the image"));
        };
        Ok(value & (1 << bit) != 0)
    }

    /// Writes `value` to bit `bit` of byte `byte` in `image`.
    fn write_bit(image: &mut [u8], byte: usize, bit: u8, value: bool) -> Result<(), RuntimeError> {
        if bit > 7 {
            return Err(invalid("gpio bit index must be between 0 and 7"));
        }
        let Some(target) = image.get_mut(byte) else {
            return Err(invalid("gpio input mapping outside the image"));
        };
        if value {
            *target |= 1 << bit;
        } else {
            *target &= !(1 << bit);
        }
        Ok(())
    }

    /// A driver error with the message `message`.
    fn invalid(message: &str) -> RuntimeError {
        RuntimeError::InvalidConfig(message.into())
    }
}

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
pub use gpio::{claim_lines, ClaimedLines, GpioBank, GpioIo};
