//! The USB tray (specification 25 §5.12): the chips, debug probes and USB-serial adapters that
//! the USB and serial sources of the one search find on this computer, each with what it is, its
//! truST state and the actions that fit it. Pure: the devices of a search and the runtimes that
//! the panel shows come in, and nothing here lists, opens or writes a device.
//!
//! - An item is a device with a USB result; other devices, such as a phone, never reach the tray.
//! - The chips that show the IDs of a boot loader or a chip's USB controller come from the chip
//!   files (`identity.usb_bootloader`), each ID once with every chip that names it. A boot loader
//!   is named as such, since other chips of its maker show it too, and lists those chips; the
//!   chip is confirmed when connecting and before any installation. A controller that one chip
//!   file names is named by that chip.
//! - A device that runs truST, or may run it, is matched with a runtime that the panel shows: the
//!   runtime it reaches at the device's `usb://` endpoint, or the one whose runtime ID the device
//!   carries. The network address where that runtime is also reachable comes from its network
//!   find, a device of the same search with one of its runtime IDs.
//! - Until V7 slice C7 lists this computer's USB for the panel, only the tests build the tray.

#![cfg_attr(
    not(test),
    allow(dead_code, reason = "V7 slice C7 builds the USB tray for the panel")
)]

use serde_json::json;
use trust_chips::ChipFile;
use trust_runtime::search::{FoundDevice, Identity, UsbIdentity, UsbRole};
use trust_runtime_client::ControlEndpoint;

use crate::texts::graph::CONNECT;
use crate::texts::usb as texts;
use crate::view::address;
use crate::view::chips;
use crate::view::types::{Action, Emphasis, ItemId, Op, Pane, UiOp, UsbItem, UsbTray};

/// What the tray reads of one device that a search of this computer found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    /// The device's ID in the search.
    pub(crate) id: String,
    /// The identity of its USB result, when it has one.
    pub(crate) usb: Option<UsbIdentity>,
    /// The truST runtime IDs found on it.
    pub(crate) runtime_ids: Vec<String>,
    /// Its network addresses.
    pub(crate) addresses: Vec<String>,
}

impl Found {
    /// What the tray reads of `device`. A result outside `trust-runtime` cannot be built, so the
    /// tests give `Found` directly.
    #[cfg_attr(
        test,
        allow(
            dead_code,
            reason = "V7 slice C7 reads the devices of this computer's search through it"
        )
    )]
    pub(crate) fn of(device: &FoundDevice) -> Self {
        Self {
            id: device.id.clone(),
            usb: device.finds.iter().find_map(|find| match &find.identity {
                Identity::Usb(usb) => Some(usb.clone()),
                _ => None,
            }),
            runtime_ids: device.runtime_ids.clone(),
            addresses: device.addresses.clone(),
        }
    }
}

/// A runtime that the panel shows, as the tray matches devices with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShownRuntime {
    /// Its card.
    pub(crate) item: ItemId,
    /// The name on its card.
    pub(crate) name: String,
    /// The control endpoint that the panel reaches it through.
    pub(crate) endpoint: Option<String>,
    /// The runtime IDs that it reports.
    pub(crate) runtime_ids: Vec<String>,
    /// The truST version that its `status` reports.
    pub(crate) version: Option<String>,
}

/// The tray of `found`, the devices of a search of this computer, with `shown`, the runtimes that
/// the panel shows, and `tool_version`, the truST version of this tool, and the chip files that
/// this build embeds.
pub(crate) fn tray(found: &[Found], shown: &[ShownRuntime], tool_version: &str) -> UsbTray {
    tray_with(found, shown, chips::all(), tool_version)
}

