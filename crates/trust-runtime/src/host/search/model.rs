//! What `comm.search.status` reports (specification 11, "One search, device identity, and
//! presence"): the search, its sources, and its devices with every protocol result. Pure data.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::connectors::DiscoveryConfidence;

/// The schema version of `comm.search.status`.
pub const SEARCH_SCHEMA_VERSION: u32 = 1;

/// A row of the source table: the IDs that `only` names, that `sources` reports, and the
/// protocol of a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceId {
    /// truST runtimes.
    TrustRuntime,
    /// ADS and TwinCAT devices.
    Ads,
    /// Modbus TCP servers.
    ModbusTcp,
    /// MQTT brokers.
    Mqtt,
    /// OPC UA servers.
    Opcua,
    /// EtherCAT SubDevices, a fieldbus scan.
    Ethercat,
    /// GPIO chips and lines.
    Gpio,
    /// Serial ports.
    Serial,
    /// USB devices.
    Usb,
    /// Bluetooth LE devices.
    Bluetooth,
}

impl SourceId {
    /// Every source, in the order of the source table.
    pub const ALL: [SourceId; 10] = [
        SourceId::TrustRuntime,
        SourceId::Ads,
        SourceId::ModbusTcp,
        SourceId::Mqtt,
        SourceId::Opcua,
        SourceId::Ethercat,
        SourceId::Gpio,
        SourceId::Serial,
        SourceId::Usb,
        SourceId::Bluetooth,
    ];

    /// The source's ID on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TrustRuntime => "trust_runtime",
            Self::Ads => "ads",
            Self::ModbusTcp => "modbus_tcp",
            Self::Mqtt => "mqtt",
            Self::Opcua => "opcua",
            Self::Ethercat => "ethercat",
            Self::Gpio => "gpio",
            Self::Serial => "serial",
            Self::Usb => "usb",
            Self::Bluetooth => "bluetooth",
        }
    }
}

/// Where a search runs: on the tool's own computer, or inside a runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchOrigin {
    /// The computer of the command line or the language server.
    ThisHost,
    /// Inside a runtime, which reaches the networks and buses of its computer.
    Runtime,
}

/// Whether a search is still running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchState {
    /// At least one source is still searching and the deadline has not passed.
    Running,
    /// Every source finished, or the deadline passed.
    Done,
    /// The search was cancelled before it ended.
    Cancelled,
}

/// The progress of one source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceState {
    /// Still running.
    Searching,
    /// Finished before the search ended.
    Found,
    /// Still running when the deadline passed; its results so far are kept.
    TimedOut,
    /// Still running when the search was cancelled; its results so far are kept.
    Cancelled,
    /// Did not run; `reason` says why.
    Skipped,
}

/// The answer of `comm.search.status`.
#[derive(Debug, Clone, Serialize)]
pub struct SearchStatus {
    /// [`SEARCH_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The search ID that `comm.search.start` answered.
    pub search_id: String,
    /// Where the search runs.
    pub origin: SearchOrigin,
    /// Whether it still runs.
    pub state: SearchState,
    /// The deadline of the search.
    pub deadline_ms: u64,
    /// The time since the start, until the search ended.
    pub elapsed_ms: u64,
    /// The change counter; pass it as `since` to read only what changes after this answer.
    pub cursor: u64,
    /// Every requested source, in the order of the source table.
    pub sources: Vec<SourceProgress>,
    /// The devices, all of them or those changed after `since`.
    pub devices: Vec<FoundDevice>,
    /// The devices that joined another after `since`; absent without `since`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub merged: Vec<MergedDevice>,
    /// Warnings about the whole search.
    pub warnings: Vec<String>,
}

/// One source's progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceProgress {
    /// The source.
    pub id: SourceId,
    /// Its progress.
    pub state: SourceState,
    /// The results it has; absent when it was skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Why it was skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// What it could not search, for example a known address that did not answer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// A device: every result that shares an address, an AMS Net ID, a USB serial number or a truST
/// runtime ID.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FoundDevice {
    /// The device ID; it stays the same for the whole search.
    pub id: String,
    /// Its network addresses.
    pub addresses: Vec<String>,
    /// Its AMS Net IDs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ams_net_ids: Vec<String>,
    /// Its USB serial numbers.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub usb_serials: Vec<String>,
    /// The truST runtime IDs found on it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub runtime_ids: Vec<String>,
    /// When any of its results was last seen, in milliseconds since the Unix epoch.
    pub last_seen_ms: u64,
    /// Every protocol result found on it.
    pub finds: Vec<ProtocolFind>,
}

