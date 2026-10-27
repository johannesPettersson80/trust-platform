//! The USB and serial sources over listings that a test gives (specification 11, "USB and
//! serial"). No test reads the bus or the ports of the computer that runs it.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use super::*;
use crate::search::model::{PortUsbDevice, SerialBus, SerialIdentity};
use crate::search::runner::SourcePlan;
use crate::search::{FoundDevice, SearchOrigin, SearchRegistry, SearchState, SearchStatus};
use crate::search::{SourceState, SEARCH_SCHEMA_VERSION};

/// The serial number of the owner's ESP32-C6: its base MAC.
const C6_MAC: &str = "20:6E:F1:17:04:C0";
/// The serial number of an ST-LINK/V2-1.
const STLINK_SERIAL: &str = "0671FF525750877267181518";

/// A listing that a test gives, which records what the source asks it.
#[derive(Clone)]
struct FakeListing {
    devices: Vec<ListedUsbDevice>,
    ports: Result<Vec<ListedPort>, String>,
    asked: Arc<Mutex<Vec<&'static str>>>,
}

impl FakeListing {
    fn new(devices: Vec<ListedUsbDevice>, ports: Vec<ListedPort>) -> Self {
        Self {
            devices,
            ports: Ok(ports),
            asked: Arc::default(),
        }
    }

    fn ask(&self, what: &'static str) {
        self.asked.lock().expect("the record of calls").push(what);
    }
}

impl PortListing for FakeListing {
    fn serial_ports(&self) -> Result<Vec<ListedPort>, String> {
        self.ask("serial_ports");
        self.ports.clone()
    }
}

impl UsbListing for FakeListing {
    fn usb_devices(&self) -> Result<Vec<ListedUsbDevice>, String> {
        self.ask("usb_devices");
        Ok(self.devices.clone())
    }
}

fn device(vid: u16, pid: u16, serial_number: Option<&str>, location: &str) -> ListedUsbDevice {
    ListedUsbDevice {
        vid,
        pid,
        release: 0x0100,
        manufacturer: None,
        product: None,
        serial_number: serial_number.map(str::to_owned),
        location: location.to_owned(),
    }
}

fn named(
    mut device: ListedUsbDevice,
    manufacturer: &str,
    product: &str,
    release: u16,
) -> ListedUsbDevice {
    device.manufacturer = Some(manufacturer.to_owned());
    device.product = Some(product.to_owned());
    device.release = release;
    device
}

fn usb_port(name: &str, vid: u16, pid: u16, serial_number: Option<&str>) -> ListedPort {
    ListedPort {
        name: name.to_owned(),
        bus: SerialBus::Usb,
        usb: Some(PortUsbDevice {
            vid,
            pid,
            manufacturer: None,
            product: None,
            serial_number: serial_number.map(str::to_owned),
        }),
    }
}

fn other_port(name: &str, bus: SerialBus) -> ListedPort {
    ListedPort {
        name: name.to_owned(),
        bus,
        usb: None,
    }
}

/// The USB source over `listing`.
fn usb_plan(listing: FakeListing) -> (SourceId, SourcePlan) {
    (
        SourceId::Usb,
        SourcePlan::Run(Box::new(move |sink: &SourceSink| run(&listing, sink))),
    )
}

/// The serial source over `listing`.
fn serial_plan(listing: FakeListing) -> (SourceId, SourcePlan) {
    (
        SourceId::Serial,
        SourcePlan::Run(Box::new(move |sink: &SourceSink| {
            serial::run(&listing, sink);
        })),
    )
}

/// Runs `plans` as one search on this computer and answers its status once it ends.
fn search_with(plans: Vec<(SourceId, SourcePlan)>) -> SearchStatus {
    let registry = SearchRegistry::new();
    let search_id = registry
        .start_plans(SearchOrigin::ThisHost, Duration::from_secs(30), plans)
        .expect("start");
    let give_up = Instant::now() + Duration::from_secs(20);
    loop {
        let status = registry.status(&search_id, None).expect("kept search");
        if status.state != SearchState::Running {
            return status;
        }
        assert!(Instant::now() < give_up, "the search ends");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn found<'a>(status: &'a SearchStatus, id: &str) -> &'a FoundDevice {
    status
        .devices
        .iter()
        .find(|device| device.id == id)
        .unwrap_or_else(|| panic!("no device {id} in {:?}", device_ids(status)))
}

