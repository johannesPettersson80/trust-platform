//! Native tests of the Discover pane's scan choices (specification 25 §2): the `EtherCAT` warning and its
//! confirmation, the Engineer role of a confirmed fieldbus scan, the remembered ticks per project with ADS
//! ticked on the first run, and the protocols that a runtime advertises beyond the ten sources. Each expected
//! value is a literal worked out from specification 25 §2, from the old pane's words, or from decision O12
//! and O14; none is read back from the code under test.

use std::collections::BTreeMap;
use std::fs;

use serde_json::json;
use trust_runtime::security::AccessRole;

use crate::devices_file::{self, Memory};
use crate::test_project::TempFolder;
use crate::view::discover::{self, RuntimeOrigin, Scan};
use crate::view::discover_context::{self, Context, OriginFacts};
use crate::view::types::{Confirm, DiscoverView, Op};

/// The Discover view of the idle pane, scanned from `scan` with the runtimes `runtimes` and `ctx`.
fn idle_view(scan: &Scan, runtimes: &[RuntimeOrigin], ctx: &Context) -> DiscoverView {
    discover::idle(scan, runtimes, ctx)
}

/// The runtime `runtime:cell`, ready to scan.
fn cell() -> Vec<RuntimeOrigin> {
    vec![RuntimeOrigin {
        id: "runtime:cell".to_owned(),
        name: "Cell".to_owned(),
        ready: true,
    }]
}

/// The facts of `runtime:cell`: its role and its `EtherCAT` adapter.
fn cell_facts(role: Option<AccessRole>) -> BTreeMap<String, OriginFacts> {
    BTreeMap::from([(
        "runtime:cell".to_owned(),
        OriginFacts {
            role,
            ethercat_adapter: Some("eth0".to_owned()),
            discover: None,
        },
    )])
}

/// The ticks that the project remembers.
fn remembered(protocols: &[&str]) -> Context {
    Context {
        memory: Memory {
            protocols: Some(protocols.iter().map(|name| (*name).to_owned()).collect()),
            ..Memory::default()
        },
        ..Context::default()
    }
}

#[test]
fn a_fieldbus_search_asks_for_confirmation_naming_bus_adapter_and_effect() {
    // Specification 25 §2 (old line 204): the EtherCAT choice carries its warning, `Can disturb a live bus, so
    // confirm before scanning`. A Search with EtherCAT ticked asks first; the confirmation names the bus, the
    // adapter of the origin's EtherCAT connection, and the effect of the scan (spec 11's source table). Only
    // that confirmed Search posts `confirm_fieldbus_scan: true`, and it keeps the `only` list. A wrong
    // implementation runs the scan without the question, names no adapter, or sends the flag without a tick.
    let scan = Scan::Runtime("runtime:cell".to_owned());
    let ctx = Context {
        origins: cell_facts(Some(AccessRole::Engineer)),
        ..remembered(&["ethercat"])
    };
    let view = idle_view(&scan, &cell(), &ctx);
    let ethercat = view
        .only
        .iter()
        .find(|choice| choice.source == "ethercat")
        .expect("the EtherCAT choice");
    assert_eq!(
        (ethercat.enabled, ethercat.checked),
        (true, true),
        "ticked and enabled from an Engineer's runtime"
    );
    assert_eq!(
        ethercat.warning.as_deref(),
        Some("Can disturb a live bus, so confirm before scanning"),
        "the warning title of the EtherCAT choice (old line 204)"
    );
    let search = &view.actions[0];
    assert_eq!(
        search.confirm,
        Some(Confirm {
            text: "EtherCAT scans the SubDevices on the adapter eth0. It reads each SubDevice's identity in PRE-OP with its process data mapped, and it never brings the bus to SAFE-OP or OP. It can disturb a live bus.".to_owned(),
            confirm: "Search".to_owned(),
            cancel: "Cancel".to_owned(),
        }),
        "the confirmation names the bus, the adapter and the effect"
    );
    assert_eq!(
        search.op,
        Op::Search {
            request: json!({
                "verb": "start",
                "origin": { "runtime": "runtime:cell" },
                "params": { "only": ["ethercat"], "confirm_fieldbus_scan": true }
            }),
        },
        "the confirmed search posts the flag with the ticked sources"
    );

    let without = idle_view(&scan, &cell(), &remembered(&["ads"]));
    assert_eq!(
        without.actions[0].confirm, None,
        "a search without EtherCAT asks nothing"
    );
    assert_eq!(
        without.actions[0].op,
        Op::Search {
            request: json!({
                "verb": "start",
                "origin": { "runtime": "runtime:cell" },
                "params": { "only": ["ads"] }
            }),
        },
        "and it carries no fieldbus flag"
    );
}