/// A device that joined another one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MergedDevice {
    /// The device ID that no longer exists.
    pub id: String,
    /// The device that holds its results now.
    pub into: String,
}

/// One protocol result on a device.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProtocolFind {
    /// The protocol that was found.
    pub protocol: SourceId,
    /// How it was found, for example `mdns`, `ads_identify` or `tcp_connect`.
    pub source: String,
    /// The evidence, from the closed confidence vocabulary.
    pub confidence: DiscoveryConfidence,
    /// A simulated (mock) result, never hardware.
    pub simulated: bool,
    /// What the protocol reads about the device.
    pub identity: Identity,
    /// The setup parameters for a connection to it, as `comm.discover` reports them.
    pub params: Value,
    /// What the result could not prove.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// When it was last seen, in milliseconds since the Unix epoch.
    pub last_seen_ms: u64,
    /// What this result shares with the other results of its device.
    #[serde(skip)]
    pub(crate) keys: Vec<DeviceKey>,
    /// The result's own identity within its protocol: the same key is the same result.
    #[serde(skip)]
    pub(crate) key: String,
}

impl ProtocolFind {
    /// A result of `protocol`, seen at `last_seen_ms`, without device keys yet.
    pub(crate) fn new(
        protocol: SourceId,
        source: impl Into<String>,
        confidence: DiscoveryConfidence,
        identity: Identity,
        params: Value,
        last_seen_ms: u64,
    ) -> Self {
        Self {
            protocol,
            source: source.into(),
            confidence,
            simulated: false,
            identity,
            params,
            warnings: Vec::new(),
            last_seen_ms,
            keys: Vec::new(),
            key: String::new(),
        }
    }

    /// The same result, which is the same result as an earlier one with the same `key` and
    /// belongs to the device that shares one of `keys`.
    pub(crate) fn keyed(mut self, key: String, keys: Vec<DeviceKey>) -> Self {
        self.key = key;
        self.keys = keys;
        self
    }

    /// The same result with `warnings`.
    pub(crate) fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    /// The same result, marked simulated.
    pub(crate) fn simulated(mut self, simulated: bool) -> Self {
        self.simulated = simulated;
        self
    }
}

/// What two results of one device can share (specification 11, "Devices"). A result with none
/// of the first four forms its own device by a `Local` key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum DeviceKey {
    /// A network address.
    Address(String),
    /// An AMS Net ID.
    AmsNetId(String),
    /// A USB serial number.
    UsbSerial(String),
    /// A truST runtime ID.
    RuntimeId(String),
    /// A bus-local identity: an EtherCAT adapter and position, a GPIO chip, a serial port, or the
    /// bus and port of a USB device without a serial number.
    Local(String),
}

impl DeviceKey {
    /// The ID of a device that this key names first.
    pub(crate) fn device_id(&self) -> String {
        match self {
            Self::Address(address) => format!("dev:ip:{address}"),
            Self::AmsNetId(net_id) => format!("dev:netid:{net_id}"),
            Self::UsbSerial(serial) => format!("dev:usb:{serial}"),
            Self::RuntimeId(id) => format!("dev:runtime:{id}"),
            Self::Local(local) => format!("dev:{local}"),
        }
    }
}

/// What a protocol reads about a device; `kind` names the protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Identity {
    /// A truST runtime.
    TrustRuntime(RuntimeIdentity),
    /// An ADS device.
    Ads(AdsIdentity),
    /// A Modbus server.
    Modbus(ModbusIdentity),
    /// An MQTT broker.
    Mqtt(MqttIdentity),
    /// One OPC UA server endpoint.
    Opcua(OpcuaIdentity),
    /// An EtherCAT SubDevice.
    Ethercat(EthercatIdentity),
    /// A GPIO line.
    Gpio(GpioIdentity),
    /// A USB device.
    Usb(UsbIdentity),
    /// A serial port.
    Serial(SerialIdentity),
}

/// A truST runtime's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeIdentity {
    /// The runtime name.
    pub name: String,
    /// The runtime ID it announces.
    pub id: String,
    /// Its control endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
    /// Its web address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web: Option<String>,
    /// The host group it announces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_group: Option<String>,
    /// Its mesh port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_port: Option<u16>,
}

/// An ADS device's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdsIdentity {
    /// Its AMS Net ID.
    pub ams_net_id: String,
    /// Its computer name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computer_name: Option<String>,
    /// Its TwinCAT version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twincat_version: Option<String>,
}

