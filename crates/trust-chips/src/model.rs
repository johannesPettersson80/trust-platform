//! The tables and keys of a chip file (specification 34, Appendix H.2). Every table refuses a key
//! the format does not define, and every closed list of words is an enum.

use core::fmt;

use serde::Deserialize;

/// One chip description file, `chips/<chip>.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChipFile {
    /// Who the chip is and how it is recognized.
    pub identity: Identity,
    /// The processor.
    pub processor: Processor,
    /// The clock limits, and the clock the firmware runs the processor at.
    pub clock: Clock,
    /// Flash and RAM.
    pub memory: Memory,
    /// The event store the chip allows.
    pub events: Events,
    /// What the truST firmware places where in the flash.
    pub layout: Layout,
    /// The chip's peripherals.
    pub interfaces: Interfaces,
    /// The chip's pins.
    pub gpio: Gpio,
    /// The truST firmware of the chip.
    pub firmware: Firmware,
    /// Where the values come from.
    pub sources: Sources,
}

/// `[identity]`: the chip's name and family, and how the firmware and the tools recognize it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// The file's name without `.toml`; `build-firmware` takes it.
    pub name: String,
    /// The manufacturer's name for the chip.
    pub model: String,
    /// The platform adapter that drives the chip.
    pub family: Family,
    /// The number the chip reports for itself: `DEV_ID` of `DBGMCU_IDCODE` in the STM32
    /// families, the chip ID of the ROM and the image header in the `esp32` family.
    pub device_id: u32,
    /// The USB IDs the chip shows while its own boot loader runs.
    pub usb_bootloader: UsbId,
}

/// A USB vendor and product ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbId {
    /// The vendor ID.
    pub vid: u16,
    /// The product ID.
    pub pid: u16,
}

/// The chip families truST has a platform adapter for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    /// The STM32U5 chips, driven by `crates/trust-platform-stm32u5`.
    Stm32u5,
    /// The RISC-V ESP32 chips, driven by `crates/trust-platform-esp32`.
    Esp32,
}

impl Family {
    /// The family's word in a chip file.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stm32u5 => "stm32u5",
            Self::Esp32 => "esp32",
        }
    }
}

impl fmt::Display for Family {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// `[processor]`: the processor core and what the firmware is built for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Processor {
    /// The processor core.
    pub core: String,
    /// Its instruction set.
    pub isa: String,
    /// The Rust target the firmware is built for.
    pub rust_target: String,
    /// The floating-point unit.
    pub fpu: Fpu,
}

/// The precision of a processor's floating-point unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fpu {
    /// No floating-point unit: `REAL` and `LREAL` use software arithmetic.
    None,
    /// Single precision.
    Single,
    /// Single and double precision.
    Double,
}

/// `[clock]`: the clock limits of the chip and the clock of the firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    /// The highest processor clock of the data sheet, in Hz.
    pub max_hz: u32,
    /// The processor clock the truST firmware configures, in Hz.
    pub firmware_hz: u32,
    /// The slowest crystal the oscillator accepts, in Hz.
    pub crystal_min_hz: u32,
    /// The fastest crystal the oscillator accepts, in Hz.
    pub crystal_max_hz: u32,
}

/// `[memory]`: the flash, in one of two tables, and the RAM regions.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    /// Flash in the chip's address space (the STM32 families).
    pub flash: Option<Flash>,
    /// Flash on the chip's SPI flash interface (the `esp32` family).
    pub spi_flash: Option<SpiFlash>,
    /// The RAM regions.
    pub ram: Vec<RamRegion>,
}

/// `[memory.flash]`: flash in the chip's address space.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flash {
    /// The address of its first byte.
    pub start: u32,
    /// Its size in KiB.
    pub kib: u32,
    /// Its erase sectors in address order.
    pub sectors: Vec<SectorRun>,
}

/// A run of erase sectors of one size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectorRun {
    /// How many sectors the run has.
    pub count: u32,
    /// The size of each, in KiB.
    pub kib: u32,
}

/// `[memory.spi_flash]`: flash on the chip's SPI flash interface, in the package or the module.
/// Its size is the device's own and is read when truST is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpiFlash {
    /// The erase unit in KiB.
    pub erase_kib: u32,
}

/// One `[[memory.ram]]` region.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RamRegion {
    /// Its name in the reference manual.
    pub name: String,
    /// The address of its first byte.
    pub start: u32,
    /// Its size in KiB.
    pub kib: u32,
    /// Whether DMA reaches it.
    pub dma: bool,
}

