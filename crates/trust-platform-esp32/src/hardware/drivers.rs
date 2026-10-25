//! The C6's I/O drivers beside its GPIO lines (specification 11 §6.5; specification 34 TARGET-01 and
//! Appendix D.2): `analog` on ADC1, `pwm` on the LEDC, `counter` on the PCNT and `can` on TWAI0 and
//! TWAI1. Each driver reads its parameters with the rules every runtime shares
//! (`trust_runtime_services::io_drivers`), plans what they ask of the C6 before it touches any
//! hardware (`crate::driver_plans`, which the host tests prove), claims its pins from the GPIO
//! bank, where a pin serves one line of one driver, GPIO lines included, and its peripheral from
//! [`DriverPeripherals`], where a peripheral serves one driver. It gives both back when its program
//! drops it, so the program loaded next claims them again (STATE-08). The process image calls each
//! driver at every cycle through `ChipDriver`, after the GPIO lines.
//!
//! The `bench-loopback` feature, for the bench only and never in a release, lets a board without
//! wires prove the drivers: a `counter` input counts the pad of a `pwm` output of the same record
//! (`pwm`, `counter`), the `can` controller runs in self-test mode and receives its own frames from
//! its `tx` pad (`can`), and every `analog` input's pad takes its internal pull-up (`analog`).
//! Without the feature none of this is built.

mod analog;
mod can;
mod counter;
mod modbus;
mod pwm;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use core::cell::RefCell;

#[cfg(feature = "bench-loopback")]
use esp_hal::gpio::interconnect::InputSignal;
use esp_hal::ledc::Ledc;
use esp_hal::peripherals::{ADC1, LEDC, PCNT, TWAI0, TWAI1};
use esp_hal::twai::AnyTwai;
use trust_runtime_core::deployment::Configuration;
use trust_runtime_services::chip::process_io::{unsupported_driver, ChipDriver, DriverRequest};

#[cfg(feature = "bench-loopback")]
use crate::driver_plans::LoopbackPads;
use crate::driver_plans::PeripheralSlot;
use crate::process_io::GpioBank;

use super::C6Net;

/// The peripherals the drivers claim, taken from `esp_hal::init` with the board and kept for the life
/// of the boot. Each serves one driver at a time and is free again when that driver is dropped.
pub(crate) struct DriverPeripherals {
    /// ADC1, for the `analog` driver.
    adc1: PeripheralSlot<ADC1<'static>>,
    /// The LEDC, its clock on and its timers clocked from the 80 MHz PLL clock, for the `pwm` driver.
    ledc: PeripheralSlot<Ledc<'static>>,
    /// The PCNT's four units, for the `counter` driver.
    pcnt: PeripheralSlot<counter::PulseUnits>,
    /// TWAI0 and TWAI1, for a `can` driver each.
    twai: [PeripheralSlot<AnyTwai<'static>>; 2],
    /// Bench only (`bench-loopback`): the pads of the running program's `pwm` outputs, each with
    /// its input signal, which a `counter` input of the same record counts without a wire.
    #[cfg(feature = "bench-loopback")]
    loopback: LoopbackPads<InputSignal<'static>>,
}

impl DriverPeripherals {
    /// The drivers' peripherals from the board's: the LEDC's clock is enabled and its timers are
    /// clocked from the 80 MHz PLL clock, and the PCNT's clock is enabled with its four units reset;
    /// ADC1 and the TWAI controllers stay as they are until a driver claims them.
    pub(crate) fn new(
        adc1: ADC1<'static>,
        ledc: LEDC<'static>,
        pcnt: PCNT<'static>,
        twai0: TWAI0<'static>,
        twai1: TWAI1<'static>,
    ) -> Self {
        Self {
            adc1: PeripheralSlot::new(adc1),
            ledc: PeripheralSlot::new(pwm::controller(ledc)),
            pcnt: PeripheralSlot::new(counter::PulseUnits::new(pcnt)),
            twai: [
                PeripheralSlot::new(AnyTwai::from(twai0)),
                PeripheralSlot::new(AnyTwai::from(twai1)),
            ],
            #[cfg(feature = "bench-loopback")]
            loopback: LoopbackPads::default(),
        }
    }
}

/// The driver that `request` names, built over its pins from `bank` and its peripheral from
/// `peripherals` (specification 11 §6.5): `analog`, `pwm`, `counter` or `can`, or `modbus-tcp` over a
/// connection of `network`, the chip's network when it serves one. `config` gives a `pwm` output
/// its safe state.
///
/// # Errors
///
/// `<driver>: <key>: <reason>` for parameters the shared rules or the C6 refuse, and
/// `the installed application uses the <name> driver, which the C6 does not carry out` for any
/// other driver (specification 34 ARCH-13). What the driver claimed before the refusal is free
/// again.
pub(crate) fn claim_driver(
    request: &DriverRequest<'_>,
    bank: &GpioBank,
    peripherals: &DriverPeripherals,
    network: Option<&Rc<RefCell<C6Net>>>,
    config: &Configuration,
) -> Result<Box<dyn ChipDriver>, String> {
    match request.name {
        analog::NAME => analog::claim(request.params, bank, &peripherals.adc1),
        pwm::NAME => pwm::claim(request.params, bank, peripherals, config),
        counter::NAME => counter::claim(request.params, bank, peripherals),
        can::NAME => can::claim(request.params, bank, &peripherals.twai),
        modbus::NAME => modbus::claim(request.params, network),
        other => Err(unsupported_driver(other, "C6")),
    }
}