fn device_ids(status: &SearchStatus) -> Vec<&str> {
    let mut ids: Vec<&str> = status
        .devices
        .iter()
        .map(|device| device.id.as_str())
        .collect();
    ids.sort_unstable();
    ids
}

fn usb_of(find: &ProtocolFind) -> &UsbIdentity {
    match &find.identity {
        Identity::Usb(usb) => usb,
        other => panic!("not a USB identity: {other:?}"),
    }
}

fn usb_find(device: &FoundDevice) -> &ProtocolFind {
    device
        .finds
        .iter()
        .find(|find| find.protocol == SourceId::Usb)
        .expect("a USB result")
}

fn serial_of(device: &FoundDevice) -> &SerialIdentity {
    device
        .finds
        .iter()
        .find_map(|find| match &find.identity {
            Identity::Serial(serial) => Some(serial),
            _ => None,
        })
        .expect("a serial result")
}

#[test]
fn usb_listing_is_passive_and_reports_vid_pid_serial() {
    // What the owner's computer shows: the ESP32-C6 on its USB-Serial/JTAG port, an iPad and a
    // keyboard; and a CH340 adapter, which has no serial number.
    let listing = FakeListing::new(
        vec![
            named(
                device(0x303A, 0x1001, Some(C6_MAC), "003-2"),
                "Espressif",
                "USB JTAG/serial debug unit",
                0x0101,
            ),
            named(
                device(0x05AC, 0x12AB, Some("00008027001A2B3C"), "001-1"),
                "Apple Inc.",
                "iPad",
                0x1600,
            ),
            device(0x04F3, 0x0103, None, "001-2"),
            device(0x1A86, 0x7523, None, "001-3"),
        ],
        vec![
            usb_port("/dev/ttyACM1", 0x303A, 0x1001, Some(C6_MAC)),
            usb_port("/dev/ttyUSB0", 0x1A86, 0x7523, None),
        ],
    );
    let asked = Arc::clone(&listing.asked);
    let status = search_with(vec![usb_plan(listing)]);

    // The source asked for the two listings, once each, and for nothing else: a listing has no
    // way to open a device.
    assert_eq!(
        *asked.lock().expect("the record of calls"),
        vec!["usb_devices", "serial_ports"]
    );
    // Its code lists through the operating system and calls nothing that opens, claims, resets
    // or writes a device, or that asks a device for a string.
    for (file, code) in [
        ("usb.rs", include_str!("usb.rs")),
        ("serial.rs", include_str!("serial.rs")),
    ] {
        for call in [
            ".open(",
            "claim_interface",
            "detach_",
            ".reset(",
            "set_configuration",
            "get_manufacturer_string",
            "get_product_string",
            "get_serial_number_string",
            "control_in",
            "control_out",
            "serialport::new",
        ] {
            assert!(!code.contains(call), "{file} calls {call}");
        }
    }
    assert!(include_str!("usb.rs").contains("nusb::list_devices()"));
    assert!(include_str!("serial.rs").contains("serialport::available_ports()"));

    assert_eq!(status.state, SearchState::Done);
    assert_eq!(status.sources[0].state, SourceState::Found);
    assert_eq!(status.sources[0].count, Some(2));
    // The iPad and the keyboard are neither recognized nor serial devices: they are not reported.
    assert_eq!(
        device_ids(&status),
        vec!["dev:serial:/dev/ttyUSB0", "dev:usb:20:6E:F1:17:04:C0"]
    );

    let c6 = found(&status, "dev:usb:20:6E:F1:17:04:C0");
    assert_eq!(c6.usb_serials, vec![C6_MAC]);
    let find = usb_find(c6);
    assert_eq!(find.source, "usb_descriptor");
    assert_eq!(find.confidence, DiscoveryConfidence::Confirmed);
    assert!(!find.simulated);
    let usb = usb_of(find);
    assert_eq!((usb.vid, usb.pid), (0x303A, 0x1001));
    assert_eq!(usb.serial_number.as_deref(), Some(C6_MAC));
    assert_eq!(usb.manufacturer.as_deref(), Some("Espressif"));
    assert_eq!(usb.product.as_deref(), Some("USB JTAG/serial debug unit"));
    assert_eq!(usb.ports, vec!["/dev/ttyACM1"]);

    // The CH340 has no serial number: it is one device with its port, by the port's name.
    let ch340 = found(&status, "dev:serial:/dev/ttyUSB0");
    assert_eq!(ch340.usb_serials, Vec::<String>::new());
    let usb = usb_of(usb_find(ch340));
    assert_eq!(
        (usb.vid, usb.pid, usb.serial_number.as_deref()),
        (0x1A86, 0x7523, None)
    );
    assert_eq!(usb.role, UsbRole::SerialAdapter);
    assert_eq!(usb.model, "WCH CH340");

    // The status reports the IDs and the serial number in its JSON.
    let json = serde_json::to_value(&status).expect("serialize");
    assert_eq!(json["schema_version"], json!(SEARCH_SCHEMA_VERSION));
    let c6 = json["devices"]
        .as_array()
        .expect("devices")
        .iter()
        .find(|device| device["id"] == json!("dev:usb:20:6E:F1:17:04:C0"))
        .expect("the ESP32-C6");
    assert_eq!(c6["usb_serials"], json!([C6_MAC]));
    let identity = &c6["finds"][0]["identity"];
    assert_eq!(identity["kind"], json!("usb"));
    assert_eq!(identity["vid"], json!(0x303A));
    assert_eq!(identity["pid"], json!(0x1001));
    assert_eq!(identity["serial_number"], json!(C6_MAC));
    assert_eq!(identity["role"], json!("chip_controller"));
}

