//! The C6's I/O drivers without their hardware (specification 11 §6.5; specification 34 Appendix
//! D.2): what the `analog`, `pwm`, `counter` and `can` drivers plan on ADC1, the LEDC, the PCNT and
//! the TWAI controllers, the refusals they give before any hardware is touched, and the rules they
//! apply at each cycle. Everything here is plain data and arithmetic, built for the host as for the
//! chip, so `driver_plans/tests.rs` proves it without a board; the chip-only drivers
//! (`drivers`) turn the plans into esp-hal calls.
//!
//! The parameters themselves are read by the rules every runtime shares
//! (`trust_runtime_services::io_drivers`); this module holds only what the C6's peripherals decide.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;
use core::fmt;

use trust_runtime_core::deployment::Configuration;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::value::Value;
use trust_runtime_services::io_drivers::can::{CanFrame, CanId, CanParams, CanReceive};
use trust_runtime_services::io_drivers::counter::{CounterInput, CounterMode, Edge};
use trust_runtime_services::io_drivers::pwm::{duty_ticks, resolution_bits, PwmOutput, FULL_DUTY};
use trust_runtime_services::status::IoDriverHealth;

use crate::process_io::GpioPin;

/// The `analog` driver's name in `io.toml`.
pub const ANALOG: &str = "analog";

/// The `pwm` driver's name in `io.toml`.
pub const PWM: &str = "pwm";

/// The `counter` driver's name in `io.toml`.
pub const COUNTER: &str = "counter";

/// The `can` driver's name in `io.toml`.
pub const CAN: &str = "can";

/// The refusal of a driver's parameter on the C6: `<driver>: <key>: <reason>`, the form of the
/// shared parameter refusal (`ParamsError` after the driver's name), which `status` reports.
#[must_use]
pub fn refusal(driver: &str, key: &str, reason: impl fmt::Display) -> String {
    format!("{driver}: {key}: {reason}")
}

/// A driver's failure at a cycle as the runtime reports it: `IoDriver("<driver>: <error>")`, the
/// kind a computer's driver gives a failed reading or write of its own hardware.
#[must_use]
pub fn driver_error(driver: &str, error: impl fmt::Display) -> RuntimeError {
    RuntimeError::IoDriver(format!("{driver}: {error}").into())
}

/// The health that the result of one exchange gives a driver's path: ok, or faulted with the
/// error, so a path is ok again after an exchange that succeeds.
#[must_use]
pub fn health_of(result: &Result<(), String>) -> IoDriverHealth {
    match result {
        Ok(()) => IoDriverHealth::Ok,
        Err(error) => IoDriverHealth::Faulted {
            error: error.as_str().into(),
        },
    }
}

/// Writes `bytes` into `image` from byte `byte`, the place of `address` (`%IW4`).
///
/// # Errors
///
/// `<address> lies outside the process image of <n> bytes`, and nothing is written.
pub fn write_image(
    image: &mut [u8],
    byte: usize,
    bytes: &[u8],
    address: &str,
) -> Result<(), String> {
    let place = byte
        .checked_add(bytes.len())
        .and_then(|end| image.get_mut(byte..end));
    match place {
        Some(place) => {
            place.copy_from_slice(bytes);
            Ok(())
        }
        None => Err(outside(address, image.len())),
    }
}

/// The `N` bytes of `image` from byte `byte`, the place of `address` (`%QW4`).
///
/// # Errors
///
/// `<address> lies outside the process image of <n> bytes`.
pub fn read_image<const N: usize>(
    image: &[u8],
    byte: usize,
    address: &str,
) -> Result<[u8; N], String> {
    byte.checked_add(N)
        .and_then(|end| image.get(byte..end))
        .and_then(|bytes| <[u8; N]>::try_from(bytes).ok())
        .ok_or_else(|| outside(address, image.len()))
}

/// The error of an address that does not fit the process image of `len` bytes.
fn outside(address: &str, len: usize) -> String {
    format!("{address} lies outside the process image of {len} bytes")
}

