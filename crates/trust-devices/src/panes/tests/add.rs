//! The add dialog (specification 25 §5.9 and §5.14). The words of the picker are the ones that
//! `network-canvas.test.ts` pins for `grouping.ts`, which the catalog now carries.

use serde_json::{json, Value};
use trust_runtime::control::offline_comm_schema_json;

use crate::gather::{ManagedState, ProjectInputs, TargetReport};
use crate::panes::forms::{setup_form, RuntimeSetup};
use crate::request::{RunMode, ViewRequest};
use crate::targets::TargetOrigin;
use crate::view::project as project_view;
use crate::view::tests::fixtures::{
    answered, endpoint, gathered, host, managed, no_project, report, request, runtime, status,
    with_simulator, NOW_MS, SIMULATOR,
};
use crate::view::types::{AddDialog, AddEntry, AddGroup, FieldKind, Op, QuickStart};

/// The picker's entries of the tool's own catalog in their order: protocol, title, badge and
/// purpose (the preservation table of 16 October 2026, §3 P4).
const ENTRIES: [(&str, &str, &str, &str); 14] = [
    (
        "modbus_tcp",
        "Modbus",
        "MB",
        "Connect PLCs, drives, meters, and remote I/O.",
    ),
    (
        "ethercat",
        "EtherCAT",
        "EC",
        "Connect EtherCAT drives, terminals, or remote I/O.",
    ),
    (
        "gpio",
        "GPIO",
        "IO",
        "Use Raspberry Pi or Linux controller pins.",
    ),
    (
        "simulated",
        "Simulated I/O",
        "SIM",
        "Practice and test without hardware.",
    ),
    (
        "loopback",
        "Loopback I/O",
        "LOOP",
        "Echo outputs back into inputs for quick checks.",
    ),
    (
        "opcua_client",
        "OPC UA client",
        "UA IN",
        "Read selected tags from another OPC UA server.",
    ),
    (
        "ads",
        "ADS client",
        "ADS IN",
        "Read tags from a TwinCAT or ADS PLC.",
    ),
    (
        "opcua",
        "OPC UA server",
        "UA OUT",
        "Share truST values with SCADA, HMI, or historians.",
    ),
    (
        "ads_server",
        "ADS server",
        "ADS OUT",
        "Share truST values with TwinCAT or ADS clients.",
    ),
    (
        "mqtt",
        "MQTT broker",
        "MQTT",
        "Send and receive process values through MQTT topics.",
    ),
    (
        "mesh",
        "Mesh / Zenoh",
        "MESH",
        "Configure a peer network; no live link is active until a runtime reports one.",
    ),
    (
        "openot",
        "OpenOT",
        "OT",
        "Configure OpenOT evidence output; no evidence is published until a runtime reports one.",
    ),
    (
        "realtime_t0",
        "Realtime T0",
        "RT",
        "Configure deterministic same-host exchange; no live exchange runs until a runtime reports one.",
    ),
    (
        "runtime_cloud",
        "Runtime cloud",
        "CLOUD",
        "Configure federation policy; no live link is active until a runtime reports one.",
    ),
];

/// The entries of `Runtimes`: title, badge, purpose and whether they can be chosen.
const RUNTIME_ENTRIES: [(&str, &str, &str, bool); 6] = [
    (
        "Connect existing runtime",
        "Computer",
        "Use a runtime already running on another computer or controller.",
        true,
    ),
    (
        "Run a runtime on this computer",
        "Computer",
        "Create a managed runtime project here, then select it as the Target and click Start.",
        true,
    ),
    ("STM32F407", "Chip", "truST firmware · Ethernet, USB", true),
    (
        "ESP32-C6",
        "Chip",
        "truST firmware · Wi-Fi, Bluetooth, USB",
        true,
    ),
    (
        "Install over SSH",
        "Computer",
        "Install on another computer over SSH.",
        false,
    ),
    (
        "Run in Docker",
        "Computer",
        "Run the runtime as a container.",
        false,
    ),
];

