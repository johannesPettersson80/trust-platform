//! The `counter` driver on the C6 (specification 11 §6.5; specification 34 Appendix D.2): inputs on
//! the PCNT's four units, one unit for each input (`driver_plans::counter_plan`). A `pulse` input
//! counts the edges of its line on the unit's first channel; a `quadrature` input decodes an
//! encoder's A and B on both of the unit's channels, four counts per cycle of A. At the start of
//! each cycle the driver reads each unit's 16-bit count and adds the change since its last read to
//! the input's 32-bit count (`WideCount`), which it writes into its `%ID` double word as a `UDINT`
//! (pulse) or a `DINT` (quadrature). The units count with their limits, thresholds and glitch
//! filters off, so the 16-bit count wraps.
//!
//! Bench only, with the `bench-loopback` feature: an input's line may name the pin of a `pwm`
//! output of the same record listed before it. The unit then counts that output's pad through the
//! pad's input buffer, which the GPIO matrix routes to it, without claiming the pin, which the
//! output holds, so a PWM output is measured without a wire (`PinSource::pin`).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use esp_hal::gpio::interconnect::InputSignal;
use esp_hal::gpio::{AnyPin, Flex, InputConfig, Level};
use esp_hal::pcnt::channel::{Channel, CtrlMode, EdgeMode};
use esp_hal::pcnt::unit::Unit;
use esp_hal::pcnt::Pcnt;
use esp_hal::peripherals::PCNT;
use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_services::chip::process_io::ChipDriver;
use trust_runtime_services::io_drivers::counter::{CounterParams, WideCount};
use trust_runtime_services::status::IoDriverHealth;

use super::DriverPeripherals;
#[cfg(feature = "bench-loopback")]
use crate::driver_plans::LoopbackPads;
use crate::driver_plans::{
    self, counter_plan, driver_error, health_of, refusal, write_image, ChannelSetting,
    ControlAction, EdgeAction, Lease, UnitSetting,
};
#[cfg(feature = "bench-loopback")]
use crate::process_io::GpioPin;
use crate::process_io::{GpioBank, PinClaim};

/// The driver's name in `io.toml`.
pub(super) const NAME: &str = driver_plans::COUNTER;

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
    /// Counts from 0 with the modes of `setting`: channel 0 counts the edges of `a`, with `b` as
    /// its control signal, and channel 1 the edges of `b`, with `a` as its control signal. A pulse
    /// input has no `b`: channel 0 counts the edges of `a`, and channel 1, which holds, keeps its
    /// signals.
    fn count(&self, setting: UnitSetting, a: InputSignal<'static>, b: Option<InputSignal<'static>>);

    /// Stops the unit and connects its channels to a low level instead of their pins, so that the
    /// program loaded next finds it idle and the pins free of it.
    fn release(&self);

    /// The unit's 16-bit count now.
    fn raw(&self) -> i16;
}

impl<const N: usize> PulseUnit for Unit<'static, N> {
    fn count(
        &self,
        setting: UnitSetting,
        a: InputSignal<'static>,
        b: Option<InputSignal<'static>>,
    ) {
        self.pause();
        match b {
            Some(b) => {
                self.channel0.set_edge_signal(a.clone());
                self.channel0.set_ctrl_signal(b.clone());
                self.channel1.set_edge_signal(b);
                self.channel1.set_ctrl_signal(a);
            }
            None => {
                self.channel0.set_edge_signal(a);
            }
        }
        set_modes(&self.channel0, setting.channel0);
        set_modes(&self.channel1, setting.channel1);
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

/// Sets the modes of the PCNT channel `channel` to `setting`: the actions of a falling and a rising
/// edge of its signal (the input modes take the falling edge first), and what a low and a high
/// control level do to them (the control modes take the low level first).
fn set_modes<const UNIT: usize, const NUM: usize>(
    channel: &Channel<'_, UNIT, NUM>,
    setting: ChannelSetting,
) {
    channel.set_input_mode(edge_mode(setting.falling), edge_mode(setting.rising));
    channel.set_ctrl_mode(ctrl_mode(setting.low), ctrl_mode(setting.high));
}

/// The PCNT's mode of the edge action `action`.
fn edge_mode(action: EdgeAction) -> EdgeMode {
    match action {
        EdgeAction::Hold => EdgeMode::Hold,
        EdgeAction::Increment => EdgeMode::Increment,
        EdgeAction::Decrement => EdgeMode::Decrement,
    }
}

/// The PCNT's mode of the control action `action`.
fn ctrl_mode(action: ControlAction) -> CtrlMode {
    match action {
        ControlAction::Keep => CtrlMode::Keep,
        ControlAction::Reverse => CtrlMode::Reverse,
    }
}

/// A pin that a counter input counts.
enum CounterPin {
    /// A pin claimed from the GPIO bank as an input without a pull, and the claim that holds it.
    Claimed {
        pin: Flex<'static>,
        _claim: PinClaim,
    },
    /// Bench only (`bench-loopback`): the pad of a `pwm` output of the same record, read through
    /// the pad's input buffer; the output holds the pin.
    #[cfg(feature = "bench-loopback")]
    Pad(InputSignal<'static>),
}

impl CounterPin {
    /// The pad's input signal, for a PCNT channel; a claimed pin's input buffer is enabled.
    fn signal(&self) -> InputSignal<'static> {
        match self {
            Self::Claimed { pin, .. } => pin.peripheral_input(),
            #[cfg(feature = "bench-loopback")]
            Self::Pad(signal) => signal.clone(),
        }
    }
}

/// Where a counter input's pins come from: the GPIO bank and, with `bench-loopback`, the pads of
/// the running program's `pwm` outputs.
struct PinSource<'a> {
    bank: &'a GpioBank,
    /// Bench only (`bench-loopback`): the pads the `pwm` outputs published.
    #[cfg(feature = "bench-loopback")]
    pads: &'a LoopbackPads<InputSignal<'static>>,
}