/// A peripheral of the board that one driver at a time holds (specification 34 STATE-08). The
/// peripheral stays in the slot; a driver holds a [`Lease`] on it, and a second claim is refused
/// until that lease is dropped.
pub struct PeripheralSlot<P> {
    shared: Rc<Shared<P>>,
}

/// A slot's peripheral and whether a driver holds it.
struct Shared<P> {
    peripheral: P,
    held: Cell<bool>,
}

impl<P> PeripheralSlot<P> {
    /// A free slot of `peripheral`.
    #[must_use]
    pub fn new(peripheral: P) -> Self {
        Self {
            shared: Rc::new(Shared {
                peripheral,
                held: Cell::new(false),
            }),
        }
    }

    /// The peripheral for one driver, held until the lease is dropped; `None` while another driver
    /// holds it.
    #[must_use]
    pub fn claim(&self) -> Option<Lease<P>> {
        if self.shared.held.replace(true) {
            return None;
        }
        Some(Lease {
            shared: Rc::clone(&self.shared),
        })
    }
}

/// A driver's hold on a peripheral of the board. Dropping it frees the peripheral, so a driver
/// declares its lease after everything it built on the peripheral, which is then dropped first.
pub struct Lease<P> {
    shared: Rc<Shared<P>>,
}

impl<P> Lease<P> {
    /// The held peripheral.
    #[must_use]
    pub fn get(&self) -> &P {
        &self.shared.peripheral
    }
}

impl<P> Drop for Lease<P> {
    fn drop(&mut self) {
        self.shared.held.set(false);
    }
}

/// The LEDC's low-speed channels on the C6.
pub const LEDC_CHANNELS: usize = 6;

/// The LEDC's timers on the C6.
pub const LEDC_TIMERS: usize = 4;

/// The finest duty resolution of a C6 LEDC timer, in bits.
pub const LEDC_MAX_BITS: u8 = 14;

/// The LEDC's clock: the 80 MHz PLL clock, which `Ledc::set_global_slow_clock(APBClk)` selects on
/// the C6 (`PCR_LEDC_SCLK_SEL` = 1, `LedcSclkConfig::PllF80m` in esp-hal's clock tree).
pub const LEDC_CLOCK_HZ: u32 = 80_000_000;

/// The largest divider of a C6 LEDC timer: `LEDC_CLK_DIV_TIMERx` holds 10 integer and 8 fractional
/// bits (esp-hal's `LEDC_TIMER_DIV_NUM_MAX`).
pub const LEDC_DIVIDER_MAX: u32 = 0x3_FFFF;

/// The lowest frequency the LEDC makes, in whole hertz. At 14 bits the divider of `f` Hz is
/// 80,000,000 × 256 / (f × 16,384) = 1,250,000 / f, which fits [`LEDC_DIVIDER_MAX`] (262,143) from
/// f = 4.77 Hz on: 250,000 at 5 Hz, 312,500 at 4 Hz.
pub const LEDC_LOWEST_HZ: u32 = 5;

/// The setting of one LEDC timer: its frequency, the duty resolution at it, and its divider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerPlan {
    /// The frequency of the outputs on the timer, in hertz.
    pub frequency_hz: u32,
    /// The duty resolution, in bits: one period counts 2^`bits` steps of the divided clock.
    pub bits: u8,
    /// The clock divider, a fixed-point number with 8 fractional bits: 256 divides by 1.
    pub divider: u32,
}