/// The tray with the chip files of `catalog`. Chips and runtimes come first, then debug probes,
/// then adapters.
fn tray_with(
    found: &[Found],
    shown: &[ShownRuntime],
    catalog: &[ChipFile],
    tool_version: &str,
) -> UsbTray {
    let mut ranked: Vec<(u8, UsbItem)> = found
        .iter()
        .filter_map(|device| {
            let usb = device.usb.as_ref()?;
            let item = item(device, usb, found, shown, catalog, tool_version);
            Some((rank(usb.role), item))
        })
        .collect();
    ranked.sort_by(|(left_rank, left), (right_rank, right)| {
        left_rank
            .cmp(right_rank)
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.id.cmp(&right.id))
    });
    let items: Vec<UsbItem> = ranked.into_iter().map(|(_, entry)| entry).collect();
    UsbTray {
        heading: texts::HEADING.to_owned(),
        empty_text: items.is_empty().then(|| texts::EMPTY.to_owned()),
        items,
    }
}

fn rank(role: UsbRole) -> u8 {
    match role {
        UsbRole::BootLoader | UsbRole::ChipController | UsbRole::TrustRuntime => 0,
        UsbRole::DebugProbe => 1,
        UsbRole::SerialAdapter | UsbRole::SerialDevice => 2,
    }
}

/// What an item says: the line under its name, its truST state and its actions.
struct Says {
    detail: String,
    version: Option<String>,
    actions: Vec<Action>,
}

/// One USB ID that the chip files name for a chip's boot loader, with every chip that names it.
#[derive(Debug)]
pub(crate) struct BootLoaderChips<'a> {
    /// The vendor ID.
    pub(crate) vid: u16,
    /// The product ID.
    pub(crate) pid: u16,
    /// The chips, in the catalog's order.
    pub(crate) chips: Vec<&'a ChipFile>,
}

/// The boot loader IDs of the chip files of `catalog`: each ID once, with every chip that names
/// it, so that a chip file that shares an ID with another is one more chip of the same entry.
pub(crate) fn boot_loaders(catalog: &[ChipFile]) -> Vec<BootLoaderChips<'_>> {
    let mut entries: Vec<BootLoaderChips<'_>> = Vec::new();
    for chip in catalog {
        let id = chip.identity.usb_bootloader;
        match entries
            .iter_mut()
            .find(|entry| entry.vid == id.vid && entry.pid == id.pid)
        {
            Some(entry) => entry.chips.push(chip),
            None => entries.push(BootLoaderChips {
                vid: id.vid,
                pid: id.pid,
                chips: vec![chip],
            }),
        }
    }
    entries
}

/// The chips of `catalog` whose boot loader shows the IDs of `usb`.
fn chips_showing<'a>(catalog: &'a [ChipFile], usb: &UsbIdentity) -> Vec<&'a ChipFile> {
    boot_loaders(catalog)
        .into_iter()
        .find(|entry| entry.vid == usb.vid && entry.pid == usb.pid)
        .map(|entry| entry.chips)
        .unwrap_or_default()
}

fn item(
    device: &Found,
    usb: &UsbIdentity,
    found: &[Found],
    shown: &[ShownRuntime],
    catalog: &[ChipFile],
    tool_version: &str,
) -> UsbItem {
    let chips = match usb.role {
        UsbRole::BootLoader | UsbRole::ChipController => chips_showing(catalog, usb),
        _ => Vec::new(),
    };
    // A boot loader keeps its own name; a controller that one chip file names is that chip.
    let only_chip = match (usb.role, chips.as_slice()) {
        (UsbRole::ChipController, [chip]) => Some(*chip),
        _ => None,
    };
    let title = only_chip.map_or_else(|| usb.model.clone(), |chip| chip.identity.model.clone());
    let runtime = match usb.role {
        UsbRole::TrustRuntime | UsbRole::ChipController => shown_runtime(device, usb, shown),
        _ => None,
    };
    let says = match runtime {
        Some(runtime) => matched(device, usb, runtime, found, tool_version),
        None => unmatched(device, usb, &chips, only_chip, &title),
    };
    UsbItem {
        id: device.id.clone(),
        title,
        detail: says.detail,
        version: says.version,
        actions: says.actions,
        install: None,
    }
}

