//! Reading a chip file and checking its rules (specification 34, Appendix H.1 and H.5).

use crate::model::{
    ChipFile, Clock, Component, Events, Family, Firmware, Flash, Fpu, Gpio, Identity, Interfaces,
    Layout, Memory, PortPins, Processor, RamRegion, Region, SectorRun,
};
use crate::{sources, ChipFileError, Problem};

/// Both families address 32 bits.
const ADDRESS_SPACE: u64 = 1 << 32;
/// Bytes in a KiB.
const KIB: u64 = 1024;
/// The most a project may set for the event store (specification 34, Appendix H.5 rule 11).
const EVENT_CAPACITY_LIMIT: u32 = 1_000_000;
/// The port letters of the STM32 families, `A` first.
const PORT_LETTERS: &str = "ABCDEFGHIJK";
/// The most pins a port of the STM32 families has.
const PINS_PER_PORT: u32 = 16;

/// Whether a family is one of the STM32 families. Their chips share the rules that name the
/// pins, the flash, the vectors and the device ID (specification 34, Appendix H.5).
const fn is_stm32(family: Family) -> bool {
    matches!(family, Family::Stm32u5)
}

impl ChipFile {
    /// Reads `chips/<name>.toml` from its text and checks it.
    ///
    /// # Errors
    ///
    /// The text is not TOML; has a key the format does not define, lacks a key it requires, or
    /// has a value of the wrong type or a word outside a closed list; or breaks a rule of
    /// Appendix H.5.
    pub fn parse(name: &str, text: &str) -> Result<Self, ChipFileError> {
        let refuse = |problem: Problem| ChipFileError {
            chip: name.to_owned(),
            problem,
        };
        let chip: Self =
            toml::from_str(text).map_err(|error| refuse(Problem::Toml(error.to_string())))?;
        let table: toml::Table = text
            .parse()
            .map_err(|error: toml::de::Error| refuse(Problem::Toml(error.to_string())))?;
        check(name, &chip, &table).map_err(refuse)?;
        Ok(chip)
    }
}

/// Applies the rules in the order of Appendix H.5 and answers with the first one broken.
fn check(name: &str, chip: &ChipFile, table: &toml::Table) -> Result<(), Problem> {
    let family = chip.identity.family;
    identity(name, &chip.identity)?;
    processor(family, &chip.processor)?;
    clock(&chip.clock)?;
    let flash = memory(family, &chip.memory)?;
    layout(family, &chip.layout, &flash)?;
    interfaces(&chip.interfaces)?;
    gpio(family, &chip.gpio)?;
    firmware(&chip.firmware)?;
    sources::check(&chip.sources, table)?;
    events(chip.events)
}

/// Rule 11: the event store's capacities form a range within 1 and 1,000,000 (specification 34,
/// Appendix H.5).
fn events(events: Events) -> Result<(), Problem> {
    if events.default_capacity == 0
        || events.default_capacity > events.max_capacity
        || events.max_capacity > EVENT_CAPACITY_LIMIT
    {
        return Err(Problem::EventStoreRange {
            default_capacity: events.default_capacity,
            max_capacity: events.max_capacity,
        });
    }
    Ok(())
}

/// A text the format requires is not empty.
pub(crate) fn filled(field: &'static str, text: &str) -> Result<(), Problem> {
    if text.trim().is_empty() {
        Err(Problem::Empty { field })
    } else {
        Ok(())
    }
}

/// Every entry of a list is a name, and none is listed twice.
pub(crate) fn listed_once(list: &'static str, names: &[String]) -> Result<(), Problem> {
    for (index, name) in names.iter().enumerate() {
        filled(list, name)?;
        if names[..index].contains(name) {
            return Err(Problem::Duplicate {
                list,
                entry: name.clone(),
            });
        }
    }
    Ok(())
}

/// Rules 1 and 2: the name is the file's, and an STM32 device ID has twelve bits.
fn identity(file: &str, identity: &Identity) -> Result<(), Problem> {
    if identity.name != file {
        return Err(Problem::NameMismatch {
            found: identity.name.clone(),
        });
    }
    let small = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    if identity.name.is_empty() || !identity.name.bytes().all(small) {
        return Err(Problem::BadName);
    }
    filled("identity.model", &identity.model)?;
    if is_stm32(identity.family) && !(1..=0xFFF).contains(&identity.device_id) {
        return Err(Problem::DeviceId {
            device_id: identity.device_id,
        });
    }
    Ok(())
}

