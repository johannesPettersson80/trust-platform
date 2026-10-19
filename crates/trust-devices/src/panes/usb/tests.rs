//! The USB tray (specification 25 §5.12): what each item shows and offers, and how a device is
//! matched with the runtime that the panel shows.

use serde_json::json;
use trust_chips::ChipFile;
use trust_runtime::search::{UsbIdentity, UsbRole};
use trust_runtime_client::ControlEndpoint;

use super::{boot_loaders, tray, tray_with, Found, ShownRuntime};
use crate::texts::usb as texts;
use crate::view::chips;
use crate::view::types::{Action, Emphasis, ItemId, Op, Pane, UiOp, UsbItem};

const TOOL_VERSION: &str = "0.24.77";
const C6_MAC: &str = "20:6E:F1:17:04:C0";
const SENSOR_HUB_ID: &str = "sensor-hub@esp32c6-206ef11704c0";

fn usb(vid: u16, pid: u16, role: UsbRole, model: &str) -> UsbIdentity {
    UsbIdentity {
        vid,
        pid,
        manufacturer: None,
        product: None,
        serial_number: None,
        role,
        model: model.to_owned(),
        version: None,
        control: None,
        ports: Vec::new(),
    }
}

/// A device that runs or may run truST, reached at `usb://<serial_number>`.
fn on_usb(mut identity: UsbIdentity, serial_number: &str) -> UsbIdentity {
    identity.serial_number = Some(serial_number.to_owned());
    identity.control = Some(format!("usb://{serial_number}"));
    identity
}

fn esp32c6() -> UsbIdentity {
    let mut identity = on_usb(
        usb(
            0x303A,
            0x1001,
            UsbRole::ChipController,
            "Espressif USB-Serial/JTAG",
        ),
        C6_MAC,
    );
    identity.ports = vec!["/dev/ttyACM1".to_owned()];
    identity
}

fn stm32_boot_loader() -> UsbIdentity {
    let mut identity = on_usb(
        usb(
            0x0483,
            0xDF11,
            UsbRole::BootLoader,
            "STM32 boot loader (DFU)",
        ),
        "356A665D3431",
    );
    identity.control = None;
    identity
}

/// The chip file of `name`, as this build embeds it.
fn chip_file(name: &str) -> ChipFile {
    chips::all()
        .iter()
        .find(|chip| chip.identity.name == name)
        .cloned()
        .unwrap_or_else(|| panic!("no chip file {name}"))
}

/// The chip files that the tests use: a new chip file does not change what they expect.
fn catalog() -> Vec<ChipFile> {
    vec![chip_file("stm32f407"), chip_file("esp32c6")]
}

fn trust_runtime(serial_number: &str) -> UsbIdentity {
    let mut identity = on_usb(
        usb(0x1209, 0x7472, UsbRole::TrustRuntime, "truST runtime"),
        serial_number,
    );
    identity.version = Some("0.24".to_owned());
    identity
}

fn found(id: &str, usb: Option<UsbIdentity>) -> Found {
    Found {
        id: id.to_owned(),
        usb,
        runtime_ids: Vec::new(),
        addresses: Vec::new(),
    }
}

fn shown(item: &str, name: &str, endpoint: &str, version: &str) -> ShownRuntime {
    ShownRuntime {
        item: ItemId(item.to_owned()),
        name: name.to_owned(),
        endpoint: Some(endpoint.to_owned()),
        runtime_ids: Vec::new(),
        version: Some(version.to_owned()),
    }
}

fn action_ids(item: &UsbItem) -> Vec<&str> {
    item.actions
        .iter()
        .map(|action| action.id.as_str())
        .collect()
}

fn item<'a>(items: &'a [UsbItem], id: &str) -> &'a UsbItem {
    items
        .iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("no tray item {id}"))
}

fn opens_install(action: &Action, id: &str) -> bool {
    action.op
        == Op::Ui {
            ui: UiOp::Open {
                pane: Pane::Install { usb: id.to_owned() },
            },
        }
}

fn focuses(action: &Action, item: &str) -> bool {
    action.op
        == Op::Ui {
            ui: UiOp::Focus {
                item: ItemId(item.to_owned()),
            },
        }
}