impl RamRegion {
    /// The region's size in bytes.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        u64::from(self.kib) * 1024
    }

    /// The address one past its last byte.
    #[must_use]
    pub fn end(&self) -> u64 {
        u64::from(self.start) + self.bytes()
    }
}

/// `[events]`: the event store a device without an operating system keeps (specification 34,
/// Appendix H.2 and H.5; specification 11, the event store).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Events {
    /// The newest events the store keeps when the project sets none.
    pub default_capacity: u32,
    /// The most a project may set.
    pub max_capacity: u32,
}

/// `[layout]`: what the truST firmware places where in the flash. Addresses are bus addresses
/// in `memory.flash` and flash offsets in `memory.spi_flash`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    /// The vector table (the STM32 families).
    pub vectors: Option<Region>,
    /// The second-stage boot loader (the `esp32` family).
    pub bootloader: Option<Region>,
    /// The partition table (the `esp32` family).
    pub partition_table: Option<Region>,
    /// The truST firmware.
    pub firmware: Region,
    /// The separately installed application.
    pub application: Region,
    /// The first of the program store's two slots, inside `application` (specification 34 §5.1).
    /// A file names both program slots or neither.
    pub application_a: Option<Region>,
    /// The second of the program store's two slots, the other half of `application`.
    pub application_b: Option<Region>,
    /// The first checkpoint slot of the retain store.
    pub checkpoint_a: Region,
    /// The second checkpoint slot of the retain store.
    pub checkpoint_b: Region,
    /// The first of the two sectors in which the chip keeps its boot counter (specification 34
    /// PORT-05 and Appendix D.2): exactly one erase unit. A file names both counter sectors or
    /// neither.
    pub boot_counter_a: Option<Region>,
    /// The second boot counter sector, also exactly one erase unit.
    pub boot_counter_b: Option<Region>,
}

impl Layout {
    /// The regions the layout has, each with its key, in the order of the format.
    #[must_use]
    pub fn regions(&self) -> Vec<(&'static str, Region)> {
        [
            ("vectors", self.vectors),
            ("bootloader", self.bootloader),
            ("partition_table", self.partition_table),
            ("firmware", Some(self.firmware)),
            ("application", Some(self.application)),
            ("checkpoint_a", Some(self.checkpoint_a)),
            ("checkpoint_b", Some(self.checkpoint_b)),
            ("boot_counter_a", self.boot_counter_a),
            ("boot_counter_b", self.boot_counter_b),
        ]
        .into_iter()
        .filter_map(|(key, region)| region.map(|region| (key, region)))
        .collect()
    }
}

/// A region of the flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// The address, or the flash offset, of its first byte.
    pub start: u32,
    /// Its size in KiB.
    pub kib: u32,
}

impl Region {
    /// The region's size in bytes.
    #[must_use]
    pub fn bytes(self) -> u64 {
        u64::from(self.kib) * 1024
    }

    /// The address, or the flash offset, one past its last byte.
    #[must_use]
    pub fn end(self) -> u64 {
        u64::from(self.start) + self.bytes()
    }
}

/// `[interfaces]`: the chip's peripherals of each kind, by the manufacturer's names. An empty
/// list means the chip has none.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interfaces {
    /// USB controllers.
    pub usb: Vec<String>,
    /// Ethernet controllers; the PHY is the board's.
    pub ethernet: Vec<String>,
    /// Wi-Fi radios.
    pub wifi: Vec<String>,
    /// Bluetooth radios.
    pub bluetooth: Vec<String>,
    /// UARTs.
    pub uart: Vec<String>,
    /// CAN controllers.
    pub can: Vec<String>,
    /// SPI controllers the firmware can give to a project.
    pub spi: Vec<String>,
    /// I2C controllers.
    pub i2c: Vec<String>,
    /// Analog-to-digital converters.
    pub adc: Vec<String>,
    /// Digital-to-analog converters.
    pub dac: Vec<String>,
    /// Hardware timers.
    pub timers: Vec<String>,
    /// Peripherals that produce PWM.
    pub pwm: Vec<String>,
}

