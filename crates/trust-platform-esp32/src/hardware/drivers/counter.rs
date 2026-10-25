//! The `counter` driver on the C6 (specification 11 §6.5; specification 34 Appendix D.2): inputs on
//! the PCNT's four units, one unit for each input. A `pulse` input counts the edges of its line on
//! the unit's first channel; a `quadrature` input decodes an encoder's A and B on both of the unit's
//! channels, four counts per cycle of A. At the start of each cycle the driver reads each unit's
//! 16-bit count and adds the change since its last read to the input's 32-bit count (`WideCount`),
//! which it writes into its `%ID` double word as a `UDINT` (pulse) or a `DINT` (quadrature). The
//! units count with their limits, thresholds and glitch filters off, so the 16-bit count wraps.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use esp_hal::gpio::{AnyPin, Flex, InputConfig, Level};
use esp_hal::pcnt::channel::{CtrlMode, EdgeMode};
use esp_hal::pcnt::unit::Unit;
use esp_hal::pcnt::Pcnt;
use esp_hal::peripherals::PCNT;
use smol_str::SmolStr;
use trust_runtime_services::chip::process_io::ChipDriver;
use trust_runtime_services::io_drivers::counter::{CounterMode, CounterParams, Edge, WideCount};
use trust_runtime_services::status::IoDriverHealth;

use super::{health_of, refusal, write_image, Lease, PeripheralSlot};
use crate::process_io::{GpioBank, PinClaim};

/// The driver's name in `io.toml`.
pub(super) const NAME: &str = "counter";

/// The PCNT's units on the C6, one for each counter.
const UNITS: usize = 4;

/// The PCNT's four units, taken from the peripheral once at boot: `Pcnt::new` enables the PCNT's
/// clock and resets every unit (no limit, threshold or filter; both channels hold), and each unit
/// keeps the clock on for as long as the board keeps it.
pub(super) struct PulseUnits {
    unit0: Unit<'static, 0>,
    unit1: Unit<'static, 1>,
    unit2: Unit<'static, 2>,
    unit3: Unit<'static, 3>,
}

impl PulseUnits {
    /// The units of `pcnt`, reset.
    pub(super) fn new(pcnt: PCNT<'static>) -> Self {
        let pcnt = Pcnt::new(pcnt);
        Self {
            unit0: pcnt.unit0,
            unit1: pcnt.unit1,
            unit2: pcnt.unit2,
            unit3: pcnt.unit3,
        }
    }

    /// Unit `index`, 0 to 3.
    fn unit(&self, index: usize) -> Option<&dyn PulseUnit> {
        match index {
            0 => Some(&self.unit0),
            1 => Some(&self.unit1),
            2 => Some(&self.unit2),
            3 => Some(&self.unit3),
            _ => None,
        }
    }
}

/// A PCNT unit, whatever its number.
trait PulseUnit {
    /// Counts the `edge`s of `line` up on channel 0, from 0; channel 1 counts nothing.
    fn count_pulses(&self, line: &Flex<'static>, edge: Edge);

    /// Decodes an encoder's A on `a` and B on `b` from 0: channel 0 counts A's edges and channel 1
    /// B's, each up or down by the other line's level, so the count rises by one at each edge while
    /// A leads B and falls by one while B leads A, four counts per cycle of A.
    fn count_quadrature(&self, a: &Flex<'static>, b: &Flex<'static>);

    /// Stops the unit and connects its channels to a low level instead of their pins, so that the
    /// program loaded next finds it idle and the pins free of it.
    fn release(&self);

    /// The unit's 16-bit count now.
    fn raw(&self) -> i16;
}

impl<const N: usize> PulseUnit for Unit<'static, N> {
    fn count_pulses(&self, line: &Flex<'static>, edge: Edge) {
        // (falling edge, rising edge): the input modes take the falling edge first.
        let (falling, rising) = match edge {
            Edge::Rising => (EdgeMode::Hold, EdgeMode::Increment),
            Edge::Falling => (EdgeMode::Increment, EdgeMode::Hold),
            Edge::Both => (EdgeMode::Increment, EdgeMode::Increment),
        };
        self.pause();
        self.channel0.set_edge_signal(line.peripheral_input());
        self.channel0.set_ctrl_mode(CtrlMode::Keep, CtrlMode::Keep);
        self.channel0.set_input_mode(falling, rising);
        self.channel1.set_ctrl_mode(CtrlMode::Keep, CtrlMode::Keep);
        self.channel1.set_input_mode(EdgeMode::Hold, EdgeMode::Hold);
        self.clear();
        self.resume();
    }

