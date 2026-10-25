//! The process image of the installed program over a chip's pins and peripherals (specification 34,
//! §7.2, TARGET-01, STATE-01 and STATE-04). A chip claims the record's GPIO lines by pin name,
//! latches each output to its level, and gives the claimed pins to [`ProcessImage`] behind
//! [`GpioPins`]; it claims the peripherals of the record's other drivers (`analog`, `pwm`,
//! `counter`, `can`; specification 11 §6.5) and gives each to the image as a [`ChipDriver`]. The
//! image reads the inputs into the flat input image and drives the outputs from the flat output
//! image: the GPIO lines first, each with its `io.toml` inversion, then the drivers in `io.toml`
//! order. The claims stay with each chip, because the chips check their pins and peripherals and
//! latch their outputs differently.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::deployment::{Configuration, GpioInput, GpioOutput};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::value::Value;

use crate::io_drivers::{entry_bytes, EntryBytes, OnError};
use crate::runner::ProcessIo;
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

/// One I/O driver of a chip beside its GPIO lines, such as `analog`, `pwm`, `counter`, `can`,
/// `modbus-tcp` or `mqtt` (specification 11 §6.5), over the peripherals, pins and connections the
/// chip claimed for it. The process image calls it at each cycle after the GPIO lines, and drops
/// it, which frees what it claimed, when the program that holds it ends.
pub trait ChipDriver {
    /// The driver's name, as `io.toml` names it.
    fn name(&self) -> &str;

    /// Reads the driver's inputs into `image`, the flat input image, at the start of a cycle.
    ///
    /// # Errors
    ///
    /// What failed, after the driver's name and of its kind, as a computer's driver reports it
    /// (`IoDriver`, `IoTransport`, `IoAddress` or `IoFreshness`); the driver's `on_error` decides
    /// what the error does to the resource.
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError>;

    /// Writes the driver's outputs from `image`, the flat output image, after the tasks ran; also the
    /// safe state's image when the resource goes to its safe state.
    ///
    /// # Errors
    ///
    /// What failed, as for [`Self::read_inputs`].
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError>;

    /// The driver's health now: degraded or faulted with the reason of its last failure, and ok again
    /// after an exchange that succeeds.
    fn health(&self) -> IoDriverHealth;

    /// The health of the driver's output path, which confirms a safe-state write (specification 11,
    /// safe-state handoff confirmation): the driver's health, unless the driver tells its input
    /// path's conditions apart, which never unconfirm a safe state.
    fn output_health(&self) -> IoDriverHealth {
        self.health()
    }

    /// What an error of the driver does to the resource; `fault` unless its parameters say
    /// otherwise.
    fn on_error(&self) -> OnError {
        OnError::Fault
    }

    /// The most heap the driver may still take while it runs, beside what its claim took, for an
    /// output image of `output_image_bytes`: the chip leaves room for it when it loads the program
    /// (specification 34 MEM-12); none unless the driver says otherwise.
    fn reserved_bytes(&self, output_image_bytes: usize) -> usize {
        let _ = output_image_bytes;
        0
    }
}

/// The installed program's process image over a chip's claimed pins: one entry per `io.toml` line,
/// in `io.toml` order, each with its bit of the flat image, and then the chip's other drivers.
pub struct ProcessImage<P: GpioPins> {
    pins: P,
    inputs: Vec<(P::Input, ImageBit)>,
    outputs: Vec<(P::Output, ImageBit)>,
    drivers: Vec<alloc::boxed::Box<dyn ChipDriver>>,
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
            drivers: Vec::new(),
        }
    }

    /// The image with `drivers`, the chip's other drivers in `io.toml` order, after its GPIO lines.
    #[must_use]
    pub fn with_drivers(mut self, drivers: Vec<alloc::boxed::Box<dyn ChipDriver>>) -> Self {
        self.drivers = drivers;
        self
    }
}

/// What a driver's error does at a cycle boundary: with `fault` it is the boundary's error, so the
/// resource faults; with `warn` or `ignore` the resource runs on, and the driver's own health says
/// what happened (specification 11 §6.5).
fn driver_result(
    driver: &dyn ChipDriver,
    result: Result<(), RuntimeError>,
) -> Result<(), RuntimeError> {
    match (result, driver.on_error()) {
        (Ok(()), _) | (Err(_), OnError::Warn | OnError::Ignore) => Ok(()),
        (Err(error), OnError::Fault) => Err(error),
    }
}