/// The timer setting of `frequency_hz`: the finest resolution the LEDC's clock allows at it, at
/// most 14 bits (`resolution_bits`), and the divider that makes it, 80,000,000 × 256 /
/// (`frequency_hz` × 2^bits) rounded to the nearest step. For example 1,000 Hz runs at 14 bits with
/// the divider 1,250 (4.88), 1,000,000 Hz at 6 bits with 320 (1.25), and 5 Hz at 14 bits with
/// 250,000.
///
/// # Errors
///
/// `the C6's LEDC cannot make <f> Hz from its 80000000 Hz clock`, and below 5 Hz, whose divider
/// does not fit the timer, `...: its lowest frequency is 5 Hz`.
pub fn timer_plan(frequency_hz: u32) -> Result<TimerPlan, String> {
    let cannot =
        || format!("the C6's LEDC cannot make {frequency_hz} Hz from its {LEDC_CLOCK_HZ} Hz clock");
    let bits = resolution_bits(LEDC_CLOCK_HZ, frequency_hz, LEDC_MAX_BITS).ok_or_else(cannot)?;
    let counts = u64::from(frequency_hz) << bits;
    if counts == 0 {
        return Err(cannot());
    }
    let divider = ((u64::from(LEDC_CLOCK_HZ) << 8) + counts / 2) / counts;
    if divider > u64::from(LEDC_DIVIDER_MAX) {
        return Err(format!(
            "{}: its lowest frequency is {LEDC_LOWEST_HZ} Hz",
            cannot()
        ));
    }
    let divider = u32::try_from(divider).map_err(|_| cannot())?;
    Ok(TimerPlan {
        frequency_hz,
        bits,
        divider,
    })
}

/// The LEDC setting of a `pwm` driver's outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PwmPlan {
    /// The timers, in the order their frequencies first appear among the outputs: entry `i` is
    /// LEDC timer `i`.
    pub timers: Vec<TimerPlan>,
    /// Each output's channel, in output order.
    pub outputs: Vec<PwmChannelPlan>,
}

/// One `pwm` output's LEDC channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PwmChannelPlan {
    /// The LEDC channel, 0 to 5: output `i` runs on channel `i`.
    pub channel: usize,
    /// The output's timer, an index of [`PwmPlan::timers`]: outputs of one frequency share it.
    pub timer: usize,
    /// The duty resolution of that timer, in bits.
    pub bits: u8,
    /// The compare value of the output's `initial` duty cycle at `bits` (`duty_ticks`), which the
    /// channel runs at until the program writes the output.
    pub initial_ticks: u32,
    /// The whole percent nearest `initial` ([`duty_percent`]), which esp-hal's channel runs at
    /// while it is configured, before the driver writes `initial_ticks`.
    pub initial_percent: u8,
}

/// The LEDC setting of `outputs`, the parameters of a `pwm` driver: output `i` on channel `i`, and
/// one timer for each frequency, at the finest resolution of that frequency ([`timer_plan`]), so
/// outputs of one frequency share a timer.
///
/// # Errors
///
/// - `pwm: outputs: the C6's LEDC has 6 channels, and the driver names <n> outputs`;
/// - `pwm: outputs[<i>].frequency_hz: the C6's LEDC has 4 timers, one for each frequency, and <f>
///   Hz is a fifth frequency`, for the first output of a fifth frequency;
/// - `pwm: outputs[<i>].frequency_hz: <reason>`, with the refusal of [`timer_plan`].
pub fn pwm_plan(outputs: &[PwmOutput]) -> Result<PwmPlan, String> {
    if outputs.len() > LEDC_CHANNELS {
        return Err(refusal(
            PWM,
            "outputs",
            format!(
                "the C6's LEDC has {LEDC_CHANNELS} channels, and the driver names {} outputs",
                outputs.len()
            ),
        ));
    }
    let mut timers: Vec<TimerPlan> = Vec::with_capacity(LEDC_TIMERS);
    let mut channels = Vec::with_capacity(outputs.len());
    for (index, output) in outputs.iter().enumerate() {
        let shared = timers
            .iter()
            .enumerate()
            .find(|(_, timer)| timer.frequency_hz == output.frequency_hz)
            .map(|(timer, plan)| (timer, plan.bits));
        let (timer, bits) = match shared {
            Some(shared) => shared,
            None => {
                let key = format!("outputs[{index}].frequency_hz");
                if timers.len() == LEDC_TIMERS {
                    return Err(refusal(
                        PWM,
                        &key,
                        format!(
                            "the C6's LEDC has {LEDC_TIMERS} timers, one for each frequency, and \
                             {} Hz is a fifth frequency",
                            output.frequency_hz
                        ),
                    ));
                }
                let plan =
                    timer_plan(output.frequency_hz).map_err(|reason| refusal(PWM, &key, reason))?;
                timers.push(plan);
                (timers.len() - 1, plan.bits)
            }
        };
        channels.push(PwmChannelPlan {
            channel: index,
            timer,
            bits,
            initial_ticks: duty_ticks(output.initial, bits),
            initial_percent: duty_percent(output.initial),
        });
    }
    Ok(PwmPlan {
        timers,
        outputs: channels,
    })
}