    fn count_quadrature(&self, a: &Flex<'static>, b: &Flex<'static>) {
        // A leads B through (A, B) = (0, 0), (1, 0), (1, 1), (0, 1): A rises while B is low, B rises
        // while A is high, A falls while B is high and B falls while A is low, each +1. B leads A
        // through the same states backwards, each -1.
        self.pause();
        // Channel 0 counts A's edges: up on a rising A and down on a falling A while B is low,
        // reversed while B is high. The control modes take the low level first.
        self.channel0.set_edge_signal(a.peripheral_input());
        self.channel0.set_ctrl_signal(b.peripheral_input());
        self.channel0
            .set_input_mode(EdgeMode::Decrement, EdgeMode::Increment);
        self.channel0
            .set_ctrl_mode(CtrlMode::Keep, CtrlMode::Reverse);
        // Channel 1 counts B's edges: up on a falling B and down on a rising B while A is low,
        // reversed while A is high.
        self.channel1.set_edge_signal(b.peripheral_input());
        self.channel1.set_ctrl_signal(a.peripheral_input());
        self.channel1
            .set_input_mode(EdgeMode::Increment, EdgeMode::Decrement);
        self.channel1
            .set_ctrl_mode(CtrlMode::Keep, CtrlMode::Reverse);
        self.clear();
        self.resume();
    }

    fn release(&self) {
        self.pause();
        self.channel0.set_input_mode(EdgeMode::Hold, EdgeMode::Hold);
        self.channel0.set_ctrl_mode(CtrlMode::Keep, CtrlMode::Keep);
        self.channel1.set_input_mode(EdgeMode::Hold, EdgeMode::Hold);
        self.channel1.set_ctrl_mode(CtrlMode::Keep, CtrlMode::Keep);
        self.channel0.set_edge_signal(Level::Low);
        self.channel0.set_ctrl_signal(Level::Low);
        self.channel1.set_edge_signal(Level::Low);
        self.channel1.set_ctrl_signal(Level::Low);
        self.clear();
    }

    fn raw(&self) -> i16 {
        self.value()
    }
}

