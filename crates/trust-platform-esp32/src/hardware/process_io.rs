//! The GPIO process image of the installed program (design §1.2 and specification 34, §7.2,
//! TARGET-01 and STATE-04): one input or output line for each `io.toml` `gpio` line, named by a pin
//! (`GPIO18`) and a bit of the flat process image, with its invert and, for an output, its initial
//! value.
//!
//! The lines are plain data the installed-application record fills; this module checks the pin against
//! the chip's file and claims it. The image itself is the shared one of
//! `trust_runtime_services::chip::process_io`: `Esp32Pins` drives the esp-hal pins behind the crate's
//! riscv32 gate, so the pin check and the inversion run in `tests/pin_lines.rs` without hardware.
//!
//! A program that is loaded in place (specification 34 STATE-08) claims its lines after the program
//! before it released them. A line holds a [`PinClaim`] with its pin, and the claim frees the pin when
//! the line is dropped, with the program's cycle, so the next program claims the same pin again. The
//! claims are the host-testable half; the esp-hal pins are the riscv32 half.

use alloc::rc::Rc;
use core::cell::Cell;
use core::fmt;
use core::sync::atomic::{AtomicU32, Ordering};

use trust_runtime_services::chip::process_io::ImageBit;

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
    /// The line's bit of the flat input image, with its inversion.
    #[must_use]
    pub const fn image_bit(self) -> ImageBit {
        ImageBit {
            byte: self.byte,
            bit: self.bit,
            invert: self.invert,
        }
    }

    /// The logical input value of the pin's physical level `high`; an inverted line flips it.
    #[must_use]
    pub const fn level(self, high: bool) -> bool {
        self.image_bit().value(high)
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
    /// The logical safe value: the output's `[[io.safe_state]]` entry, false without one
    /// (specification 34, TARGET-08).
    pub safe: bool,
}

impl OutputLine {
    /// The line's bit of the flat output image, with its inversion.
    #[must_use]
    pub const fn image_bit(self) -> ImageBit {
        ImageBit {
            byte: self.byte,
            bit: self.bit,
            invert: self.invert,
        }
    }

    /// The pin's physical level for the logical output value `value`; an inverted line flips it.
    #[must_use]
    pub const fn level(self, value: bool) -> bool {
        self.image_bit().level(value)
    }

    /// The pin's physical level in the output's safe state: its safe value, with its inversion.
    #[must_use]
    pub const fn safe_level(self) -> bool {
        self.level(self.safe)
    }
}

/// The safe levels of the configured outputs, which the terminal fault path drives before the chip
/// resets (specification 34, TARGET-08): one bit per GPIO number for each output, and one for each
/// output whose safe level is high. An output is recorded once it has become an output, so the
/// fault path drives only pins it can drive.
#[derive(Debug, Default)]
pub struct SafeLevels {
    outputs: AtomicU32,
    high: AtomicU32,
}

impl SafeLevels {
    /// No output recorded.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            outputs: AtomicU32::new(0),
            high: AtomicU32::new(0),
        }
    }

    /// Records `pin` as an output whose safe level is high when `high`, and low otherwise.
    pub fn record(&self, pin: GpioPin, high: bool) {
        let bit = 1 << pin.number();
        if high {
            self.high.fetch_or(bit, Ordering::Relaxed);
        } else {
            self.high.fetch_and(!bit, Ordering::Relaxed);
        }
        self.outputs.fetch_or(bit, Ordering::Relaxed);
    }

    /// Forgets `pin`: the line that held it was dropped with its program (specification 34 STATE-08),
    /// so the fault path no longer drives it, and a pin the next program claims is recorded again.
    pub fn forget(&self, pin: GpioPin) {
        let bit = 1 << pin.number();
        self.outputs.fetch_and(!bit, Ordering::Relaxed);
        self.high.fetch_and(!bit, Ordering::Relaxed);
    }

    /// The bank-0 masks of the recorded outputs: the pins to drive high, and the pins to drive low.
    #[must_use]
    pub fn masks(&self) -> (u32, u32) {
        let outputs = self.outputs.load(Ordering::Relaxed);
        let high = self.high.load(Ordering::Relaxed) & outputs;
        (high, outputs & !high)
    }
}