/// `Connect` at `endpoint`: enabled exactly when this truST's control client reaches it.
fn assert_connect(action: &Action, endpoint: &str, label: &str) {
    let reachable = ControlEndpoint::parse(endpoint).is_some();
    assert_eq!(
        (action.id.as_str(), action.label.as_str()),
        ("connect", "Connect")
    );
    assert_eq!(action.enabled, reachable);
    assert_eq!(
        action.reason.as_deref(),
        (!reachable).then_some(texts::CONNECT_OVER_USB_UNAVAILABLE)
    );
    assert_eq!(
        action.op,
        Op::Rust {
            action: "runtime.connect".to_owned(),
            payload: json!({ "endpoint": endpoint, "label": label }),
        }
    );
}

/// The devices of one search on a bench: a chip on its boot loader, a probe, an adapter, an
/// ESP32-C6, and three truST runtimes, two of which the panel shows. The network device is not
/// on USB.
fn bench() -> (Vec<Found>, Vec<ShownRuntime>) {
    let mut probe = usb(0x0483, 0x374B, UsbRole::DebugProbe, "ST-LINK/V2-1");
    probe.ports = vec!["/dev/ttyACM0".to_owned()];
    let mut adapter = usb(0x1A86, 0x7523, UsbRole::SerialAdapter, "WCH CH340");
    adapter.ports = vec!["/dev/ttyUSB0".to_owned()];
    let devices = vec![
        found("dev:serial:/dev/ttyUSB0", Some(adapter)),
        found("dev:usb:0671FF525750877267181518", Some(probe)),
        found("dev:usb:356A665D3431", Some(stm32_boot_loader())),
        found("dev:usb:20:6E:F1:17:04:C0", Some(esp32c6())),
        found("dev:usb:RT-1", Some(trust_runtime("RT-1"))),
        found("dev:usb:RT-2", Some(trust_runtime("RT-2"))),
        found("dev:usb:RT-3", Some(trust_runtime("RT-3"))),
        found("dev:ip:192.168.10.20", None),
    ];
    let runtimes = vec![
        shown("runtime:line-2", "Line 2", "usb://RT-2", "0.24.70"),
        shown("runtime:line-3", "Line 3", "usb://RT-3", TOOL_VERSION),
    ];
    (devices, runtimes)
}

#[test]
fn usb_tray_offers_install_update_connect_or_show() {
    let (devices, runtimes) = bench();
    let view = tray_with(&devices, &runtimes, &catalog(), TOOL_VERSION);
    assert_eq!(view.heading, "Found on this computer (USB)");
    assert_eq!(view.empty_text, None);
    let titles: Vec<&str> = view.items.iter().map(|item| item.title.as_str()).collect();
    assert_eq!(
        titles,
        vec![
            "ESP32-C6",
            "STM32 boot loader (DFU)",
            "truST runtime",
            "truST runtime",
            "truST runtime",
            "ST-LINK/V2-1",
            "WCH CH340",
        ],
        "chips and runtimes first, then probes, then adapters; the network device is no item"
    );

    // A chip on its boot loader: named as the boot loader, with the supported chips that the
    // chip files name for it, no truST yet, and Install.
    let chip = item(&view.items, "dev:usb:356A665D3431");
    assert_eq!(
        chip.detail,
        "Supported chip with these IDs: STM32F407. The chip is confirmed when connecting and \
         before any installation."
    );
    assert_eq!(chip.version.as_deref(), Some("No truST yet"));
    assert_eq!(action_ids(chip), vec!["install_trust"]);
    assert_eq!(chip.actions[0].label, "Install truST");
    assert_eq!(chip.actions[0].emphasis, Emphasis::Primary);
    assert!(opens_install(&chip.actions[0], "dev:usb:356A665D3431"));

    // The ESP32-C6: truST is not confirmed, so Connect, which confirms it, and Install.
    let c6 = item(&view.items, "dev:usb:20:6E:F1:17:04:C0");
    assert_eq!(c6.detail, "528 KB RAM");
    assert_eq!(c6.version.as_deref(), Some("truST not confirmed"));
    assert_eq!(action_ids(c6), vec!["connect", "install_trust"]);
    assert_connect(&c6.actions[0], "usb://20:6E:F1:17:04:C0", "ESP32-C6");
    assert_eq!(c6.actions[1].emphasis, Emphasis::Secondary);

    // A truST runtime that the panel does not show: its version and Connect.
    let unshown = item(&view.items, "dev:usb:RT-1");
    assert_eq!(unshown.detail, "Serial number RT-1");
    assert_eq!(unshown.version.as_deref(), Some("truST 0.24"));
    assert_eq!(action_ids(unshown), vec!["connect"]);
    assert_connect(&unshown.actions[0], "usb://RT-1", "truST runtime");

    // Runtimes that the panel shows: Show, and first Update truST when the version differs.
    let older = item(&view.items, "dev:usb:RT-2");
    assert_eq!(older.detail, "This is Line 2.");
    assert_eq!(older.version.as_deref(), Some("truST 0.24.70"));
    assert_eq!(action_ids(older), vec!["update_trust", "show"]);
    assert_eq!(older.actions[0].label, "Update truST");
    assert_eq!(older.actions[0].emphasis, Emphasis::Primary);
    assert!(opens_install(&older.actions[0], "dev:usb:RT-2"));
    assert!(focuses(&older.actions[1], "runtime:line-2"));
    let current = item(&view.items, "dev:usb:RT-3");
    assert_eq!(current.version.as_deref(), Some("truST 0.24.77"));
    assert_eq!(action_ids(current), vec!["show"]);
    assert_eq!(current.actions[0].label, "Show");
    assert!(focuses(&current.actions[0], "runtime:line-3"));

    // A probe installs through itself; an adapter offers nothing. Neither is a chip.
    let probe = item(&view.items, "dev:usb:0671FF525750877267181518");
    assert_eq!(probe.detail, "Debug probe · /dev/ttyACM0");
    assert_eq!(probe.version, None);
    assert_eq!(action_ids(probe), vec!["install_trust"]);
    let adapter = item(&view.items, "dev:serial:/dev/ttyUSB0");
    assert_eq!(adapter.detail, "USB-serial adapter · /dev/ttyUSB0");
    assert_eq!(adapter.version, None);
    assert_eq!(adapter.actions, Vec::<Action>::new());
    assert!(view.items.iter().all(|item| item.install.is_none()));
}

