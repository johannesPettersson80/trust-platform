//! The serial source of the one search (specification 11, "USB and serial"): every serial port
//! that the operating system lists, with the USB device behind it when there is one. It never
//! opens a port. `serialport::available_ports` reads what the operating system lists: on Linux,
//! built without libudev, the folder `/sys/class/tty` and the sysfs attributes of the USB device
//! behind each port; on macOS IOKit; on Windows the device manager.
//!
//! The listing is a seam (`PortListing`): the source decodes plain data, so its tests (with the
//! USB source's, in `usb_tests.rs`) run without the ports of a computer. The USB source reads the
//! same listing to name the ports behind each USB device.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::connectors::DiscoveryConfidence;

use super::super::model::{
    DeviceKey, Identity, PortUsbDevice, ProtocolFind, SerialBus, SerialIdentity, SourceId,
};
use super::super::runner::SourceSink;

/// How a serial result is found.
const SOURCE: &str = "port_listing";
/// Linux's folder of the terminal devices. `serialport` reads it and panics without it.
const TTY_CLASS: &str = "/sys/class/tty";

/// A serial port as the operating system lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListedPort {
    /// The port's name, such as `/dev/ttyUSB0` or `COM3`.
    pub(crate) name: String,
    /// How it is attached.
    pub(crate) bus: SerialBus,
    /// The USB device behind it.
    pub(crate) usb: Option<PortUsbDevice>,
}

/// What lists the serial ports: the operating system, or a test's list. A listing only lists; it
/// has no way to open a port.
pub(crate) trait PortListing {
    /// The serial ports; the error says why there is no list.
    fn serial_ports(&self) -> Result<Vec<ListedPort>, String>;
}

/// The operating system's listings.
pub(crate) struct SystemListing;

impl PortListing for SystemListing {
    fn serial_ports(&self) -> Result<Vec<ListedPort>, String> {
        if let Some(reason) = unavailable() {
            return Err(reason);
        }
        let ports = serialport::available_ports()
            .map_err(|error| format!("The serial ports could not be listed: {error}."))?;
        Ok(ports.into_iter().filter(uart_present).map(listed).collect())
    }
}

/// Why this computer offers no serial port listing, when it offers none. On Linux the listing
/// reads `/sys/class/tty`, and `serialport` panics without that folder, so it is looked for first.
pub(crate) fn unavailable() -> Option<String> {
    (cfg!(target_os = "linux") && !Path::new(TTY_CLASS).is_dir())
        .then(|| format!("Serial ports cannot be listed on this computer: {TTY_CLASS} is missing."))
}

/// Whether a port's UART is there. Linux lists a `ttyS` port for every place where an 8250 UART
/// may sit, and reports type 0 (`PORT_UNKNOWN`) in `/sys/class/tty/<name>/type` for one without
/// a UART. Reading that attribute does not open the port.
fn uart_present(port: &serialport::SerialPortInfo) -> bool {
    if !cfg!(target_os = "linux") || port.port_type != serialport::SerialPortType::Unknown {
        return true;
    }
    let Some(name) = Path::new(&port.port_name).file_name() else {
        return true;
    };
    std::fs::read_to_string(Path::new(TTY_CLASS).join(name).join("type"))
        .ok()
        .is_none_or(|kind| kind.trim() != "0")
}

/// A port of `serialport`'s listing, with the USB device behind it.
fn listed(port: serialport::SerialPortInfo) -> ListedPort {
    let (bus, usb) = match port.port_type {
        serialport::SerialPortType::UsbPort(info) => (
            SerialBus::Usb,
            Some(PortUsbDevice {
                vid: info.vid,
                pid: info.pid,
                manufacturer: text(info.manufacturer),
                product: text(info.product),
                serial_number: text(info.serial_number),
            }),
        ),
        serialport::SerialPortType::PciPort => (SerialBus::Pci, None),
        serialport::SerialPortType::BluetoothPort => (SerialBus::Bluetooth, None),
        serialport::SerialPortType::Unknown => (SerialBus::Other, None),
    };
    ListedPort {
        name: port.port_name,
        bus,
        usb,
    }
}

/// A string that the operating system keeps, trimmed; `None` when it is empty.
pub(super) fn text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// The serial source: it lists the ports once and reports each of them.
pub(crate) fn search(sink: &SourceSink) {
    run(&SystemListing, sink);
}

/// Reports every port of `listing`, or why there is no list.
pub(crate) fn run(listing: &dyn PortListing, sink: &SourceSink) {
    match listing.serial_ports() {
        Ok(ports) => {
            for find in finds(&ports, now_ms()) {
                sink.found(find);
            }
        }
        Err(reason) => sink.warn(reason),
    }
}

/// The result of each port, seen at `seen_ms`. A port forms a device by its name, which the USB
/// device behind it shares, and by that device's serial number when it has one.
pub(crate) fn finds(ports: &[ListedPort], seen_ms: u64) -> Vec<ProtocolFind> {
    ports.iter().map(|port| port_find(port, seen_ms)).collect()
}

fn port_find(port: &ListedPort, seen_ms: u64) -> ProtocolFind {
    let mut keys = vec![port_key(&port.name)];
    keys.extend(
        port.usb
            .as_ref()
            .and_then(|usb| usb.serial_number.clone())
            .map(DeviceKey::UsbSerial),
    );
    let identity = Identity::Serial(SerialIdentity {
        port: port.name.clone(),
        bus: port.bus,
        usb: port.usb.clone(),
    });
    ProtocolFind::new(
        SourceId::Serial,
        SOURCE,
        DiscoveryConfidence::Confirmed,
        identity,
        json!({ "port": port.name }),
        seen_ms,
    )
    .keyed(port.name.clone(), keys)
}

/// The key that a serial port and the USB device behind it share.
pub(super) fn port_key(name: &str) -> DeviceKey {
    DeviceKey::Local(format!("serial:{name}"))
}

/// Now, in milliseconds since the Unix epoch.
pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}