#[test]
fn usb_identifies_stm32_bootloader_stlink_and_esp32c6_jtag() {
    let devices = vec![
        named(
            device(0x0483, 0xDF11, Some("356A665D3431"), "001-1"),
            "STMicroelectronics",
            "STM32  BOOTLOADER",
            0x2200,
        ),
        named(
            device(0x0483, 0x374B, Some(STLINK_SERIAL), "001-2"),
            "STMicroelectronics",
            "STM32 STLink",
            0x0100,
        ),
        device(0x0483, 0x3748, Some("48FF6D065077505448391887"), "001-3"),
        device(0x0483, 0x374E, Some("002C00283132510B33333639"), "001-4"),
        named(
            device(0x303A, 0x1001, Some(C6_MAC), "001-5"),
            "Espressif",
            "USB JTAG/serial debug unit",
            0x0101,
        ),
        // The STM32F407 running truST, on the pid.codes test ID of development.
        named(
            device(0x1209, 0x0001, Some("0034002D3431471535363630"), "001-6"),
            "truST",
            "truST runtime",
            0x0024,
        ),
        // truST's own ID.
        named(
            device(0x1209, 0x7472, Some("0034002D3431471535363631"), "001-7"),
            "truST",
            "truST runtime",
            0x0102,
        ),
        // Somebody else's prototype on the shared test ID, without a serial port.
        named(
            device(0x1209, 0x0001, Some("1"), "001-8"),
            "Example",
            "Prototype",
            0x0100,
        ),
    ];
    let ports = vec![usb_port(
        "/dev/ttyACM0",
        0x0483,
        0x374B,
        Some(STLINK_SERIAL),
    )];
    let results = finds(&devices, &ports, 7);
    let identities: Vec<(&str, UsbRole, &str)> = results
        .iter()
        .map(|find| {
            let usb = usb_of(find);
            (
                usb.serial_number.as_deref().unwrap_or_default(),
                usb.role,
                usb.model.as_str(),
            )
        })
        .collect();
    assert_eq!(
        identities,
        vec![
            (
                "356A665D3431",
                UsbRole::BootLoader,
                "STM32 boot loader (DFU)"
            ),
            (STLINK_SERIAL, UsbRole::DebugProbe, "ST-LINK/V2-1"),
            (
                "48FF6D065077505448391887",
                UsbRole::DebugProbe,
                "ST-LINK/V2"
            ),
            ("002C00283132510B33333639", UsbRole::DebugProbe, "STLINK-V3"),
            (C6_MAC, UsbRole::ChipController, "Espressif USB-Serial/JTAG"),
            (
                "0034002D3431471535363630",
                UsbRole::TrustRuntime,
                "truST runtime"
            ),
            (
                "0034002D3431471535363631",
                UsbRole::TrustRuntime,
                "truST runtime"
            ),
        ],
        "the prototype on the shared test ID is not truST and has no serial port"
    );
    for find in &results {
        assert_eq!(find.protocol, SourceId::Usb);
        assert_eq!(find.confidence, DiscoveryConfidence::Confirmed);
        assert_eq!(find.last_seen_ms, 7);
    }

    // The boot loader: no truST yet. Every STM32 with a USB boot loader shows these IDs, so the
    // search names no chip, and the chip is confirmed before anything is installed.
    let boot_loader = &results[0];
    assert_eq!(usb_of(boot_loader).version, None);
    assert_eq!(usb_of(boot_loader).control, None);
    assert_eq!(
        boot_loader.warnings,
        vec![
            "Every STM32 with a USB boot loader shows these IDs; the chip is confirmed when \
             connecting and before any installation."
        ]
    );
    assert_eq!(boot_loader.params, json!({}));
    // The probe's virtual COM port is behind it.
    assert_eq!(usb_of(&results[1]).ports, vec!["/dev/ttyACM0"]);
    // The ESP32-C6's controller: neither the chip nor truST is confirmed, and a connection
    // reaches it at its serial number.
    let c6 = &results[4];
    assert_eq!(usb_of(c6).version, None);
    assert_eq!(
        usb_of(c6).control.as_deref(),
        Some("usb://20:6E:F1:17:04:C0")
    );
    assert_eq!(
        c6.params,
        json!({ "control_endpoint": "usb://20:6E:F1:17:04:C0" })
    );
    assert_eq!(
        c6.warnings,
        vec![
            "This controller shows the same IDs with or without truST, and on other Espressif \
             chips; connecting confirms the chip and truST."
        ]
    );
    // truST runtimes: the version of `bcdDevice` and the `usb://` endpoint.
    let runtime = usb_of(&results[5]);
    assert_eq!(runtime.version.as_deref(), Some("0.24"));
    assert_eq!(
        runtime.control.as_deref(),
        Some("usb://0034002D3431471535363630")
    );
    assert_eq!(
        results[5].params,
        json!({ "control_endpoint": "usb://0034002D3431471535363630" })
    );
    assert_eq!(usb_of(&results[6]).version.as_deref(), Some("1.2"));

    // A `bcdDevice` that is not BCD carries no version, and a serial number that a `usb://`
    // endpoint cannot name gives no endpoint.
    assert_eq!(release_version(0x00AF), None);
    assert_eq!(release_version(0x0100), Some("1.0".to_owned()));
    let odd = named(
        device(0x1209, 0x7472, Some("has space"), "001-9"),
        "truST",
        "truST runtime",
        0x0024,
    );
    let odd = finds(std::slice::from_ref(&odd), &[], 7);
    assert_eq!(usb_of(&odd[0]).control, None);
    assert_eq!(odd[0].params, json!({}));
}

