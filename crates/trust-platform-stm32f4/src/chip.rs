//! Chip data. Where the STM32F4 chips this adapter drives differ, the difference is a value here:
//! system clock, flash sectors, RAM, device identity, ports and the pins the firmware keeps for
//! itself. Code reads the selected chip ([`CHIP`]); it does not branch on a chip name
//! (specification 34, ARCH-10, TARGET-03 and Appendix D).
//!
//! Sources: RM0090 and DS8626 for the STM32F407.

use crate::pins::{at, Pin, PinError, Port, Reserved, Wiring};

/// Start of the main flash memory, the same on every F4 chip.
pub const FLASH_START: u32 = 0x0800_0000;

/// A run of equally sized flash sectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectorRun {
    /// Number of sectors in the run.
    pub count: u8,
    /// Size of each sector in KiB.
    pub kib: u16,
}

/// One erase unit of the main flash memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sector {
    /// Position of the sector, 0 for the first.
    pub index: u8,
    /// Address of the first byte.
    pub start: u32,
    /// Size in bytes.
    pub bytes: u32,
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

/// The pins of the console UART, USART2 with alternate function 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsolePins {
    /// Transmit pin.
    pub tx: Pin,
    /// Receive pin.
    pub rx: Pin,
}

/// Why the chip a firmware runs on is not the chip it was built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// DBGMCU_IDCODE names another device.
    Device {
        /// Device ID read from the chip.
        found: u16,
        /// Device ID of the chip the firmware was built for.
        expected: u16,
    },
    /// The flash size register differs.
    Flash {
        /// Flash size in KiB read from the chip.
        found_kib: u16,
        /// Flash size in KiB of the chip the firmware was built for.
        expected_kib: u16,
    },
}

/// Everything that differs between the supported chips.
#[derive(Debug, Clone, Copy)]
pub struct Chip {
    /// Name of the chip.
    pub name: &'static str,
    /// DEV_ID, bits 11:0 of DBGMCU_IDCODE (RM0090, debug MCU).
    pub device_id: u16,
    /// Flash size in KiB, as the flash size register reports it.
    pub flash_kib: u16,
    /// The flash sectors in address order, from [`FLASH_START`].
    pub sectors: &'static [SectorRun],
    /// The RAM regions.
    pub ram: &'static [RamRegion],
    /// System clock (SYSCLK and HCLK) the firmware configures.
    pub sysclk_hz: u32,
    /// The GPIO ports the chip has.
    pub ports: &'static [PortSpec],
    /// The pins the firmware keeps for itself.
    pub reserved: &'static [Reservation],
    /// The console UART pins.
    pub console: ConsolePins,
}

impl Chip {
    /// Number of flash sectors.
    pub fn sector_count(&self) -> usize {
        self.sectors.iter().map(|run| usize::from(run.count)).sum()
    }

    /// The sector at `index`, counted from the start of the flash memory.
    pub fn sector(&self, index: usize) -> Option<Sector> {
        let mut first = 0_usize;
        let mut start = FLASH_START;
        for run in self.sectors {
            let count = usize::from(run.count);
            let bytes = u32::from(run.kib) * 1024;
            if index < first + count {
                let within = u32::try_from(index - first).ok()?;
                return Some(Sector {
                    index: u8::try_from(index).ok()?,
                    start: start + within * bytes,
                    bytes,
                });
            }
            start += u32::from(run.count) * bytes;
            first += count;
        }
        None
    }

    /// The sector that holds `address`.
    pub fn sector_containing(&self, address: u32) -> Option<Sector> {
        (0..self.sector_count())
            .filter_map(|index| self.sector(index))
            .find(|sector| address >= sector.start && address - sector.start < sector.bytes)
    }

    /// Address one past the last byte of the flash memory.
    pub fn flash_end(&self) -> u32 {
        FLASH_START + u32::from(self.flash_kib) * 1024
    }