#[test]
fn ethercat_needs_the_engineer_role_of_its_runtime() {
    // Decision O12 and specification 25 §2: a confirmed fieldbus scan needs the Engineer role. From a runtime
    // whose connection is a Viewer, EtherCAT is disabled with the role reason, and it cannot be ticked. A
    // wrong implementation lets the Viewer tick it, or words the reason another way.
    let scan = Scan::Runtime("runtime:cell".to_owned());
    let ctx = Context {
        origins: cell_facts(Some(AccessRole::Viewer)),
        ..remembered(&["ethercat"])
    };
    let view = idle_view(&scan, &cell(), &ctx);
    let ethercat = view
        .only
        .iter()
        .find(|choice| choice.source == "ethercat")
        .expect("the EtherCAT choice");
    assert_eq!(
        (
            ethercat.enabled,
            ethercat.checked,
            ethercat.action.is_none()
        ),
        (false, false, true),
        "disabled, unticked, and without a tick that could record it"
    );
    assert_eq!(
        ethercat.reason.as_deref(),
        Some("Scanning EtherCAT needs the Engineer role; this connection has the Viewer role."),
    );
}

#[test]
fn the_protocol_selection_is_remembered_per_project_and_ads_is_ticked_first() {
    // Specification 25 §2 (old lines 136-155 and 512-523): on the first run ADS is ticked; once the project
    // remembers a selection, that selection is ticked instead, and each project keeps its own
    // `<project>/.trust-runtime/devices.toml` under `[discover]`. A wrong implementation ticks nothing on the
    // first run, keeps one selection for every project, or writes the file where the CLI does not read it.
    let first = idle_view(&Scan::ThisHost, &[], &Context::default());
    let ticked: Vec<&str> = first
        .only
        .iter()
        .filter(|choice| choice.checked)
        .map(|choice| choice.source.as_str())
        .collect();
    assert_eq!(ticked, ["ads"], "the first run ticks ADS and nothing else");

    let saved = idle_view(&Scan::ThisHost, &[], &remembered(&["modbus_tcp"]));
    let ticked: Vec<&str> = saved
        .only
        .iter()
        .filter(|choice| choice.checked)
        .map(|choice| choice.source.as_str())
        .collect();
    assert_eq!(
        ticked,
        ["modbus_tcp"],
        "a remembered selection replaces ADS"
    );

    let project_a = TempFolder::new("devices-remember-a");
    let project_b = TempFolder::new("devices-remember-b");
    devices_file::save(
        &project_a.path,
        &Memory {
            protocols: Some(vec!["mqtt".to_owned()]),
            ..Memory::default()
        },
    )
    .expect("the project's file is saved");
    assert_eq!(
        devices_file::load(&project_a.path)
            .expect("the file reads back")
            .protocols,
        Some(vec!["mqtt".to_owned()]),
        "the project remembers its selection"
    );
    assert_eq!(
        devices_file::load(&project_b.path)
            .expect("an empty project reads")
            .protocols,
        None,
        "another project remembers nothing, so ADS is ticked there"
    );
    let text = fs::read_to_string(devices_file::path(&project_a.path)).expect("the file is there");
    assert!(
        text.contains("[discover]") && text.contains("protocols = [\"mqtt\"]"),
        "the file holds the table and the list the CLI reads: {text}"
    );
}

#[test]
fn an_advertised_protocol_beyond_the_ten_is_shown_by_its_name() {
    // Specification 25 §2 (old line 94): a protocol that the origin runtime's `comm.schema` lists with
    // `discover`, beyond the ten sources, is a choice under `Known address or subnet`, named by its advertised
    // label and noted `discoverable`. This version cannot scan it, so the choice is disabled with its reason.
    // A protocol without `discover` is not offered. A wrong implementation drops it, names it by its ID, or
    // offers a tick that the search would refuse.
    let schema = json!({ "protocols": [
        { "id": "ads", "label": "ADS", "actions": ["add", "discover"] },
        { "id": "profinet", "label": "PROFINET", "actions": ["add", "discover"] },
        { "id": "bacnet", "label": "BACnet", "actions": ["add"] }
    ]});
    let ctx = Context {
        origins: BTreeMap::from([(
            "runtime:cell".to_owned(),
            OriginFacts {
                role: None,
                ethercat_adapter: None,
                discover: Some(discover_context::discover_from_schema(&schema)),
            },
        )]),
        ..Context::default()
    };
    let view = idle_view(&Scan::Runtime("runtime:cell".to_owned()), &cell(), &ctx);
    assert_eq!(
        view.only.len(),
        11,
        "the ten sources and the advertised protocol"
    );
    let profinet = view
        .only
        .iter()
        .find(|choice| choice.source == "profinet")
        .expect("PROFINET is a choice");
    assert_eq!(
        (
            profinet.label.as_str(),
            profinet.group.as_str(),
            profinet.note.as_deref(),
            profinet.enabled,
            profinet.checked,
        ),
        (
            "PROFINET",
            "known_address",
            Some("discoverable"),
            false,
            false
        )
    );
    assert_eq!(
        profinet.reason.as_deref(),
        Some("This version of truST cannot scan it yet. Update truST to scan it.")
    );
    assert!(
        profinet.action.is_none(),
        "a disabled choice records nothing"
    );
    assert!(
        view.only.iter().all(|choice| choice.source != "bacnet"),
        "a protocol that the schema lists without discover is not offered"
    );
}