/// The boot loader IDs that chip files name (specification 34, Appendix H.2), each with the
/// chips that name it, in the order of `texts`.
fn boot_loader_ids(texts: &[String]) -> Vec<((u16, u16), Vec<String>)> {
    let mut ids: Vec<((u16, u16), Vec<String>)> = Vec::new();
    for text in texts {
        let file: toml::Table = text.parse().expect("a chip file is TOML");
        let identity = file["identity"].as_table().expect("[identity]");
        let name = identity["name"].as_str().expect("identity.name").to_owned();
        let id = identity["usb_bootloader"]
            .as_table()
            .expect("identity.usb_bootloader");
        let number = |key: &str| {
            id[key]
                .as_integer()
                .and_then(|value| u16::try_from(value).ok())
                .expect("a USB ID")
        };
        let key = (number("vid"), number("pid"));
        match ids.iter_mut().find(|(known, _)| *known == key) {
            Some((_, chips)) => chips.push(name),
            None => ids.push((key, vec![name])),
        }
    }
    ids
}

/// Each boot loader ID of the chip files is one entry of the table, that of a chip's boot loader
/// or USB controller, however many chip files name it; the chip files, not the table, say which
/// chips show it. The table holds no such entry that no chip file names.
fn assert_table_follows(texts: &[String]) {
    let ids = boot_loader_ids(texts);
    assert!(!ids.is_empty(), "chips/ holds the chip files");
    for ((vid, pid), chips) in &ids {
        let entries: Vec<&Known> = KNOWN
            .iter()
            .filter(|known| known.vid == *vid && known.pid == *pid)
            .collect();
        assert_eq!(entries.len(), 1, "{vid:04X}:{pid:04X} of {chips:?}");
        assert!(
            matches!(
                entries[0].role,
                UsbRole::BootLoader | UsbRole::ChipController
            ),
            "{chips:?}"
        );
    }
    for known in KNOWN
        .iter()
        .filter(|known| matches!(known.role, UsbRole::BootLoader | UsbRole::ChipController))
    {
        assert!(
            ids.iter().any(|(id, _)| *id == (known.vid, known.pid)),
            "no chip file names {} ({:04X}:{:04X})",
            known.model,
            known.vid,
            known.pid
        );
    }
}

