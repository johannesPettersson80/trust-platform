//! The `analog` driver on the C6 (specification 11 §6.5; specification 34 Appendix D.2): inputs on
//! ADC1, whose channels 0 to 6 are GPIO0 to GPIO6 (`driver_plans::adc1_channel_of`). At the start
//! of each cycle each input is converted once, with the chip's eFuse calibration at the attenuation
//! of its whole range, and the millivolts at its pin are written as an `INT` into its `%IW` word,
//! limited to the `INT` range. The C6 has no digital-to-analog converter, so an output is refused.
//!
//! Bench only, with the `bench-loopback` feature: every input's pad takes its internal pull-up
//! after ADC1 made it analog (`bench_pull_up`), so that an input without a wire reads a known level
//! near the top of its range instead of a floating pad.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use esp_hal::analog::adc::{Adc, AdcCalCurve, AdcChannel, AdcConfig, AdcPin, Attenuation};
use esp_hal::gpio::AnyPin;
#[cfg(feature = "bench-loopback")]
use esp_hal::gpio::{Flex, InputConfig, Pin, Pull};
use esp_hal::peripherals::{ADC1, GPIO0, GPIO1, GPIO2, GPIO3, GPIO4, GPIO5, GPIO6};
use esp_hal::Blocking;
use smol_str::SmolStr;
use trust_runtime_services::chip::process_io::ChipDriver;
use trust_runtime_services::io_drivers::analog::{input_value, AnalogParams};
use trust_runtime_services::status::IoDriverHealth;

use crate::driver_plans::{
    self, adc1_channel_of, health_of, not_analog_input, refusal, write_image, Lease, PeripheralSlot,
};
use crate::hardware::clock::now_us;
use crate::process_io::{GpioBank, GpioPin, PinClaim};

/// The driver's name in `io.toml`.
pub(super) const NAME: &str = driver_plans::ANALOG;

/// The calibration of every input. esp-hal 1.2.2 gives the C6 three schemes: `AdcCalBasic`, which
/// only removes the zero offset and reads raw counts; `AdcCalLine`, which adds the gain of one
/// reference point and reads millivolts; and `AdcCalCurve`, which adds to the line ESP-IDF's error
/// polynomial for the C6 (version 1 or 2 of its calibration block) and reads millivolts. The driver
/// uses `AdcCalCurve`, the scheme ESP-IDF itself uses on the C6 (curve fitting), because it reads
/// millivolts from the chip's own eFuse values (the zero code and the reference point at the
/// attenuation, and the channel's offset) and corrects the bend that a line leaves, which is
/// largest towards the top of the range.
type Calibration = AdcCalCurve<ADC1<'static>>;

/// The attenuation that measures the C6's whole input range: 11 dB, esp-hal's largest (ESP-IDF's
/// `ADC_ATTEN_DB_12`). The range it gives is measured on the bench and written in specification 34
/// Appendix D.2.
const ATTENUATION: Attenuation = Attenuation::_11dB;

/// The longest one input's conversion may take, in microseconds. esp-hal 1.2.2 waits 40 µs in each
/// one-shot conversion on the C6 before it restarts it, and the conversion itself takes a few
/// microseconds, so a conversion that has not finished after 1 ms has stopped.
const CONVERSION_TIMEOUT_US: u64 = 1_000;

/// ADC1's one-shot converter and the lease on ADC1 that it was built on. The converter holds a copy
/// of the peripheral; it is declared first, so it is dropped before the lease frees ADC1.
struct Converter {
    adc: Adc<'static, ADC1<'static>, Blocking>,
    _adc1: Lease<ADC1<'static>>,
}

/// One input: its word of the input image, its pin and ADC1 channel, and the claim on the pin.
struct AnalogInput {
    /// The first byte of the `%IW` word.
    byte: usize,
    /// The word's address, `%IW<byte>`, for an error.
    address: SmolStr,
    /// The input's pin, for an error.
    pin: GpioPin,
    /// The pin's ADC1 channel with its calibration.
    channel: Box<dyn AnalogChannel>,
    _claim: PinClaim,
}

