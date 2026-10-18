//! The USB source of the one search (specification 11, "USB and serial"): the USB devices that the
//! operating system lists, recognized by their vendor and product IDs, each with the serial ports
//! behind it. The listing is passive (owner, 15 October 2026; specification 34 TARGET-04): it
//! reads the descriptors that the operating system already holds and never opens, claims, resets
//! or writes a device.
//!
//! - `nusb::list_devices` reads sysfs on Linux (`/sys/bus/usb/devices`), IOKit on macOS, and on
//!   Windows the device manager and the hub's cached connection information; `wait` runs the
//!   listing on the source's thread.
//! - Only the strings that the operating system keeps are read (`DeviceInfo::manufacturer_string`,
//!   `product_string` and `serial_number`). The `get_` methods, which may ask the device for a
//!   string, are never called.
//! - The listing is a seam (`UsbListing`): the decoding takes plain data, so the tests run it
//!   without a bus.
//! - A boot loader or a chip's USB controller is recognized by its IDs, but the search names no
//!   chip: several chips may show one ID, and the chip files say which ones in their
//!   `identity.usb_bootloader`. The panel lists them from the chip files; the chip itself is
//!   confirmed when connecting and before any installation.

use nusb::MaybeFuture;
use serde_json::json;

use crate::connectors::DiscoveryConfidence;

use super::super::model::{DeviceKey, Identity, ProtocolFind, SourceId, UsbIdentity, UsbRole};
use super::super::runner::SourceSink;
use super::serial::{self, ListedPort, PortListing, SystemListing};

/// How a USB result is found.
const SOURCE: &str = "usb_descriptor";
/// What truST calls a truST runtime on USB.
const TRUST_RUNTIME: &str = "truST runtime";
/// The maker that truST's descriptors name.
const TRUST_MANUFACTURER: &str = "truST";
/// What truST calls a device it does not know that has a serial port and names no product.
const UNNAMED_SERIAL_DEVICE: &str = "USB serial device";
/// The longest serial number that a `usb://` endpoint names.
const MAX_ENDPOINT_SERIAL: usize = 126;

/// A USB device as the operating system lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListedUsbDevice {
    /// The vendor ID.
    pub(crate) vid: u16,
    /// The product ID.
    pub(crate) pid: u16,
    /// `bcdDevice`, the device's release number.
    pub(crate) release: u16,
    /// The maker.
    pub(crate) manufacturer: Option<String>,
    /// The product name.
    pub(crate) product: Option<String>,
    /// The serial number.
    pub(crate) serial_number: Option<String>,
    /// The bus and the ports on the way to it, such as `001-1.4`.
    pub(crate) location: String,
}

/// What lists the USB devices and the serial ports: the operating system, or a test's list. A
/// listing only lists; it has no way to open a device.
pub(crate) trait UsbListing: PortListing {
    /// The USB devices; the error says why there is no list.
    fn usb_devices(&self) -> Result<Vec<ListedUsbDevice>, String>;
}

impl UsbListing for SystemListing {
    fn usb_devices(&self) -> Result<Vec<ListedUsbDevice>, String> {
        let devices = nusb::list_devices()
            .wait()
            .map_err(|error| format!("The USB devices could not be listed: {error}."))?;
        Ok(devices.map(|device| listed(&device)).collect())
    }
}

/// A device of `nusb`'s listing, read from what the operating system keeps.
fn listed(device: &nusb::DeviceInfo) -> ListedUsbDevice {
    let ports: Vec<String> = device.port_chain().iter().map(u8::to_string).collect();
    ListedUsbDevice {
        vid: device.vendor_id(),
        pid: device.product_id(),
        release: device.device_version(),
        manufacturer: serial::text(device.manufacturer_string().map(str::to_owned)),
        product: serial::text(device.product_string().map(str::to_owned)),
        serial_number: serial::text(device.serial_number().map(str::to_owned)),
        location: format!("{}-{}", device.bus_id(), ports.join(".")),
    }
}

/// The USB source: it lists the devices once and reports each that truST recognizes or that has
/// a serial port.
pub(crate) fn search(sink: &SourceSink) {
    run(&SystemListing, sink);
}