/// The whole percent nearest to the duty cycle `duty` (hundredths of a percent; above 10,000 it is
/// 100 %), a half rounded up, which esp-hal's channel runs at while it is configured, before the
/// driver writes the exact compare value.
#[must_use]
pub fn duty_percent(duty: u16) -> u8 {
    let rounded = (u32::from(duty.min(FULL_DUTY)) + 50) / 100;
    u8::try_from(rounded).unwrap_or(100)
}

/// The duty cycle of the output at `address` in the safe state: its `[[io.safe_state]]` value,
/// which `io.toml` reads as a `WORD` for a `%QW` address and the safe state writes into the output
/// image as one, and 0 without one (specification 11 §6.5).
#[must_use]
pub fn safe_duty(config: &Configuration, address: &IoAddress) -> u16 {
    config
        .safe_state
        .iter()
        .find(|entry| entry.address == *address)
        .and_then(|entry| match &entry.value {
            Value::Word(duty) => Some(*duty),
            _ => None,
        })
        .unwrap_or(0)
}

/// The static level of the pin of the `pwm` output at `address` while the LEDC does not drive it:
/// the level its GPIO latch holds under the channel, which the terminal fault path and the driver's
/// drop hand the pin back to. High when the output's safe duty cycle ([`safe_duty`]) is 100 %, and
/// low otherwise: a static level is a duty cycle of 0 % or 100 %, and anything short of 100 % is
/// the de-energized one.
#[must_use]
pub fn safe_high(config: &Configuration, address: &IoAddress) -> bool {
    safe_duty(config, address) >= FULL_DUTY
}

/// The PCNT's units on the C6, one for each counter input.
pub const PCNT_UNITS: usize = 4;

/// What a PCNT channel does to its unit's count at an edge of its signal: the ESP32-C6 TRM's
/// `PCNT_CHn_POS_MODE_Um` (rising edge) and `PCNT_CHn_NEG_MODE_Um` (falling edge), chapter Pulse
/// Count Controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeAction {
    /// The count stays.
    Hold,
    /// The count rises by one.
    Increment,
    /// The count falls by one.
    Decrement,
}

/// What the level of a PCNT channel's control signal does to the action of an edge: the TRM's
/// `PCNT_CHn_HCTRL_MODE_Um` (high level) and `PCNT_CHn_LCTRL_MODE_Um` (low level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    /// The action stands.
    Keep,
    /// An increment becomes a decrement, and a decrement an increment.
    Reverse,
}

/// The modes of one PCNT channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSetting {
    /// The action at a falling edge of the channel's signal.
    pub falling: EdgeAction,
    /// The action at a rising edge of the channel's signal.
    pub rising: EdgeAction,
    /// What a low control signal does to the action.
    pub low: ControlAction,
    /// What a high control signal does to the action.
    pub high: ControlAction,
}

/// The modes of a PCNT unit's two channels. Channel 0 counts the edges of the input's `line` (an
/// encoder's A) under the level of `line_b`, and channel 1 the edges of `line_b` under the level of
/// `line`; a pulse input has no `line_b`, and its channel 1 holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitSetting {
    /// Channel 0's modes.
    pub channel0: ChannelSetting,
    /// Channel 1's modes.
    pub channel1: ChannelSetting,
}

/// A channel that never changes the count.
const IDLE: ChannelSetting = ChannelSetting {
    falling: EdgeAction::Hold,
    rising: EdgeAction::Hold,
    low: ControlAction::Keep,
    high: ControlAction::Keep,
};

/// The unit setting of a `pulse` input that counts the `edge`s of its line: channel 0 adds one at
/// each such edge, whatever the level of its control signal, and channel 1 holds.
#[must_use]
pub fn pulse_setting(edge: Edge) -> UnitSetting {
    let (falling, rising) = match edge {
        Edge::Rising => (EdgeAction::Hold, EdgeAction::Increment),
        Edge::Falling => (EdgeAction::Increment, EdgeAction::Hold),
        Edge::Both => (EdgeAction::Increment, EdgeAction::Increment),
    };
    UnitSetting {
        channel0: ChannelSetting {
            falling,
            rising,
            low: ControlAction::Keep,
            high: ControlAction::Keep,
        },
        channel1: IDLE,
    }
}

