//! Native tests of the Discover pane's cards (specification 25 §2): the `TwinCAT` cards and their ports, the
//! finds that no port answered, the `Ignore` of a find and `Show ignored`, and the reasons that disable a
//! card's actions. Each expected value is a literal worked out from specification 25 §2, from the old pane's
//! words, or from decision O14; none is read back from the code under test.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use crate::view::discover::{project_in, RuntimeOrigin, Scan};
use crate::view::discover_context::{Context, Discover, OriginFacts};
use crate::view::types::{DiscoverView, Op, Pane, Pill, Tone, UiOp};
use trust_runtime::search::configured::ConfiguredDevice;
use trust_runtime::search::project_choices::Memory;

/// The time of the views, in milliseconds since the Unix epoch.
const NOW_MS: u64 = 1_760_000_000_000;

/// A search status of `sources` and `devices`, as `comm.search.status` answers it, for the search `s-1`.
fn status(sources: Value, devices: Value) -> Map<String, Value> {
    let mut body = Map::new();
    body.insert("search_id".to_owned(), json!("s-1"));
    body.insert("state".to_owned(), json!("done"));
    body.insert("sources".to_owned(), sources);
    body.insert("devices".to_owned(), devices);
    body
}

/// A device with one ADS find: its AMS Net ID, its name and the ports that answered (`responding_ads_ports`).
fn ads_device(id: &str, name: &str, net_id: &str, ports: Value) -> Value {
    let mut params = json!({
        "ams_net_id": net_id,
        "host": "10.0.0.5",
        "name": name,
    });
    params["responding_ads_ports"] = ports;
    json!({
        "id": id,
        "addresses": ["10.0.0.5"],
        "ams_net_ids": [net_id],
        "last_seen_ms": NOW_MS - 3000,
        "finds": [{
            "protocol": "ads",
            "source": "ads_identify",
            "confidence": "confirmed",
            "simulated": false,
            "identity": { "kind": "ads", "ams_net_id": net_id, "name": name },
            "params": params,
            "warnings": []
        }]
    })
}

/// The runtime `runtime:cell`, ready to scan: the origin of the tests that scan from a runtime.
fn cell() -> Vec<RuntimeOrigin> {
    vec![RuntimeOrigin {
        id: "runtime:cell".to_owned(),
        name: "Cell".to_owned(),
        ready: true,
    }]
}

/// The Discover view of `answer`, read with `ctx` and projected for `scan`.
fn view_with(
    answer: &Map<String, Value>,
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    ctx: &Context,
) -> DiscoverView {
    project_in(answer, scan, runtimes, ctx, NOW_MS).expect("the status reads as a search status")
}

/// The ids of the cards of `view`, in order.
fn card_ids(view: &DiscoverView) -> Vec<&str> {
    view.cards.iter().map(|card| card.id.as_str()).collect()
}

/// A status with two `TwinCAT` devices: `PLC-A` answered on the ports 852 and 851, `PLC-B` on none.
fn two_ads_devices() -> Map<String, Value> {
    status(
        json!([{ "id": "ads", "state": "found", "count": 2 }]),
        json!([
            ads_device(
                "dev:ads:5.6.7.8.1.1",
                "PLC-A",
                "5.6.7.8.1.1",
                json!([852, 851])
            ),
            ads_device("dev:ads:9.9.9.9.1.1", "PLC-B", "9.9.9.9.1.1", json!([])),
        ]),
    )
}