    /// Total RAM in KiB.
    pub fn ram_kib(&self) -> u32 {
        self.ram.iter().map(|region| u32::from(region.kib)).sum()
    }

    /// RAM in KiB that DMA reaches.
    pub fn dma_ram_kib(&self) -> u32 {
        self.ram
            .iter()
            .filter(|region| region.dma)
            .map(|region| u32::from(region.kib))
            .sum()
    }

    /// Checks DBGMCU_IDCODE and the flash size register against this chip, so that firmware built
    /// for one chip stops on another. The F405, F407, F415 and F417 share one device ID; the
    /// check separates the families, and the build selects the chip.
    pub fn verify_identity(&self, dbgmcu_idcode: u32, flash_kib: u16) -> Result<(), IdentityError> {
        let found = (dbgmcu_idcode & 0x0fff) as u16;
        if found != self.device_id {
            return Err(IdentityError::Device {
                found,
                expected: self.device_id,
            });
        }
        if flash_kib != self.flash_kib {
            return Err(IdentityError::Flash {
                found_kib: flash_kib,
                expected_kib: self.flash_kib,
            });
        }
        Ok(())
    }

    /// Reads a pin name for this chip: the pin must exist, and the firmware must not keep it.
    /// A `const fn`, so that a board's wiring can be checked when the firmware is built
    /// ([`Chip::check_wiring`]).
    pub const fn pin(&self, name: &str) -> Result<Pin, PinError> {
        let pin = match Pin::parse(name) {
            Ok(pin) => pin,
            Err(error) => return Err(PinError::Name(error)),
        };
        let mut index = 0;
        while index < self.ports.len() {
            let spec = &self.ports[index];
            if spec.port.index() == pin.port().index() {
                if pin.number() >= spec.pins {
                    return Err(PinError::NoSuchPin(pin));
                }
                let mut kept = 0;
                while kept < self.reserved.len() {
                    let entry = &self.reserved[kept];
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

    /// Checks a board's wiring against this chip: both lines name a pin the chip has and the
    /// firmware does not keep, and they name different pins. A `const fn`: a firmware that
    /// asserts it in a constant fails its build, not its boot, on a wrong line.
    pub const fn check_wiring(&self, wiring: &Wiring<'_>) -> Result<(), PinError> {
        let input = match self.pin(wiring.input) {
            Ok(pin) => pin,
            Err(error) => return Err(error),
        };
        let output = match self.pin(wiring.output) {
            Ok(pin) => pin,
            Err(error) => return Err(error),
        };
        if input.port().index() == output.port().index() && input.number() == output.number() {
            return Err(PinError::AlreadyClaimed(output));
        }
        Ok(())
    }

    /// Number of pins the chip has.
    pub fn pin_count(&self) -> u32 {
        self.ports.iter().map(|spec| u32::from(spec.pins)).sum()
    }
}

/// The three little-endian words of the 96-bit unique ID at 0x1FFF_7A10, rebuilt from the fields
/// the HAL reads there: the X and Y wafer coordinates, the wafer number and the seven lot
/// characters. Missing lot bytes read as zero.
pub fn uid_words(x: u16, y: u16, wafer: u8, lot: &[u8]) -> [u32; 3] {
    let lot_byte = |index: usize| lot.get(index).copied().unwrap_or(0);
    [
        u32::from(x) | (u32::from(y) << 16),
        u32::from_le_bytes([wafer, lot_byte(0), lot_byte(1), lot_byte(2)]),
        u32::from_le_bytes([lot_byte(3), lot_byte(4), lot_byte(5), lot_byte(6)]),
    ]
}

const PA2_PA3: ConsolePins = ConsolePins {
    tx: at(Port::A, 2),
    rx: at(Port::A, 3),
};

/// The STM32F407 (the VG on the STM32F407G-DISC1): Cortex-M4F at up to 168 MHz, 1 MiB flash,
/// 128 KiB SRAM and 64 KiB core-coupled memory, up to 140 pins on ports A to I.
pub const STM32F407: Chip = Chip {
    name: "STM32F407",
    // RM0090: 0x413 is shared by the STM32F405, F407, F415 and F417.
    device_id: 0x413,
    flash_kib: 1024,
    // RM0090 flash organization: four 16 KiB, one 64 KiB and seven 128 KiB sectors.
    sectors: &[
        SectorRun { count: 4, kib: 16 },
        SectorRun { count: 1, kib: 64 },
        SectorRun { count: 7, kib: 128 },
    ],
    ram: &[
        RamRegion {
            name: "SRAM1",
            start: 0x2000_0000,
            kib: 112,
            dma: true,
        },
        RamRegion {
            name: "SRAM2",
            start: 0x2001_c000,
            kib: 16,
            dma: true,
        },
        RamRegion {
            name: "CCM",
            start: 0x1000_0000,
            kib: 64,
            dma: false,
        },
    ],
    // HSE through the PLL; the PLL runs from the HSI when there is no crystal (crate::crystal).
    sysclk_hz: 168_000_000,
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
            pins: 12,
        },
    ],
    reserved: &[
        Reservation {
            pin: at(Port::A, 2),
            reason: Reserved::Console,
        },
        Reservation {
            pin: at(Port::A, 3),
            reason: Reserved::Console,
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
        Reservation {
            pin: at(Port::H, 1),
            reason: Reserved::Oscillator,
        },
    ],
    console: PA2_PA3,
};

/// The chip this build is for: the STM32F407, the one supported chip. Its `stm32f407` feature
/// selects the chip's peripheral crates for a bare-metal build.
pub const CHIP: Chip = STM32F407;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SYSCLK_HZ;
    use std::string::ToString;

    const KIB: u32 = 1024;

    /// RM0090 flash organization of the STM32F40x/F41x: address and size of sectors 0 to 11.
    const F407_SECTORS: [(u32, u32); 12] = [
        (0x0800_0000, 16 * KIB),
        (0x0800_4000, 16 * KIB),
        (0x0800_8000, 16 * KIB),
        (0x0800_c000, 16 * KIB),
        (0x0801_0000, 64 * KIB),
        (0x0802_0000, 128 * KIB),
        (0x0804_0000, 128 * KIB),
        (0x0806_0000, 128 * KIB),
        (0x0808_0000, 128 * KIB),
        (0x080a_0000, 128 * KIB),
        (0x080c_0000, 128 * KIB),
        (0x080e_0000, 128 * KIB),
    ];

    fn pin(chip: &Chip, name: &str) -> Result<Pin, PinError> {
        chip.pin(name)
    }

    fn assert_sector_table(chip: &Chip, table: &[(u32, u32)]) {
        assert_eq!(chip.sector_count(), table.len());
        for (index, (start, bytes)) in table.iter().copied().enumerate() {
            let sector = chip.sector(index).unwrap();
            assert_eq!(
                sector,
                Sector {
                    index: index as u8,
                    start,
                    bytes
                }
            );
            // The first and the last byte belong to the sector, the byte after belongs to the next.
            assert_eq!(chip.sector_containing(start), Some(sector));
            assert_eq!(chip.sector_containing(start + bytes - 1), Some(sector));
            if index + 1 < table.len() {
                assert_eq!(
                    chip.sector_containing(start + bytes).unwrap().index as usize,
                    index + 1
                );
            }
        }
        assert_eq!(chip.sector(table.len()), None);
        assert_eq!(chip.sector_containing(FLASH_START - 1), None);
        assert_eq!(chip.sector_containing(chip.flash_end()), None);
        // The sectors tile the flash memory exactly.
        let last = table[table.len() - 1];
        assert_eq!(last.0 + last.1, chip.flash_end());
        let total: u32 = table.iter().map(|(_, bytes)| bytes).sum();
        assert_eq!(total, u32::from(chip.flash_kib) * KIB);
    }

    #[test]
    fn f407_sector_map_matches_the_reference_manual() {
        assert_sector_table(&STM32F407, &F407_SECTORS);
        assert_eq!(STM32F407.flash_end(), 0x0810_0000);
    }

    #[test]
    fn f407_ram_is_192_kib_and_the_core_coupled_part_is_cpu_only() {
        let ram = STM32F407.ram;
        assert_eq!(STM32F407.ram_kib(), 192);
        assert_eq!(STM32F407.dma_ram_kib(), 128);
        // SRAM1 and SRAM2 are one contiguous 128 KiB block at 0x2000_0000.
        assert_eq!(ram[0].start + u32::from(ram[0].kib) * KIB, ram[1].start);
        assert_eq!(ram[1].start + u32::from(ram[1].kib) * KIB, 0x2002_0000);
        assert_eq!(
            (ram[2].name, ram[2].start, ram[2].kib),
            ("CCM", 0x1000_0000, 64)
        );
        assert!(!ram[2].dma);
    }

    #[test]
    fn f407_values_are_the_ones_read_on_the_owners_disc1() {
        assert_eq!(STM32F407.sysclk_hz, 168_000_000);
        assert_eq!(STM32F407.flash_kib, 1024);
        assert_eq!(STM32F407.device_id, 0x413);
        assert_eq!(STM32F407.name, "STM32F407");
    }

    #[test]
    fn identity_accepts_the_chips_read_over_swd() {
        // Spec 34 D.4: DBGMCU_IDCODE 0x100F6413 and 1024 KiB on the owner's STM32F407G-DISC1.
        assert_eq!(STM32F407.verify_identity(0x100f_6413, 1024), Ok(()));
        // Any revision of the same device passes; the revision is bits 31:16.
        assert_eq!(STM32F407.verify_identity(0x2000_6413, 1024), Ok(()));
    }

    #[test]
    fn identity_rejects_another_device_or_flash_size() {
        // F407 firmware on another chip: an STM32F401xD/E, device 0x433.
        assert_eq!(
            STM32F407.verify_identity(0x1001_6433, 512),
            Err(IdentityError::Device {
                found: 0x433,
                expected: 0x413
            })
        );
        // The right device with the wrong flash size (a 512 KiB F405 or F407 variant).
        assert_eq!(
            STM32F407.verify_identity(0x100f_6413, 512),
            Err(IdentityError::Flash {
                found_kib: 512,
                expected_kib: 1024
            })
        );
        // Only the device ID bits decide; erased or zero registers never match.
        assert!(STM32F407.verify_identity(0, 1024).is_err());
        assert!(STM32F407.verify_identity(u32::MAX, 1024).is_err());
        assert!(STM32F407.verify_identity(0x100f_6000, 1024).is_err());
        assert!(STM32F407.verify_identity(0x100f_6413, 0).is_err());
        assert!(STM32F407.verify_identity(0x100f_6413, u16::MAX).is_err());
    }

    #[test]
    fn the_unique_id_words_are_rebuilt_from_the_fields() {
        // Spec 34 D.4: UID 0x0034002D 0x34314715 0x35363630 on the owner's STM32F407G-DISC1:
        // X 0x2D, Y 0x34, wafer 0x15, lot "G140665".
        assert_eq!(
            uid_words(0x002d, 0x0034, 0x15, b"G140665"),
            [0x0034_002d, 0x3431_4715, 0x3536_3630]
        );
        // A short lot string reads as zeros, never out of bounds.
        assert_eq!(uid_words(1, 2, 3, b"AB"), [0x0002_0001, 0x0042_4103, 0]);
        assert_eq!(uid_words(0, 0, 0, b""), [0, 0, 0]);
        assert_eq!(
            uid_words(u16::MAX, u16::MAX, u8::MAX, b"\xff\xff\xff\xff\xff\xff\xff"),
            [u32::MAX; 3]
        );
    }

    #[test]
    fn f407_has_ports_a_to_i_and_140_pins() {
        assert_eq!(STM32F407.pin_count(), 140);
        for letter in b'A'..=b'H' {
            for number in 0..16 {
                let name = std::format!("P{}{number}", char::from(letter));
                let found = pin(&STM32F407, &name);
                let reserved =
                    ["PA2", "PA3", "PA13", "PA14", "PH0", "PH1"].contains(&name.as_str());
                assert_eq!(found.is_err(), reserved, "{name}");
            }
        }
        for number in 0..12 {
            assert!(pin(&STM32F407, &std::format!("PI{number}")).is_ok());
        }
        for number in 12..16 {
            let name = std::format!("PI{number}");
            let error = pin(&STM32F407, &name).unwrap_err();
            assert_eq!(error, PinError::NoSuchPin(Pin::parse(&name).unwrap()));
        }
        for name in ["PJ0", "PK0", "PJ15", "PK15"] {
            let error = pin(&STM32F407, name).unwrap_err();
            assert!(matches!(error, PinError::NoSuchPort(_)), "{name}");
        }
    }

    #[test]
    fn the_bench_pins_of_the_disc1_are_ordinary_pins_of_the_chip() {
        // LEDs LD3 to LD6 on PD13, PD12, PD14, PD15 and the user button B1 on PA0 (UM1472).
        for name in ["PD12", "PD13", "PD14", "PD15", "PA0", "pd12"] {
            assert!(pin(&STM32F407, name).is_ok(), "{name}");
        }
    }

    #[test]
    fn pins_the_firmware_keeps_are_refused_with_the_reason() {
        let cases = [
            ("PA2", Reserved::Console),
            ("PA3", Reserved::Console),
            ("PA13", Reserved::Debug),
            ("PA14", Reserved::Debug),
            ("PH0", Reserved::Oscillator),
            ("PH1", Reserved::Oscillator),
        ];
        for (name, reason) in cases {
            assert_eq!(
                STM32F407.pin(name),
                Err(PinError::Reserved(Pin::parse(name).unwrap(), reason)),
                "{name}"
            );
        }
        assert_eq!(STM32F407.reserved.len(), cases.len());
    }

    #[test]
    fn bad_names_come_back_as_name_errors() {
        use crate::pins::PinNameError;
        assert_eq!(
            STM32F407.pin("PD16"),
            Err(PinError::Name(PinNameError::Range))
        );
        assert_eq!(STM32F407.pin(""), Err(PinError::Name(PinNameError::Syntax)));
        assert_eq!(
            STM32F407.pin("12"),
            Err(PinError::Name(PinNameError::Syntax))
        );
    }

    #[test]
    fn a_pin_is_checked_in_a_constant_as_well_as_at_run_time() {
        // `Chip::pin` is a `const fn`: a board configuration can be checked when the firmware
        // is built, and the answers are the ones the run-time calls give.
        const LED: Result<Pin, PinError> = STM32F407.pin("PD12");
        const KEPT: Result<Pin, PinError> = STM32F407.pin("PA2");
        const ABSENT: Result<Pin, PinError> = STM32F407.pin("PI12");
        const NO_PORT: Result<Pin, PinError> = STM32F407.pin("PK3");
        const TYPO: Result<Pin, PinError> = STM32F407.pin("PD");
        let named = |line: &str| Pin::parse(line).unwrap();
        assert_eq!(LED, Ok(named("PD12")));
        assert_eq!(
            KEPT,
            Err(PinError::Reserved(named("PA2"), Reserved::Console))
        );
        assert_eq!(ABSENT, Err(PinError::NoSuchPin(named("PI12"))));
        assert_eq!(NO_PORT, Err(PinError::NoSuchPort(Port::K)));
        assert!(matches!(TYPO, Err(PinError::Name(_))));
        assert_eq!(LED, STM32F407.pin("PD12"));
        assert_eq!(KEPT, STM32F407.pin("PA2"));
        assert_eq!(ABSENT, STM32F407.pin("PI12"));
        assert_eq!(NO_PORT, STM32F407.pin("PK3"));
        assert_eq!(TYPO, STM32F407.pin("PD"));
    }

    #[test]
    fn a_wiring_is_checked_as_a_whole() {
        use crate::pins::{PinNameError, Pull};
        // The bench wiring of the STM32F407G-DISC1, as the bench firmware states it.
        const DISC1: Wiring<'static> = Wiring {
            input: "PA0",
            input_pull: Pull::Down,
            output: "PD12",
            output_safe_high: false,
        };
        // As a constant, the way a firmware asserts its wiring at build time.
        const CHECKED: Result<(), PinError> = STM32F407.check_wiring(&DISC1);
        assert_eq!(CHECKED, Ok(()));
        let named = |line: &str| Pin::parse(line).unwrap();
        // One pin on both lines.
        let twice = Wiring {
            input: "pd12",
            ..DISC1
        };
        assert_eq!(
            STM32F407.check_wiring(&twice),
            Err(PinError::AlreadyClaimed(named("PD12")))
        );
        // The console pin, a line that is no pin, a pin the chip lacks.
        let console = Wiring {
            output: "PA2",
            ..DISC1
        };
        assert_eq!(
            STM32F407.check_wiring(&console),
            Err(PinError::Reserved(named("PA2"), Reserved::Console))
        );
        let typo = Wiring {
            input: "PA",
            ..DISC1
        };
        assert_eq!(
            STM32F407.check_wiring(&typo),
            Err(PinError::Name(PinNameError::Syntax))
        );
        let absent = Wiring {
            output: "PI12",
            ..DISC1
        };
        assert_eq!(
            STM32F407.check_wiring(&absent),
            Err(PinError::NoSuchPin(named("PI12")))
        );
        // The first failing line is the one reported.
        let both = Wiring {
            input: "PZ1",
            output: "PA2",
            ..DISC1
        };
        assert_eq!(
            STM32F407.check_wiring(&both),
            Err(PinError::Name(PinNameError::Port))
        );
    }

    #[test]
    fn table_entries_print_the_pin_they_are_meant_to_name() {
        let chip = &STM32F407;
        let names: std::vec::Vec<_> = chip
            .reserved
            .iter()
            .map(|entry| entry.pin.to_string())
            .collect();
        assert_eq!(names, ["PA2", "PA3", "PA13", "PA14", "PH0", "PH1"]);
        assert_eq!(chip.console.tx.to_string(), "PA2");
        assert_eq!(chip.console.rx.to_string(), "PA3");
        // Every reserved pin and the console pins are pins the chip has.
        for entry in chip.reserved {
            let spec = chip
                .ports
                .iter()
                .find(|spec| spec.port == entry.pin.port())
                .unwrap();
            assert!(entry.pin.number() < spec.pins);
        }
    }

    #[test]
    fn the_selected_chip_sets_the_profile_clock() {
        assert_eq!(SYSCLK_HZ, CHIP.sysclk_hz);
        assert_eq!((CHIP.name, SYSCLK_HZ), ("STM32F407", 168_000_000));
    }

    // The data-sheet limit (F407: 168 MHz), checked when the tests are built.
    const _: () = assert!(STM32F407.sysclk_hz <= 168_000_000);

    #[test]
    fn the_system_clock_divides_into_the_buses_and_into_whole_microseconds() {
        // F407: APB1 at most 42 MHz, APB2 at most 84 MHz.
        assert_eq!(STM32F407.sysclk_hz / 4, 42_000_000);
        assert_eq!(STM32F407.sysclk_hz / 2, 84_000_000);
        // Whole microseconds and milliseconds per cycle count: the fixture's clocks need them.
        assert_eq!(STM32F407.sysclk_hz % 1_000_000, 0);
    }
}