/// The unit setting of a `quadrature` input. A leads B through (A, B) = (0, 0), (1, 0), (1, 1),
/// (0, 1): A rises while B is low, B rises while A is high, A falls while B is high and B falls
/// while A is low, each +1; B leads A through the same states backwards, each -1, so the count
/// changes by four for each cycle of A (specification 11 §6.5). Channel 0 counts A's edges, up on a
/// rising A and down on a falling A while B is low, reversed while B is high; channel 1 counts B's
/// edges, up on a falling B and down on a rising B while A is low, reversed while A is high.
pub const QUADRATURE_SETTING: UnitSetting = UnitSetting {
    channel0: ChannelSetting {
        falling: EdgeAction::Decrement,
        rising: EdgeAction::Increment,
        low: ControlAction::Keep,
        high: ControlAction::Reverse,
    },
    channel1: ChannelSetting {
        falling: EdgeAction::Increment,
        rising: EdgeAction::Decrement,
        low: ControlAction::Keep,
        high: ControlAction::Reverse,
    },
};

/// One `counter` input's PCNT unit and the lines it counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterPlan<'a> {
    /// The PCNT unit, 0 to 3: input `i` counts on unit `i`.
    pub unit: usize,
    /// `line`: the line whose edges a pulse input counts, or an encoder's A.
    pub line: &'a str,
    /// `line_b`, an encoder's B; `None` for a pulse input.
    pub line_b: Option<&'a str>,
    /// The modes of the unit's channels.
    pub setting: UnitSetting,
}

impl CounterPlan<'_> {
    /// Whether the input decodes an encoder, whose count is a `DINT`; a pulse count is a `UDINT`.
    #[must_use]
    pub fn quadrature(&self) -> bool {
        self.line_b.is_some()
    }
}

/// The PCNT setting of `inputs`, the parameters of a `counter` driver, in input order: input `i`
/// counts on unit `i`, a `pulse` input with [`pulse_setting`] of its `edge`, and a `quadrature`
/// input with [`QUADRATURE_SETTING`] on its `line` and `line_b`.
///
/// # Errors
///
/// - `counter: inputs: the C6's PCNT has 4 counters, and the driver names <n> inputs`;
/// - `counter: inputs[<i>].line_b: is required on the C6 with mode = "quadrature": it counts the
///   encoder's B line on a pin of its own`, for the first `quadrature` input without `line_b`,
///   which the shared rules accept because a Linux counter device has the encoder's lines itself.
pub fn counter_plan(inputs: &[CounterInput]) -> Result<Vec<CounterPlan<'_>>, String> {
    if inputs.len() > PCNT_UNITS {
        return Err(refusal(
            COUNTER,
            "inputs",
            format!(
                "the C6's PCNT has {PCNT_UNITS} counters, and the driver names {} inputs",
                inputs.len()
            ),
        ));
    }
    inputs
        .iter()
        .enumerate()
        .map(|(unit, input)| {
            let (line_b, setting) = match &input.mode {
                CounterMode::Pulse { edge } => (None, pulse_setting(*edge)),
                CounterMode::Quadrature {
                    line_b: Some(line_b),
                } => (Some(line_b.as_str()), QUADRATURE_SETTING),
                CounterMode::Quadrature { line_b: None } => {
                    return Err(refusal(
                        COUNTER,
                        &format!("inputs[{unit}].line_b"),
                        "is required on the C6 with mode = \"quadrature\": it counts the \
                         encoder's B line on a pin of its own",
                    ));
                }
            };
            Ok(CounterPlan {
                unit,
                line: input.line.as_str(),
                line_b,
                setting,
            })
        })
        .collect()
}

/// The clock of the C6's TWAI controllers: the 40 MHz crystal (esp-hal 1.2.2, `BaudRate`).
pub const TWAI_CLOCK_HZ: u32 = 40_000_000;