/// The `analog` driver: its inputs on ADC1.
struct AnalogDriver {
    /// ADC1's converter; `None` for a driver without inputs, which holds no ADC1.
    converter: Option<Converter>,
    inputs: Vec<AnalogInput>,
    health: IoDriverHealth,
}

/// Builds the `analog` driver of the parameters `params` (JSON): each input's pin is claimed from
/// `bank` and enabled on ADC1 at the full-range attenuation with its calibration, and ADC1 is
/// claimed from `adc1`.
///
/// # Errors
///
/// - the shared parameter refusal, `analog: <key>: <reason>`;
/// - `analog: outputs[0]: the ESP32-C6 has no digital-to-analog converter`;
/// - `analog: inputs: the C6's eFuse holds no ADC calibration, so its inputs cannot be read in
///   millivolts`, for a chip whose calibration block is not version 1 or 2;
/// - `analog: inputs: ADC1 is already used by another analog driver`;
/// - `analog: inputs[<i>].line: <reason>` for a pin that is not a pin name, that the chip lacks or
///   keeps, or that another line holds;
/// - `analog: inputs[<i>].line: GPIO<n> is not an analog input of the C6: its ADC1 channels 0 to 6
///   are GPIO0 to GPIO6` (`driver_plans::not_analog_input`).
pub(super) fn claim(
    params: &str,
    bank: &GpioBank,
    adc1: &PeripheralSlot<ADC1<'static>>,
) -> Result<Box<dyn ChipDriver>, String> {
    let params = AnalogParams::from_json(params).map_err(|error| format!("{NAME}: {error}"))?;
    if !params.outputs.is_empty() {
        return Err(refusal(
            NAME,
            "outputs[0]",
            "the ESP32-C6 has no digital-to-analog converter",
        ));
    }
    if params.inputs.is_empty() {
        return Ok(Box::new(AnalogDriver {
            converter: None,
            inputs: Vec::new(),
            health: IoDriverHealth::Ok,
        }));
    }
    // Without the calibration block, esp-hal would measure the zero and the reference itself, by
    // conversions it waits for without a bound before `Adc::new` clocks ADC1, and would take the
    // reference as a nominal 1,100 mV rather than the chip's own.
    if !matches!(esp_hal::efuse::rtc_calib_version(), 1 | 2) {
        return Err(refusal(
            NAME,
            "inputs",
            "the C6's eFuse holds no ADC calibration, so its inputs cannot be read in millivolts",
        ));
    }
    let lease = adc1.claim().ok_or_else(|| {
        refusal(
            NAME,
            "inputs",
            "ADC1 is already used by another analog driver",
        )
    })?;
    let mut config = AdcConfig::new();
    let mut inputs = Vec::with_capacity(params.inputs.len());
    for (index, input) in params.inputs.iter().enumerate() {
        let key = format!("inputs[{index}].line");
        let (pin, any, claim) = bank
            .claim_pin(&input.line)
            .map_err(|error| refusal(NAME, &key, error))?;
        let not_analog = || refusal(NAME, &key, not_analog_input(pin));
        let number = adc1_channel_of(pin).ok_or_else(not_analog)?;
        let channel = enable(number, any, &mut config).map_err(|_| not_analog())?;
        let byte = input.address.byte as usize;
        inputs.push(AnalogInput {
            byte,
            address: format!("%IW{byte}").into(),
            pin,
            channel,
            _claim: claim,
        });
    }
    // SAFETY: `PeripheralSlot::claim` refused ADC1 to every other driver until `lease` is dropped,
    // and `Converter` declares the converter built from this copy before the lease, so the copy is
    // dropped before ADC1 can be claimed again. At most one instance of ADC1 is in use at a time,
    // which `clone_unchecked` asks of its caller.
    let peripheral = unsafe { lease.get().clone_unchecked() };
    let adc = Adc::new(peripheral, config);
    Ok(Box::new(AnalogDriver {
        converter: Some(Converter { adc, _adc1: lease }),
        inputs,
        health: IoDriverHealth::Ok,
    }))
}

