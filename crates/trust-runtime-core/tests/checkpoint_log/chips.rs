//! The checkpoint slots of the supported chips, read from their chip files (specification 34
//! Appendix H), with the program unit of each chip's flash.

use trust_runtime_core::retain::checkpoint::FlashGeometry;

const STM32F407: &str = include_str!("../../../../chips/stm32f407.toml");
const ESP32C6: &str = include_str!("../../../../chips/esp32c6.toml");

/// The STM32F407 programs 32-bit words, the parallelism of its 2.7 to 3.6 V supply (RM0090).
const STM32F407_PROGRAM_UNIT: u32 = 4;
/// `esp-storage` 0.10.0 writes the ESP32-C6's flash in 4-byte words (`FlashStorage::WRITE_SIZE`).
const ESP32C6_PROGRAM_UNIT: u32 = 4;

/// A checkpoint slot as a chip file places it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// A bus address on the STM32F407, a flash offset on the ESP32-C6.
    pub start: u32,
    pub len: u32,
    /// The erase unit at the slot's start.
    pub erase_unit: u32,
    /// The number of that erase unit in the flash: the sector number on the STM32F407.
    pub first_unit: u32,
}

/// A supported chip's checkpoint flash.
#[derive(Debug, Clone, Copy)]
pub struct ChipFlash {
    pub name: &'static str,
    pub program_unit: u32,
    /// `checkpoint_a`, then `checkpoint_b`.
    pub slots: [Region; 2],
}

impl ChipFlash {
    /// The log's geometry for this chip; both slots are the same size and erase unit.
    pub fn geometry(&self) -> FlashGeometry {
        let [a, b] = self.slots;
        assert_eq!(
            (a.len, a.erase_unit),
            (b.len, b.erase_unit),
            "{}: the two checkpoint slots differ",
            self.name
        );
        FlashGeometry::new(self.program_unit, a.erase_unit, a.len)
            .unwrap_or_else(|error| panic!("{}: {error}", self.name))
    }
}

/// The STM32F407 and the ESP32-C6.
pub fn chips() -> [ChipFlash; 2] {
    [stm32f407(), esp32c6()]
}

fn stm32f407() -> ChipFlash {
    let flash_start = table_value(STM32F407, "[memory.flash]", "start");
    let sectors = sector_runs(STM32F407);
    let slot = |key| {
        let (start, len) = layout_region(STM32F407, key);
        let (number, sector_start, sector_len) = sector_at(flash_start, &sectors, start);
        assert_eq!(sector_start, start, "{key} does not start on a sector");
        Region {
            start,
            len,
            erase_unit: sector_len,
            first_unit: number,
        }
    };
    ChipFlash {
        name: "stm32f407",
        program_unit: STM32F407_PROGRAM_UNIT,
        slots: [slot("checkpoint_a"), slot("checkpoint_b")],
    }
}

fn esp32c6() -> ChipFlash {
    let erase_unit = table_value(ESP32C6, "[memory.spi_flash]", "erase_kib") * 1024;
    let slot = |key| {
        let (start, len) = layout_region(ESP32C6, key);
        Region {
            start,
            len,
            erase_unit,
            first_unit: start / erase_unit,
        }
    };
    ChipFlash {
        name: "esp32c6",
        program_unit: ESP32C6_PROGRAM_UNIT,
        slots: [slot("checkpoint_a"), slot("checkpoint_b")],
    }
}

/// The region `key = { start = …, kib = … }` of the file's layout: its start and length in bytes.
fn layout_region(file: &str, key: &str) -> (u32, u32) {
    let line = file
        .lines()
        .map(str::trim)
        .find(|line| {
            line.split_once('=')
                .is_some_and(|(name, _)| name.trim() == key)
        })
        .unwrap_or_else(|| panic!("the chip file has no {key}"));
    (field(line, "start"), field(line, "kib") * 1024)
}

/// The value of `key` in the file's table `table`.
fn table_value(file: &str, table: &str, key: &str) -> u32 {
    file.lines()
        .map(str::trim)
        .skip_while(|line| *line != table)
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| number(value.trim()))
        })
        .unwrap_or_else(|| panic!("the chip file has no {key} in {table}"))
}

/// The flash sectors as runs of `(count, bytes)`, in address order.
fn sector_runs(file: &str) -> Vec<(u32, u32)> {
    file.lines()
        .map(str::trim)
        .skip_while(|line| !line.starts_with("sectors = ["))
        .skip(1)
        .take_while(|line| !line.starts_with(']'))
        .map(|line| (field(line, "count"), field(line, "kib") * 1024))
        .collect()
}

/// The sector that holds `address`: its number, start and length.
fn sector_at(flash_start: u32, runs: &[(u32, u32)], address: u32) -> (u32, u32, u32) {
    let mut start = flash_start;
    let mut number = 0;
    for &(count, len) in runs {
        for _ in 0..count {
            if (start..start + len).contains(&address) {
                return (number, start, len);
            }
            start += len;
            number += 1;
        }
    }
    panic!("no sector holds {address:#x}");
}

/// The number after `name =` in an inline table such as `{ start = 0x0800_8000, kib = 16 }`.
fn field(text: &str, name: &str) -> u32 {
    let (_, rest) = text
        .split_once(&format!("{name} ="))
        .unwrap_or_else(|| panic!("{text:?} has no {name}"));
    let value = rest
        .trim_start()
        .split([',', '}', ' '])
        .next()
        .unwrap_or_default();
    number(value)
}

/// A decimal or `0x` hexadecimal TOML integer, with or without underscores.
fn number(text: &str) -> u32 {
    let digits = text.replace('_', "");
    match digits.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => digits.parse(),
    }
    .unwrap_or_else(|_| panic!("{text:?} is not a number"))
}