/// A device that is a runtime the panel shows: which runtime it is, its version, `Show`, and
/// first `Update truST` when its version is not this tool's (specification 34 TARGET-05).
fn matched(
    device: &Found,
    usb: &UsbIdentity,
    runtime: &ShownRuntime,
    found: &[Found],
    tool_version: &str,
) -> Says {
    let address = network_address(device, runtime, found);
    let version = runtime
        .version
        .as_deref()
        .or(usb.version.as_deref())
        .map_or_else(|| texts::TRUST.to_owned(), texts::trust_version);
    let mut actions = Vec::new();
    if runtime
        .version
        .as_deref()
        .is_some_and(|reported| reported.trim() != tool_version.trim())
    {
        actions.push(enabled(
            "update_trust",
            texts::UPDATE_TRUST,
            Emphasis::Primary,
            install_op(device),
        ));
    }
    actions.push(enabled(
        "show",
        texts::SHOW,
        Emphasis::Secondary,
        Op::Ui {
            ui: UiOp::Focus {
                item: runtime.item.clone(),
            },
        },
    ));
    Says {
        detail: texts::this_is(&runtime.name, address.as_deref()),
        version: Some(version),
        actions,
    }
}

/// A device that is no runtime the panel shows, by what it is. A boot loader or a controller
/// offers `Install truST` only while a chip file names its IDs.
fn unmatched(
    device: &Found,
    usb: &UsbIdentity,
    chips: &[&ChipFile],
    only_chip: Option<&ChipFile>,
    title: &str,
) -> Says {
    let installable = |emphasis| (!chips.is_empty()).then(|| install(device, emphasis));
    match usb.role {
        UsbRole::BootLoader => Says {
            detail: chip_list(chips),
            version: Some(texts::NO_TRUST_YET.to_owned()),
            actions: installable(Emphasis::Primary).into_iter().collect(),
        },
        UsbRole::ChipController => Says {
            detail: only_chip.map_or_else(|| chip_list(chips), memory),
            version: Some(texts::TRUST_NOT_CONFIRMED.to_owned()),
            actions: std::iter::once(connect(usb, title))
                .chain(installable(Emphasis::Secondary))
                .collect(),
        },
        UsbRole::TrustRuntime => Says {
            detail: usb.serial_number.as_deref().map_or_else(
                || texts::with_ports(&usb.model, &usb.ports),
                texts::serial_number,
            ),
            version: Some(
                usb.version
                    .as_deref()
                    .map_or_else(|| texts::TRUST.to_owned(), texts::trust_version),
            ),
            actions: vec![connect(usb, title)],
        },
        UsbRole::DebugProbe => Says {
            detail: texts::with_ports(texts::DEBUG_PROBE, &usb.ports),
            version: None,
            actions: vec![install(device, Emphasis::Primary)],
        },
        UsbRole::SerialAdapter | UsbRole::SerialDevice => Says {
            detail: texts::with_ports(texts::SERIAL_ADAPTER, &usb.ports),
            version: None,
            actions: Vec::new(),
        },
    }
}

/// The supported chips that show a device's IDs, by their models, with the note that the chip is
/// confirmed before anything is installed.
fn chip_list(chips: &[&ChipFile]) -> String {
    let models: Vec<&str> = chips
        .iter()
        .map(|chip| chip.identity.model.as_str())
        .collect();
    texts::supported_chips(&models)
}

/// A chip's flash and RAM, from its chip file.
fn memory(chip: &ChipFile) -> String {
    let ram: u32 = chip.memory.ram.iter().map(|region| region.kib).sum();
    texts::flash_and_ram(chip.memory.flash.as_ref().map(|flash| flash.kib), ram)
}