#[test]
fn an_ads_card_lists_its_responding_ports_and_hides_without_one() {
    // Specification 25 §2: a TwinCAT card counts its responding ports (old line 345), shows each as a
    // button in port order (old lines 358-373), and offers `Add to canvas` (old line 407) with the
    // connection it adds (`connection.ads_add`). An ADS find that no port answered is left out (old lines
    // 131-134), and then the ADS source row carries the verification text (old lines 237-241) with the
    // default ports (`adsDiscoveryPorts.ts`). A wrong implementation keeps the silent find as a card, lists
    // the ports unsorted, or offers the form `+ Add` instead of the connection.
    let answer = two_ads_devices();
    let view = view_with(&answer, &Scan::ThisHost, &cell(), &Context::default());
    assert_eq!(
        card_ids(&view),
        ["dev:ads:5.6.7.8.1.1"],
        "the find that no port answered is left out"
    );
    let card = &view.cards[0];
    assert_eq!(
        card.responding_ports.as_deref(),
        Some("2 responding ADS ports"),
        "the count of the responding ports (old line 345)"
    );
    let ports: Vec<(&str, &str)> = card
        .ports
        .iter()
        .map(|port| (port.id.as_str(), port.label.as_str()))
        .collect();
    assert_eq!(
        ports,
        [("ads_port_851", "851"), ("ads_port_852", "852")],
        "one button per responding port, in port order"
    );
    let add = &card.actions[0];
    assert_eq!(
        (add.id.as_str(), add.label.as_str(), add.enabled),
        ("add_to_canvas", "Add to canvas", true),
        "the card's action on a device the canvas does not hold (old line 407)"
    );
    let Op::Followed { first, .. } = &add.op else {
        panic!("the add is followed by the tag browser: {:?}", add.op);
    };
    assert_eq!(
        **first,
        Op::Rust {
            action: "connection.ads_add".to_owned(),
            payload: json!({
                "runtime": "runtime:cell",
                "label": "PLC-A",
                "target": {
                    "ams_net_id": "5.6.7.8.1.1",
                    "host": "10.0.0.5",
                    "name": "PLC-A",
                    "responding_ads_ports": [852, 851]
                }
            }),
        },
        "one ADS connection per responding port is added to the owning runtime"
    );
    assert_eq!(
        card.project, None,
        "the panel shows no runtime, so no card carries the project's mark"
    );

    let silent = status(
        json!([{ "id": "ads", "state": "found", "count": 1 }]),
        json!([ads_device(
            "dev:ads:9.9.9.9.1.1",
            "PLC-B",
            "9.9.9.9.1.1",
            json!([])
        )]),
    );
    let view = view_with(&silent, &Scan::ThisHost, &cell(), &Context::default());
    assert!(view.cards.is_empty(), "no card for a find without a port");
    let ads_row = view
        .sources
        .iter()
        .find(|row| row.id == "ads")
        .expect("the ADS source row");
    assert_eq!(
        ads_row.reason.as_deref(),
        Some("TwinCAT identity was found, but none of the configured ADS ports (301, 501, 851, 852, 853, 854) responded."),
        "the verification text names the default ports (old lines 237-241)"
    );
    assert_eq!(
        (ads_row.word.as_str(), ads_row.count),
        ("0 found", Some(0)),
        "the row counts the devices that answered, as the old pane did (`verified.length`)"
    );
    let configured = Context {
        ads_ports: vec![852, 851],
        ..Context::default()
    };
    let view = view_with(&silent, &Scan::ThisHost, &cell(), &configured);
    let ads_row = view
        .sources
        .iter()
        .find(|row| row.id == "ads")
        .expect("the ADS source row");
    assert_eq!(
        ads_row.reason.as_deref(),
        Some("TwinCAT identity was found, but none of the configured ADS ports (851, 852) responded."),
        "the configured ports are named, sorted, as the project set them"
    );
}

#[test]
fn an_ads_card_of_a_device_on_the_canvas_marks_it_and_offers_manage_tags() {
    // Specification 25 §2 (old lines 355-357 and 406): a device whose AMS Net ID a connection on the canvas
    // holds is marked `On canvas` and offers `Manage tags`, which browses the device's own tags. A wrong
    // implementation marks the device by its address, or keeps `Add to canvas` for it.
    let answer = two_ads_devices();
    let on_canvas = ConfiguredDevice::from_connection(
        "ads",
        &json!({ "connections": [{
            "name": "line1", "target_net_id": "5.6.7.8.1.1", "host": "10.0.0.5", "ams_port": 851
        }] }),
    )
    .expect("the connection names a device");
    let context = Context {
        project: Some(on_canvas),
        ..Context::default()
    };
    let view = view_with(&answer, &Scan::ThisHost, &cell(), &context);
    let card = &view.cards[0];
    assert_eq!(
        card.project,
        Some(Pill {
            word: "On canvas".to_owned(),
            tone: Tone::Ok,
            live: false
        })
    );
    let manage = &card.actions[0];
    assert_eq!(
        (manage.id.as_str(), manage.label.as_str(), manage.enabled),
        ("manage_tags", "Manage tags", true)
    );
    assert_eq!(
        manage.op,
        Op::Ui {
            ui: UiOp::Open {
                pane: Pane::Browse {
                    request: json!({
                        "source": {
                            "found": "dev:ads:5.6.7.8.1.1",
                            "protocol": "ads",
                            "target": {
                                "ams_net_id": "5.6.7.8.1.1",
                                "host": "10.0.0.5",
                                "name": "PLC-A",
                                "responding_ads_ports": [852, 851]
                            }
                        },
                        "kind": "symbols",
                        "trust_server_certificate": false
                    }),
                },
            },
        }
    );
}