/// The bit timing of one bit rate on the C6's TWAI, as the driver hands it to esp-hal's
/// `TimingConfig`. A bit is 1 + `tseg_1` + `tseg_2` time quanta, sampled after 1 + `tseg_1` of
/// them (ESP32-C6 TRM, chapter Two-wire Automotive Interface, bit timing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanBitTiming {
    /// `baud_rate_prescaler`: twice the time quantum, in cycles of the 40 MHz clock. esp-hal 1.2.2
    /// halves it on the C6, whose clock is half the 80 MHz its timings assume, and writes half of
    /// that less one into `TWAI_BAUD_PRESC`, of which the controller's quantum is 2 ×
    /// (`TWAI_BAUD_PRESC` + 1) clock cycles.
    pub prescaler: u16,
    /// Time segment 1, in quanta: the propagation and first phase segments, before the sample
    /// point.
    pub tseg_1: u8,
    /// Time segment 2, in quanta: the second phase segment, after the sample point.
    pub tseg_2: u8,
    /// The resynchronization jump width, in quanta.
    pub sync_jump_width: u8,
}

/// The bit timing of `bitrate` on the C6's TWAI: a time quantum of `quantum` cycles of its 40 MHz
/// clock, and a bit of 1 + `tseg_1` + `tseg_2` quanta sampled after 1 + `tseg_1` of them, with a
/// resynchronization jump of 3 quanta. These are ESP-IDF's timings for the C6: 20 quanta sampled at
/// 80 %, and at 800,000 bit/s, where a bit is 50 clock cycles, 25 quanta of 2 cycles sampled at
/// 68 %. For example 10,000 bit/s: 200 cycles × 20 quanta = 4,000 cycles of 40 MHz, 100 µs.
/// `None` for a rate the table does not hold, which the CiA 301 rates of the shared rules all are.
#[must_use]
pub fn can_bit_timing(bitrate: u32) -> Option<CanBitTiming> {
    let (quantum, tseg_1, tseg_2): (u16, u8, u8) = match bitrate {
        10_000 => (200, 15, 4),
        20_000 => (100, 15, 4),
        50_000 => (40, 15, 4),
        100_000 => (20, 15, 4),
        125_000 => (16, 15, 4),
        250_000 => (8, 15, 4),
        500_000 => (4, 15, 4),
        800_000 => (2, 16, 8),
        1_000_000 => (2, 15, 4),
        _ => return None,
    };
    Some(CanBitTiming {
        // esp-hal 1.2.2 halves the prescaler on the C6, whose clock is half the 80 MHz its timings
        // assume, so it is given twice the quantum.
        prescaler: quantum * 2,
        tseg_1,
        tseg_2,
        sync_jump_width: 3,
    })
}

/// The single acceptance filter of a `can` driver's receives, in the controller's code and mask
/// form, where a mask bit that is set makes the frame's identifier bit match the code's (esp-hal's
/// `new_from_code_mask`). The filter keeps data frames only; it lets more identifiers through than
/// the receives name, and the exchange drops those.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptanceFilter {
    /// A filter of 11-bit identifiers.
    Standard {
        /// The identifier to match: the first receive's.
        code: u16,
        /// The identifier bits that must match: those on which every receive agrees.
        mask: u16,
    },
    /// A filter of 29-bit identifiers.
    Extended {
        /// The identifier to match: the first receive's.
        code: u32,
        /// The identifier bits that must match: those on which every receive agrees.
        mask: u32,
    },
}

/// The acceptance filter of `receives` when their identifiers are all standard or all extended:
/// the identifier bits on which they agree must match, and the others may be anything. `None` with
/// no receive, or with both formats, where every frame passes, as the controller's default filter
/// lets them.
#[must_use]
pub fn acceptance_filter(receives: &[CanReceive]) -> Option<AcceptanceFilter> {
    let first = receives.first()?;
    let differ = receives
        .iter()
        .fold(0, |differ, receive| differ | (receive.id.id ^ first.id.id));
    if receives.iter().all(|receive| !receive.id.extended) {
        let code = u16::try_from(first.id.id)
            .ok()
            .filter(|code| *code <= 0x7FF)?;
        let mask = u16::try_from(!differ & 0x7FF).ok()?;
        Some(AcceptanceFilter::Standard { code, mask })
    } else if receives.iter().all(|receive| receive.id.extended) {
        let code = first.id.id;
        (code <= 0x1FFF_FFFF).then_some(AcceptanceFilter::Extended {
            code,
            mask: !differ & 0x1FFF_FFFF,
        })
    } else {
        None
    }
}

