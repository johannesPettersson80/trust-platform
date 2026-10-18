//! The tables a chip's values are written in: its flash sectors, its RAM regions, its GPIO ports
//! and the pins the firmware keeps for itself, and the lookup of a pin name in them (specification
//! 34, Appendix H.2 and D.5). A chip's own module holds its values; the rules are written here once.

use crate::pins::{Pin, PinError, Port, Reserved};

/// A run of equally sized flash sectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectorRun {
    /// Number of sectors in the run.
    pub count: u16,
    /// Size of each sector in KiB.
    pub kib: u16,
}

/// A RAM region of the chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamRegion {
    /// Name in the reference manual.
    pub name: &'static str,
    /// Address of the first byte.
    pub start: u32,
    /// Size in KiB.
    pub kib: u16,
    /// Whether DMA reaches it. The F407's core-coupled memory is for the CPU only.
    pub dma: bool,
}

/// How many pins a GPIO port has on the chip, counted from pin 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortSpec {
    /// The port.
    pub port: Port,
    /// Pins `0` to `pins - 1` exist.
    pub pins: u8,
}

/// A pin the firmware keeps for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reservation {
    /// The pin.
    pub pin: Pin,
    /// What uses it.
    pub reason: Reserved,
}

/// Reads a pin name against a chip's GPIO ports and the pins it keeps: the pin must exist, and the
/// firmware must not keep it. A `const fn`, so that a board's wiring can be checked when the
/// firmware is built.
pub const fn pin_in(
    ports: &[PortSpec],
    reserved: &[Reservation],
    name: &str,
) -> Result<Pin, PinError> {
    let pin = match Pin::parse(name) {
        Ok(pin) => pin,
        Err(error) => return Err(PinError::Name(error)),
    };
    let mut index = 0;
    while index < ports.len() {
        let spec = &ports[index];
        if spec.port.index() == pin.port().index() {
            if pin.number() >= spec.pins {
                return Err(PinError::NoSuchPin(pin));
            }
            let mut kept = 0;
            while kept < reserved.len() {
                let entry = &reserved[kept];
                if entry.pin.port().index() == pin.port().index()
                    && entry.pin.number() == pin.number()
                {
                    return Err(PinError::Reserved(pin, entry.reason));
                }
                kept += 1;
            }
            return Ok(pin);
        }
        index += 1;
    }
    Err(PinError::NoSuchPort(pin.port()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chip with ports A (16 pins) and J (12 pins), and PA11 kept for USB.
    const PORTS: [PortSpec; 2] = [
        PortSpec {
            port: Port::A,
            pins: 16,
        },
        PortSpec {
            port: Port::J,
            pins: 12,
        },
    ];
    const KEPT: [Reservation; 1] = [Reservation {
        pin: crate::pins::at(Port::A, 11),
        reason: Reserved::Usb,
    }];

    #[test]
    fn a_pin_the_table_lists_is_found_and_one_it_lacks_is_refused_with_the_reason() {
        // Specification 34, Appendix D.5 (pins): PJ11 is the last pin of port J, PJ12 is not a
        // pin, and port K does not exist. Catches a pin count off by one, and a missing port.
        assert_eq!(
            pin_in(&PORTS, &KEPT, "PJ11"),
            Ok(Pin::new(Port::J, 11).unwrap())
        );
        assert_eq!(
            pin_in(&PORTS, &KEPT, "PJ12"),
            Err(PinError::NoSuchPin(Pin::new(Port::J, 12).unwrap()))
        );
        assert_eq!(
            pin_in(&PORTS, &KEPT, "PK0"),
            Err(PinError::NoSuchPort(Port::K))
        );
    }

    #[test]
    fn a_pin_the_firmware_keeps_is_refused_and_its_neighbours_are_not() {
        // Specification 34, Appendix D.5 (pins): PA11 is kept for USB. The pins next to it on
        // its port and the same number on another port stay usable. Catches a kept pin given
        // to a project, a reason swapped for another, and a kept pin matched by number alone.
        assert_eq!(
            pin_in(&PORTS, &KEPT, "PA11"),
            Err(PinError::Reserved(
                Pin::new(Port::A, 11).unwrap(),
                Reserved::Usb
            ))
        );
        assert_eq!(
            pin_in(&PORTS, &KEPT, "PA10"),
            Ok(Pin::new(Port::A, 10).unwrap())
        );
        assert_eq!(
            pin_in(&PORTS, &KEPT, "PJ11"),
            Ok(Pin::new(Port::J, 11).unwrap())
        );
    }
}
