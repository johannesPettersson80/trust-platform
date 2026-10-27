//! Native tests of `Compare with project` in the Discover pane (specification 25 §2; specification 11,
//! "Compare with the project"): the action, the reason it gives while it cannot run, and the four lists
//! (configured devices not found, found devices the project does not use, devices that differ, configured
//! devices not compared). Each expected value is a literal worked out from the specification.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};
use trust_runtime::search::project_choices::Memory;

use super::discover_compare::{
    ads_device, cell, ethercat_device, modbus_device, runtime_with, status, usb_device, view_with,
};
use crate::view::discover::{self, Scan};
use crate::view::discover_compare::project_of;
use crate::view::discover_context::Context;
use crate::view::types::{Action, CompareList, DiscoverView, Op};

/// The rows of a list as (label, value) pairs.
fn rows(list: &CompareList) -> Vec<(&str, &str)> {
    list.rows
        .iter()
        .map(|row| (row.label.as_str(), row.value.as_str()))
        .collect()
}

/// The action `id` of the pane's own actions.
fn action<'a>(view: &'a DiscoverView, id: &str) -> &'a Action {
    view.actions
        .iter()
        .find(|action| action.id == id)
        .unwrap_or_else(|| panic!("the pane offers the action {id}"))
}

/// A finished search of the sources the panel compares, in the state `state`.
fn search_in(state: &str, devices: Value) -> Map<String, Value> {
    let mut answer = status(
        json!([
            { "id": "ads", "state": "found", "count": 2 },
            { "id": "modbus_tcp", "state": "found", "count": 3 },
            { "id": "ethercat", "state": "found", "count": 1 },
            { "id": "opcua", "state": "found", "count": 0 },
            { "id": "usb", "state": "found", "count": 1 }
        ]),
        devices,
    );
    answer.insert("state".to_owned(), json!(state));
    answer
}

/// The devices the tests find: two `TwinCAT` systems, three Modbus devices, an `EtherCAT` EL2008 at
/// position 3 and a USB probe.
fn found_devices() -> Value {
    json!([
        ads_device("5.6.7.8.1.1", "PLC-A"),
        ads_device("9.9.9.9.1.1", "PLC-B"),
        modbus_device("10.0.0.5", Some("DRV-2")),
        modbus_device("10.0.0.6", None),
        modbus_device("10.0.0.7", None),
        ethercat_device(3, "EL2008"),
        usb_device()
    ])
}

/// The context of a panel whose canvas holds a `TwinCAT` system, a Modbus drive that states product code DRV-1, a
/// Modbus meter, a Modbus device at 10.0.0.9 that nobody finds, an `EtherCAT` EL2008 at position 2 and an OPC UA
/// server named by a host name, with the lists asked for when `compare` is true.
fn context(compare: bool) -> Context {
    let known = [runtime_with(vec![
        (
            "ads",
            json!({ "connections": [{
                "name": "line1", "target_net_id": "5.6.7.8.1.1", "host": "10.0.0.5", "ams_port": 851
            }] }),
        ),
        (
            "modbus_tcp",
            json!({ "address": "10.0.0.5:502", "unit_id": 1, "product_code": "DRV-1" }),
        ),
        ("modbus_tcp", json!({ "address": "10.0.0.6:502" })),
        ("modbus_tcp", json!({ "address": "10.0.0.9:502" })),
        (
            "ethercat",
            json!({ "adapter": "eth1", "modules": [{ "model": "EL2008", "slot": 2 }] }),
        ),
        (
            "opcua_client",
            json!({ "connections": [{
                "endpoint_url": "opc.tcp://plc01:4840", "security_policy": "none", "security_mode": "none"
            }] }),
        ),
    ])];
    Context {
        project: project_of(&known),
        compare,
        ..Context::default()
    }
}