/// The table and the chip files agree, and stay in agreement when another chip file names a boot
/// loader ID that one already names: that is one more chip of the same entry.
#[test]
fn usb_table_names_each_chip_files_boot_loader() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../chips");
    let mut texts = Vec::new();
    for entry in std::fs::read_dir(&folder).expect("the chips folder") {
        let path = entry.expect("a chip file").path();
        if path
            .extension()
            .is_some_and(|extension| extension == "toml")
        {
            texts.push(std::fs::read_to_string(&path).expect("read a chip file"));
        }
    }
    texts.sort();
    assert_table_follows(&texts);

    // A second STM32 chip with the same DFU boot loader, such as the STM32U5A9.
    texts.push(
        "[identity]\nname = \"stm32u5a9\"\nusb_bootloader = { vid = 0x0483, pid = 0xDF11 }\n"
            .to_owned(),
    );
    assert_table_follows(&texts);
    let ids = boot_loader_ids(&texts);
    let stm32 = ids
        .iter()
        .find(|(id, _)| *id == (0x0483, 0xDF11))
        .map(|(_, chips)| chips)
        .expect("the STM32 boot loader");
    assert!(stm32.iter().any(|chip| chip == "stm32u5a9"), "{stm32:?}");
}

#[test]
fn serial_ports_listed_with_the_usb_device_behind_them() {
    let mut stlink_port = usb_port("/dev/ttyACM0", 0x0483, 0x374B, Some(STLINK_SERIAL));
    if let Some(usb) = &mut stlink_port.usb {
        usb.manufacturer = Some("STMicroelectronics".to_owned());
        usb.product = Some("STM32 STLink".to_owned());
    }
    let listing = FakeListing::new(
        vec![
            device(0x0483, 0x374B, Some(STLINK_SERIAL), "001-2"),
            // Two CH340 adapters without serial numbers: which port belongs to which cannot be
            // told.
            device(0x1A86, 0x7523, None, "001-3"),
            device(0x1A86, 0x7523, None, "001-4"),
            device(0x0403, 0x6001, Some("A50285BI"), "001-5"),
            // An FT-X adapter without a serial number, the only one of its kind.
            device(0x0403, 0x6015, None, "001-6"),
        ],
        vec![
            stlink_port,
            usb_port("/dev/ttyUSB0", 0x1A86, 0x7523, None),
            usb_port("/dev/ttyUSB1", 0x1A86, 0x7523, None),
            usb_port("/dev/ttyUSB2", 0x0403, 0x6001, Some("A50285BI")),
            usb_port("/dev/ttyUSB3", 0x0403, 0x6015, None),
            other_port("/dev/ttyAMA10", SerialBus::Other),
            other_port("/dev/rfcomm0", SerialBus::Bluetooth),
        ],
    );
    let status = search_with(vec![serial_plan(listing.clone()), usb_plan(listing)]);

    assert_eq!(status.state, SearchState::Done);
    let counts: Vec<(SourceId, SourceState, Option<u32>)> = status
        .sources
        .iter()
        .map(|source| (source.id, source.state, source.count))
        .collect();
    assert_eq!(
        counts,
        vec![
            (SourceId::Serial, SourceState::Found, Some(7)),
            (SourceId::Usb, SourceState::Found, Some(5)),
        ]
    );
    assert_eq!(
        device_ids(&status),
        vec![
            "dev:serial:/dev/rfcomm0",
            "dev:serial:/dev/ttyAMA10",
            "dev:serial:/dev/ttyUSB0",
            "dev:serial:/dev/ttyUSB1",
            "dev:serial:/dev/ttyUSB3",
            "dev:usb-port:001-3",
            "dev:usb-port:001-4",
            "dev:usb:0671FF525750877267181518",
            "dev:usb:A50285BI",
        ]
    );

    // The probe and its port are one device by the serial number; the port names the probe.
    let stlink = found(&status, "dev:usb:0671FF525750877267181518");
    assert_eq!(stlink.finds.len(), 2);
    let port = serial_of(stlink);
    assert_eq!(port.port, "/dev/ttyACM0");
    assert_eq!(port.bus, SerialBus::Usb);
    assert_eq!(
        port.usb,
        Some(PortUsbDevice {
            vid: 0x0483,
            pid: 0x374B,
            manufacturer: Some("STMicroelectronics".to_owned()),
            product: Some("STM32 STLink".to_owned()),
            serial_number: Some(STLINK_SERIAL.to_owned()),
        })
    );
    assert_eq!(usb_of(usb_find(stlink)).ports, vec!["/dev/ttyACM0"]);
    let serial_find = stlink
        .finds
        .iter()
        .find(|find| find.protocol == SourceId::Serial)
        .expect("the port's result");
    assert_eq!(serial_find.source, "port_listing");
    assert_eq!(serial_find.confidence, DiscoveryConfidence::Confirmed);
    assert_eq!(serial_find.params, json!({ "port": "/dev/ttyACM0" }));

    // The FT232 with a serial number joins its port; the FT-X without one joins its port by the
    // port's name, since no other device has its IDs.
    assert_eq!(found(&status, "dev:usb:A50285BI").finds.len(), 2);
    let ftx = found(&status, "dev:serial:/dev/ttyUSB3");
    assert_eq!(ftx.finds.len(), 2);
    assert_eq!(usb_of(usb_find(ftx)).model, "FTDI FT-X");

    // The two CH340s cannot be told apart: each adapter and each port is a device of its own,
    // and each port still names the USB device behind it.
    for location in ["dev:usb-port:001-3", "dev:usb-port:001-4"] {
        let adapter = found(&status, location);
        assert_eq!(adapter.finds.len(), 1);
        assert_eq!(usb_of(usb_find(adapter)).ports, Vec::<String>::new());
    }
    for name in ["/dev/ttyUSB0", "/dev/ttyUSB1"] {
        let port = serial_of(found(&status, &format!("dev:serial:{name}")));
        assert_eq!(
            port.usb.as_ref().map(|usb| (usb.vid, usb.pid)),
            Some((0x1A86, 0x7523))
        );
    }

    // A UART of the computer's own chip and a Bluetooth port have no USB device behind them.
    let uart = serial_of(found(&status, "dev:serial:/dev/ttyAMA10"));
    assert_eq!((uart.bus, uart.usb.as_ref()), (SerialBus::Other, None));
    let bluetooth = serial_of(found(&status, "dev:serial:/dev/rfcomm0"));
    assert_eq!(bluetooth.bus, SerialBus::Bluetooth);

    let json = serde_json::to_value(serial_of(found(&status, "dev:serial:/dev/ttyUSB3")))
        .expect("serialize");
    assert_eq!(
        json,
        json!({ "port": "/dev/ttyUSB3", "bus": "usb", "usb": { "vid": 0x0403, "pid": 0x6015 } })
    );
}

#[test]
fn a_missing_listing_is_a_warning() {
    let mut listing = FakeListing::new(vec![device(0x0483, 0x3748, Some("1"), "001-1")], vec![]);
    listing.ports =
        Err("Serial ports cannot be listed on this computer: /sys/class/tty is missing.".into());
    let status = search_with(vec![serial_plan(listing.clone()), usb_plan(listing)]);

    assert_eq!(status.state, SearchState::Done);
    for source in &status.sources {
        assert_eq!(source.state, SourceState::Found, "{:?}", source.id);
        assert_eq!(
            source.warnings,
            vec!["Serial ports cannot be listed on this computer: /sys/class/tty is missing."]
        );
    }
    assert_eq!(status.sources[0].count, Some(0));
    // The USB devices are still reported, without their ports.
    assert_eq!(status.sources[1].count, Some(1));
    assert_eq!(device_ids(&status), vec!["dev:usb:1"]);
}