/// The quick starts: title and description.
const QUICK_STARTS: [(&str, &str); 4] = [
    (
        "Local simulated counter",
        "No hardware. Adds simulated inputs/outputs with a counter pattern.",
    ),
    (
        "Loopback sanity check",
        "Echoes outputs back into inputs for a fast local wiring check.",
    ),
    (
        "Modbus drive or meter",
        "TCP device at 192.168.1.50:502 with Unit ID 1.",
    ),
    (
        "MQTT broker",
        "Broker at 127.0.0.1:1883 with default process topics.",
    ),
];

/// The tool's own catalog, as `trust-runtime comm schema` answers it.
pub(super) fn library() -> Value {
    offline_comm_schema_json(None).expect("the tool's own catalog")
}

/// The tool's own catalog with only the protocols `ids`, then the protocols `more`.
fn catalog_of(ids: &[&str], more: Vec<Value>) -> Value {
    let mut catalog = library();
    let mut protocols: Vec<Value> = catalog["protocols"]
        .as_array()
        .expect("protocols")
        .iter()
        .filter(|protocol| ids.iter().any(|id| protocol["id"] == *id))
        .cloned()
        .collect();
    protocols.extend(more);
    catalog["protocols"] = Value::Array(protocols);
    catalog
}

/// A catalog as a runtime that predates the words of the picker and the forms answers it.
pub(super) fn without_words(mut catalog: Value) -> Value {
    if let Some(object) = catalog.as_object_mut() {
        object.remove("picker_groups");
    }
    for protocol in catalog["protocols"].as_array_mut().expect("protocols") {
        if let Some(object) = protocol.as_object_mut() {
            object.remove("picker");
        }
        for field in protocol["fields"].as_array_mut().expect("fields") {
            if let Some(object) = field.as_object_mut() {
                object.remove("option_labels");
                object.remove("list");
            }
        }
    }
    catalog
}

/// The dialog of a workspace without a project whose own catalog is `catalog`.
fn dialog_with(catalog: Result<Value, String>, targets: Vec<TargetReport>) -> AddDialog {
    dialog_for(catalog, targets, &request(host(RunMode::Simulate, "")))
}

fn dialog_for(
    catalog: Result<Value, String>,
    targets: Vec<TargetReport>,
    view_request: &ViewRequest,
) -> AddDialog {
    let project = ProjectInputs {
        catalog,
        ..no_project()
    };
    project_view(&gathered(project, targets), view_request, None, NOW_MS).add
}

/// The dialog while the simulator runs and serves the catalog `schema`.
fn running_simulator_dialog(schema: Value) -> AddDialog {
    let mut simulator = runtime("RESOURCE", "RESOURCE", "connected", Vec::new());
    simulator["mode"] = json!("simulate");
    simulator["control_endpoint"] = json!(SIMULATOR);
    let mut target = answered(
        TargetOrigin::Simulator,
        SIMULATOR,
        None,
        status("running", "admin"),
        report("host:line-pc", "line-pc", &["127.0.0.1"], vec![simulator]),
    );
    target.live.schema = Some(Ok(schema));
    dialog_for(
        Ok(library()),
        vec![target],
        &request(with_simulator(host(RunMode::Simulate, ""), false)),
    )
}

fn labels(dialog: &AddDialog) -> Vec<&str> {
    dialog
        .groups
        .iter()
        .map(|group| group.label.as_str())
        .collect()
}

fn entries(dialog: &AddDialog) -> Vec<&AddEntry> {
    dialog
        .groups
        .iter()
        .flat_map(|group| &group.entries)
        .collect()
}

fn entry<'a>(dialog: &'a AddDialog, protocol: &str) -> &'a AddEntry {
    entries(dialog)
        .into_iter()
        .find(|entry| entry.protocol == protocol)
        .unwrap_or_else(|| panic!("an entry {protocol}"))
}