#[test]
fn compare_with_project_lists_the_missing_the_unused_and_the_differing() {
    // Specification 25 §2 and specification 11, "Compare with the project": the four lists of one comparison.
    // Missing is the Modbus device at 10.0.0.9, unit 1, that nobody found. Unused are the Modbus device at
    // 10.0.0.7 and the TwinCAT system PLC-B, by device ID (`dev:ip:...` before `dev:netid:...`), and the
    // system says where it stands because its title is a name. Two devices differ, by device ID: the EL2008
    // stands at position 3 where the project has 2, and the drive at 10.0.0.5 reports DRV-2 where the project
    // has DRV-1. The OPC UA server named `plc01` is not compared, because a search reports addresses and
    // never looks a name up. The USB probe is no list's business. A wrong implementation lists the meter that
    // is matched, lists the host-named server as missing, drops a list, words a row differently from the
    // card's difference row, or lists the USB probe as unused.
    let view = view_with(&search_in("done", found_devices()), &context(true));

    let compare = view
        .compare
        .as_ref()
        .expect("a finished search with a project and the lists asked for carries the lists");
    assert_eq!(compare.heading, "Compared with the project");
    assert_eq!(compare.missing.heading, "Configured devices not found");
    assert_eq!(
        rows(&compare.missing),
        vec![("Modbus TCP", "10.0.0.9:502, unit 1")]
    );
    assert_eq!(
        compare.unused.heading,
        "Found devices the project does not use"
    );
    assert_eq!(
        rows(&compare.unused),
        vec![
            ("10.0.0.7", "Modbus TCP"),
            ("PLC-B", "ADS and TwinCAT at 10.0.0.5"),
        ]
    );
    assert_eq!(
        compare.differs.heading,
        "Devices that differ from the project"
    );
    assert_eq!(
        rows(&compare.differs),
        vec![
            (
                "EL2008",
                "Position: The project has 2; the device reports 3."
            ),
            (
                "10.0.0.5",
                "Product code: The project has DRV-1; the device reports DRV-2."
            ),
        ]
    );
    assert_eq!(
        compare.not_compared.heading,
        "Configured devices not compared"
    );
    assert_eq!(
        rows(&compare.not_compared),
        vec![("OPC UA", "plc01:4840 (its host name is not looked up)")]
    );
    // Every list says what it holds when it is empty, in the words of specification 25 §2.
    assert_eq!(
        compare.missing.empty_text,
        "Every configured device was found."
    );
    assert_eq!(
        compare.unused.empty_text,
        "Every found device is in the project."
    );
    assert_eq!(
        compare.differs.empty_text,
        "No device differs from the project."
    );
    assert_eq!(
        compare.not_compared.empty_text,
        "Every configured device was compared."
    );
}

#[test]
fn the_lists_say_what_they_hold_when_nothing_is_left_over() {
    // Specification 25 §2: a comparison that finds nothing to report still answers, list by list, so that
    // "nothing is missing" reads as a result and not as a pane that did not run. The canvas holds one Modbus
    // meter and the search finds exactly it. A wrong implementation leaves the lists out when they are empty.
    let known = [runtime_with(vec![(
        "modbus_tcp",
        json!({ "address": "10.0.0.6:502" }),
    )])];
    let answer = status(
        json!([{ "id": "modbus_tcp", "state": "found", "count": 1 }]),
        json!([modbus_device("10.0.0.6", None)]),
    );
    let context = Context {
        project: project_of(&known),
        compare: true,
        ..Context::default()
    };

    let view = view_with(&answer, &context);

    let compare = view.compare.expect("the lists are asked for");
    for list in [
        &compare.missing,
        &compare.unused,
        &compare.differs,
        &compare.not_compared,
    ] {
        assert!(list.rows.is_empty(), "{}: nothing to report", list.heading);
    }
}

