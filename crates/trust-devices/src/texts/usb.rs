//! The texts of the USB tray (specification 25 §5.12): its heading and empty text, what each item
//! is and its truST state, and its actions with their reasons.

/// The tray's heading.
pub const HEADING: &str = "Found on this computer (USB)";
/// What the tray shows without an item.
pub const EMPTY: &str = "No chips, debug probes, or USB-serial adapters on this computer's USB.";

/// The truST state of a chip on its boot loader.
pub const NO_TRUST_YET: &str = "No truST yet";
/// The truST state of a chip whose descriptors are the same with or without truST.
pub const TRUST_NOT_CONFIRMED: &str = "truST not confirmed";
/// The truST state of a truST runtime whose version is not known.
pub const TRUST: &str = "truST";

/// The truST state of a truST runtime: its version.
#[must_use]
pub fn trust_version(version: &str) -> String {
    format!("truST {version}")
}

/// What a boot loader or a chip's USB controller shows while no chip file names its IDs.
pub const NO_SUPPORTED_CHIP: &str = "No supported chip shows these IDs.";
/// That the chip behind a boot loader or a controller is confirmed before anything is written.
const CHIP_CONFIRMED: &str = "The chip is confirmed when connecting and before any installation.";

/// The supported chips that show the IDs of a boot loader or a chip's USB controller, by their
/// models as the chip files give them, with the note that the chip is confirmed.
#[must_use]
pub fn supported_chips(models: &[&str]) -> String {
    match models {
        [] => NO_SUPPORTED_CHIP.to_owned(),
        [model] => format!("Supported chip with these IDs: {model}. {CHIP_CONFIRMED}"),
        _ => format!(
            "Supported chips with these IDs: {}. {CHIP_CONFIRMED}",
            models.join(", ")
        ),
    }
}

/// What a debug probe is.
pub const DEBUG_PROBE: &str = "Debug probe";
/// What a USB-serial adapter, or another device with a serial port, is.
pub const SERIAL_ADAPTER: &str = "USB-serial adapter";

/// What a probe or adapter is, with the serial ports behind it.
#[must_use]
pub fn with_ports(kind: &str, ports: &[String]) -> String {
    if ports.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind} · {}", ports.join(", "))
    }
}

/// The serial number of a truST runtime that the panel does not show yet.
#[must_use]
pub fn serial_number(serial_number: &str) -> String {
    format!("Serial number {serial_number}")
}

/// Which runtime a device is, and the network address where it is also reachable.
#[must_use]
pub fn this_is(name: &str, address: Option<&str>) -> String {
    match address {
        Some(address) => format!("This is {name}, also reachable at {address}."),
        None => format!("This is {name}."),
    }
}

/// A chip's flash and RAM, in the units of its data sheet: `1 MB flash · 192 KB RAM`. A chip
/// whose flash sits outside it shows its RAM only.
#[must_use]
pub fn flash_and_ram(flash_kib: Option<u32>, ram_kib: u32) -> String {
    let ram = format!("{} RAM", size(ram_kib));
    match flash_kib {
        Some(flash_kib) => format!("{} flash · {ram}", size(flash_kib)),
        None => ram,
    }
}

/// A size given in units of 1,024 bytes: in MB when it is a whole number of them, else in KB.
fn size(kib: u32) -> String {
    if kib >= 1024 && kib.is_multiple_of(1024) {
        format!("{} MB", kib / 1024)
    } else {
        format!("{kib} KB")
    }
}

/// The action that opens the installation of truST.
pub const INSTALL_TRUST: &str = "Install truST";
/// The action that opens the update of a runtime's truST.
pub const UPDATE_TRUST: &str = "Update truST";
/// The action that zooms to the runtime of a device.
pub const SHOW: &str = "Show";
/// Why `Connect` is disabled while this truST cannot reach a runtime over USB.
pub const CONNECT_OVER_USB_UNAVAILABLE: &str = "This version of truST cannot connect over USB yet.";
/// Why `Connect` is disabled for a device without a serial number.
pub const CONNECT_WITHOUT_SERIAL_NUMBER: &str =
    "This device reports no serial number, so truST cannot reach it over USB.";