/// The runtime that the panel shows for a device that runs or may run truST: the one it reaches
/// at the device's `usb://` endpoint or at a runtime of it (`usb://<serial>/<runtime>`), else the
/// one whose runtime ID the device carries.
fn shown_runtime<'a>(
    device: &Found,
    usb: &UsbIdentity,
    shown: &'a [ShownRuntime],
) -> Option<&'a ShownRuntime> {
    let on_usb = usb.control.as_deref().and_then(|control| {
        shown.iter().find(|runtime| {
            runtime
                .endpoint
                .as_deref()
                .is_some_and(|endpoint| reaches(endpoint.trim(), control))
        })
    });
    on_usb.or_else(|| {
        shown
            .iter()
            .find(|runtime| shares_runtime_id(&device.runtime_ids, runtime))
    })
}

fn reaches(endpoint: &str, control: &str) -> bool {
    endpoint == control
        || endpoint
            .strip_prefix(control)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn shares_runtime_id(runtime_ids: &[String], runtime: &ShownRuntime) -> bool {
    runtime
        .runtime_ids
        .iter()
        .any(|id| runtime_ids.contains(id))
}

/// The network address where a matched runtime is also reachable: the device's own address, the
/// address of the runtime's network find (another device of the search with one of the runtime's
/// IDs), or the host of the network endpoint that the panel reaches the runtime through.
fn network_address(device: &Found, runtime: &ShownRuntime, found: &[Found]) -> Option<String> {
    device
        .addresses
        .first()
        .cloned()
        .or_else(|| {
            found
                .iter()
                .filter(|other| other.id != device.id)
                .filter(|other| shares_runtime_id(&other.runtime_ids, runtime))
                .find_map(|other| other.addresses.first().cloned())
        })
        .or_else(|| runtime.endpoint.as_deref().and_then(network_host))
}

/// The host of a network control endpoint (`tcp://` or `tls://`) that names another computer.
fn network_host(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    let network = endpoint.starts_with("tcp://") || endpoint.starts_with("tls://");
    if !network {
        return None;
    }
    address::endpoint_host(endpoint).filter(|host| !address::is_loopback(host))
}

/// `Connect` at the device's `usb://` endpoint. It stays disabled, with its reason, while the
/// control client of this truST cannot reach that endpoint, and for a device without a serial
/// number.
fn connect(usb: &UsbIdentity, label: &str) -> Action {
    let endpoint = usb.control.as_deref();
    let op = Op::Rust {
        action: "runtime.connect".to_owned(),
        payload: json!({ "endpoint": endpoint, "label": label }),
    };
    match endpoint {
        None => disabled("connect", CONNECT, texts::CONNECT_WITHOUT_SERIAL_NUMBER, op),
        Some(endpoint) if ControlEndpoint::parse(endpoint).is_none() => {
            disabled("connect", CONNECT, texts::CONNECT_OVER_USB_UNAVAILABLE, op)
        }
        Some(_) => enabled("connect", CONNECT, Emphasis::Primary, op),
    }
}

/// `Install truST`, which opens the installation in the inspector (specification 25 §5.12).
fn install(device: &Found, emphasis: Emphasis) -> Action {
    enabled(
        "install_trust",
        texts::INSTALL_TRUST,
        emphasis,
        install_op(device),
    )
}

fn install_op(device: &Found) -> Op {
    Op::Ui {
        ui: UiOp::Open {
            pane: Pane::Install {
                usb: device.id.clone(),
            },
        },
    }
}

fn enabled(id: &str, label: &str, emphasis: Emphasis, op: Op) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: true,
        reason: None,
        emphasis,
        confirm: None,
        op,
    }
}

/// A disabled action: always with its reason, and drawn neutral.
fn disabled(id: &str, label: &str, reason: &str, op: Op) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: false,
        reason: Some(reason.to_owned()),
        emphasis: Emphasis::Quiet,
        confirm: None,
        op,
    }
}

#[cfg(test)]
mod tests;