impl<P: GpioPins> ProcessIo for ProcessImage<P> {
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        for (pin, bit) in &self.inputs {
            let high = self.pins.is_high(pin);
            write_bit(image, *bit, bit.value(high))?;
        }
        for driver in &mut self.drivers {
            let result = driver.read_inputs(image);
            driver_result(driver.as_ref(), result)?;
        }
        Ok(())
    }

    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        for (pin, bit) in &mut self.outputs {
            let value = read_bit(image, *bit)?;
            self.pins.drive(pin, bit.level(value));
        }
        for driver in &mut self.drivers {
            let result = driver.write_outputs(image);
            driver_result(driver.as_ref(), result)?;
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
        // Every driver is attempted too, whatever its `on_error`: the safe state is not a scan. A
        // driver's safe state is confirmed only by a write that succeeds and an output path that is
        // ok at once after it: a handoff still pending, or one under `warn`, is not confirmed
        // (specification 11, safe-state handoff confirmation).
        for driver in &mut self.drivers {
            let result = driver
                .write_outputs(image)
                .and_then(|()| match driver.output_health() {
                    IoDriverHealth::Ok => Ok(()),
                    IoDriverHealth::Degraded { error } | IoDriverHealth::Faulted { error } => {
                        Err(RuntimeError::IoDriver(
                            format!(
                                "{}: the safe state is not confirmed: {error}",
                                driver.name()
                            )
                            .into(),
                        ))
                    }
                });
            if let Err(error) = result
                && first.is_none()
            {
                first = Some(error);
            }
        }
        first.map_or(Ok(()), Err)
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        let mut statuses = vec![IoDriverStatus {
            name: "gpio".into(),
            health: IoDriverHealth::Ok,
        }];
        statuses.extend(self.drivers.iter().map(|driver| IoDriverStatus {
            name: driver.name().into(),
            health: driver.health(),
        }));
        statuses
    }

    /// The sum of the drivers' [`ChipDriver::reserved_bytes`].
    fn reserved_bytes(&self, output_image_bytes: usize) -> usize {
        self.drivers.iter().fold(0, |sum: usize, driver| {
            sum.saturating_add(driver.reserved_bytes(output_image_bytes))
        })
    }
}

/// The record's `gpio` driver lines, in `io.toml` order (specification 34, TARGET-01). A disabled
/// driver and a driver of another name are skipped: [`record_drivers`] gives the others.
#[must_use]
pub fn record_lines(config: &Configuration) -> (Vec<GpioInput>, Vec<GpioOutput>) {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for driver in config.io_drivers.iter().filter(|driver| driver.enabled) {
        if driver.name.as_str() == "gpio" {
            inputs.extend(driver.inputs.iter().cloned());
            outputs.extend(driver.outputs.iter().cloned());
        }
    }
    (inputs, outputs)
}

/// An enabled driver of the record other than `gpio`, with the JSON text of its parameters, for the
/// chip to claim (specification 34 §5.1.1, section 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverRequest<'a> {
    /// The driver's name, as `io.toml` names it.
    pub name: &'a str,
    /// Its `params` table as JSON.
    pub params: &'a str,
}

/// The record's enabled drivers other than `gpio`, in `io.toml` order, for the chip to claim; a
/// disabled driver is skipped.
///
/// # Errors
///
/// For a driver of a version-1 record, which carries no driver parameters, so that no driver but
/// `gpio` can run from it (specification 34 §5.1.1, section 8): `the installed application uses the
/// <name> driver: rebuild the project: its record holds no driver parameters`.
pub fn record_drivers(config: &Configuration) -> Result<Vec<DriverRequest<'_>>, String> {
    config
        .io_drivers
        .iter()
        .filter(|driver| driver.enabled && driver.name.as_str() != "gpio")
        .map(|driver| match &driver.params {
            Some(params) => Ok(DriverRequest {
                name: driver.name.as_str(),
                params,
            }),
            None => Err(format!(
                "the installed application uses the {} driver: rebuild the project: its record \
                 holds no driver parameters",
                driver.name
            )),
        })
        .collect()
}