/// Rule 4: the Rust target is of the family's architecture and agrees with the FPU, and a
/// RISC-V target's extensions are the instruction set.
fn processor(family: Family, processor: &Processor) -> Result<(), Problem> {
    filled("processor.core", &processor.core)?;
    filled("processor.isa", &processor.isa)?;
    let target = processor.rust_target.as_str();
    let architecture = target.split('-').next().unwrap_or(target);
    if is_stm32(family) {
        if !architecture.starts_with("thumb") {
            return Err(Problem::TargetFamily {
                target: target.to_owned(),
            });
        }
        let hard_float = target.ends_with("-eabihf");
        if hard_float == (processor.fpu == Fpu::None) {
            return Err(Problem::TargetFpu {
                target: target.to_owned(),
            });
        }
        return Ok(());
    }
    let Some(extensions) = architecture.strip_prefix("riscv32") else {
        return Err(Problem::TargetFamily {
            target: target.to_owned(),
        });
    };
    if processor.isa != format!("RV32{}", extensions.to_ascii_uppercase()) {
        return Err(Problem::TargetIsa {
            target: target.to_owned(),
        });
    }
    let fpu = if extensions.contains(['d', 'g']) {
        Fpu::Double
    } else if extensions.contains('f') {
        Fpu::Single
    } else {
        Fpu::None
    };
    if fpu != processor.fpu {
        return Err(Problem::TargetFpu {
            target: target.to_owned(),
        });
    }
    Ok(())
}

/// Rule 3: the firmware clock is within the chip's, and the crystals form a range.
fn clock(clock: &Clock) -> Result<(), Problem> {
    if clock.firmware_hz == 0 || clock.firmware_hz > clock.max_hz {
        return Err(Problem::FirmwareClock {
            firmware_hz: clock.firmware_hz,
            max_hz: clock.max_hz,
        });
    }
    if clock.crystal_min_hz == 0 || clock.crystal_min_hz > clock.crystal_max_hz {
        return Err(Problem::CrystalRange {
            min_hz: clock.crystal_min_hz,
            max_hz: clock.crystal_max_hz,
        });
    }
    Ok(())
}

/// Where layout regions may lie, and where an erase unit starts or ends.
enum FlashSpace<'a> {
    /// Flash in the address space, from `start` to `end`, erased in its sector runs.
    Mapped {
        /// The address of the first byte.
        start: u64,
        /// The address one past the last byte.
        end: u64,
        /// The sector runs, which add up to the flash.
        sectors: &'a [SectorRun],
    },
    /// SPI flash, from offset 0, erased in units of this many bytes; its size is the device's.
    Spi {
        /// The erase unit in bytes.
        erase_bytes: u64,
    },
}

impl FlashSpace<'_> {
    /// The first address of the flash.
    fn origin(&self) -> u64 {
        match *self {
            Self::Mapped { start, .. } => start,
            Self::Spi { .. } => 0,
        }
    }

    /// Whether the bytes from `start` to `end` lie inside the flash. An SPI flash's size is the
    /// device's, so every offset is inside it.
    fn holds(&self, start: u64, end: u64) -> bool {
        match *self {
            Self::Mapped {
                start: first,
                end: last,
                ..
            } => first <= start && end <= last,
            Self::Spi { .. } => true,
        }
    }

    /// The address one past the erase unit that starts at `address`, or `None` when no erase unit
    /// starts there.
    fn unit_end(&self, address: u64) -> Option<u64> {
        match *self {
            Self::Mapped { start, sectors, .. } => {
                let mut run_start = start;
                for run in sectors {
                    let sector = u64::from(run.kib) * KIB;
                    let run_end = run_start + u64::from(run.count) * sector;
                    if (run_start..run_end).contains(&address) {
                        return (address - run_start)
                            .is_multiple_of(sector)
                            .then_some(address + sector);
                    }
                    run_start = run_end;
                }
                None
            }
            Self::Spi { erase_bytes } => address
                .is_multiple_of(erase_bytes)
                .then_some(address + erase_bytes),
        }
    }

    /// Whether an erase unit starts or ends at `address`.
    fn on_boundary(&self, address: u64) -> bool {
        match *self {
            Self::Mapped { start, sectors, .. } => {
                let mut run_start = start;
                if address == run_start {
                    return true;
                }
                for run in sectors {
                    let sector = u64::from(run.kib) * KIB;
                    let run_end = run_start + u64::from(run.count) * sector;
                    if address > run_start && address <= run_end {
                        return (address - run_start).is_multiple_of(sector);
                    }
                    run_start = run_end;
                }
                false
            }
            Self::Spi { erase_bytes } => address.is_multiple_of(erase_bytes),
        }
    }
}