#[test]
fn usb_tray_says_why_it_is_empty_or_cannot_connect() {
    let network = vec![found("dev:ip:192.168.10.20", None)];
    for devices in [Vec::new(), network] {
        let view = tray(&devices, &[], TOOL_VERSION);
        assert_eq!(view.items, Vec::<UsbItem>::new());
        assert_eq!(
            view.empty_text.as_deref(),
            Some("No chips, debug probes, or USB-serial adapters on this computer's USB.")
        );
    }

    // A truST runtime without a serial number has no `usb://` endpoint to connect to.
    let mut nameless = trust_runtime("RT-9");
    nameless.serial_number = None;
    nameless.control = None;
    let view = tray(
        &[found("dev:usb-port:001-4", Some(nameless))],
        &[],
        TOOL_VERSION,
    );
    let connect = &view.items[0].actions[0];
    assert!(!connect.enabled);
    assert_eq!(
        connect.reason.as_deref(),
        Some("This device reports no serial number, so truST cannot reach it over USB.")
    );
    assert_eq!(connect.emphasis, Emphasis::Quiet);
}

#[test]
fn usb_runtime_merges_with_its_network_find_by_runtime_id() {
    // The panel reaches Sensor hub over USB, and the runtime reports its runtime ID; the search
    // also found the runtime on the network, with that ID. The tray item is Sensor hub, at the
    // address of its network find, and the network device is no tray item.
    let mut network = found("dev:ip:192.168.10.44", None);
    network.runtime_ids = vec![SENSOR_HUB_ID.to_owned()];
    network.addresses = vec!["192.168.10.44".to_owned()];
    let devices = vec![found("dev:usb:20:6E:F1:17:04:C0", Some(esp32c6())), network];
    let mut sensor_hub = shown(
        "runtime:sensor-hub",
        "Sensor hub",
        "usb://20:6E:F1:17:04:C0",
        TOOL_VERSION,
    );
    sensor_hub.runtime_ids = vec![SENSOR_HUB_ID.to_owned()];
    let catalog = catalog();
    let view = tray_with(
        &devices,
        std::slice::from_ref(&sensor_hub),
        &catalog,
        TOOL_VERSION,
    );
    assert_eq!(view.items.len(), 1);
    let item = &view.items[0];
    assert_eq!(item.title, "ESP32-C6");
    assert_eq!(
        item.detail,
        "This is Sensor hub, also reachable at 192.168.10.44."
    );
    assert_eq!(item.version.as_deref(), Some("truST 0.24.77"));
    assert_eq!(action_ids(item), vec!["show"]);
    assert!(focuses(&item.actions[0], "runtime:sensor-hub"));

    // The search joined the USB result and the network find into one device, and the panel
    // reaches the runtime over the network: the runtime ID matches them.
    let mut merged = found("dev:usb:20:6E:F1:17:04:C0", Some(esp32c6()));
    merged.runtime_ids = vec![SENSOR_HUB_ID.to_owned()];
    merged.addresses = vec!["192.168.10.44".to_owned()];
    let mut over_network = shown(
        "runtime:sensor-hub",
        "Sensor hub",
        "tcp://192.168.10.44:5680",
        "0.24.70",
    );
    over_network.runtime_ids = vec![SENSOR_HUB_ID.to_owned()];
    let view = tray_with(
        std::slice::from_ref(&merged),
        std::slice::from_ref(&over_network),
        &catalog,
        TOOL_VERSION,
    );
    let item = &view.items[0];
    assert_eq!(
        item.detail,
        "This is Sensor hub, also reachable at 192.168.10.44."
    );
    assert_eq!(item.version.as_deref(), Some("truST 0.24.70"));
    assert_eq!(action_ids(item), vec!["update_trust", "show"]);

    // Another runtime ID is another runtime: the chip stays unconfirmed.
    over_network.runtime_ids = vec!["mixer@pi".to_owned()];
    let view = tray_with(
        std::slice::from_ref(&merged),
        std::slice::from_ref(&over_network),
        &catalog,
        TOOL_VERSION,
    );
    assert_eq!(
        view.items[0].version.as_deref(),
        Some("truST not confirmed")
    );

    // A runtime of the chip, at `usb://<serial>/<runtime>`, is the chip's runtime too.
    let line = shown(
        "runtime:line-4",
        "Line 4",
        "usb://20:6E:F1:17:04:C0/line4",
        TOOL_VERSION,
    );
    let view = tray_with(
        &[found("dev:usb:20:6E:F1:17:04:C0", Some(esp32c6()))],
        std::slice::from_ref(&line),
        &catalog,
        TOOL_VERSION,
    );
    assert_eq!(view.items[0].detail, "This is Line 4.");
}