/// The process-image bytes that the entries of the record's drivers other than `gpio` take
/// (specification 34 §11.3), which an online change counts as wired as it counts the lines.
///
/// # Errors
///
/// The refusal of [`record_drivers`], and `<name>: <key>: <reason>` for parameters a driver refuses.
pub fn record_entry_bytes(config: &Configuration) -> Result<Vec<EntryBytes>, String> {
    let mut entries = Vec::new();
    for request in record_drivers(config)? {
        if let Some(bytes) = entry_bytes(request.name, request.params) {
            entries.extend(bytes.map_err(|error| format!("{}: {error}", request.name))?);
        }
    }
    Ok(entries)
}

/// The refusal of a driver that `chip` does not carry out (specification 34 ARCH-13): `the installed
/// application uses the <name> driver, which the <chip> does not carry out`.
#[must_use]
pub fn unsupported_driver(name: &str, chip: &str) -> String {
    format!("the installed application uses the {name} driver, which the {chip} does not carry out")
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

    /// A chip driver of the tests: it writes its byte of the input image and copies its byte of the
    /// output image to `written`, or fails each exchange with `failure`, and its errors do what
    /// `on_error` says.
    struct FakeDriver {
        name: &'static str,
        byte: usize,
        value: u8,
        written: Rc<RefCell<Vec<u8>>>,
        failure: Option<&'static str>,
        on_error: OnError,
    }

    impl ChipDriver for FakeDriver {
        fn name(&self) -> &str {
            self.name
        }

        fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
            if let Some(failure) = self.failure {
                return Err(RuntimeError::IoTransport(
                    format!("{}: {failure}", self.name).into(),
                ));
            }
            image[self.byte] = self.value;
            Ok(())
        }

        fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
            if let Some(failure) = self.failure {
                return Err(RuntimeError::IoTransport(
                    format!("{}: {failure}", self.name).into(),
                ));
            }
            self.written.borrow_mut().push(image[self.byte]);
            Ok(())
        }

        fn health(&self) -> IoDriverHealth {
            match self.failure {
                Some(failure) => IoDriverHealth::Degraded {
                    error: failure.into(),
                },
                None => IoDriverHealth::Ok,
            }
        }

        fn on_error(&self) -> OnError {
            self.on_error
        }
    }

    /// The image runs its other drivers after the GPIO lines, in order, and lists their health after
    /// `gpio` (specification 11 §6.5). A failing driver whose `on_error` is `warn` lets the boundary
    /// succeed and the next driver run; one whose `on_error` is `fault` makes the boundary fail with
    /// the driver's own error, of its own kind (here `IoTransport("can: bus off")`). The safe state
    /// tries every driver, whatever its `on_error`, and reports the first failure. Catches drivers
    /// that are never called, a warning that stops the resource, a fault that is swallowed, and an
    /// error whose kind the image rewrites, which would turn a Modbus exception into another error.
    #[test]
    fn the_image_runs_its_other_drivers_after_the_gpio_lines() {
        let (gpio, _) = pins(Vec::new(), 0);
        let written = Rc::new(RefCell::new(Vec::new()));
        let driver = |name, byte, value, failure, on_error| -> alloc::boxed::Box<dyn ChipDriver> {
            alloc::boxed::Box::new(FakeDriver {
                name,
                byte,
                value,
                written: Rc::clone(&written),
                failure,
                on_error,
            })
        };
        let mut image = ProcessImage::new(gpio, Vec::new(), Vec::new()).with_drivers(vec![
            driver("analog", 0, 7, None, OnError::Fault),
            driver("can", 1, 0, Some("bus off"), OnError::Warn),
            driver("counter", 2, 9, None, OnError::Fault),
        ]);
        let mut inputs = [0u8; 3];
        assert_eq!(image.read_inputs(&mut inputs), Ok(()));
        assert_eq!(inputs, [7, 0, 9], "the drivers after the warned one ran");
        assert_eq!(image.write_outputs(&[4, 5, 6]), Ok(()));
        assert_eq!(*written.borrow(), vec![4, 6]);
        let names: Vec<_> = image
            .driver_health()
            .into_iter()
            .map(|status| (status.name, status.health))
            .collect();
        assert_eq!(
            names,
            vec![
                (SmolStr::new("gpio"), IoDriverHealth::Ok),
                (SmolStr::new("analog"), IoDriverHealth::Ok),
                (
                    SmolStr::new("can"),
                    IoDriverHealth::Degraded {
                        error: "bus off".into()
                    }
                ),
                (SmolStr::new("counter"), IoDriverHealth::Ok),
            ]
        );

        let (gpio, _) = pins(Vec::new(), 0);
        written.borrow_mut().clear();
        let mut faulting = ProcessImage::new(gpio, Vec::new(), Vec::new()).with_drivers(vec![
            driver("can", 0, 0, Some("bus off"), OnError::Fault),
            driver("pwm", 1, 0, None, OnError::Fault),
        ]);
        assert_eq!(
            faulting.read_inputs(&mut [0u8; 2]),
            Err(RuntimeError::IoTransport("can: bus off".into()))
        );
        assert_eq!(
            faulting.write_safe_outputs(&[0, 3]),
            Err(RuntimeError::IoTransport("can: bus off".into()))
        );
        assert_eq!(
            *written.borrow(),
            vec![3],
            "the safe state reached the next driver"
        );
    }

    /// A driver whose write succeeds while its output path is not ok: a handoff still pending.
    struct PendingDriver;

    impl ChipDriver for PendingDriver {
        fn name(&self) -> &'static str {
            "modbus-tcp"
        }

        fn read_inputs(&mut self, _image: &mut [u8]) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn write_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn health(&self) -> IoDriverHealth {
            IoDriverHealth::Degraded {
                error: "modbus output handoff pending".into(),
            }
        }
    }

    /// Specification 11, safe-state handoff confirmation: a driver's safe state is confirmed only by
    /// a write that succeeds and an output path that is ok at once after it. A driver whose write
    /// returns while its handoff is pending (degraded) leaves the safe state unconfirmed, with
    /// `modbus-tcp: the safe state is not confirmed: modbus output handoff pending`; its ordinary
    /// write still succeeds. Catches a chip that reports a safe state that never reached the device.
    #[test]
    fn a_safe_state_whose_handoff_is_pending_is_not_confirmed() {
        let (gpio, _) = pins(Vec::new(), 0);
        let mut image = ProcessImage::new(gpio, Vec::new(), Vec::new())
            .with_drivers(vec![alloc::boxed::Box::new(PendingDriver)]);
        assert_eq!(image.write_outputs(&[0]), Ok(()));
        assert_eq!(
            image.write_safe_outputs(&[0]),
            Err(RuntimeError::IoDriver(
                "modbus-tcp: the safe state is not confirmed: modbus output handoff pending".into()
            ))
        );
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
            params: Some("{}".into()),
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
            params: Some("{}".into()),
        });
        let (inputs, outputs) = record_lines(&config);
        assert_eq!(inputs.len(), 1, "the enabled gpio driver's one input line");
        assert_eq!(inputs[0].line.as_str(), "GPIO18");
        assert_eq!(outputs, Vec::new());
    }

    /// An enabled driver that is not `gpio` is given to the chip to claim with its parameters' text,
    /// and a chip that does not carry it out refuses it with its name and the chip's name, the text
    /// `status` reports (specification 34, TARGET-01 and ARCH-13). Catches a driver dropped on the
    /// way to the chip, and a refusal that names the wrong driver or chip.
    #[test]
    fn an_enabled_driver_of_another_name_goes_to_the_chip_with_its_parameters() {
        let mut config = Configuration::default();
        config.io_drivers.push(IoDriver {
            name: "modbus".into(),
            enabled: true,
            inputs: Vec::new(),
            outputs: Vec::new(),
            params: Some(r#"{"unit_id":1}"#.into()),
        });
        assert_eq!(
            record_drivers(&config),
            Ok(vec![DriverRequest {
                name: "modbus",
                params: r#"{"unit_id":1}"#,
            }])
        );
        assert_eq!(
            unsupported_driver("modbus", "test-chip"),
            "the installed application uses the modbus driver, which the test-chip does not carry out"
        );
    }

    /// A driver other than `gpio` from a version-1 record, which carries no driver parameters, is
    /// refused with the advice to rebuild (specification 34 §5.1.1, section 8). Catches a chip that
    /// runs a driver without its parameters, with defaults the project never chose.
    #[test]
    fn a_driver_of_a_version_1_record_asks_for_a_rebuild() {
        let mut config = Configuration::default();
        config.io_drivers.push(IoDriver {
            name: "modbus-tcp".into(),
            enabled: true,
            inputs: Vec::new(),
            outputs: Vec::new(),
            params: None,
        });
        assert_eq!(
            record_drivers(&config).expect_err("no parameters"),
            "the installed application uses the modbus-tcp driver: rebuild the project: its record \
             holds no driver parameters"
        );
    }
}
