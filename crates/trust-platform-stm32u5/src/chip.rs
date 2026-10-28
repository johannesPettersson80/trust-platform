//! Chip data of the STM32U5A9 (specification 34, Appendix D.5 and H): its device ID, its flash
//! sectors, its RAM regions, its GPIO ports and the pins the firmware keeps. The values are those of
//! `chips/stm32u5a9.toml`, and a host test compares them (`tests/chip_file.rs`). Code reads the chip
//! through [`CHIP`]; the pin rule is the one the STM32F4 adapter uses too.

use trust_platform_stm32::pins::{at, Pin, PinError, Port, Reserved};
use trust_platform_stm32::table::{pin_in, PortSpec, RamRegion, Reservation, SectorRun};

/// What differs between the chips of the STM32 family this adapter drives: its identity, its flash,
/// its RAM, its clock and its pins.
#[derive(Debug, Clone, Copy)]
pub struct Chip {
    /// Name of the chip.
    pub name: &'static str,
    /// DEV_ID, bits 11:0 of DBGMCU_IDCODE (RM0456, debug MCU).
    pub device_id: u16,
    /// Flash size in KiB, as the flash size register reports it.
    pub flash_kib: u16,
    /// The flash sectors in address order, from the first byte of bank 1.
    pub sectors: &'static [SectorRun],
    /// The RAM regions.
    pub ram: &'static [RamRegion],
    /// System clock (SYSCLK and HCLK) the firmware configures, in Hz.
    pub sysclk_hz: u32,
    /// The GPIO ports the chip has.
    pub ports: &'static [PortSpec],
    /// The pins the firmware keeps for itself.
    pub reserved: &'static [Reservation],
}

impl Chip {
    /// Reads a pin name for this chip: the pin must exist, and the firmware must not keep it. A
    /// `const fn`, so that a board's wiring can be checked when the firmware is built.
    pub const fn pin(&self, name: &str) -> Result<Pin, PinError> {
        pin_in(self.ports, self.reserved, name)
    }
}

/// The STM32U5A9 (specification 34, Appendix D.5). Ports A to I have 16 pins and port J has PJ0 to
/// PJ11; the chip has no port K. The firmware keeps PA11 and PA12 for USB, PA13 and PA14 for SWD and
/// PH0 for the HSE in bypass mode.
pub const STM32U5A9: Chip = Chip {
    name: "STM32U5A9",
    // RM0456 DBGMCU_IDCODE DEV_ID of the U5 family; chips/stm32u5a9.toml, identity.device_id.
    device_id: 0x481,
    flash_kib: 4096,
    // Two banks of 256 pages of 8 KiB (DS13543 §3.4).
    sectors: &[SectorRun { count: 512, kib: 8 }],
    // SRAM1, SRAM2, SRAM3 and SRAM5 are one block from 0x2000_0000 (DS13543 §3.5).
    ram: &[
        RamRegion {
            name: "SRAM1",
            start: 0x2000_0000,
            kib: 768,
            dma: true,
        },
        RamRegion {
            name: "SRAM2",
            start: 0x200C_0000,
            kib: 64,
            dma: true,
        },
        RamRegion {
            name: "SRAM3",
            start: 0x200D_0000,
            kib: 832,
            dma: true,
        },
        RamRegion {
            name: "SRAM5",
            start: 0x201A_0000,
            kib: 832,
            dma: true,
        },
        RamRegion {
            name: "SRAM4",
            start: 0x2800_0000,
            kib: 16,
            dma: true,
        },
        // The backup SRAM in the VBAT domain: reached by no DMA (DS13543 §3.5).
        RamRegion {
            name: "BKPSRAM",
            start: 0x4201_6400,
            kib: 2,
            dma: false,
        },
    ],
    // SYSCLK and HCLK from PLL1 at voltage range 1 (D.5; see crate::clock).
    sysclk_hz: 160_000_000,
    ports: &[
        PortSpec {
            port: Port::A,
            pins: 16,
        },
        PortSpec {
            port: Port::B,
            pins: 16,
        },
        PortSpec {
            port: Port::C,
            pins: 16,
        },
        PortSpec {
            port: Port::D,
            pins: 16,
        },
        PortSpec {
            port: Port::E,
            pins: 16,
        },
        PortSpec {
            port: Port::F,
            pins: 16,
        },
        PortSpec {
            port: Port::G,
            pins: 16,
        },
        PortSpec {
            port: Port::H,
            pins: 16,
        },
        PortSpec {
            port: Port::I,
            pins: 16,
        },
        PortSpec {
            port: Port::J,
            pins: 12,
        },
    ],
    reserved: &[
        Reservation {
            pin: at(Port::A, 11),
            reason: Reserved::Usb,
        },
        Reservation {
            pin: at(Port::A, 12),
            reason: Reserved::Usb,
        },
        Reservation {
            pin: at(Port::A, 13),
            reason: Reserved::Debug,
        },
        Reservation {
            pin: at(Port::A, 14),
            reason: Reserved::Debug,
        },
        Reservation {
            pin: at(Port::H, 0),
            reason: Reserved::Oscillator,
        },
    ],
};

/// The chip this build is for: the STM32U5A9, the one chip of this adapter.
pub const CHIP: Chip = STM32U5A9;