/// The pins that lines hold now, one bit for each pin number. A line claims its pin here when the
/// program that owns it is loaded, and a pin that is held cannot be claimed again until its line is
/// dropped (specification 34 STATE-08, TARGET-01).
#[derive(Debug, Clone, Default)]
pub struct PinClaims(Rc<Cell<u32>>);

impl PinClaims {
    /// Claims `pin` for a line.
    ///
    /// # Errors
    ///
    /// [`PinError::Claimed`] when a line holds the pin, and [`PinError::NoSuchPin`] for a number
    /// above 31, which the one-word set cannot hold.
    pub fn claim(&self, pin: GpioPin) -> Result<PinClaim, PinError> {
        let bit = 1_u32
            .checked_shl(u32::from(pin.number()))
            .ok_or(PinError::NoSuchPin(pin))?;
        if self.0.get() & bit != 0 {
            return Err(PinError::Claimed(pin));
        }
        self.0.set(self.0.get() | bit);
        Ok(PinClaim {
            claims: self.clone(),
            bit,
        })
    }

    /// Whether a line holds `pin` now.
    #[must_use]
    pub fn holds(&self, pin: GpioPin) -> bool {
        1_u32
            .checked_shl(u32::from(pin.number()))
            .is_some_and(|bit| self.0.get() & bit != 0)
    }
}

/// A pin held by a line. Dropping it frees the pin, so the program that is loaded next can claim it.
#[derive(Debug)]
pub struct PinClaim {
    claims: PinClaims,
    bit: u32,
}

impl Drop for PinClaim {
    fn drop(&mut self) {
        self.claims.0.set(self.claims.0.get() & !self.bit);
    }
}

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
mod gpio {
    use alloc::vec::Vec;

    use esp_hal::gpio::{AnyPin, Flex, Input, InputConfig, Level};
    use trust_runtime_core::deployment::{Configuration, GpioInput, GpioOutput};
    use trust_runtime_services::chip::process_io::{safe_value, GpioPins, ProcessImage};

    use super::{
        check_pin, GpioPin, InputLine, OutputLine, PinClaim, PinClaims, PinError, CHIP_GPIO_PINS,
        CHIP_GPIO_RESERVED,
    };

    /// The chip's GPIO bank, one entry per pin, taken once from the HAL's `Peripherals`. A line claims
    /// its pin by name, so no board's pins are built in (specification 34, ARCH-10 and TARGET-01). The
    /// bank keeps every pin for the life of the boot and lends one to the line that claims it, so a
    /// program loaded later claims the pins the program before it released (specification 34 STATE-08).
    pub struct GpioBank {
        pins: [Option<AnyPin<'static>>; CHIP_GPIO_PINS as usize],
        claims: PinClaims,
    }

    impl GpioBank {
        /// The bank of every pin of the chip, in pin order.
        #[must_use]
        pub fn new(pins: [Option<AnyPin<'static>>; CHIP_GPIO_PINS as usize]) -> Self {
            Self {
                pins,
                claims: PinClaims::default(),
            }
        }

        /// A copy of `pin` for a line to drive, and the claim that holds it. The claim is refused while
        /// another line holds the pin.
        fn claim(&self, pin: GpioPin) -> Result<(AnyPin<'static>, PinClaim), PinError> {
            let claim = self.claims.claim(pin)?;
            let owned = self
                .pins
                .get(usize::from(pin.number()))
                .and_then(Option::as_ref)
                .ok_or(PinError::NoSuchPin(pin))?;
            // SAFETY: `PinClaims::claim` above refused the pin if a line held it, and a line keeps
            // its `PinClaim` next to the driver built from this copy (`ClaimedInput` and
            // `ClaimedOutput` own both and drop them together), so the claim is released only when
            // that driver is gone. At most one driver of the pin exists at a time, which is what
            // `clone_unchecked` asks of its caller.
            let copy = unsafe { owned.clone_unchecked() };
            Ok((copy, claim))
        }
    }

    /// A claimed input line: its esp-hal driver and the claim on its pin.
    pub struct ClaimedInput {
        pin: Input<'static>,
        _claim: PinClaim,
    }

    /// A claimed output line: its esp-hal driver and the claim on its pin. It is registered with the
    /// terminal fault path, and forgotten there when it is dropped.
    pub struct ClaimedOutput {
        pin: Flex<'static>,
        number: GpioPin,
        _claim: PinClaim,
    }