fn words(entry: &AddEntry) -> (&str, &str, &str, &str) {
    (
        entry.protocol.as_str(),
        entry.title.as_str(),
        entry.badge.as_str(),
        entry.purpose.as_str(),
    )
}

fn payload(entry: &AddEntry) -> (&str, &Value) {
    match &entry.action.op {
        Op::Rust { action, payload } => (action.as_str(), payload),
        other => panic!("{} opens no Rust action: {other:?}", entry.protocol),
    }
}

#[test]
fn picker_groups_in_canonical_order_runtimes_first() {
    let dialog = dialog_with(Ok(library()), Vec::new());
    assert_eq!(dialog.runtimes.label, "Runtimes");
    assert!(!dialog.runtimes.advanced);
    assert_eq!(
        labels(&dialog),
        [
            "Devices and I/O",
            "Read tags from another PLC or server",
            "Share truST values",
            "Send and receive messages",
            "Advanced integrations",
        ]
    );

    // Ported from `groups protocols by user intent in the S-09 order`.
    let ids = [
        "opcua",
        "modbus_tcp",
        "mqtt",
        "mesh",
        "gpio",
        "opcua_client",
        "ads_server",
        "ads",
    ];
    let dialog = dialog_with(Ok(catalog_of(&ids, Vec::new())), Vec::new());
    let protocols: Vec<Vec<&str>> = dialog
        .groups
        .iter()
        .map(|group| {
            group
                .entries
                .iter()
                .map(|entry| entry.protocol.as_str())
                .collect()
        })
        .collect();
    assert_eq!(
        protocols,
        [
            vec!["modbus_tcp", "gpio"],
            vec!["opcua_client", "ads"],
            vec!["opcua", "ads_server"],
            vec!["mqtt"],
            vec!["mesh"],
        ],
        "Modbus TCP comes before GPIO"
    );

    // Ported from `omits empty groups and keeps advanced choices separate`.
    let dialog = dialog_with(
        Ok(catalog_of(&["modbus_tcp", "mesh"], Vec::new())),
        Vec::new(),
    );
    assert_eq!(
        labels(&dialog),
        ["Devices and I/O", "Advanced integrations"]
    );
    assert!(!dialog.groups[0].advanced);
    assert!(dialog.groups[1].advanced);
}

#[test]
fn picker_entries_word_for_word() {
    let dialog = dialog_with(Ok(library()), Vec::new());
    let shown: Vec<(&str, &str, &str, &str)> = entries(&dialog).into_iter().map(words).collect();
    assert_eq!(shown, ENTRIES);
    // Each entry opens its protocol's form for the runtime that the dialog adds to.
    let target = dialog.target.as_ref().expect("the simulator is the target");
    assert_eq!(target.name, "Simulator");
    let modbus = entry(&dialog, "modbus_tcp");
    assert!(modbus.action.enabled);
    assert_eq!(
        payload(modbus),
        (
            "form.open",
            &json!({ "protocol": "modbus_tcp", "runtime": target.id.as_str() })
        )
    );
}

#[test]
fn client_and_server_badges_and_direction_copy() {
    let dialog = dialog_with(Ok(library()), Vec::new());
    let group_of = |protocol: &str| {
        dialog
            .groups
            .iter()
            .find(|group| group.entries.iter().any(|entry| entry.protocol == protocol))
            .map(|group| group.label.as_str())
    };
    for (protocol, badge, group) in [
        ("opcua", "UA OUT", "Share truST values"),
        (
            "opcua_client",
            "UA IN",
            "Read tags from another PLC or server",
        ),
        ("ads_server", "ADS OUT", "Share truST values"),
        ("ads", "ADS IN", "Read tags from another PLC or server"),
    ] {
        assert_eq!(entry(&dialog, protocol).badge, badge, "{protocol}");
        assert_eq!(group_of(protocol), Some(group), "{protocol}");
    }
    assert!(entry(&dialog, "opcua_client")
        .purpose
        .contains("Read selected tags"));
    assert!(entry(&dialog, "opcua")
        .purpose
        .contains("Share truST values"));
    assert!(entry(&dialog, "ads").purpose.contains("Read tags"));
    assert!(entry(&dialog, "ads_server")
        .purpose
        .contains("Share truST values"));
}