/// A Modbus server's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModbusIdentity {
    /// The unit ID that answered.
    pub unit_id: u8,
    /// The vendor name from the FC43/14 basic objects, when the device answers them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_name: Option<String>,
    /// The product code from the FC43/14 basic objects, when the device answers them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_code: Option<String>,
    /// The revision from the FC43/14 basic objects, when the device answers them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

/// An MQTT broker's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MqttIdentity {
    /// Whether the broker port is the TLS port.
    pub tls: bool,
    /// Whether the broker needs a login; absent when the probe could not tell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_required: Option<bool>,
}

/// One endpoint of an OPC UA server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpcuaIdentity {
    /// The endpoint URL.
    pub endpoint_url: String,
    /// Its security policy; absent when the server announced one truST does not use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_policy: Option<String>,
    /// Its security mode; absent when the server announced one truST does not use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_mode: Option<String>,
    /// The server's application name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_name: Option<String>,
    /// The server's application URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_uri: Option<String>,
    /// The server's product URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_uri: Option<String>,
    /// Whether the endpoint uses a security policy or mode truST cannot use.
    pub unsupported: bool,
    /// The SHA-256 fingerprint of the server certificate announced for the endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate_sha256: Option<String>,
}

/// An EtherCAT SubDevice's identity (specification 11, "Identity").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EthercatIdentity {
    /// The bus adapter.
    pub adapter: String,
    /// Its slot on the bus.
    pub slot: u16,
    /// Its configured station address.
    pub configured_address: u16,
    /// Its alias address.
    pub alias: u16,
    /// Its name.
    pub model: String,
    /// Its channel count.
    pub channels: u16,
    /// Its vendor ID.
    pub vendor_id: u32,
    /// Its product code.
    pub product_id: u32,
    /// Its revision.
    pub revision: u32,
    /// Its serial number.
    pub serial: u32,
    /// The bus's mapped input bytes.
    pub input_bytes: usize,
    /// The bus's mapped output bytes.
    pub output_bytes: usize,
}

/// A GPIO line's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GpioIdentity {
    /// The GPIO chip.
    pub chip: String,
    /// The chip's label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chip_label: Option<String>,
    /// The line number.
    pub line: u32,
    /// The line's direction, when it is in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
}

/// What a USB device is, as truST recognizes it (specification 11, "USB and serial").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbRole {
    /// A supported chip's own boot loader, through which truST is installed: the chip has no
    /// truST yet.
    BootLoader,
    /// A supported chip's own USB controller, which shows the same descriptors with or without
    /// truST: truST is not confirmed until a connection confirms it.
    ChipController,
    /// A truST runtime, by truST's own USB identity.
    TrustRuntime,
    /// A debug probe.
    DebugProbe,
    /// A USB-serial adapter that truST knows.
    SerialAdapter,
    /// Another device that has a serial port.
    SerialDevice,
}

/// A USB device's identity: what the descriptors that the operating system keeps say, and what
/// truST recognizes in them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbIdentity {
    /// The vendor ID.
    pub vid: u16,
    /// The product ID.
    pub pid: u16,
    /// The maker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// The product name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    /// The serial number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    /// What the device is.
    pub role: UsbRole,
    /// What truST calls it, such as `ST-LINK/V2-1` or `STM32 boot loader (DFU)`; for a device
    /// truST does not know, its product name. The search names no chip: the chip files say which
    /// chips show the IDs of a boot loader or a chip's USB controller.
    pub model: String,
    /// The truST version that a truST runtime's descriptor carries: its major and minor version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Where a connection reaches the truST runtime that the device runs or may run: `usb://` and
    /// its serial number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
    /// The serial ports behind it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<String>,
}

/// How a serial port is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SerialBus {
    /// Through a USB device.
    Usb,
    /// On a PCI card.
    Pci,
    /// Over Bluetooth.
    Bluetooth,
    /// Another way, such as a UART of the computer's own chip.
    Other,
}

/// A serial port's identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SerialIdentity {
    /// The port's name, such as `/dev/ttyUSB0` or `COM3`.
    pub port: String,
    /// How it is attached.
    pub bus: SerialBus,
    /// The USB device behind it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usb: Option<PortUsbDevice>,
}

/// The USB device behind a serial port, as the port listing describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortUsbDevice {
    /// The vendor ID.
    pub vid: u16,
    /// The product ID.
    pub pid: u16,
    /// The maker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    /// The product name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    /// The serial number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
}