/// Reports the devices of `listing` with the ports behind them. Without a port listing the
/// devices are reported without their ports.
pub(crate) fn run(listing: &dyn UsbListing, sink: &SourceSink) {
    let devices = match listing.usb_devices() {
        Ok(devices) => devices,
        Err(reason) => {
            sink.warn(reason);
            return;
        }
    };
    let ports = listing.serial_ports().unwrap_or_else(|reason| {
        sink.warn(reason);
        Vec::new()
    });
    for find in finds(&devices, &ports, serial::now_ms()) {
        sink.found(find);
    }
}

/// The result of each device that truST recognizes or that has a serial port, seen at
/// `seen_ms`. Another device, such as a phone, a keyboard or a storage device, gives none.
pub(crate) fn finds(
    devices: &[ListedUsbDevice],
    ports: &[ListedPort],
    seen_ms: u64,
) -> Vec<ProtocolFind> {
    devices
        .iter()
        .filter_map(|device| device_find(device, devices, ports, seen_ms))
        .collect()
}

fn device_find(
    device: &ListedUsbDevice,
    devices: &[ListedUsbDevice],
    ports: &[ListedPort],
    seen_ms: u64,
) -> Option<ProtocolFind> {
    let behind = ports_behind(device, devices, ports);
    let known = recognize(device);
    let (role, model) = match known {
        Some(known) => (known.role, known.model.to_owned()),
        None if !behind.is_empty() => (
            UsbRole::SerialDevice,
            device
                .product
                .clone()
                .unwrap_or_else(|| UNNAMED_SERIAL_DEVICE.to_owned()),
        ),
        None => return None,
    };
    let control = match role {
        UsbRole::TrustRuntime | UsbRole::ChipController => {
            device.serial_number.as_deref().and_then(usb_endpoint)
        }
        _ => None,
    };
    let version = if role == UsbRole::TrustRuntime {
        release_version(device.release)
    } else {
        None
    };
    let mut keys: Vec<DeviceKey> = device
        .serial_number
        .iter()
        .cloned()
        .map(DeviceKey::UsbSerial)
        .collect();
    keys.extend(behind.iter().map(|port| serial::port_key(port)));
    if keys.is_empty() {
        keys.push(DeviceKey::Local(format!("usb-port:{}", device.location)));
    }
    let params = control.as_ref().map_or_else(
        || json!({}),
        |control| json!({ "control_endpoint": control }),
    );
    let warnings = known
        .and_then(|known| known.note)
        .map(|note| vec![note.to_owned()])
        .unwrap_or_default();
    let key = format!(
        "{:04x}:{:04x}:{}@{}",
        device.vid,
        device.pid,
        device.serial_number.as_deref().unwrap_or_default(),
        device.location
    );
    let identity = Identity::Usb(UsbIdentity {
        vid: device.vid,
        pid: device.pid,
        manufacturer: device.manufacturer.clone(),
        product: device.product.clone(),
        serial_number: device.serial_number.clone(),
        role,
        model,
        version,
        control,
        ports: behind,
    });
    Some(
        ProtocolFind::new(
            SourceId::Usb,
            SOURCE,
            DiscoveryConfidence::Confirmed,
            identity,
            params,
            seen_ms,
        )
        .with_warnings(warnings)
        .keyed(key, keys),
    )
}

/// The names of the serial ports behind `device`: the ports whose USB device has its vendor ID,
/// product ID and serial number. For a device without a serial number these are the ports with its
/// vendor and product ID, unless another listed device without a serial number has the same pair.
fn ports_behind(
    device: &ListedUsbDevice,
    devices: &[ListedUsbDevice],
    ports: &[ListedPort],
) -> Vec<String> {
    let same_pair = |vid: u16, pid: u16| vid == device.vid && pid == device.pid;
    if device.serial_number.is_none() {
        let alike = devices
            .iter()
            .filter(|other| other.serial_number.is_none() && same_pair(other.vid, other.pid))
            .count();
        if alike > 1 {
            return Vec::new();
        }
    }
    let mut names: Vec<String> = ports
        .iter()
        .filter(|port| {
            port.usb.as_ref().is_some_and(|usb| {
                same_pair(usb.vid, usb.pid) && usb.serial_number == device.serial_number
            })
        })
        .map(|port| port.name.clone())
        .collect();
    names.sort();
    names
}