#[test]
fn advanced_titles_and_badges() {
    let dialog = dialog_with(Ok(library()), Vec::new());
    let advanced = dialog
        .groups
        .iter()
        .find(|group| group.label == "Advanced integrations")
        .expect("the advanced group");
    assert!(advanced.advanced);
    let shown: Vec<(&str, &str)> = advanced
        .entries
        .iter()
        .map(|entry| (entry.title.as_str(), entry.badge.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("Mesh / Zenoh", "MESH"),
            ("OpenOT", "OT"),
            ("Realtime T0", "RT"),
            ("Runtime cloud", "CLOUD"),
        ]
    );
    for (protocol, words) in [
        ("mesh", "peer network"),
        ("openot", "OpenOT evidence"),
        ("realtime_t0", "deterministic"),
        ("runtime_cloud", "federation"),
    ] {
        let purpose = &entry(&dialog, protocol).purpose;
        assert!(purpose.contains(words), "{protocol}: {purpose}");
        assert!(!purpose.contains("pretending"), "{protocol}: {purpose}");
    }
}

#[test]
fn discovery_is_never_a_picker_entry() {
    let current = dialog_with(Ok(library()), Vec::new());
    // A runtime that predates the picker's words serves `discovery` with every other protocol.
    let older = running_simulator_dialog(without_words(library()));
    for dialog in [&current, &older] {
        assert!(
            entries(dialog)
                .iter()
                .all(|entry| entry.protocol != "discovery"),
            "discovery is no protocol card: {:?}",
            labels(dialog)
        );
        assert!(!labels(dialog).contains(&"Other choices"));
    }
    assert_eq!(
        entries(&older).into_iter().map(words).collect::<Vec<_>>(),
        ENTRIES,
        "the older runtime's entries take the tool's own words"
    );
    // Ported from `does not render runtime discovery as a protocol card`.
    let dialog = dialog_with(
        Ok(catalog_of(&["discovery", "modbus_tcp"], Vec::new())),
        Vec::new(),
    );
    assert_eq!(labels(&dialog), ["Devices and I/O"]);
    assert_eq!(entries(&dialog).len(), 1);
}

#[test]
fn unknown_protocols_land_in_other_choices() {
    let unknown = |id: &str| {
        json!({ "id": id, "title": id, "purpose": format!("{id} purpose"),
                "supports_test": false, "actions": ["add"], "fields": [] })
    };
    let catalog = catalog_of(
        &["modbus_tcp", "discovery"],
        vec![unknown("mystery"), unknown("blank")],
    );
    let dialog = dialog_with(Ok(catalog), Vec::new());
    assert_eq!(labels(&dialog), ["Devices and I/O", "Other choices"]);
    let other = &dialog.groups[1];
    assert!(other.advanced);
    let shown: Vec<(&str, &str, &str, &str)> = other.entries.iter().map(words).collect();
    assert_eq!(
        shown,
        [
            ("mystery", "mystery", "MYS", "mystery purpose"),
            ("blank", "blank", "BLA", "blank purpose"),
        ]
    );
    assert_eq!(entries(&dialog).len(), 3, "nothing is dropped");
}

#[test]
fn advanced_toggle_counts_hidden_entries() {
    let dialog = dialog_with(Ok(library()), Vec::new());
    let toggle = dialog.advanced_toggle.expect("the advanced toggle");
    assert_eq!(toggle.show_label, "Show advanced integrations");
    assert_eq!(toggle.hide_label, "Hide advanced integrations");
    assert_eq!(toggle.count, 4);

    let unknown = json!({ "id": "mystery", "title": "Mystery", "purpose": "", "fields": [] });
    let catalog = catalog_of(&["mesh", "mqtt"], vec![unknown]);
    let dialog = dialog_with(Ok(catalog), Vec::new());
    assert_eq!(
        dialog.advanced_toggle.map(|toggle| toggle.count),
        Some(2),
        "Mesh / Zenoh and the other choice are hidden"
    );

    let dialog = dialog_with(Ok(catalog_of(&["modbus_tcp"], Vec::new())), Vec::new());
    assert_eq!(dialog.advanced_toggle, None);
}