/// One input: its double word of the input image, its unit and its count, and its pins.
struct CounterLine {
    /// The first byte of the `%ID` double word.
    byte: usize,
    /// The double word's address, `%ID<byte>`, for an error.
    address: SmolStr,
    /// The input's PCNT unit.
    unit: usize,
    /// Whether the input decodes an encoder, whose count is a `DINT`; a pulse count is a `UDINT`.
    quadrature: bool,
    /// The 32-bit count over the unit's 16-bit one.
    count: WideCount,
    /// The input's pins, `line` and for an encoder `line_b`, as inputs, with their claims.
    _pins: Vec<(Flex<'static>, PinClaim)>,
}

/// The `counter` driver: its inputs on the PCNT.
struct CounterDriver {
    inputs: Vec<CounterLine>,
    health: IoDriverHealth,
    /// Declared last: every unit is released (`Drop`) and its pins dropped before the PCNT is free
    /// again.
    units: Lease<PulseUnits>,
}

impl Drop for CounterDriver {
    fn drop(&mut self) {
        let units = self.units.get();
        for input in &self.inputs {
            if let Some(unit) = units.unit(input.unit) {
                unit.release();
            }
        }
    }
}

/// Builds the `counter` driver of the parameters `params` (JSON): the PCNT is claimed from `pcnt`,
/// each input's pins from `bank`, and input `i` counts on unit `i` from 0.
///
/// # Errors
///
/// - the shared parameter refusal, `counter: <key>: <reason>`;
/// - `counter: inputs: the C6's PCNT has 4 counters, and the driver names <n> inputs`;
/// - `counter: inputs: the PCNT is already used by another counter driver`;
/// - `counter: inputs[<i>].line: <reason>` or `counter: inputs[<i>].line_b: <reason>` for a pin
///   that is not a pin name, that the chip lacks or keeps, or that another line holds, the encoder's
///   other line included.
pub(super) fn claim(
    params: &str,
    bank: &GpioBank,
    pcnt: &PeripheralSlot<PulseUnits>,
) -> Result<Box<dyn ChipDriver>, String> {
    let params = CounterParams::from_json(params).map_err(|error| format!("{NAME}: {error}"))?;
    if params.inputs.len() > UNITS {
        return Err(refusal(
            NAME,
            "inputs",
            format!(
                "the C6's PCNT has {UNITS} counters, and the driver names {} inputs",
                params.inputs.len()
            ),
        ));
    }
    let units = pcnt.claim().ok_or_else(|| {
        refusal(
            NAME,
            "inputs",
            "the PCNT is already used by another counter driver",
        )
    })?;
    // Every pin first, so that a refused pin leaves every unit as it was.
    let mut pins = Vec::with_capacity(params.inputs.len());
    for (index, input) in params.inputs.iter().enumerate() {
        let mut lines = vec![input_pin(
            bank,
            &input.line,
            &format!("inputs[{index}].line"),
        )?];
        if let CounterMode::Quadrature { line_b } = &input.mode {
            lines.push(input_pin(bank, line_b, &format!("inputs[{index}].line_b"))?);
        }
        pins.push(lines);
    }
    // The driver exists before the first unit counts, so a unit that counts is released if a
    // later one were refused.
    let mut driver = CounterDriver {
        inputs: Vec::with_capacity(pins.len()),
        health: IoDriverHealth::Ok,
        units,
    };
    for (index, (input, lines)) in params.inputs.iter().zip(pins).enumerate() {
        let Some(unit) = driver.units.get().unit(index) else {
            return Err(refusal(NAME, "inputs", "the C6's PCNT has no such counter"));
        };
        let quadrature = match (&input.mode, lines.as_slice()) {
            (CounterMode::Pulse { edge }, [(line, _)]) => {
                unit.count_pulses(line, *edge);
                false
            }
            (CounterMode::Quadrature { .. }, [(a, _), (b, _)]) => {
                unit.count_quadrature(a, b);
                true
            }
            _ => {
                return Err(refusal(
                    NAME,
                    "inputs",
                    "the input's lines do not match its mode",
                ))
            }
        };
        let count = WideCount::new(unit.raw());
        let byte = input.address.byte as usize;
        driver.inputs.push(CounterLine {
            byte,
            address: format!("%ID{byte}").into(),
            unit: index,
            quadrature,
            count,
            _pins: lines,
        });
    }
    Ok(Box::new(driver))
}

/// The pin `name` claimed from `bank` as an input without a pull, for a unit to count.
///
/// # Errors
///
/// `counter: <key>: <reason>` for a pin that is not a pin name, that the chip lacks or keeps, or
/// that another line holds.
fn input_pin(bank: &GpioBank, name: &str, key: &str) -> Result<(Flex<'static>, PinClaim), String> {
    let (_, pin, claim): (_, AnyPin<'static>, PinClaim) = bank
        .claim_pin(name)
        .map_err(|error| refusal(NAME, key, error))?;
    let mut input = Flex::new(pin);
    input.apply_input_config(&InputConfig::default());
    input.set_input_enable(true);
    Ok((input, claim))
}

impl ChipDriver for CounterDriver {
    fn name(&self) -> &str {
        NAME
    }

    /// Adds each unit's change since the last read to its input's count and writes the count into
    /// its `%ID` double word. Every input is written even when one double word lies outside the
    /// image, and the first failure is the error.
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), String> {
        let units = self.units.get();
        let mut result = Ok(());
        for input in &mut self.inputs {
            let Some(unit) = units.unit(input.unit) else {
                continue;
            };
            let total = input.count.update(unit.raw());
            let bytes = if input.quadrature {
                input.count.signed().to_le_bytes()
            } else {
                total.to_le_bytes()
            };
            let written = write_image(image, input.byte, &bytes, &input.address);
            if result.is_ok() {
                result = written;
            }
        }
        self.health = health_of(&result);
        result
    }

    /// The counter driver has no outputs.
    fn write_outputs(&mut self, _image: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }
}