/// The `usb://` endpoint of a serial number: one of 1 to 126 printable ASCII characters other than
/// space, `/`, `?` and `#`. Another serial number names no endpoint.
fn usb_endpoint(serial_number: &str) -> Option<String> {
    let usable = (1..=MAX_ENDPOINT_SERIAL).contains(&serial_number.len())
        && serial_number
            .chars()
            .all(|ch| ch.is_ascii_graphic() && !matches!(ch, '/' | '?' | '#'));
    usable.then(|| format!("usb://{serial_number}"))
}

/// The major and minor version that truST's descriptor carries in `bcdDevice`, one byte each in
/// BCD: `0x0024` is `0.24`. A byte that is not BCD gives no version.
fn release_version(release: u16) -> Option<String> {
    let [major, minor] = release.to_be_bytes();
    Some(format!("{}.{}", bcd(major)?, bcd(minor)?))
}

fn bcd(byte: u8) -> Option<u8> {
    let (tens, ones) = (byte >> 4, byte & 0x0F);
    (tens < 10 && ones < 10).then_some(tens * 10 + ones)
}

/// The identity that truST recognizes in a device's IDs and, for an ID that others use too, in
/// its strings.
fn recognize(device: &ListedUsbDevice) -> Option<&'static Known> {
    KNOWN.iter().find(|known| {
        known.vid == device.vid && known.pid == device.pid && (!known.shared || says_trust(device))
    })
}

/// Whether the strings say truST: the maker `truST` or the product `truST runtime`.
fn says_trust(device: &ListedUsbDevice) -> bool {
    device.manufacturer.as_deref() == Some(TRUST_MANUFACTURER)
        || device.product.as_deref() == Some(TRUST_RUNTIME)
}

/// One USB identity that truST recognizes.
struct Known {
    vid: u16,
    pid: u16,
    role: UsbRole,
    model: &'static str,
    /// Others use this ID too, so the strings must say truST.
    shared: bool,
    /// What the result cannot prove.
    note: Option<&'static str>,
}

const fn probe(pid: u16, model: &'static str) -> Known {
    Known {
        vid: 0x0483,
        pid,
        role: UsbRole::DebugProbe,
        model,
        shared: false,
        note: None,
    }
}

const fn adapter(vid: u16, pid: u16, model: &'static str) -> Known {
    Known {
        vid,
        pid,
        role: UsbRole::SerialAdapter,
        model,
        shared: false,
        note: None,
    }
}

const fn trust(pid: u16, shared: bool) -> Known {
    Known {
        vid: 0x1209,
        pid,
        role: UsbRole::TrustRuntime,
        model: TRUST_RUNTIME,
        shared,
        note: None,
    }
}