#[test]
fn runtimes_group_lists_chips_and_disabled_ssh_docker_with_details() {
    let dialog = dialog_with(Ok(library()), Vec::new());
    let shown: Vec<(&str, &str, &str, bool)> = dialog
        .runtimes
        .entries
        .iter()
        .map(|entry| {
            (
                entry.title.as_str(),
                entry.badge.as_str(),
                entry.purpose.as_str(),
                entry.action.enabled,
            )
        })
        .collect();
    assert_eq!(shown, RUNTIME_ENTRIES);
    let by_title = |title: &str| {
        dialog
            .runtimes
            .entries
            .iter()
            .find(|entry| entry.title == title)
            .unwrap_or_else(|| panic!("an entry {title}"))
    };
    for later in ["Install over SSH", "Run in Docker"] {
        assert_eq!(
            by_title(later).action.reason.as_deref(),
            Some("Available in a later release."),
            "{later}"
        );
    }
    assert_eq!(
        payload(by_title("Connect existing runtime")),
        ("form.open", &json!({ "setup": "connect" }))
    );
    assert_eq!(
        payload(by_title("Run a runtime on this computer")),
        ("form.open", &json!({ "setup": "local" }))
    );
    assert_eq!(
        payload(by_title("STM32F407")),
        ("runtime.install_chip", &json!({ "chip": "stm32f407" }))
    );
}

#[test]
fn runtime_setup_forms_keep_their_words() {
    let connect = setup_form(RuntimeSetup::Connect);
    assert_eq!(connect.title, "Connect existing runtime");
    assert_eq!(connect.breadcrumb, "Devices & Connections / Runtime setup");
    assert_eq!(
        connect.purpose.as_deref(),
        Some(
            "Add a truST runtime that is already running on another computer or controller. \
             truST checks the address before it is shown as connected."
        )
    );
    assert_eq!(
        connect.runtime, None,
        "it adds a runtime and writes to none"
    );
    let fields = &connect.sections[0].fields;
    assert_eq!(fields[0].label, "Runtime address");
    assert_eq!(fields[0].placeholder.as_deref(), Some("10.0.0.5:5680"));
    assert_eq!(
        fields[0].help,
        "Host name or IP plus port. Advanced: tcp://host:port or unix:///path/to/socket."
    );
    assert_eq!(fields[1].label, "Runtime auth token (optional)");
    assert_eq!(fields[1].kind, FieldKind::Secret);
    assert!(!fields[1].required);
    assert_eq!(fields[1].placeholder.as_deref(), Some("Optional"));
    assert_eq!(
        connect.note.as_deref(),
        Some("If you do not know the address, use Discover instead.")
    );
    assert_eq!(connect.buttons[0].label, "Add runtime");

    let local = setup_form(RuntimeSetup::Local);
    assert_eq!(local.title, "Add a runtime");
    assert_eq!(
        local.purpose.as_deref(),
        Some(
            "A second runtime (PLC) on this host. It's created as its own project; start it \
             to bring it online on the canvas."
        )
    );
    let fields = &local.sections[0].fields;
    assert_eq!(fields[0].label, "Name");
    assert_eq!(fields[0].placeholder.as_deref(), Some("cell1"));
    assert_eq!(fields[1].label, "Template");
    assert_eq!(fields[1].value, json!("simulate"));
    let templates: Vec<(&str, &str)> = fields[1]
        .options
        .iter()
        .map(|option| (option.value.as_str(), option.label.as_str()))
        .collect();
    assert_eq!(
        templates,
        [
            ("simulate", "Simulator (no hardware)"),
            ("empty", "Empty (loopback I/O)"),
        ]
    );
    assert_eq!(local.buttons[0].label, "Add runtime");
}

