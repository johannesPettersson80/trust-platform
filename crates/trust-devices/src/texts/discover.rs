//! The texts of the Discover pane (specification 25 §2): the names of the search's sources and of
//! the protocols and sources of its results, the words of a source's progress and its reasons, the
//! scan rules of each origin, the empty result, the card actions and the identity rows. Texts
//! ported from the panel's `TypeScript` (`DiscoverPane.tsx`, `discoverPaneModel.ts`) keep its words.
//! The view chooses which text applies; only these modules hold the words.

/// The name of a search source, as its source row and its scan choice show it, and of the protocol
/// that a result reports: a token without a name reads in words, with spaces for underscores.
#[must_use]
pub fn source_name(source: &str) -> String {
    let name = match source {
        "trust_runtime" => "truST runtimes",
        "ads" => "ADS and TwinCAT",
        "modbus_tcp" => "Modbus TCP",
        "mqtt" => "MQTT",
        "opcua" => "OPC UA",
        "ethercat" => "EtherCAT",
        "gpio" => "GPIO",
        "serial" => "Serial ports",
        "usb" => "USB",
        "bluetooth" => "Bluetooth LE",
        _ => return words(source),
    };
    name.to_owned()
}

/// How a result was found, as its card's protocol line shows it. A token without a name reads in
/// words.
#[must_use]
pub fn discovery_source(source: &str) -> String {
    let label = match source {
        "mdns" => "Network discovery",
        "mesh" => "Mesh",
        "tcp_connect" => "Known address",
        "ads_broadcast" => "Network scan",
        "ads_identify" => "ADS identify",
        "ads_port_browse_results" => "ADS port browse",
        "ads_port_unavailable" => "ADS port",
        "lds" => "Discovery server",
        "opcua_get_endpoints" => "OPC UA endpoints",
        "tls_handshake" => "Secure handshake",
        "mqtt_connack" | "mqtt_connack_rejected" | "mqtt_connack_auth_rejected" => "MQTT handshake",
        "modbus_device_id" | "modbus_device_id_exception" => "Modbus device ID",
        "modbus_safe_read" | "modbus_safe_read_exception" => "Modbus read",
        "usb_descriptor" => "USB descriptor",
        "usb_devices" => "USB devices",
        "usb_bootloader" => "USB boot loader",
        "port_listing" | "serial_ports" => "Port listing",
        "gpio_sysfs" => "GPIO lines",
        "ethercat_bus" | "ethercat_mock" => "EtherCAT bus scan",
        _ => return words(source),
    };
    label.to_owned()
}

