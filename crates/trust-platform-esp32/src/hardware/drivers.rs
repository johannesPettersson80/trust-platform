//! The C6's I/O drivers beside its GPIO lines (specification 11 §6.5; specification 34 TARGET-01 and
//! Appendix D.2): `analog` on ADC1, `pwm` on the LEDC, `counter` on the PCNT and `can` on TWAI0 and
//! TWAI1. Each driver reads its parameters with the rules every runtime shares
//! (`trust_runtime_services::io_drivers`), claims its pins from the GPIO bank, where a pin serves one
//! line of one driver, GPIO lines included, and its peripheral from [`DriverPeripherals`], where a
//! peripheral serves one driver. It gives both back when its program drops it, so the program
//! loaded next claims them again (STATE-08). The process image calls each driver at every cycle
//! through `ChipDriver`, after the GPIO lines.

mod analog;
mod can;
mod counter;
mod pwm;

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::cell::Cell;
use core::fmt;

use esp_hal::ledc::Ledc;
use esp_hal::peripherals::{ADC1, LEDC, PCNT, TWAI0, TWAI1};
use esp_hal::twai::AnyTwai;
use trust_runtime_core::deployment::Configuration;
use trust_runtime_services::chip::process_io::{unsupported_driver, ChipDriver, DriverRequest};
use trust_runtime_services::status::IoDriverHealth;

use crate::process_io::GpioBank;

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
        }
    }
}

/// The driver that `request` names, built over its pins from `bank` and its peripheral from
/// `peripherals` (specification 11 §6.5): `analog`, `pwm`, `counter` or `can`. `config` gives a
/// `pwm` output its safe state.
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
    config: &Configuration,
) -> Result<Box<dyn ChipDriver>, String> {
    match request.name {
        analog::NAME => analog::claim(request.params, bank, &peripherals.adc1),
        pwm::NAME => pwm::claim(request.params, bank, &peripherals.ledc, config),
        counter::NAME => counter::claim(request.params, bank, &peripherals.pcnt),
        can::NAME => can::claim(request.params, bank, &peripherals.twai),
        other => Err(unsupported_driver(other, "C6")),
    }
}

/// A peripheral of the board that one driver at a time holds (specification 34 STATE-08). The
/// peripheral stays in the slot; a driver holds a [`Lease`] on it, and a second claim is refused
/// until that lease is dropped.
struct PeripheralSlot<P> {
    shared: Rc<Shared<P>>,
}

/// A slot's peripheral and whether a driver holds it.
struct Shared<P> {
    peripheral: P,
    held: Cell<bool>,
}

impl<P> PeripheralSlot<P> {
    /// A free slot of `peripheral`.
    fn new(peripheral: P) -> Self {
        Self {
            shared: Rc::new(Shared {
                peripheral,
                held: Cell::new(false),
            }),
        }
    }

    /// The peripheral for one driver, held until the lease is dropped; `None` while another driver
    /// holds it.
    fn claim(&self) -> Option<Lease<P>> {
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
struct Lease<P> {
    shared: Rc<Shared<P>>,
}

impl<P> Lease<P> {
    /// The held peripheral.
    fn get(&self) -> &P {
        &self.shared.peripheral
    }
}

impl<P> Drop for Lease<P> {
    fn drop(&mut self) {
        self.shared.held.set(false);
    }
}

/// The refusal of a driver's parameter on the C6: `<driver>: <key>: <reason>`, the form of the
/// shared parameter refusal (`ParamsError` after the driver's name), which `status` reports.
fn refusal(driver: &str, key: &str, reason: impl fmt::Display) -> String {
    format!("{driver}: {key}: {reason}")
}

/// The health that the result of one exchange gives a driver's path: ok, or faulted with the
/// error, so a path is ok again after an exchange that succeeds.
fn health_of(result: &Result<(), String>) -> IoDriverHealth {
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
fn write_image(image: &mut [u8], byte: usize, bytes: &[u8], address: &str) -> Result<(), String> {
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
fn read_image<const N: usize>(image: &[u8], byte: usize, address: &str) -> Result<[u8; N], String> {
    byte.checked_add(N)
        .and_then(|end| image.get(byte..end))
        .and_then(|bytes| <[u8; N]>::try_from(bytes).ok())
        .ok_or_else(|| outside(address, image.len()))
}

/// The error of an address that does not fit the process image of `len` bytes.
fn outside(address: &str, len: usize) -> String {
    format!("{address} lies outside the process image of {len} bytes")
}