/// Rules 5 and 6: the flash is in the family's table and its sectors add up, and the RAM regions
/// are distinct.
fn memory(family: Family, memory: &Memory) -> Result<FlashSpace<'_>, Problem> {
    let flash = match (&memory.flash, &memory.spi_flash) {
        (Some(flash), None) if is_stm32(family) => mapped(flash)?,
        (None, Some(spi)) if !is_stm32(family) => {
            if spi.erase_kib == 0 {
                return Err(Problem::ZeroSize {
                    what: "memory.spi_flash.erase_kib".to_owned(),
                });
            }
            FlashSpace::Spi {
                erase_bytes: u64::from(spi.erase_kib) * KIB,
            }
        }
        _ => return Err(Problem::FlashKind { family }),
    };
    ram(&memory.ram, &flash)?;
    Ok(flash)
}

/// The flash in the address space, once its sectors add up to it below 4 GiB.
fn mapped(flash: &Flash) -> Result<FlashSpace<'_>, Problem> {
    if flash.kib == 0 || flash.sectors.is_empty() {
        return Err(Problem::ZeroSize {
            what: "memory.flash".to_owned(),
        });
    }
    let mut sum_kib = 0_u64;
    for run in &flash.sectors {
        if run.count == 0 || run.kib == 0 {
            return Err(Problem::ZeroSize {
                what: "memory.flash.sectors".to_owned(),
            });
        }
        sum_kib = sum_kib.saturating_add(u64::from(run.count) * u64::from(run.kib));
    }
    if sum_kib != u64::from(flash.kib) {
        return Err(Problem::SectorSum {
            sum_kib,
            flash_kib: flash.kib,
        });
    }
    let start = u64::from(flash.start);
    let end = start + u64::from(flash.kib) * KIB;
    if end > ADDRESS_SPACE {
        return Err(Problem::AddressRange {
            what: "memory.flash".to_owned(),
        });
    }
    Ok(FlashSpace::Mapped {
        start,
        end,
        sectors: &flash.sectors,
    })
}

/// Whether two ranges of addresses share a byte.
fn overlap(first: (u64, u64), second: (u64, u64)) -> bool {
    first.0 < second.1 && second.0 < first.1
}

/// Rule 6: the RAM regions have names of their own, lie below 4 GiB and overlap neither one
/// another nor the flash.
fn ram(regions: &[RamRegion], flash: &FlashSpace<'_>) -> Result<(), Problem> {
    if regions.is_empty() {
        return Err(Problem::ZeroSize {
            what: "memory.ram".to_owned(),
        });
    }
    for (index, region) in regions.iter().enumerate() {
        filled("memory.ram.name", &region.name)?;
        if region.kib == 0 {
            return Err(Problem::ZeroSize {
                what: format!("memory.ram {}", region.name),
            });
        }
        if region.end() > ADDRESS_SPACE {
            return Err(Problem::AddressRange {
                what: format!("memory.ram {}", region.name),
            });
        }
        let span = (u64::from(region.start), region.end());
        for earlier in &regions[..index] {
            if earlier.name == region.name {
                return Err(Problem::Duplicate {
                    list: "memory.ram",
                    entry: region.name.clone(),
                });
            }
            if overlap((u64::from(earlier.start), earlier.end()), span) {
                return Err(Problem::RamOverlap {
                    first: earlier.name.clone(),
                    second: region.name.clone(),
                });
            }
        }
        if let FlashSpace::Mapped { start, end, .. } = *flash
            && overlap((start, end), span)
        {
            return Err(Problem::FlashOverlapsRam {
                ram: region.name.clone(),
            });
        }
    }
    Ok(())
}

