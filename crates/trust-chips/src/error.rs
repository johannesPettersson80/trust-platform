//! Why a chip file is refused (specification 34, Appendix H.1 and H.5).

use crate::model::Family;

/// A chip file that cannot be used, and why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("chips/{chip}.toml: {problem}")]
pub struct ChipFileError {
    /// The chip's name, as the file is named.
    pub chip: String,
    /// What is wrong with the file.
    pub problem: Problem,
}

/// What is wrong with a chip file: the first rule of Appendix H.5 it breaks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    /// The text is not TOML, has a key the format does not define, lacks a key it requires, has a
    /// value of the wrong type, or has a word outside a closed list.
    #[error("{0}")]
    Toml(String),
    /// `identity.name` is not the file's name.
    #[error("identity.name is {found:?}, not the file's name")]
    NameMismatch {
        /// The name the file gives.
        found: String,
    },
    /// The chip's name is not small letters and digits.
    #[error("a chip's name is small letters and digits")]
    BadName,
    /// A text the format requires is empty.
    #[error("{field} is empty")]
    Empty {
        /// The key of the text.
        field: &'static str,
    },
    /// An STM32 device ID is not a twelve-bit `DEV_ID`.
    #[error("identity.device_id {device_id:#x} is not 1 to 0xFFF, the twelve bits of DEV_ID")]
    DeviceId {
        /// The device ID the file gives.
        device_id: u32,
    },
    /// The firmware clock is 0 or above the chip's highest clock.
    #[error(
        "clock.firmware_hz {firmware_hz} is not more than 0 and at most clock.max_hz {max_hz}"
    )]
    FirmwareClock {
        /// The firmware clock in Hz.
        firmware_hz: u32,
        /// The chip's highest clock in Hz.
        max_hz: u32,
    },
    /// The crystal range starts at 0 or ends before it starts.
    #[error("the crystals from {min_hz} to {max_hz} Hz are no range")]
    CrystalRange {
        /// `clock.crystal_min_hz`.
        min_hz: u32,
        /// `clock.crystal_max_hz`.
        max_hz: u32,
    },
    /// The Rust target is not of the family's processor architecture.
    #[error("the Rust target {target} is not of the family's architecture")]
    TargetFamily {
        /// The Rust target the file gives.
        target: String,
    },
    /// The Rust target's float ABI or extensions disagree with `processor.fpu`.
    #[error("the Rust target {target} does not agree with processor.fpu")]
    TargetFpu {
        /// The Rust target the file gives.
        target: String,
    },
    /// A RISC-V target's extensions are not `processor.isa`.
    #[error("the Rust target {target} does not agree with processor.isa")]
    TargetIsa {
        /// The Rust target the file gives.
        target: String,
    },
    /// The flash is not in the family's table, or is in both.
    #[error(
        "the {family} family describes its flash in its own table: memory.flash for stm32u5, memory.spi_flash for esp32"
    )]
    FlashKind {
        /// The chip's family.
        family: Family,
    },
    /// Something that holds memory or pins has none.
    #[error("{what} is empty")]
    ZeroSize {
        /// What is empty.
        what: String,
    },
    /// The flash sectors do not add up to the flash.
    #[error("the flash sectors add up to {sum_kib} KiB, not to memory.flash.kib {flash_kib}")]
    SectorSum {
        /// The sum of the sectors in KiB.
        sum_kib: u64,
        /// `memory.flash.kib`.
        flash_kib: u32,
    },
    /// A memory does not lie below 4 GiB.
    #[error("{what} does not lie below 4 GiB")]
    AddressRange {
        /// The memory.
        what: String,
    },
    /// Two RAM regions overlap.
    #[error("the RAM regions {first} and {second} overlap")]
    RamOverlap {
        /// The earlier region.
        first: String,
        /// The later region.
        second: String,
    },
    /// A RAM region overlaps the flash.
    #[error("the RAM region {ram} overlaps the flash")]
    FlashOverlapsRam {
        /// The RAM region.
        ram: String,
    },
    /// A name is listed twice.
    #[error("{entry:?} is listed twice in {list}")]
    Duplicate {
        /// The list.
        list: &'static str,
        /// The name listed twice.
        entry: String,
    },
    /// The family's layout lacks a region.
    #[error("layout.{region} is missing")]
    MissingRegion {
        /// The region's key.
        region: &'static str,
    },
    /// The layout has a region of the other family.
    #[error("layout.{region} does not belong to this family")]
    UnexpectedRegion {
        /// The region's key.
        region: &'static str,
    },
    /// A layout region lies outside the flash.
    #[error("layout.{region} lies outside the flash")]
    RegionOutsideFlash {
        /// The region's key.
        region: &'static str,
    },
    /// A layout region does not start and end on an erase-unit boundary.
    #[error("layout.{region} does not start and end on an erase-unit boundary")]
    RegionNotAligned {
        /// The region's key.
        region: &'static str,
    },
    /// A boot counter sector is not exactly one erase unit.
    #[error("layout.{region} is not exactly one erase unit")]
    NotOneEraseUnit {
        /// The region's key.
        region: &'static str,
    },
    /// Two layout regions overlap.
    #[error("layout.{first} and layout.{second} overlap")]
    RegionsOverlap {
        /// The earlier region's key.
        first: &'static str,
        /// The later region's key.
        second: &'static str,
    },
    /// A program slot lies outside `application`.
    #[error("layout.{slot} lies outside layout.application")]
    OutsideApplication {
        /// The slot's key.
        slot: &'static str,
    },
    /// The two program slots are not of one size.
    #[error("layout.{first} and layout.{second} are not the same size")]
    UnequalSlots {
        /// The first slot's key.
        first: &'static str,
        /// The second slot's key.
        second: &'static str,
    },
    /// The region the chip starts from is not where the chip starts.
    #[error("layout.{region} does not start where the chip starts")]
    BootRegion {
        /// The region's key.
        region: &'static str,
    },
    /// The pins are not in the family's key, or are in both.
    #[error(
        "the {family} family counts its pins in its own key: gpio.ports for stm32u5, gpio.pins for esp32"
    )]
    GpioKind {
        /// The chip's family.
        family: Family,
    },
    /// A port is not a letter A to K with 1 to 16 pins.
    #[error("gpio.ports has port {port:?}, which is not a letter A to K with 1 to 16 pins")]
    BadPort {
        /// The port the file gives.
        port: String,
    },
    /// A reserved pin is not a pin of the chip.
    #[error("gpio.reserved names {pin:?}, which is not a pin of this chip")]
    BadPin {
        /// The pin the file gives.
        pin: String,
    },
    /// The firmware crate is not a directory under `firmware/`.
    #[error("firmware.crate {path:?} is not a directory under firmware/")]
    FirmwareCrate {
        /// The directory the file gives.
        path: String,
    },
    /// The firmware does not hold the engine.
    #[error("firmware.components does not list engine")]
    NoEngine,
    /// No document is named.
    #[error("sources.documents is empty")]
    NoDocuments,
    /// `sources.measured_on` is missing although values are measured, or given although none are.
    #[error("sources.measured_on is given exactly when sources.measured is not empty")]
    MeasuredOn,
    /// A source path names no key of the file outside `[sources]`.
    #[error("the source path {path:?} names no key of this file outside [sources]")]
    UnknownSourcePath {
        /// The path.
        path: String,
    },
    /// A source path is listed twice, or inside another listed path.
    #[error("the source path {path:?} is listed twice or inside another listed path")]
    RepeatedSourcePath {
        /// The path.
        path: String,
    },
    /// The event store's capacities do not form the range of Appendix H.5 rule 11.
    #[error(
        "the event store's default_capacity {default_capacity} and max_capacity {max_capacity} are not 1 <= default_capacity <= max_capacity <= 1000000"
    )]
    EventStoreRange {
        /// `events.default_capacity`.
        default_capacity: u32,
        /// `events.max_capacity`.
        max_capacity: u32,
    },
}