    impl Drop for ClaimedOutput {
        fn drop(&mut self) {
            // The fault path drives only the outputs of the program that runs (TARGET-08).
            crate::hardware::emergency::forget_output(self.number);
        }
    }

    /// The claimed GPIO lines of the installed application: the input pins with their image lines, and
    /// the output pins with theirs, in `io.toml` order.
    pub type ClaimedLines = (
        Vec<(ClaimedInput, InputLine)>,
        Vec<(ClaimedOutput, OutputLine)>,
    );

    /// Claims the record's GPIO lines by pin name, in `io.toml` order: each pin is checked against
    /// the chip's file and taken from the bank, and an output is driven to its initial level
    /// before its driver is enabled (specification 34, STATE-01 and TARGET-01). Each output's safe
    /// level, its `[[io.safe_state]]` value in `config`, is recorded for the terminal fault path
    /// (TARGET-08). A line that fails releases the pins of the lines before it.
    ///
    /// # Errors
    ///
    /// The [`PinError`] of a line that is not a pin name, a pin the chip lacks or keeps, or one
    /// pin named by two lines.
    pub fn claim_lines(
        bank: &GpioBank,
        inputs: &[GpioInput],
        outputs: &[GpioOutput],
        config: &Configuration,
    ) -> Result<ClaimedLines, PinError> {
        let reserved = CHIP_GPIO_RESERVED.map(GpioPin::from_number);
        let mut input_lines = Vec::with_capacity(inputs.len());
        for input in inputs {
            let pin = check_pin(&input.line, CHIP_GPIO_PINS, &reserved)?;
            let (any, claim) = bank.claim(pin)?;
            input_lines.push((
                ClaimedInput {
                    pin: Input::new(any, InputConfig::default()),
                    _claim: claim,
                },
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
            let (any, claim) = bank.claim(pin)?;
            let line = OutputLine {
                byte: output.address.byte as usize,
                bit: output.address.bit,
                pin,
                invert: output.invert,
                initial: output.initial,
                safe: safe_value(config, &output.address),
            };
            let mut handle = Flex::new(any);
            handle.set_level(level(line.level(line.initial)));
            handle.set_output_enable(true);
            // The terminal fault path drives this pin to its safe level (specification 34,
            // TARGET-08; design §1.1, R4).
            crate::hardware::emergency::register_output(pin, line.safe_level());
            output_lines.push((
                ClaimedOutput {
                    pin: handle,
                    number: pin,
                    _claim: claim,
                },
                line,
            ));
        }
        Ok((input_lines, output_lines))
    }

    /// The esp-hal pins behind the shared process image: an input is read, and an output is driven,
    /// through the handle its claim returned (specification 34, TARGET-01).
    pub struct Esp32Pins;

    impl GpioPins for Esp32Pins {
        type Input = ClaimedInput;
        type Output = ClaimedOutput;

        fn is_high(&self, input: &ClaimedInput) -> bool {
            input.pin.is_high()
        }

        fn drive(&mut self, output: &mut ClaimedOutput, high: bool) {
            output.pin.set_level(level(high));
        }
    }

    /// The installed program's GPIO process image over the esp-hal pins, one entry per configured line
    /// in `io.toml` order.
    pub type GpioIo = ProcessImage<Esp32Pins>;

    /// Binds the claimed lines to the shared process image. Each output is driven to its initial level
    /// before the program runs, as the claim left it (specification 34, STATE-01).
    #[must_use]
    pub fn bind_lines(
        inputs: Vec<(ClaimedInput, InputLine)>,
        mut outputs: Vec<(ClaimedOutput, OutputLine)>,
    ) -> GpioIo {
        for (claimed, line) in &mut outputs {
            claimed.pin.set_level(level(line.level(line.initial)));
        }
        ProcessImage::new(
            Esp32Pins,
            inputs
                .into_iter()
                .map(|(pin, line)| (pin, line.image_bit()))
                .collect(),
            outputs
                .into_iter()
                .map(|(pin, line)| (pin, line.image_bit()))
                .collect(),
        )
    }

    /// The pin level of a logical value.
    fn level(value: bool) -> Level {
        if value {
            Level::High
        } else {
            Level::Low
        }
    }
}

#[cfg(all(target_arch = "riscv32", target_os = "none"))]
pub use gpio::{
    bind_lines, claim_lines, ClaimedInput, ClaimedLines, ClaimedOutput, Esp32Pins, GpioBank, GpioIo,
};