#[test]
fn quick_starts_only_without_connections() {
    let cell = runtime(
        "cell1",
        "cell1",
        "connected",
        vec![endpoint("cell1", "modbus_tcp", "Drive A", "connected")],
    );
    let managed_cell = managed(
        "cell1",
        "tcp://127.0.0.1:9901",
        ManagedState::Running,
        Some((
            status("running", "admin"),
            report("host:line-pc", "line-pc", &["127.0.0.1"], vec![cell]),
        )),
    );
    let mut view_request = request(host(RunMode::Simulate, ""));
    let dialog = dialog_for(Ok(library()), vec![managed_cell.clone()], &view_request);
    assert_eq!(
        dialog.target.as_ref().map(|target| target.name.as_str()),
        Some("Simulator")
    );
    assert_eq!(dialog.runtime_choices.len(), 2, "the simulator and cell1");
    let starts: Vec<(&str, &str)> = dialog
        .quick_starts
        .iter()
        .map(|start| (start.title.as_str(), start.description.as_str()))
        .collect();
    assert_eq!(starts, QUICK_STARTS);
    let Op::Rust { action, payload } = &dialog.quick_starts[2].action.op else {
        panic!("a quick start opens a form");
    };
    assert_eq!(action, "form.open");
    assert_eq!(
        payload["prefill"],
        json!({ "address": "192.168.1.50:502", "unit_id": 1 })
    );

    // Selecting the runtime that has a connection makes it the target: no quick starts.
    let cell1 = dialog
        .runtime_choices
        .iter()
        .find(|choice| choice.name == "cell1")
        .expect("cell1")
        .id
        .clone();
    view_request.view.selected = Some(cell1.0.clone());
    let dialog = dialog_for(Ok(library()), vec![managed_cell], &view_request);
    assert_eq!(dialog.target.map(|target| target.id), Some(cell1));
    assert_eq!(dialog.quick_starts, Vec::<QuickStart>::new());
}

#[test]
fn catalog_from_the_runtime_or_the_library() {
    // No runtime serves a catalog: the tool's own one, before anything runs.
    let dialog = dialog_with(Ok(library()), Vec::new());
    assert_eq!(dialog.groups.len(), 5);
    assert_eq!(dialog.catalog_state, None);

    // The running simulator serves its own catalog, which the dialog shows.
    let dialog = running_simulator_dialog(catalog_of(&["mqtt"], Vec::new()));
    assert_eq!(labels(&dialog), ["Send and receive messages"]);
    assert_eq!(entry(&dialog, "mqtt").title, "MQTT broker");

    // A runtime that predates the picker's words gets them from the tool's own catalog.
    let older = without_words(catalog_of(&["modbus_tcp", "ethercat"], Vec::new()));
    let dialog = running_simulator_dialog(older);
    assert_eq!(labels(&dialog), ["Devices and I/O"]);
    assert_eq!(words(entry(&dialog, "modbus_tcp")), ENTRIES[0]);

    // A served catalog without protocols offers nothing, and says so.
    let dialog = running_simulator_dialog(json!({ "schema_version": 4, "protocols": [] }));
    assert_eq!(dialog.groups, Vec::<AddGroup>::new());
    assert_eq!(
        dialog.catalog_state.as_deref(),
        Some("No device or connection types are available.")
    );

    // Without any catalog, the dialog says that it is unavailable.
    let dialog = dialog_with(Err("no catalog".to_owned()), Vec::new());
    assert_eq!(
        dialog.catalog_state.as_deref(),
        Some("Catalog unavailable (needs a runtime that serves the schema).")
    );
    assert_eq!(dialog.runtimes.entries.len(), 6, "Runtimes stays");
}