/// ADC1's channel `number` on `pin`, one of GPIO0 to GPIO6, enabled in `config` at the full-range
/// attenuation with its calibration, which turns the pin's pad into an analog input; with
/// `bench-loopback`, the pad then takes its internal pull-up ([`bench_pull_up`]).
///
/// # Errors
///
/// The pin itself, unchanged, when it is not the pin of channel `number`.
fn enable(
    number: u8,
    pin: AnyPin<'static>,
    config: &mut AdcConfig<ADC1<'static>>,
) -> Result<Box<dyn AnalogChannel>, AnyPin<'static>> {
    // ADC1's pins are each a type of their own, which `enable_pin_with_cal` takes.
    macro_rules! adc_input {
        ($gpio:ident) => {{
            let pin = pin.downcast::<$gpio<'static>>()?;
            let input = config.enable_pin_with_cal::<$gpio<'static>, Calibration>(pin, ATTENUATION);
            #[cfg(feature = "bench-loopback")]
            let input = {
                let mut input = input;
                bench_pull_up(input.pin.reborrow());
                input
            };
            let input: Box<dyn AnalogChannel> = Box::new(input);
            Ok(input)
        }};
    }
    match number {
        0 => adc_input!(GPIO0),
        1 => adc_input!(GPIO1),
        2 => adc_input!(GPIO2),
        3 => adc_input!(GPIO3),
        4 => adc_input!(GPIO4),
        5 => adc_input!(GPIO5),
        6 => adc_input!(GPIO6),
        _ => Err(pin),
    }
}

/// Bench only, with the `bench-loopback` feature: gives the pad of an analog input its internal
/// pull-up, so that an input without a wire reads a known level near the top of its range, the
/// pad's supply through the pull-up, rather than a floating pad. ADC1's `enable_pin_with_cal`
/// clears the pad's pulls when it makes the pad analog, so the pull-up is set after it, through a
/// GPIO driver over a reborrow of the input's pin: making it resets the pad to a GPIO with its
/// input and output off, the state the analog pad is already in, and dropping it leaves the pad as
/// it is.
#[cfg(feature = "bench-loopback")]
fn bench_pull_up<'d>(pad: impl Pin + 'd) {
    let mut pad = Flex::new(pad);
    pad.apply_input_config(&InputConfig::default().with_pull(Pull::Up));
}

/// One pin of ADC1 with its calibration, whatever the pin's type, so that the seven pin types of
/// ADC1 fit one list.
trait AnalogChannel {
    /// The calibrated millivolts at the pin, or `None` when the conversion has not finished by
    /// `deadline_us` (microseconds since boot).
    fn millivolts(
        &mut self,
        adc: &mut Adc<'static, ADC1<'static>, Blocking>,
        deadline_us: u64,
    ) -> Option<u16>;
}

impl<P: AdcChannel> AnalogChannel for AdcPin<P, ADC1<'static>, Calibration> {
    fn millivolts(
        &mut self,
        adc: &mut Adc<'static, ADC1<'static>, Blocking>,
        deadline_us: u64,
    ) -> Option<u16> {
        loop {
            // The one error of a one-shot read is that the conversion has not finished yet.
            if let Ok(millivolts) = adc.read_oneshot(self) {
                return Some(millivolts);
            }
            if now_us() >= deadline_us {
                return None;
            }
        }
    }
}

impl ChipDriver for AnalogDriver {
    fn name(&self) -> &str {
        NAME
    }

    /// Converts each input once and writes its millivolts, as an `INT`, into its `%IW` word. Every
    /// input is converted even when one fails, and the first failure is the error.
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), String> {
        let Some(converter) = self.converter.as_mut() else {
            return Ok(());
        };
        let mut result = Ok(());
        for input in &mut self.inputs {
            let deadline = now_us().saturating_add(CONVERSION_TIMEOUT_US);
            let written = match input.channel.millivolts(&mut converter.adc, deadline) {
                Some(millivolts) => {
                    let value = input_value(i32::from(millivolts));
                    write_image(image, input.byte, &value.to_le_bytes(), &input.address)
                }
                None => Err(format!(
                    "{}: the conversion did not finish within {} ms",
                    input.pin,
                    CONVERSION_TIMEOUT_US / 1_000
                )),
            };
            if result.is_ok() {
                result = written;
            }
        }
        self.health = health_of(&result);
        result
    }

    /// The C6's analog driver has no outputs.
    fn write_outputs(&mut self, _image: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        self.health.clone()
    }
}