/// The USB identities that truST recognizes, each with the source of its vendor and product ID.
const KNOWN: &[Known] = &[
    // The boot loaders and USB controllers of the chips of truST's chip files. Each is one entry,
    // however many chip files name its IDs as their `identity.usb_bootloader`.
    //
    // ST AN2606 (STM32 system memory boot mode): the system boot loader's USB DFU device, which
    // every STM32 with a USB DFU boot loader shows.
    Known {
        vid: 0x0483,
        pid: 0xDF11,
        role: UsbRole::BootLoader,
        model: "STM32 boot loader (DFU)",
        shared: false,
        note: Some(
            "Every STM32 with a USB boot loader shows these IDs; the chip is confirmed when \
             connecting and before any installation.",
        ),
    },
    // OpenOCD tcl/interface/esp_usb_jtag.cfg, "Espressif builtin USB-JTAG adapter":
    // `adapter usb vid_pid 0x303a 0x1001`. The controller shows these IDs with or without truST,
    // in the ROM's download mode, and on Espressif's other chips with this controller (S1b
    // decision O3).
    Known {
        vid: 0x303A,
        pid: 0x1001,
        role: UsbRole::ChipController,
        model: "Espressif USB-Serial/JTAG",
        shared: false,
        note: Some(
            "This controller shows the same IDs with or without truST, and on other Espressif \
             chips; connecting confirms the chip and truST.",
        ),
    },
    // truST's own IDs (S1b decision O2). 1209:7472 is requested from pid.codes in
    // pidcodes/pidcodes.github.com#1320, "Add 1209:7472 for the truST PLC runtime".
    trust(0x7472, false),
    // pid.codes 1209:0001, "reserved for use in private testing": truST's ID during development.
    // Anyone may test with it, so its strings must say truST too.
    trust(0x0001, true),
    // ST-LINK debug probes: OpenOCD src/jtag/drivers/stlink_usb.c.
    probe(0x3748, "ST-LINK/V2"),   // STLINK_V2_PID
    probe(0x374B, "ST-LINK/V2-1"), // STLINK_V2_1_PID
    probe(0x3752, "ST-LINK/V2-1"), // STLINK_V2_1_NO_MSD_PID
    probe(0x374D, "STLINK-V3"),    // STLINK_V3_USBLOADER_PID
    probe(0x374E, "STLINK-V3"),    // STLINK_V3E_PID
    probe(0x374F, "STLINK-V3"),    // STLINK_V3S_PID
    probe(0x3753, "STLINK-V3"),    // STLINK_V3_2VCP_PID
    probe(0x3754, "STLINK-V3"),    // STLINK_V3E_NO_MSD_PID
    probe(0x3755, "STLINK-V3"),    // STLINK_V3P_USBLOADER_PID
    probe(0x3757, "STLINK-V3"),    // STLINK_V3P_PID
    // USB-serial adapters: the ID tables of Linux's USB-serial drivers, as Linux 6.18's
    // modules.alias lists them.
    // ftdi_sio, drivers/usb/serial/ftdi_sio_ids.h: FTDI_8U232AM_PID, FTDI_8U2232C_PID,
    // FTDI_4232H_PID, FTDI_232H_PID and FTDI_FTX_PID.
    adapter(0x0403, 0x6001, "FTDI FT232"),
    adapter(0x0403, 0x6010, "FTDI FT2232"),
    adapter(0x0403, 0x6011, "FTDI FT4232H"),
    adapter(0x0403, 0x6014, "FTDI FT232H"),
    adapter(0x0403, 0x6015, "FTDI FT-X"),
    // cp210x, drivers/usb/serial/cp210x.c.
    adapter(0x10C4, 0xEA60, "Silicon Labs CP210x"),
    adapter(0x10C4, 0xEA63, "Silicon Labs CP210x"),
    adapter(0x10C4, 0xEA70, "Silicon Labs CP210x"),
    adapter(0x10C4, 0xEA71, "Silicon Labs CP210x"),
    adapter(0x10C4, 0xEA7A, "Silicon Labs CP210x"),
    adapter(0x10C4, 0xEA7B, "Silicon Labs CP210x"),
    // ch341, drivers/usb/serial/ch341.c; and cdc_acm for 1A86:55D3, the CH343.
    adapter(0x1A86, 0x7523, "WCH CH340"),
    adapter(0x1A86, 0x7522, "WCH CH340"),
    adapter(0x1A86, 0x5523, "WCH CH341"),
    adapter(0x1A86, 0x55D3, "WCH CH343"),
    // pl2303, drivers/usb/serial/pl2303.h: PL2303_PRODUCT_ID and PL2303_PRODUCT_ID_GC, _GT, _GL,
    // _GE and _GS.
    adapter(0x067B, 0x2303, "Prolific PL2303"),
    adapter(0x067B, 0x23A3, "Prolific PL2303GC"),
    adapter(0x067B, 0x23C3, "Prolific PL2303GT"),
    adapter(0x067B, 0x23D3, "Prolific PL2303GL"),
    adapter(0x067B, 0x23E3, "Prolific PL2303GE"),
    adapter(0x067B, 0x23F3, "Prolific PL2303GS"),
];

#[cfg(test)]
#[path = "usb_tests.rs"]
mod tests;