#[test]
fn two_chip_files_sharing_a_boot_loader_are_one_entry_naming_both() {
    // The STM32U5A9 shows the same DFU boot loader as the STM32F407.
    let mut stm32u5a9 = chip_file("stm32f407");
    stm32u5a9.identity.name = "stm32u5a9".to_owned();
    stm32u5a9.identity.model = "STM32U5A9".to_owned();
    let catalog = vec![chip_file("stm32f407"), chip_file("esp32c6"), stm32u5a9];
    let grouped = boot_loaders(&catalog);
    let entries: Vec<(u16, u16, Vec<&str>)> = grouped
        .iter()
        .map(|entry| {
            let names = entry
                .chips
                .iter()
                .map(|chip| chip.identity.name.as_str())
                .collect();
            (entry.vid, entry.pid, names)
        })
        .collect();
    assert_eq!(
        entries,
        vec![
            (0x0483, 0xDF11, vec!["stm32f407", "stm32u5a9"]),
            (0x303A, 0x1001, vec!["esp32c6"]),
        ]
    );

    // The tray names the boot loader as such and lists both chips.
    let devices = [found("dev:usb:356A665D3431", Some(stm32_boot_loader()))];
    let view = tray_with(&devices, &[], &catalog, TOOL_VERSION);
    let boot_loader = &view.items[0];
    assert_eq!(boot_loader.title, "STM32 boot loader (DFU)");
    assert_eq!(
        boot_loader.detail,
        "Supported chips with these IDs: STM32F407, STM32U5A9. The chip is confirmed when \
         connecting and before any installation."
    );
    assert_eq!(boot_loader.version.as_deref(), Some("No truST yet"));
    assert_eq!(action_ids(boot_loader), vec!["install_trust"]);

    // Without a chip file that names its IDs, a boot loader offers nothing to install.
    let view = tray_with(&devices, &[], &[chip_file("esp32c6")], TOOL_VERSION);
    assert_eq!(view.items[0].detail, "No supported chip shows these IDs.");
    assert_eq!(view.items[0].actions, Vec::<Action>::new());
}
