//! The `pwm` driver on the C6 (specification 11 §6.5; specification 34 Appendix D.2): outputs on the
//! LEDC's six low-speed channels. Outputs of one frequency share one of its four timers, which runs
//! at the finest duty resolution the LEDC's 80 MHz clock allows at that frequency, at most 14 bits
//! (`driver_plans::pwm_plan`). After the tasks ran, each output's channel takes the duty cycle of
//! its `%QW` word, in hundredths of a percent; before the program writes it, the channel runs at
//! `initial`.
//!
//! Each output's pin is a GPIO output whose latch holds the output's safe level: high when its
//! `[[io.safe_state]]` duty is 100 %, low otherwise. The GPIO matrix routes the channel's signal over
//! that latch. The terminal fault path routes the pin back to its latch (`emergency`), and so does
//! the driver when its program drops it, so a pin that the LEDC no longer drives stays at its safe
//! level.
//!
//! Bench only, with the `bench-loopback` feature: each output also enables its pad's input buffer
//! and publishes the pad's input signal, so that a `counter` input of the same record, listed after
//! the `pwm` driver, counts the output through the pad without a wire (`PwmLine::loop_back`).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

#[cfg(feature = "bench-loopback")]
use esp_hal::gpio::interconnect::InputSignal;
use esp_hal::gpio::{AnyPin, DriveMode, Flex, Level, NoPin, OutputConfig, OutputSignal};
use esp_hal::ledc::channel::{self, Channel, ChannelHW, ChannelIFace};
use esp_hal::ledc::timer::{self, config::Duty, LSClockSource, TimerHW, TimerIFace};
use esp_hal::ledc::{LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::peripherals::LEDC;
use esp_hal::time::Rate;
use smol_str::SmolStr;
use trust_runtime_core::deployment::Configuration;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_services::chip::process_io::ChipDriver;
use trust_runtime_services::io_drivers::pwm::{duty_ticks, PwmOutput, PwmParams};
use trust_runtime_services::status::IoDriverHealth;

use super::DriverPeripherals;
use crate::driver_plans::{
    self, driver_error, health_of, pwm_plan, read_image, refusal, Lease, PwmChannelPlan, TimerPlan,
    LEDC_CLOCK_HZ, LEDC_MAX_BITS, LEDC_TIMERS,
};
#[cfg(feature = "bench-loopback")]
use crate::driver_plans::{LoopbackPad, LoopbackPads};
use crate::hardware::emergency;
use crate::process_io::{GpioBank, GpioPin, PinClaim};

/// The driver's name in `io.toml`.
pub(super) const NAME: &str = driver_plans::PWM;

/// The duty resolutions of a timer, 1 to 14 bits.
const RESOLUTIONS: usize = LEDC_MAX_BITS as usize;

/// The LEDC as the PWM driver uses it: its clock enabled, which also resets it once after boot, and
/// its low-speed timers clocked from the 80 MHz PLL clock.
pub(super) fn controller(ledc: LEDC<'static>) -> Ledc<'static> {
    let mut controller = Ledc::new(ledc);
    controller.set_global_slow_clock(LSGlobalClkSource::APBClk);
    controller
}

/// One output: its word of the output image, its channel and timer resolution, and its pin.
struct PwmLine {
    /// The first byte of the `%QW` word.
    byte: usize,
    /// The word's address, `%QW<byte>`, for an error.
    address: SmolStr,
    /// The duty resolution of the output's timer, in bits.
    bits: u8,
    /// The compare value the channel runs at.
    ticks: u32,
    /// The channel's number, which names its signal in the GPIO matrix.
    number: channel::Number,
    /// The LEDC channel, configured without a pin; the GPIO matrix routes its signal to `pin`.
    output: Channel<'static, LowSpeed>,
    /// The output's pin: a GPIO output whose latch holds the safe level.
    pin: Flex<'static>,
    /// The pin's number, under which the terminal fault path knows it.
    gpio: GpioPin,
    /// Bench only (`bench-loopback`): the pad this output published for a `counter` input; it is
    /// taken back when the output is dropped, before the claim frees the pin.
    #[cfg(feature = "bench-loopback")]
    _pad: Option<LoopbackPad<InputSignal<'static>>>,
    _claim: PinClaim,
}

impl Drop for PwmLine {
    /// The pin leaves the channel for its latch, which holds the safe level, and the terminal fault
    /// path forgets it; the claim then frees it for the program loaded next.
    fn drop(&mut self) {
        emergency::forget_output(self.gpio);
        signal(self.number).disconnect_from(&self.pin);
    }
}

#[cfg(feature = "bench-loopback")]
impl PwmLine {
    /// Bench only, with the `bench-loopback` feature: enables the input buffer of this output's pad
    /// and publishes the pad's input signal in `pads`, so that a `counter` input of the same record
    /// whose line names this output's pin counts the output through the pad, without a wire. The
    /// output still drives the pad as without the feature; it takes the pad back when it is
    /// dropped.
    fn loop_back(mut self, pads: &LoopbackPads<InputSignal<'static>>) -> Self {
        self._pad = Some(pads.publish(self.gpio, self.pin.peripheral_input()));
        self
    }
}

/// The `pwm` driver: its outputs on the LEDC.
struct PwmDriver {
    outputs: Vec<PwmLine>,
    health: IoDriverHealth,
    /// Declared last: the outputs leave their channels before the LEDC is free again.
    _ledc: Lease<Ledc<'static>>,
}

/// Builds the `pwm` driver of the parameters `params` (JSON): the LEDC is claimed from
/// `peripherals`, each frequency gets a timer, each output a channel in `io.toml` order and its pin
/// from `bank`, which becomes an output at its safe level in `config` and then carries the channel
/// at `initial` (`driver_plans::pwm_plan`).
///
/// # Errors
///
/// - the shared parameter refusal, `pwm: <key>: <reason>`;
/// - the refusals of `driver_plans::pwm_plan`: more than 6 outputs, a fifth frequency, or a
///   frequency the LEDC cannot make, below 5 Hz;
/// - `pwm: outputs: the LEDC is already used by another pwm driver`;
/// - `pwm: outputs[<i>].line: <reason>` for a pin that is not a pin name, that the chip lacks or
///   keeps, or that another line holds.
pub(super) fn claim(
    params: &str,
    bank: &GpioBank,
    peripherals: &DriverPeripherals,
    config: &Configuration,
) -> Result<Box<dyn ChipDriver>, String> {
    let params = PwmParams::from_json(params).map_err(|error| format!("{NAME}: {error}"))?;
    let plan = pwm_plan(&params.outputs)?;
    let ledc = peripherals.ledc.claim().ok_or_else(|| {
        refusal(
            NAME,
            "outputs",
            "the LEDC is already used by another pwm driver",
        )
    })?;
    // Every pin first, so that a refused pin leaves the LEDC and the pins as they were.
    let mut pins = Vec::with_capacity(params.outputs.len());
    for (index, output) in params.outputs.iter().enumerate() {
        let pin = bank
            .claim_pin(&output.line)
            .map_err(|error| refusal(NAME, &format!("outputs[{index}].line"), error))?;
        pins.push(pin);
    }
    for (index, timer) in plan.timers.iter().enumerate() {
        configure_timer(ledc.get(), index, *timer);
    }
    let mut outputs = Vec::with_capacity(pins.len());
    for ((output, line_plan), pin) in params.outputs.iter().zip(&plan.outputs).zip(pins) {
        let safe_high = driver_plans::safe_high(config, &output.address);
        let line = start(output, line_plan, pin, safe_high)?;
        // Bench only: the output's pad is published for a `counter` input of the same record.
        #[cfg(feature = "bench-loopback")]
        let line = line.loop_back(&peripherals.loopback);
        outputs.push(line);
    }
    Ok(Box::new(PwmDriver {
        outputs,
        health: IoDriverHealth::Ok,
        _ledc: ledc,
    }))
}

/// Configures LEDC timer `index` for `plan`. esp-hal 1.2.2's `Timer::configure` works its divider
/// out from `apb_clk_frequency()`, while the C6's LEDC runs from the 80 MHz PLL clock, so the driver
/// writes the divider it worked out from that clock: `configure` records the resolution, which
/// `configure_hw` writes with the driver's divider, and its own divider, or its refusal of one, is
/// replaced.
fn configure_timer(ledc: &Ledc<'static>, index: usize, plan: TimerPlan) {
    let mut ledc_timer = ledc.timer::<LowSpeed>(timer_number(index));
    let _ = ledc_timer.configure(timer::config::Config {
        duty: duty_of(usize::from(plan.bits)),
        clock_source: LSClockSource::APBClk,
        frequency: Rate::from_hz(plan.frequency_hz),
    });
    ledc_timer.configure_hw(plan.divider);
    ledc_timer.update_hw();
}

/// Starts `output` on the channel and timer of `plan`: the channel is set to the output's initial
/// duty without a pin, then the pin becomes an output at its safe level, high when `safe_high`, the
/// GPIO matrix routes the channel to it, and the terminal fault path learns the pin.
///
/// # Errors
///
/// The LEDC's refusal of the channel, which a timer set up by [`configure_timer`] does not give.
fn start(
    output: &PwmOutput,
    plan: &PwmChannelPlan,
    pin: (GpioPin, AnyPin<'static>, PinClaim),
    safe_high: bool,
) -> Result<PwmLine, String> {
    let key = format!("outputs[{}]", plan.channel);
    let unknown = || refusal(NAME, &key, "the C6's LEDC has no channel or timer for it");
    let number = channel_number(plan.channel).ok_or_else(unknown)?;
    let setting = TIMER_SETTINGS
        .get(plan.timer)
        .and_then(|settings| settings.get(usize::from(plan.bits).wrapping_sub(1)))
        .ok_or_else(unknown)?;
    let mut ledc_channel: Channel<'static, LowSpeed> = Channel::new(number, NoPin);
    ledc_channel
        .configure(channel::config::Config {
            timer: setting,
            duty_pct: plan.initial_percent,
            drive_mode: DriveMode::PushPull,
        })
        .map_err(|error| {
            refusal(
                NAME,
                &key,
                format!("the LEDC refused its channel: {error:?}"),
            )
        })?;
    ledc_channel.set_duty_hw(plan.initial_ticks);
    let (gpio, any, claim) = pin;
    let mut pin = Flex::new(any);
    pin.set_level(if safe_high { Level::High } else { Level::Low });
    pin.apply_output_config(&OutputConfig::default());
    pin.set_output_enable(true);
    signal(number).connect_to(&pin);
    // The terminal fault path routes the pin back to its latch and drives it to the safe level
    // (specification 34 TARGET-08).
    emergency::register_output(gpio, safe_high);
    let byte = output.address.byte as usize;
    Ok(PwmLine {
        byte,
        address: format!("%QW{byte}").into(),
        bits: plan.bits,
        ticks: plan.initial_ticks,
        number,
        output: ledc_channel,
        pin,
        gpio,
        #[cfg(feature = "bench-loopback")]
        _pad: None,
        _claim: claim,
    })
}

/// LEDC channel `index`, 0 to 5.
fn channel_number(index: usize) -> Option<channel::Number> {
    match index {
        0 => Some(channel::Number::Channel0),
        1 => Some(channel::Number::Channel1),
        2 => Some(channel::Number::Channel2),
        3 => Some(channel::Number::Channel3),
        4 => Some(channel::Number::Channel4),
        5 => Some(channel::Number::Channel5),
        _ => None,
    }
}

/// The GPIO matrix's signal of LEDC channel `number`.
fn signal(number: channel::Number) -> OutputSignal {
    match number {
        channel::Number::Channel0 => OutputSignal::LEDC_LS_SIG0,
        channel::Number::Channel1 => OutputSignal::LEDC_LS_SIG1,
        channel::Number::Channel2 => OutputSignal::LEDC_LS_SIG2,
        channel::Number::Channel3 => OutputSignal::LEDC_LS_SIG3,
        channel::Number::Channel4 => OutputSignal::LEDC_LS_SIG4,
        channel::Number::Channel5 => OutputSignal::LEDC_LS_SIG5,
    }
}

/// LEDC timer `index`, 0 to 3; above 3, timer 3.
const fn timer_number(index: usize) -> timer::Number {
    match index {
        0 => timer::Number::Timer0,
        1 => timer::Number::Timer1,
        2 => timer::Number::Timer2,
        _ => timer::Number::Timer3,
    }
}

/// The duty resolution of `bits`, 1 to 14 bits; above 14, 14 bits.
const fn duty_of(bits: usize) -> Duty {
    match bits {
        0 | 1 => Duty::Duty1Bit,
        2 => Duty::Duty2Bit,
        3 => Duty::Duty3Bit,
        4 => Duty::Duty4Bit,
        5 => Duty::Duty5Bit,
        6 => Duty::Duty6Bit,
        7 => Duty::Duty7Bit,
        8 => Duty::Duty8Bit,
        9 => Duty::Duty9Bit,
        10 => Duty::Duty10Bit,
        11 => Duty::Duty11Bit,
        12 => Duty::Duty12Bit,
        13 => Duty::Duty13Bit,
        _ => Duty::Duty14Bit,
    }
}

/// A configured LEDC timer as an esp-hal channel sees it: its number and its duty resolution. An
/// esp-hal `Channel` keeps a `&dyn TimerIFace` for its whole life, so the driver gives it one of the
/// `'static` entries of [`TIMER_SETTINGS`]; the hardware timer is configured through esp-hal's own
/// `Timer` ([`configure_timer`]), and the channel reads only the number and the resolution.
#[derive(Debug, Clone, Copy)]
struct TimerSetting {
    number: timer::Number,
    duty: Duty,
}

impl TimerIFace<LowSpeed> for TimerSetting {
    fn freq(&self) -> Option<Rate> {
        Some(Rate::from_hz(LEDC_CLOCK_HZ))
    }

    /// A channel never configures its timer; the driver does, through esp-hal's `Timer`.
    fn configure(
        &mut self,
        _config: timer::config::Config<LSClockSource>,
    ) -> Result<(), timer::Error> {
        Err(timer::Error::FrequencyUnset)
    }

    fn is_configured(&self) -> bool {
        true
    }

    fn duty(&self) -> Option<Duty> {
        Some(self.duty)
    }

    fn number(&self) -> timer::Number {
        self.number
    }

    /// Only a duty fade reads the frequency, and the driver starts none.
    fn frequency(&self) -> u32 {
        0
    }
}

/// One setting for each timer and resolution: `TIMER_SETTINGS[timer][bits - 1]`.
static TIMER_SETTINGS: [[TimerSetting; RESOLUTIONS]; LEDC_TIMERS] = timer_settings();

/// The table of [`TIMER_SETTINGS`].
const fn timer_settings() -> [[TimerSetting; RESOLUTIONS]; LEDC_TIMERS] {
    let mut table = [[TimerSetting {
        number: timer::Number::Timer0,
        duty: Duty::Duty1Bit,
    }; RESOLUTIONS]; LEDC_TIMERS];
    let mut timer = 0;
    while timer < LEDC_TIMERS {
        let mut bits = 1;
        while bits <= RESOLUTIONS {
            table[timer][bits - 1] = TimerSetting {
                number: timer_number(timer),
                duty: duty_of(bits),
            };
            bits += 1;
        }
        timer += 1;
    }
    table
}

impl ChipDriver for PwmDriver {
    fn name(&self) -> &str {
        NAME
    }

    /// The PWM driver has no inputs.
    fn read_inputs(&mut self, _image: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Sets each channel to the duty cycle of its `%QW` word, `duty_ticks` at its timer's
    /// resolution, when that differs from the one it runs at; the LEDC takes it at the channel's next
    /// period. Every output is written even when one word lies outside the image, and the first
    /// failure is the error.
    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let mut result = Ok(());
        for line in &mut self.outputs {
            match read_image::<2>(image, line.byte, &line.address) {
                Ok(word) => {
                    let ticks = duty_ticks(u16::from_le_bytes(word), line.bits);
                    if ticks != line.ticks {
                        line.output.set_duty_hw(ticks);
                        line.ticks = ticks;
                    }
                }
                Err(error) => {
                    if result.is_ok() {
                        result = Err(error);
                    }
                }
            }
        }
        self.health = health_of(&result);
        result.map_err(|error| driver_error(NAME, error))
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }
}