#[test]
fn a_device_not_found_by_a_search_that_skipped_its_source_is_not_compared() {
    // Specification 11, "Not compared": a search that did not run EtherCAT cannot call the project's module
    // missing, so the module is listed as not compared, with the reason in words. A wrong implementation lists
    // it as missing.
    let known = [runtime_with(vec![(
        "ethercat",
        json!({ "adapter": "eth1", "modules": [{ "model": "EL2008", "slot": 2 }] }),
    )])];
    let answer = status(
        json!([
            { "id": "modbus_tcp", "state": "found", "count": 0 },
            { "id": "ethercat", "state": "skipped", "reason": "not here" }
        ]),
        json!([]),
    );
    let context = Context {
        project: project_of(&known),
        compare: true,
        ..Context::default()
    };

    let compare = view_with(&answer, &context)
        .compare
        .expect("the lists are asked for");

    assert_eq!(rows(&compare.missing), Vec::<(&str, &str)>::new());
    assert_eq!(
        rows(&compare.not_compared),
        vec![(
            "EtherCAT",
            "EL2008 at position 2 on eth1 (its source did not run in this search)"
        )]
    );
}

#[test]
fn an_ignored_device_is_not_listed_as_unused_but_its_difference_is() {
    // Decision O14 and specification 11, "Ignored devices": the project ignores the Modbus device at 10.0.0.7,
    // which is in no connection, and the drive at 10.0.0.5, whose product code differs. Neither card shows
    // (`Show ignored` is off), yet the unused list leaves the first out, and the differs list keeps the second
    // under the drive's title. A wrong implementation lists the ignored device as unused, or loses the title
    // of a device whose card the pane hides.
    let known = [runtime_with(vec![(
        "modbus_tcp",
        json!({ "address": "10.0.0.5:502", "product_code": "DRV-1" }),
    )])];
    let answer = status(
        json!([{ "id": "modbus_tcp", "state": "found", "count": 2 }]),
        json!([
            modbus_device("10.0.0.5", Some("DRV-2")),
            modbus_device("10.0.0.7", None)
        ]),
    );
    let context = Context {
        project: project_of(&known),
        compare: true,
        memory: Memory {
            protocols: None,
            ignored: BTreeSet::from(["dev:ip:10.0.0.5".to_owned(), "dev:ip:10.0.0.7".to_owned()]),
        },
        ..Context::default()
    };

    let view = view_with(&answer, &context);

    assert!(view.cards.is_empty(), "both ignored cards are hidden");
    let compare = view.compare.expect("the lists are asked for");
    assert_eq!(rows(&compare.unused), Vec::<(&str, &str)>::new());
    assert_eq!(
        rows(&compare.differs),
        vec![(
            "10.0.0.5",
            "Product code: The project has DRV-1; the device reports DRV-2."
        )]
    );
}

#[test]
fn compare_with_project_runs_only_on_a_finished_search_and_says_why_not() {
    // Specification 25 §2 and specification 11, "Compare with the project": the action is disabled, with its
    // reason, while the panel shows no runtime ("no project"), before any search, while the search runs, and
    // after a cancelled search, which has not looked everywhere. It is enabled on a finished search, and
    // then shows the lists. The reasons are checked in that order. A wrong implementation lets a running
    // search be compared, or compares a cancelled search and calls what it did not look for missing.
    let ready = context(false);
    let no_project = Context {
        project: None,
        ..context(false)
    };
    let done = search_in("done", found_devices());
    let running = search_in("running", found_devices());
    let cancelled = search_in("cancelled", found_devices());

    let states = [
        (
            "no project",
            view_with(&done, &no_project),
            Some("The panel shows no runtime, so there is no project to compare with."),
        ),
        (
            "no search",
            discover::idle(&Scan::ThisHost, &cell(), &ready),
            Some("Search first, then compare the result with the project."),
        ),
        (
            "running",
            view_with(&running, &ready),
            Some("Wait for the search to end, then compare it with the project."),
        ),
        (
            "cancelled",
            view_with(&cancelled, &ready),
            Some(
                "A cancelled search has not looked everywhere. Search again to compare it with the project.",
            ),
        ),
        ("finished", view_with(&done, &ready), None),
    ];

    for (name, view, reason) in states {
        let compare = action(&view, "compare");
        assert_eq!(compare.label, "Compare with project", "{name}");
        assert_eq!(compare.enabled, reason.is_none(), "{name}");
        assert_eq!(compare.reason.as_deref(), reason, "{name}");
        assert!(
            view.compare.is_none(),
            "{name}: the lists were not asked for"
        );
    }
}