/// What a `can` driver's parameters ask of the C6, checked before any hardware is touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanPlan<'a> {
    /// The TWAI controller of `line`: 0 for `TWAI0`, 1 for `TWAI1`.
    pub controller: usize,
    /// The bit timing of `bitrate`.
    pub timing: CanBitTiming,
    /// `tx`: the pin that goes to the transceiver's input.
    pub tx: &'a str,
    /// `rx`: the pin that comes from the transceiver's output.
    pub rx: &'a str,
    /// The acceptance filter of the receives.
    pub filter: Option<AcceptanceFilter>,
}

/// The plan of a `can` driver's parameters `params` on the C6.
///
/// # Errors
///
/// The first of, in this order:
/// - `can: line: <line> is not a CAN controller of the C6: its controllers are TWAI0 and TWAI1`;
/// - `can: bitrate: <line> cannot run at <rate> bit/s from its 40000000 Hz clock`;
/// - `can: tx: <line> needs the pin that goes to the transceiver's input`;
/// - `can: rx: <line> needs the pin that comes from the transceiver's output`.
pub fn can_plan(params: &CanParams) -> Result<CanPlan<'_>, String> {
    let line = &params.line;
    let controller = match line.as_str() {
        "TWAI0" => 0,
        "TWAI1" => 1,
        _ => {
            return Err(refusal(
                CAN,
                "line",
                format!(
                    "{line} is not a CAN controller of the C6: its controllers are TWAI0 and TWAI1"
                ),
            ));
        }
    };
    let timing = can_bit_timing(params.bitrate).ok_or_else(|| {
        refusal(
            CAN,
            "bitrate",
            format!(
                "{line} cannot run at {} bit/s from its {TWAI_CLOCK_HZ} Hz clock",
                params.bitrate
            ),
        )
    })?;
    let tx = params.tx.as_deref().ok_or_else(|| {
        refusal(
            CAN,
            "tx",
            format!("{line} needs the pin that goes to the transceiver's input"),
        )
    })?;
    let rx = params.rx.as_deref().ok_or_else(|| {
        refusal(
            CAN,
            "rx",
            format!("{line} needs the pin that comes from the transceiver's output"),
        )
    })?;
    Ok(CanPlan {
        controller,
        timing,
        tx,
        rx,
        filter: acceptance_filter(&params.receive),
    })
}

/// Puts `frame` among `pending`, the frames that wait for the controller: it replaces the waiting
/// frame of its identifier in that frame's place, bringing its data up to date, or is added after
/// the others. So at most one frame of an identifier waits, with the newest data, and the frames
/// keep the order they fell due in.
pub fn queue_frame(pending: &mut VecDeque<CanFrame>, frame: CanFrame) {
    match pending.iter_mut().find(|queued| queued.id == frame.id) {
        Some(queued) => *queued = frame,
        None => pending.push_back(frame),
    }
}

/// The frame the controller received with the identifier `id` and the data `payload`, for the
/// exchange: its first 8 bytes at most, as a frame carries no more; `None` for a remote frame
/// (`remote`), which carries no data.
#[must_use]
pub fn received_frame(id: CanId, remote: bool, payload: &[u8]) -> Option<CanFrame> {
    if remote {
        return None;
    }
    let len = payload.len().min(8);
    let mut data = [0; 8];
    data.get_mut(..len)?.copy_from_slice(payload.get(..len)?);
    Some(CanFrame {
        id,
        len: u8::try_from(len).ok()?,
        data,
    })
}