#[test]
fn ignore_hides_a_find_until_stop_ignoring() {
    // Decision O14 and specification 25 §2: `Ignore` hides a find from the found list until it is undone;
    // the project remembers the device's ID. A wrong implementation keeps the ignored find in the list,
    // forgets the ID, or offers no way to undo it. The card of the other find offers `Ignore`, whose change
    // names the device and the search it is in.
    let answer = two_ads_devices();
    let context = Context {
        memory: Memory {
            protocols: None,
            ignored: BTreeSet::from(["dev:ads:5.6.7.8.1.1".to_owned()]),
        },
        ..Context::default()
    };
    let view = view_with(&answer, &Scan::ThisHost, &cell(), &context);
    assert!(
        view.cards.is_empty(),
        "the ignored find leaves the found list"
    );

    let answer = status(
        json!([{ "id": "ads", "state": "found", "count": 2 }]),
        json!([
            ads_device("dev:ads:5.6.7.8.1.1", "PLC-A", "5.6.7.8.1.1", json!([851])),
            ads_device("dev:ads:7.7.7.7.1.1", "PLC-C", "7.7.7.7.1.1", json!([851])),
        ]),
    );
    let view = view_with(&answer, &Scan::ThisHost, &cell(), &context);
    assert_eq!(card_ids(&view), ["dev:ads:7.7.7.7.1.1"]);
    let ignore = view.cards[0].actions.last().expect("the card's Ignore");
    assert_eq!(
        (ignore.id.as_str(), ignore.label.as_str(), ignore.enabled),
        ("ignore", "Ignore", true)
    );
    assert_eq!(
        ignore.op,
        Op::Search {
            request: json!({
                "verb": "status",
                "origin": "this_host",
                "search_id": "s-1",
                "change": { "kind": "ignore", "device": "dev:ads:7.7.7.7.1.1", "ignored": true }
            }),
        },
        "Ignore records the device and answers the search's view"
    );
}

#[test]
fn show_ignored_offers_stop_ignoring() {
    // Decision O14 and specification 25 §2: `Show ignored` shows the finds that the project ignores, and
    // each of them offers `Stop ignoring`; the toggle is ticked while it shows them and its change flips it.
    // A wrong implementation hides them with no way back, or flips the toggle without recording the change.
    let answer = two_ads_devices();
    let context = Context {
        memory: Memory {
            protocols: None,
            ignored: BTreeSet::from(["dev:ads:5.6.7.8.1.1".to_owned()]),
        },
        show_ignored: true,
        ..Context::default()
    };
    let view = view_with(&answer, &Scan::ThisHost, &cell(), &context);
    assert_eq!(
        card_ids(&view),
        ["dev:ads:5.6.7.8.1.1"],
        "the ignored find is shown, and the one that no port answered stays out"
    );
    let stop = view.cards[0]
        .actions
        .last()
        .expect("the card's Stop ignoring");
    assert_eq!(
        (stop.id.as_str(), stop.label.as_str()),
        ("stop_ignoring", "Stop ignoring")
    );
    assert_eq!(
        stop.op,
        Op::Search {
            request: json!({
                "verb": "status",
                "origin": "this_host",
                "search_id": "s-1",
                "change": { "kind": "ignore", "device": "dev:ads:5.6.7.8.1.1", "ignored": false }
            }),
        }
    );
    assert!(
        view.show_ignored.checked,
        "the toggle is ticked while it shows them"
    );
    assert_eq!(view.show_ignored.label, "Show ignored");
    assert_eq!(
        view.show_ignored.action.op,
        Op::Search {
            request: json!({
                "verb": "status",
                "origin": "this_host",
                "search_id": "s-1",
                "change": { "kind": "show_ignored", "shown": false }
            }),
        },
        "the toggle's change turns it off"
    );
}

#[test]
fn a_card_names_why_its_panel_session_cannot_add_it() {
    // Specification 25 §2 (old lines 334-340 and 385-389, `discoverySession.ts`): a find of a search that this
    // panel session did not start says that the context changed; a find whose origin runtime does not advertise
    // its protocol says that the discovery action is gone. The text is shown as the card's reason, and the
    // primary action carries it. A wrong implementation enables the action anyway, or uses another text.
    let answer = two_ads_devices();
    let other_session = Context {
        session_current: false,
        ..Context::default()
    };
    let view = view_with(&answer, &Scan::ThisHost, &cell(), &other_session);
    let card = &view.cards[0];
    assert_eq!(
        card.stale_reason.as_deref(),
        Some("Discovery context changed. Scan again before adding this result.")
    );
    assert!(
        !card.actions[0].enabled,
        "the disabled action names the reason"
    );
    assert_eq!(
        card.actions[0].reason.as_deref(),
        Some("Discovery context changed. Scan again before adding this result.")
    );
    assert!(
        card.actions[card.actions.len() - 1].enabled,
        "Ignore is never disabled by the session"
    );

    let mut origins = BTreeMap::new();
    origins.insert(
        "runtime:cell".to_owned(),
        OriginFacts {
            role: None,
            ethercat_adapter: None,
            discover: Some(Discover {
                ids: BTreeSet::from(["modbus_tcp".to_owned()]),
                beyond: Vec::new(),
            }),
        },
    );
    let not_advertised = Context {
        origins,
        ..Context::default()
    };
    let view = view_with(
        &answer,
        &Scan::Runtime("runtime:cell".to_owned()),
        &cell(),
        &not_advertised,
    );
    assert_eq!(
        view.cards[0].stale_reason.as_deref(),
        Some("This discovery action is no longer available. Refresh the runtime and scan again.")
    );
}