/// The layout regions only one family has, and whether `family` has them.
fn family_regions(family: Family, layout: &Layout) -> [(&'static str, bool, bool); 3] {
    let stm32 = is_stm32(family);
    [
        ("vectors", layout.vectors.is_some(), stm32),
        ("bootloader", layout.bootloader.is_some(), !stm32),
        ("partition_table", layout.partition_table.is_some(), !stm32),
    ]
}

/// Rule 7: the family's regions, each inside the flash and on erase-unit boundaries, none
/// overlapping another, and the chip starting from the first.
fn layout(family: Family, layout: &Layout, flash: &FlashSpace<'_>) -> Result<(), Problem> {
    for (region, present, belongs) in family_regions(family, layout) {
        if belongs && !present {
            return Err(Problem::MissingRegion { region });
        }
        if present && !belongs {
            return Err(Problem::UnexpectedRegion { region });
        }
    }
    let regions = layout.regions();
    for (index, &(key, region)) in regions.iter().enumerate() {
        if region.kib == 0 {
            return Err(Problem::ZeroSize {
                what: format!("layout.{key}"),
            });
        }
        let span = (u64::from(region.start), region.end());
        if !flash.holds(span.0, span.1) {
            return Err(Problem::RegionOutsideFlash { region: key });
        }
        if !flash.on_boundary(span.0) || !flash.on_boundary(span.1) {
            return Err(Problem::RegionNotAligned { region: key });
        }
        let earlier = regions[..index]
            .iter()
            .find(|(_, other)| overlap((u64::from(other.start), other.end()), span));
        if let Some(&(first, _)) = earlier {
            return Err(Problem::RegionsOverlap { first, second: key });
        }
    }
    // A Cortex-M fetches its stack pointer and reset vector from the first byte of its flash;
    // the ESP32 ROM loads the second-stage boot loader from offset 0.
    let boot = if is_stm32(family) {
        "vectors"
    } else {
        "bootloader"
    };
    let misplaced = regions
        .iter()
        .any(|&(key, region)| key == boot && u64::from(region.start) != flash.origin());
    if misplaced {
        return Err(Problem::BootRegion { region: boot });
    }
    program_slots(layout, flash)?;
    firmware_slots(layout)?;
    one_unit_pair(
        ("device_a", layout.device_a),
        ("device_b", layout.device_b),
        flash,
    )?;
    one_unit_pair(
        ("pairing_a", layout.pairing_a),
        ("pairing_b", layout.pairing_b),
        flash,
    )?;
    boot_counter(layout, flash)
}

/// The second firmware slot and the boot loader's selection, when the file names them
/// (specification 34 TARGET-04 and Appendix H.2): both or neither, and the second slot of the first
/// slot's size, because an update writes a firmware that ran from either.
fn firmware_slots(layout: &Layout) -> Result<(), Problem> {
    match (layout.firmware_b, layout.firmware_select) {
        (Some(_), None) => Err(Problem::MissingRegion {
            region: "firmware_select",
        }),
        (None, Some(_)) => Err(Problem::MissingRegion {
            region: "firmware_b",
        }),
        (Some(second), Some(_)) if second.kib != layout.firmware.kib => {
            Err(Problem::UnequalSlots {
                first: "firmware",
                second: "firmware_b",
            })
        }
        (None, None) | (Some(_), Some(_)) => Ok(()),
    }
}

/// Two sectors that a file names both or neither, each exactly one erase unit: the device settings
/// (TARGET-02) and the pairings (specification 11 §6.9.6), whose writes erase one whole sector.
fn one_unit_pair(
    (first_key, first): (&'static str, Option<Region>),
    (second_key, second): (&'static str, Option<Region>),
    flash: &FlashSpace<'_>,
) -> Result<(), Problem> {
    let (first, second) = match (first, second) {
        (None, None) => return Ok(()),
        (Some(first), Some(second)) => (first, second),
        (Some(_), None) => return Err(Problem::MissingRegion { region: second_key }),
        (None, Some(_)) => return Err(Problem::MissingRegion { region: first_key }),
    };
    for (key, region) in [(first_key, first), (second_key, second)] {
        if flash.unit_end(u64::from(region.start)) != Some(region.end()) {
            return Err(Problem::NotOneEraseUnit { region: key });
        }
    }
    Ok(())
}

/// The boot counter's two sectors, when the file names them (specification 34 PORT-05 and Appendix
/// D.2 and H.2): both or neither, each exactly one erase unit, because the counter erases a whole
/// sector when its log is full and may not erase more of the flash than that. Inside the flash, on
/// erase-unit boundaries and apart from every other region, each is held by the loop of [`layout`]
/// over [`Layout::regions`], which includes them.
fn boot_counter(layout: &Layout, flash: &FlashSpace<'_>) -> Result<(), Problem> {
    let (a, b) = match (layout.boot_counter_a, layout.boot_counter_b) {
        (None, None) => return Ok(()),
        (Some(a), Some(b)) => (a, b),
        (Some(_), None) => {
            return Err(Problem::MissingRegion {
                region: "boot_counter_b",
            });
        }
        (None, Some(_)) => {
            return Err(Problem::MissingRegion {
                region: "boot_counter_a",
            });
        }
    };
    for (key, region) in [("boot_counter_a", a), ("boot_counter_b", b)] {
        if flash.unit_end(u64::from(region.start)) != Some(region.end()) {
            return Err(Problem::NotOneEraseUnit { region: key });
        }
    }
    Ok(())
}

/// The program store's two slots, when the file names them (specification 34 §5.1 and Appendix
/// H.2): both or neither, of one size. Each lies inside `application`, apart from the checkpoint
/// slots and the other slot, and on erase-unit boundaries, because an install erases whole units
/// of its slot.
fn program_slots(layout: &Layout, flash: &FlashSpace<'_>) -> Result<(), Problem> {
    let (a, b) = match (layout.application_a, layout.application_b) {
        (None, None) => return Ok(()),
        (Some(a), Some(b)) => (a, b),
        (Some(_), None) => {
            return Err(Problem::MissingRegion {
                region: "application_b",
            });
        }
        (None, Some(_)) => {
            return Err(Problem::MissingRegion {
                region: "application_a",
            });
        }
    };
    for (key, region) in [("application_a", a), ("application_b", b)] {
        if region.kib == 0 {
            return Err(Problem::ZeroSize {
                what: format!("layout.{key}"),
            });
        }
    }
    program_slot(("application_a", a), ("application_b", b), layout, flash)?;
    program_slot(("application_b", b), ("application_a", a), layout, flash)?;
    if a.kib != b.kib {
        return Err(Problem::UnequalSlots {
            first: "application_a",
            second: "application_b",
        });
    }
    Ok(())
}

/// One program slot, against the checkpoint slots and the other slot (`partner`), and against
/// `application`. The checks run in the order that names the most specific refusal.
fn program_slot(
    (key, slot): (&'static str, Region),
    (partner_key, partner): (&'static str, Region),
    layout: &Layout,
    flash: &FlashSpace<'_>,
) -> Result<(), Problem> {
    let span = (u64::from(slot.start), slot.end());
    if !flash.on_boundary(span.0) || !flash.on_boundary(span.1) {
        return Err(Problem::RegionNotAligned { region: key });
    }
    let others = [
        ("checkpoint_a", layout.checkpoint_a),
        ("checkpoint_b", layout.checkpoint_b),
        (partner_key, partner),
    ];
    for (other_key, other) in others {
        if overlap((u64::from(other.start), other.end()), span) {
            return Err(Problem::RegionsOverlap {
                first: other_key,
                second: key,
            });
        }
    }
    let application = (
        u64::from(layout.application.start),
        layout.application.end(),
    );
    if span.0 < application.0 || application.1 < span.1 {
        return Err(Problem::OutsideApplication { slot: key });
    }
    Ok(())
}

/// Rule 8 for the interfaces: each list names each peripheral once.
fn interfaces(interfaces: &Interfaces) -> Result<(), Problem> {
    interfaces
        .lists()
        .into_iter()
        .try_for_each(|(list, names)| listed_once(list, names))
}

/// The pins of a chip, as its family names them.
enum Pins {
    /// The STM32 families: the pins of each port, port A first; 0 for a port the chip lacks.
    Ports([u32; PORT_LETTERS.len()]),
    /// `esp32`: `GPIO0` to the GPIO before this count.
    Numbered(u32),
}

impl Pins {
    /// The pin `name` names, as its port and number, when the chip has it.
    fn find(&self, name: &str) -> Option<(usize, u32)> {
        match self {
            Self::Ports(sizes) => {
                let rest = name.strip_prefix('P')?;
                let letter = rest.chars().next()?;
                let port = PORT_LETTERS.find(letter)?;
                let number = decimal(&rest[letter.len_utf8()..])?;
                (number < *sizes.get(port)?).then_some((port, number))
            }
            Self::Numbered(count) => {
                let number = decimal(name.strip_prefix("GPIO")?)?;
                (number < *count).then_some((0, number))
            }
        }
    }
}

/// A decimal number without a sign or a leading zero.
fn decimal(text: &str) -> Option<u32> {
    let digits = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if digits && (text == "0" || !text.starts_with('0')) {
        text.parse().ok()
    } else {
        None
    }
}

/// The pins of each STM32 port, once every port is a letter A to K with 1 to 16 pins, listed
/// once.
fn port_sizes(ports: &[PortPins]) -> Result<[u32; PORT_LETTERS.len()], Problem> {
    let mut sizes = [0; PORT_LETTERS.len()];
    for entry in ports {
        let bad = || Problem::BadPort {
            port: entry.port.clone(),
        };
        let mut letters = entry.port.chars();
        let (Some(letter), None) = (letters.next(), letters.next()) else {
            return Err(bad());
        };
        let index = PORT_LETTERS.find(letter).ok_or_else(bad)?;
        if !(1..=PINS_PER_PORT).contains(&entry.pins) {
            return Err(bad());
        }
        let size = sizes.get_mut(index).ok_or_else(bad)?;
        if *size != 0 {
            return Err(Problem::Duplicate {
                list: "gpio.ports",
                entry: entry.port.clone(),
            });
        }
        *size = entry.pins;
    }
    Ok(sizes)
}

/// Rule 8 for the pins: the family's key, and reserved pins of the chip, each listed once.
fn gpio(family: Family, gpio: &Gpio) -> Result<(), Problem> {
    let pins = match (&gpio.ports, gpio.pins) {
        (Some(ports), None) if is_stm32(family) => Pins::Ports(port_sizes(ports)?),
        (None, Some(count)) if !is_stm32(family) => {
            if count == 0 {
                return Err(Problem::ZeroSize {
                    what: "gpio.pins".to_owned(),
                });
            }
            Pins::Numbered(count)
        }
        _ => return Err(Problem::GpioKind { family }),
    };
    let mut kept: Vec<(usize, u32)> = Vec::with_capacity(gpio.reserved.len());
    for entry in &gpio.reserved {
        let pin = pins.find(&entry.pin).ok_or_else(|| Problem::BadPin {
            pin: entry.pin.clone(),
        })?;
        if kept.contains(&pin) {
            return Err(Problem::Duplicate {
                list: "gpio.reserved",
                entry: entry.pin.clone(),
            });
        }
        kept.push(pin);
    }
    Ok(())
}

/// Rule 9: the firmware crate is under `firmware/`, and the firmware holds the engine and lists
/// each component once.
fn firmware(firmware: &Firmware) -> Result<(), Problem> {
    let under_firmware = firmware
        .crate_dir
        .strip_prefix("firmware/")
        .is_some_and(|rest| {
            !rest.is_empty()
                && !rest.contains('\\')
                && rest
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
        });
    if !under_firmware {
        return Err(Problem::FirmwareCrate {
            path: firmware.crate_dir.clone(),
        });
    }
    filled("firmware.bin", &firmware.bin)?;
    if !firmware.components.contains(&Component::Engine) {
        return Err(Problem::NoEngine);
    }
    for (index, component) in firmware.components.iter().enumerate() {
        if firmware.components[..index].contains(component) {
            return Err(Problem::Duplicate {
                list: "firmware.components",
                entry: component.name().to_owned(),
            });
        }
    }
    Ok(())
}