/// A `can` driver's health from the health of its two paths: the worse, faulted before degraded
/// before ok, and the inputs' path when both are as bad.
#[must_use]
pub fn worse_health(inputs: &IoDriverHealth, outputs: &IoDriverHealth) -> IoDriverHealth {
    let rank = |health: &IoDriverHealth| match health {
        IoDriverHealth::Ok => 0,
        IoDriverHealth::Degraded { .. } => 1,
        IoDriverHealth::Faulted { .. } => 2,
    };
    if rank(outputs) > rank(inputs) {
        outputs.clone()
    } else {
        inputs.clone()
    }
}

/// The highest GPIO with an ADC1 channel on the C6.
const ADC1_LAST_GPIO: u8 = 6;

/// ADC1's channel of `pin` on the C6, whose channels 0 to 6 are GPIO0 to GPIO6 (specification 34
/// Appendix D.2); `None` for any other pin.
#[must_use]
pub fn adc1_channel_of(pin: GpioPin) -> Option<u8> {
    let number = pin.number();
    (number <= ADC1_LAST_GPIO).then_some(number)
}

/// Why an `analog` input refuses `pin`, which has no ADC1 channel: `GPIO<n> is not an analog input
/// of the C6: its ADC1 channels 0 to 6 are GPIO0 to GPIO6`.
#[must_use]
pub fn not_analog_input(pin: GpioPin) -> String {
    format!(
        "{pin} is not an analog input of the C6: its ADC1 channels 0 to {ADC1_LAST_GPIO} are \
         GPIO0 to GPIO{ADC1_LAST_GPIO}"
    )
}

/// Bench only, with the `bench-loopback` feature: the pads that the `pwm` outputs of the running
/// program drive, each with what a `counter` input of the same record needs to count it (on the
/// chip, the input signal of the pad, through its input buffer), so that the bench measures a PWM
/// output with a counter and no wire. An output publishes its pad when it starts and takes it back
/// when its program drops it ([`LoopbackPad`]); a counter input whose line names the pin of a
/// published pad counts that pad instead of claiming the pin, which the output holds. A pin's pad
/// is published by the one output that holds the pin, so the outputs must come before the counter
/// in `io.toml`; a counter listed first claims the pin itself, and the output is then refused.
#[cfg(any(test, feature = "bench-loopback"))]
pub struct LoopbackPads<S> {
    pads: Rc<Cell<Vec<(GpioPin, S)>>>,
}

#[cfg(any(test, feature = "bench-loopback"))]
impl<S> Default for LoopbackPads<S> {
    fn default() -> Self {
        Self {
            pads: Rc::new(Cell::new(Vec::new())),
        }
    }
}

#[cfg(any(test, feature = "bench-loopback"))]
impl<S: Clone> LoopbackPads<S> {
    /// Publishes the pad of `pin` with `signal` until the returned guard is dropped.
    #[must_use]
    pub fn publish(&self, pin: GpioPin, signal: S) -> LoopbackPad<S> {
        let mut pads = self.pads.take();
        pads.push((pin, signal));
        self.pads.set(pads);
        LoopbackPad {
            pads: Rc::clone(&self.pads),
            pin,
        }
    }

    /// The signal of the published pad of `pin`; `None` when no `pwm` output drives the pin now.
    #[must_use]
    pub fn find(&self, pin: GpioPin) -> Option<S> {
        let pads = self.pads.take();
        let found = pads
            .iter()
            .find(|(published, _)| *published == pin)
            .map(|(_, signal)| signal.clone());
        self.pads.set(pads);
        found
    }
}

/// Bench only, with the `bench-loopback` feature: a pad that a `pwm` output published in
/// [`LoopbackPads`], which dropping the guard takes back.
#[cfg(any(test, feature = "bench-loopback"))]
pub struct LoopbackPad<S> {
    pads: Rc<Cell<Vec<(GpioPin, S)>>>,
    pin: GpioPin,
}

#[cfg(any(test, feature = "bench-loopback"))]
impl<S> Drop for LoopbackPad<S> {
    fn drop(&mut self) {
        let mut pads = self.pads.take();
        pads.retain(|(published, _)| *published != self.pin);
        self.pads.set(pads);
    }
}

#[cfg(test)]
mod tests;