impl PinSource<'_> {
    /// The pin `name` for a unit to count, claimed from the bank as an input without a pull. Bench
    /// only, with `bench-loopback`: when a `pwm` output of the same record drives the pin, its
    /// published pad, which the unit reads without claiming the pin.
    ///
    /// # Errors
    ///
    /// `counter: <key>: <reason>` for a pin that is not a pin name, that the chip lacks or keeps,
    /// or that another line holds.
    fn pin(&self, name: &str, key: &str) -> Result<CounterPin, String> {
        #[cfg(feature = "bench-loopback")]
        if let Some(signal) = GpioPin::parse(name)
            .ok()
            .and_then(|pin| self.pads.find(pin))
        {
            return Ok(CounterPin::Pad(signal));
        }
        let (_, pin, claim): (_, AnyPin<'static>, PinClaim) = self
            .bank
            .claim_pin(name)
            .map_err(|error| refusal(NAME, key, error))?;
        let mut input = Flex::new(pin);
        input.apply_input_config(&InputConfig::default());
        input.set_input_enable(true);
        Ok(CounterPin::Claimed {
            pin: input,
            _claim: claim,
        })
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
    /// The input's pins, `line` and for an encoder `line_b`.
    _pins: Vec<CounterPin>,
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

/// Builds the `counter` driver of the parameters `params` (JSON): the PCNT is claimed from
/// `peripherals`, each input's pins from `bank`, and input `i` counts on unit `i` from 0
/// (`driver_plans::counter_plan`).
///
/// # Errors
///
/// - the shared parameter refusal, `counter: <key>: <reason>`;
/// - the refusals of `driver_plans::counter_plan`, before any hardware is touched: more than 4
///   inputs, or a `quadrature` input without `line_b`;
/// - `counter: inputs: the PCNT is already used by another counter driver`;
/// - `counter: inputs[<i>].line: <reason>` or `counter: inputs[<i>].line_b: <reason>` for a pin
///   that is not a pin name, that the chip lacks or keeps, or that another line holds, the encoder's
///   other line included.
pub(super) fn claim(
    params: &str,
    bank: &GpioBank,
    peripherals: &DriverPeripherals,
) -> Result<Box<dyn ChipDriver>, String> {
    let params = CounterParams::from_json(params).map_err(|error| format!("{NAME}: {error}"))?;
    let plans = counter_plan(&params.inputs)?;
    let units = peripherals.pcnt.claim().ok_or_else(|| {
        refusal(
            NAME,
            "inputs",
            "the PCNT is already used by another counter driver",
        )
    })?;
    let source = PinSource {
        bank,
        #[cfg(feature = "bench-loopback")]
        pads: &peripherals.loopback,
    };
    // Every pin first, so that a refused pin leaves every unit as it was.
    let mut pins = Vec::with_capacity(plans.len());
    for (index, plan) in plans.iter().enumerate() {
        let mut lines = vec![source.pin(plan.line, &format!("inputs[{index}].line"))?];
        if let Some(line_b) = plan.line_b {
            lines.push(source.pin(line_b, &format!("inputs[{index}].line_b"))?);
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
    for ((plan, lines), input) in plans.iter().zip(pins).zip(&params.inputs) {
        let Some(unit) = driver.units.get().unit(plan.unit) else {
            return Err(refusal(NAME, "inputs", "the C6's PCNT has no such counter"));
        };
        match (plan.quadrature(), lines.as_slice()) {
            (false, [line]) => unit.count(plan.setting, line.signal(), None),
            (true, [a, b]) => unit.count(plan.setting, a.signal(), Some(b.signal())),
            _ => {
                return Err(refusal(
                    NAME,
                    "inputs",
                    "the input's lines do not match its mode",
                ))
            }
        }
        let count = WideCount::new(unit.raw());
        let byte = input.address.byte as usize;
        driver.inputs.push(CounterLine {
            byte,
            address: format!("%ID{byte}").into(),
            unit: plan.unit,
            quadrature: plan.quadrature(),
            count,
            _pins: lines,
        });
    }
    Ok(Box::new(driver))
}

impl ChipDriver for CounterDriver {
    fn name(&self) -> &str {
        NAME
    }

    /// Adds each unit's change since the last read to its input's count and writes the count into
    /// its `%ID` double word. Every input is written even when one double word lies outside the
    /// image, and the first failure is the error.
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
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
        result.map_err(|error| driver_error(NAME, error))
    }

    /// The counter driver has no outputs.
    fn write_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }
}