impl Interfaces {
    /// Every list with its key, in the order of the format.
    #[must_use]
    pub fn lists(&self) -> [(&'static str, &[String]); 12] {
        [
            ("interfaces.usb", self.usb.as_slice()),
            ("interfaces.ethernet", self.ethernet.as_slice()),
            ("interfaces.wifi", self.wifi.as_slice()),
            ("interfaces.bluetooth", self.bluetooth.as_slice()),
            ("interfaces.uart", self.uart.as_slice()),
            ("interfaces.can", self.can.as_slice()),
            ("interfaces.spi", self.spi.as_slice()),
            ("interfaces.i2c", self.i2c.as_slice()),
            ("interfaces.adc", self.adc.as_slice()),
            ("interfaces.dac", self.dac.as_slice()),
            ("interfaces.timers", self.timers.as_slice()),
            ("interfaces.pwm", self.pwm.as_slice()),
        ]
    }
}

/// `[gpio]`: the chip's pins on its largest package, and the pins the firmware keeps.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gpio {
    /// The GPIO ports and their sizes (the STM32 families); a pin is named like `PD12`.
    pub ports: Option<Vec<PortPins>>,
    /// The number of GPIOs (the `esp32` family); a pin is named like `GPIO18`.
    pub pins: Option<u32>,
    /// The pins the firmware keeps for itself, which a project cannot use.
    pub reserved: Vec<ReservedPin>,
}

/// One GPIO port and how many pins it has.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortPins {
    /// The port's letter.
    pub port: String,
    /// Its pins, counted from 0.
    pub pins: u32,
}

/// A pin the firmware keeps, and why.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservedPin {
    /// The pin's name in its family's form.
    pub pin: String,
    /// What uses it.
    pub reason: PinReason,
}

/// Why the firmware keeps a pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinReason {
    /// The console UART.
    Console,
    /// The debug port.
    Debug,
    /// The crystal oscillator.
    Oscillator,
    /// The chip's USB port.
    Usb,
}

/// `[firmware]`: the truST firmware of the chip.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Firmware {
    /// The firmware crate's directory, relative to the repository root.
    #[serde(rename = "crate")]
    pub crate_dir: String,
    /// The firmware binary.
    pub bin: String,
    /// The runtime components the firmware holds.
    pub components: Vec<Component>,
}

/// A runtime component (specification 34, ARCH-14 and Appendix H.3): the engine, or one runtime
/// service of ARCH-11 above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Component {
    /// The engine, `trust-runtime-core`: bytecode execution, tasks and cycles, the process image.
    Engine,
    /// The control connection and its protocol.
    Control,
    /// Live Values: read, write, force and release.
    LiveValues,
    /// Debugging.
    Debug,
    /// Program loading, including online change.
    Deploy,
    /// Access rights: tokens and roles, TLS and signed deployment.
    Access,
    /// Discovery and the mesh.
    Discovery,
    /// The communication services: the `io.toml` drivers and Bluetooth devices.
    Communication,
    /// The web pages and the HMI.
    WebHmi,
    /// The `modbus-rtu` driver.
    ModbusRtu,
    /// The `ethercat` driver.
    Ethercat,
    /// The OPC UA server and client.
    Opcua,
    /// The Beckhoff ADS client and server.
    Ads,
    /// The runtime cloud, the fleet and pairing.
    Cloud,
    /// The historian with its alerts, and the Prometheus metrics.
    Observability,
    /// The `OpenOT` telemetry.
    Openot,
    /// Simulation.
    Simulation,
}

impl Component {
    /// Every component, in the order of the format.
    pub const ALL: [Self; 17] = [
        Self::Engine,
        Self::Control,
        Self::LiveValues,
        Self::Debug,
        Self::Deploy,
        Self::Access,
        Self::Discovery,
        Self::Communication,
        Self::WebHmi,
        Self::ModbusRtu,
        Self::Ethercat,
        Self::Opcua,
        Self::Ads,
        Self::Cloud,
        Self::Observability,
        Self::Openot,
        Self::Simulation,
    ];

    /// The component's word in a chip file.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Engine => "engine",
            Self::Control => "control",
            Self::LiveValues => "live-values",
            Self::Debug => "debug",
            Self::Deploy => "deploy",
            Self::Access => "access",
            Self::Discovery => "discovery",
            Self::Communication => "communication",
            Self::WebHmi => "web-hmi",
            Self::ModbusRtu => "modbus-rtu",
            Self::Ethercat => "ethercat",
            Self::Opcua => "opcua",
            Self::Ads => "ads",
            Self::Cloud => "cloud",
            Self::Observability => "observability",
            Self::Openot => "openot",
            Self::Simulation => "simulation",
        }
    }
}

/// `[sources]`: where the values come from (specification 34, Appendix H.4).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    /// The manufacturer's documents the values come from.
    pub documents: Vec<String>,
    /// Key paths of the values read on a chip.
    pub measured: Vec<String>,
    /// On which device, and when, the measured values were read.
    pub measured_on: Option<String>,
    /// Key paths of manufacturer values the repository's records do not yet confirm.
    pub unchecked: Vec<String>,
}