/// A token read in words: underscores become spaces and the first letter is capitalized.
fn words(token: &str) -> String {
    let spaced = token.trim().replace('_', " ");
    let mut characters = spaced.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

// ---- a source's progress (specification 25 §2, lines 55-58) ----

/// The word of a source that is still searching.
pub const SEARCHING: &str = "searching";
/// The word of a source that the deadline ended while it searched.
pub const TIMED_OUT: &str = "timed out";
/// The word of a source that the cancelled search ended while it searched.
pub const CANCELLED: &str = "cancelled";
/// The word of a source that did not run.
pub const SKIPPED: &str = "skipped";
/// The reason of a skipped source whose answer gives none.
pub const SKIPPED_WITHOUT_REASON: &str = "This source did not run.";

/// The word of a source with `count` results, such as `2 found`.
#[must_use]
pub fn found(count: u32) -> String {
    format!("{count} found")
}

/// The reason of a runtime whose truST version cannot search: it needs `version` or later
/// (specification 34 TARGET-05; design §1.4.1).
#[must_use]
pub fn update_needed(version: &str) -> String {
    format!("This runtime's truST version cannot search; update it to {version} or later.")
}

// ---- the scans of each origin (specification 25 §2, lines 82-88) ----

/// Why `EtherCAT` and GPIO cannot scan from this computer.
pub const CHOOSE_RUNTIME_FOR_HARDWARE: &str =
    "Choose a running runtime for EtherCAT or GPIO scans.";
/// Why `EtherCAT` and GPIO cannot scan from a stopped runtime (specification 25 §2).
pub const START_RUNTIME_FOR_HARDWARE: &str =
    "Start or connect a runtime before scanning EtherCAT or GPIO.";
/// Why Bluetooth cannot be scanned in this version of truST: the search skips it from every origin
/// (specification 11, "Sources").
pub const BLUETOOTH_UNAVAILABLE: &str =
    "Bluetooth search is not available in this truST version yet.";
/// Why a scan other than `EtherCAT` and GPIO cannot run from the stopped runtime `runtime`.
#[must_use]
pub fn start_or_connect(runtime: &str) -> String {
    format!("Start or connect {runtime} before scanning from it.")
}
/// Why a scan cannot run from a runtime that the panel no longer shows.
pub const START_A_RUNTIME: &str = "Start or connect a runtime before scanning from it.";
/// Why the result of a runtime that the panel no longer shows cannot be added.
pub const STALE_RUNTIME: &str =
    "The runtime that found this result is no longer available. Scan again before adding it.";

// ---- the empty result (specification 25 §2, lines 75-77) ----

/// The checks that an empty result suggests: device power, network reachability, port and firewall
/// restrictions, and the supplied address or subnet.
pub const EMPTY_CHECKS: &str = "Nothing found. Check that the device is powered on, on the same network, and not blocked by a port or firewall. Verify the address or subnet for known-address scans.";
/// What an empty result adds when the selected runtime scans the hardware.
pub const HARDWARE_FROM_SELECTED: &str = "Hardware scans are running from the selected runtime.";
/// What an empty result adds when another runtime could run the hardware scans.
pub const CHOOSE_RUNNING_RUNTIME: &str =
    "For EtherCAT or GPIO, choose a running runtime in Scan from.";
/// What an empty result adds when no runtime can run the hardware scans.
pub const START_RUNTIME_FIRST: &str = "For EtherCAT or GPIO, start or connect a runtime first.";

// ---- the card actions (specification 25 §2, lines 73-90) ----

/// The action of a discovered runtime card: it adds the runtime to the fleet.
pub const ADOPT: &str = "Adopt";
/// Why a device's connection cannot be added without a runtime to hold it.
pub const ADD_NEEDS_RUNTIME: &str =
    "Add a runtime first, so that the device has a runtime to connect through.";
/// Why a device's connection cannot be added: no connection of truST uses the device.
pub const ADD_NOT_OFFERED: &str = "This device has no connection to add yet.";

// ---- the identity rows of a card and the known-address inputs ----

/// The identity row of a device's AMS Net ID.
pub const ROW_AMS_NET_ID: &str = "AMS Net ID";
/// The identity row of a device's USB serial number.
pub const ROW_USB_SERIAL: &str = "USB serial number";
/// The identity row of a device's runtime ID.
pub const ROW_RUNTIME_ID: &str = "Runtime ID";
/// The identity row of a runtime's platform.
pub const ROW_PLATFORM: &str = "Platform";
/// The label of the known addresses input (specification 25 §2, `Known address or subnet`).
pub const TARGET_HOSTS_LABEL: &str = "Known address";
/// The label of the subnet input (specification 25 §2, `Known address or subnet`).
pub const TARGET_CIDR_LABEL: &str = "Subnet";
/// The placeholder of the known addresses input.
pub const TARGET_HOSTS_PLACEHOLDER: &str = "host:port";
/// The placeholder of the subnet input.
pub const TARGET_CIDR_PLACEHOLDER: &str = "192.168.1.0/24";

// ---- the pane's own words (specification 25 §2): the old pane's `DiscoverPane.tsx`, line by line ----

/// The breadcrumb of the pane (`DiscoverPane.tsx` line 230).
pub const BREADCRUMB: &str = super::graph::PANEL;
/// The accessible name of the close button (`DiscoverPane.tsx` line 233).
pub const CLOSE_LABEL: &str = super::browse::CLOSE;
/// The heading of the scan choices (`DiscoverPane.tsx` line 262).
pub const HEADING_FIND: &str = "What do you want to find?";
/// The heading of the origin choices (`DiscoverPane.tsx` line 275).
pub const HEADING_SCAN_FROM: &str = "Scan from";
/// The heading of the known address and subnet inputs (`DiscoverPane.tsx` line 300).
pub const HEADING_TARGETS: &str = "Known address or subnet";
/// The heading of the advanced settings (`DiscoverPane.tsx` line 271).
pub const HEADING_ADVANCED: &str = "Advanced scan settings";
/// The heading of the runtime's hardware scans (`DiscoverPane.tsx` line 306).
pub const HEADING_RUNTIME: &str = "Runtime hardware scans ⚠";
/// The help that shows when no discovery is available (`DiscoverPane.tsx` lines 251-252).
pub const NO_DISCOVERY_YET: &str = "No discovery is available yet. Load a project or connect a runtime that advertises discoverable protocols.";
/// The help that shows above the scan choices (`DiscoverPane.tsx` line 257).
pub const CHOOSE_WHAT_TO_FIND: &str =
    "Choose what to find, then scan. Only the selected device types are searched.";
/// The note of the ADS scan (`DiscoverPane.tsx` line 33).
pub const NOTE_NETWORK_BROADCAST: &str = "network broadcast";
/// The note of the truST runtime scan (`DiscoverPane.tsx` line 34).
pub const NOTE_MDNS: &str = "mDNS";
/// The note of the Modbus TCP scan (`DiscoverPane.tsx` line 35).
pub const NOTE_LOCAL_NETWORK_SCAN: &str = "local network scan";
/// The note of a scan from a known host (`DiscoverPane.tsx` lines 39, 42-44).
pub const NOTE_AT_HOST: &str = "at host";
/// The note of the `EtherCAT` scan (`DiscoverPane.tsx` line 48).
pub const NOTE_BUS_ENUMERATION: &str = "bus enumeration";
/// The note of the GPIO scan (`DiscoverPane.tsx` line 49).
pub const NOTE_LOCAL_PINS: &str = "local pins on the runtime";
/// The label of a device's host in its details (`DiscoverPane.tsx` line 379, `Host:`).
pub const ROW_HOST: &str = super::browse::HOST;
/// The label of the search action (`DiscoverPane.tsx` line 425 says `Scan`; specification 25 §2
/// names the action `Search`).
pub const SEARCH: &str = "Search";
/// The label of the search action while a scan runs (`DiscoverPane.tsx` line 425).
pub const SCANNING: &str = "Scanning…";
/// Why the search action cannot run while a scan runs (specification 25 §2: disabled with its
/// reason while a scan runs; the old pane gave no reason here).
pub const SEARCH_RUNNING: &str = "A scan is running. Wait for it to end, or cancel it.";
/// Why the search action cannot run when no source that the origin can scan is ticked
/// (`DiscoverPane.tsx` line 421).
pub const SELECT_A_SCAN_TYPE: &str = "Select at least one available scan type.";
/// The label of the cancel action, which shows while a scan runs.
pub const CANCEL: &str = super::forms::CANCEL;
/// The label of the action that opens the ADS port settings (`DiscoverPane.tsx` line 295).
pub const CONFIGURE_ADS_PORTS: &str = "Configure ADS ports to scan";
/// The VS Code setting that `Configure ADS ports to scan` opens (`networkCanvasPanel.ts` lines
/// 661-665).
pub const ADS_PORTS_SETTING: &str = "trust.ads.discoveryPorts";
/// The mark of a simulated result, which is never presented as hardware (specification 11,
/// "Confidence").
pub const SIMULATED_MARK: &str = "Simulated";
/// The line that asks to update truST when a status names a source or protocol that this build does
/// not know (specification 25 §2).
pub const UPDATE_TRUST: &str = "This search reports a source or protocol that this version of truST does not know. Update truST to see its results.";
/// The group of the choices under `What do you want to find?`.
pub const GROUP_FIND: &str = "find";
/// The group of the choices under `Known address or subnet`.
pub const GROUP_KNOWN_ADDRESS: &str = "known_address";
/// The group of the choices under `Runtime hardware scans ⚠`.
pub const GROUP_RUNTIME_HARDWARE: &str = "runtime_hardware";