#[test]
fn the_lists_never_show_for_a_search_that_cannot_be_compared() {
    // Specification 25 §2: the lists are the comparison of a finished search. When the user has asked for them
    // and then starts a search, or the search is cancelled, or the panel loses its runtimes, the pane shows
    // none and the action reads `Compare with project` again, disabled with its reason. A wrong implementation
    // keeps the lists of the earlier search on a search that still runs.
    let asked = context(true);
    let no_project = Context {
        project: None,
        ..context(true)
    };

    for (name, view) in [
        (
            "running",
            view_with(&search_in("running", found_devices()), &asked),
        ),
        (
            "cancelled",
            view_with(&search_in("cancelled", found_devices()), &asked),
        ),
        (
            "no project",
            view_with(&search_in("done", found_devices()), &no_project),
        ),
        (
            "no search",
            discover::idle(&Scan::ThisHost, &cell(), &asked),
        ),
    ] {
        assert!(view.compare.is_none(), "{name}: no lists");
        let compare = action(&view, "compare");
        assert_eq!(compare.label, "Compare with project", "{name}");
        assert!(!compare.enabled, "{name}");
    }
}

#[test]
fn the_compare_action_toggles_the_lists_through_a_change_of_the_search() {
    // Specification 25 §2: `Compare with project` is a status request of the search the pane shows, with the
    // change `{"kind": "compare", "shown": true}`, like `Show ignored`; while the lists show it reads `Hide
    // comparison` and asks for `shown: false`. It sits after Search and before `Configure ADS ports to scan`.
    // A wrong implementation posts a start (which would search again), forgets the search ID, or never offers
    // the way back.
    let hidden = view_with(&search_in("done", found_devices()), &context(false));
    let showing = view_with(&search_in("done", found_devices()), &context(true));

    assert_eq!(
        hidden
            .actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>(),
        ["search", "compare", "configure_ads_ports"]
    );
    let compare = action(&hidden, "compare");
    assert_eq!(compare.label, "Compare with project");
    assert_eq!(
        compare.op,
        Op::Search {
            request: json!({
                "verb": "status",
                "origin": "this_host",
                "search_id": "s-1",
                "change": { "kind": "compare", "shown": true }
            }),
        }
    );
    let back = action(&showing, "compare");
    assert_eq!(back.label, "Hide comparison");
    assert!(back.enabled);
    assert_eq!(
        back.op,
        Op::Search {
            request: json!({
                "verb": "status",
                "origin": "this_host",
                "search_id": "s-1",
                "change": { "kind": "compare", "shown": false }
            }),
        }
    );
    // While a search runs, Cancel sits between Search and Compare.
    let running = view_with(&search_in("running", found_devices()), &context(false));
    assert_eq!(
        running
            .actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>(),
        ["search", "cancel", "compare", "configure_ads_ports"]
    );
}

#[test]
fn the_lists_do_not_change_the_cards_marks() {
    // Specification 25 §2: the lists and the marks of the cards are two views of one comparison, so asking for
    // the lists changes no card. A wrong implementation compares twice and lets the two disagree, or hides a
    // card to show a list.
    let answer = search_in("done", found_devices());
    let without = view_with(&answer, &context(false));
    let with = view_with(&answer, &context(true));

    assert_eq!(without.cards, with.cards);
    assert!(without.compare.is_none());
    assert!(with.compare.is_some());
    // The view of a status that names no cards still serializes the lists once.
    let sent = serde_json::to_value(&with).expect("the view serializes");
    assert!(sent.get("compare").is_some_and(Value::is_object));
}
