//! The Discover pane's view (specification 25 §2): one search status turned into the cards, source
//! rows, scan choices and empty-result text that the pane draws. Each expected value is a literal
//! worked out from the search status written in the test or from the line of specification 25 that
//! the test names; none is read back from the code under test.

use serde_json::{json, Value};

use crate::view::discover::{project, RuntimeOrigin, Scan};
use crate::view::types::{DiscoverView, Op, SourceRow, Tone};

/// The time of the view, in milliseconds since the Unix epoch.
const NOW_MS: u64 = 1_760_000_000_000;

/// The discovery sources that a result can name, in the order of the search's own table.
const FIND_SOURCES: &[&str] = &[
    "mdns",
    "mesh",
    "tcp_connect",
    "ads_broadcast",
    "ads_identify",
    "ads_port_browse_results",
    "ads_port_unavailable",
    "lds",
    "opcua_get_endpoints",
    "tls_handshake",
    "mqtt_connack",
    "mqtt_connack_rejected",
    "mqtt_connack_auth_rejected",
    "modbus_device_id",
    "modbus_device_id_exception",
    "modbus_safe_read",
    "modbus_safe_read_exception",
    "usb_descriptor",
    "usb_devices",
    "usb_bootloader",
    "port_listing",
    "serial_ports",
    "gpio_sysfs",
    "ethercat_bus",
    "ethercat_mock",
];

/// The search sources, in the order of the search's own table.
const SOURCES: &[&str] = &[
    "trust_runtime",
    "ads",
    "modbus_tcp",
    "mqtt",
    "opcua",
    "ethercat",
    "gpio",
    "serial",
    "usb",
    "bluetooth",
];

/// The reason of the Bluetooth scan in this version of truST.
const BLUETOOTH: &str = "Bluetooth search is not available in this truST version yet.";

/// A runtime the panel offers as an origin, ready to scan or stopped.
fn origin(id: &str, name: &str, ready: bool) -> RuntimeOrigin {
    RuntimeOrigin {
        id: id.to_owned(),
        name: name.to_owned(),
        ready,
    }
}

/// A search status of `sources` and `devices`, as `comm.search.status` answers it.
fn status(state: &str, sources: Value, devices: Value) -> Value {
    let mut body = json!({
        "schema_version": 1,
        "search_id": "s-1",
        "origin": "this_host",
        "state": state,
        "deadline_ms": 10000,
        "elapsed_ms": 1200,
        "cursor": 3,
        "warnings": []
    });
    if let Value::Object(members) = &mut body {
        members.insert("sources".to_owned(), sources);
        members.insert("devices".to_owned(), devices);
    }
    body
}

/// A source that ran and found `count` results.
fn found(id: &str, count: u64) -> Value {
    json!({ "id": id, "state": "found", "count": count })
}

/// A source that did not run, for `reason`.
fn skipped(id: &str, reason: &str) -> Value {
    json!({ "id": id, "state": "skipped", "reason": reason })
}

/// A device with the results `finds`, seen three seconds before the view.
fn device(id: &str, addresses: &[&str], finds: &[Value]) -> Value {
    json!({
        "id": id,
        "addresses": addresses,
        "last_seen_ms": NOW_MS - 3000,
        "finds": finds,
    })
}

/// A Modbus TCP result on `host`, found by a port that answers.
fn modbus_find(host: &str) -> Value {
    json!({
        "protocol": "modbus_tcp",
        "source": "tcp_connect",
        "confidence": "port_reachable",
        "simulated": false,
        "identity": { "kind": "modbus", "unit_id": 1 },
        "params": { "host": host, "port": 502, "unit_id": 1 },
        "warnings": []
    })
}

/// An ADS result of the AMS Net ID `net_id`, confirmed by its identify answer.
fn ads_find(net_id: &str) -> Value {
    json!({
        "protocol": "ads",
        "source": "ads_identify",
        "confidence": "confirmed",
        "simulated": false,
        "identity": { "kind": "ads", "ams_net_id": net_id, "computer_name": "PLC-7" },
        "params": { "ams_net_id": net_id, "host": "192.168.1.20" },
        "warnings": []
    })
}

/// A truST runtime found by mDNS, whose control endpoint is `endpoint`.
fn runtime_find(endpoint: &str) -> Value {
    json!({
        "protocol": "trust_runtime",
        "source": "mdns",
        "confidence": "confirmed",
        "simulated": false,
        "identity": {
            "kind": "trust_runtime",
            "name": "Line runtime",
            "id": "rt-line",
            "control": endpoint,
            "version": "0.24.75",
            "platform": "linux-x86_64"
        },
        "params": {
            "control_endpoint": endpoint,
            "name": "Line runtime",
            "host": "10.0.0.6"
        },
        "warnings": []
    })
}

/// The Discover view of `answer`, projected for `scan` with the origins `runtimes`.
fn view_of(answer: &Value, scan: &Scan, runtimes: &[RuntimeOrigin]) -> DiscoverView {
    let body = answer
        .as_object()
        .expect("a search status is a JSON object");
    project(body, scan, runtimes, NOW_MS).expect("the status reads as a search status")
}

/// The origin `runtime:cell`, ready to scan.
fn ready_cell() -> Vec<RuntimeOrigin> {
    vec![origin("runtime:cell", "Cell", true)]
}

#[test]
fn one_card_per_device_with_every_protocol() {
    // Specification 25:64-65: "one card per device with every protocol found on it". A wrong
    // implementation makes one card per result, which gives three cards and drops the ADS
    // protocol of the first address, or it keeps only the first protocol of a device.
    let answer = status(
        "done",
        json!([
            found("modbus_tcp", 1),
            found("ads", 1),
            found("trust_runtime", 1)
        ]),
        json!([
            device(
                "dev:ip:192.168.1.20",
                &["192.168.1.20"],
                &[modbus_find("192.168.1.20"), ads_find("192.168.1.20.1.1")]
            ),
            device(
                "dev:ip:10.0.0.6",
                &["10.0.0.6"],
                &[runtime_find("tcp://10.0.0.6:5680")]
            ),
        ]),
    );
    let view = view_of(&answer, &Scan::ThisHost, &ready_cell());
    assert_eq!(view.cards.len(), 2, "two devices make two cards");
    let first = &view.cards[0];
    assert_eq!(first.id, "dev:ip:192.168.1.20");
    let names: Vec<&str> = first.protocols.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Modbus TCP", "ADS and TwinCAT"]);
    let confidence: Vec<&str> = first
        .protocols
        .iter()
        .map(|p| p.confidence.as_str())
        .collect();
    assert_eq!(confidence, ["Port reachable only", "Confirmed"]);
    assert_eq!(first.title, "PLC-7", "the name the ADS protocol reads wins");
    assert_eq!(
        first.last_seen, "3 s ago",
        "seen three seconds before the view"
    );
}

#[test]
fn card_actions_fit_the_identity() {
    // Specification 25:737-743 (every runtime is handled the same way) and 89-90 (adopting a
    // discovered runtime). A runtime card offers `Adopt` and no connection; a device card offers
    // `+ Add` with the connection it would create on the runtime that owns the search. A wrong
    // implementation offers a device's connection on a runtime card, or `Adopt` on a device.
    let answer = status(
        "done",
        json!([found("trust_runtime", 1), found("ads", 1)]),
        json!([
            device(
                "dev:ip:10.0.0.6",
                &["10.0.0.6"],
                &[runtime_find("tcp://10.0.0.6:5680")]
            ),
            device(
                "dev:netid:192.168.1.20.1.1",
                &["192.168.1.20"],
                &[ads_find("192.168.1.20.1.1")]
            ),
        ]),
    );
    let view = view_of(&answer, &Scan::ThisHost, &ready_cell());
    let runtime_card = &view.cards[0];
    assert_eq!(
        runtime_card.actions.len(),
        1,
        "a runtime card offers one action"
    );
    let adopt = &runtime_card.actions[0];
    assert_eq!(
        (adopt.id.as_str(), adopt.label.as_str()),
        ("adopt", "Adopt")
    );
    assert!(adopt.enabled, "a runtime with an endpoint can be adopted");
    assert_eq!(
        adopt.op,
        Op::Rust {
            action: "runtime.adopt".to_owned(),
            payload: json!({
                "runtime": "dev:ip:10.0.0.6",
                "endpoint": "tcp://10.0.0.6:5680",
                "label": "Line runtime"
            }),
        },
        "adopting keeps the discovered label and the endpoint as typed"
    );
    let device_card = &view.cards[1];
    assert_eq!(
        device_card.actions.len(),
        1,
        "a device card offers one action"
    );
    let add = &device_card.actions[0];
    assert_eq!((add.id.as_str(), add.label.as_str()), ("add", "+ Add"));
    assert!(add.enabled, "the owning runtime takes the connection");
    assert_eq!(
        add.op,
        Op::Rust {
            action: "form.open".to_owned(),
            payload: json!({
                "protocol": "ads",
                "runtime": "runtime:cell",
                "prefill": { "ams_net_id": "192.168.1.20.1.1", "host": "192.168.1.20" }
            }),
        },
        "the form opens the ADS connection on the owning runtime, prefilled from the find"
    );
}

#[test]
fn source_rows_use_the_reason_texts() {
    // Specification 25:55-58: each source shows `searching`, `{n} found`, `timed out` or `skipped`
    // with its reason. A wrong implementation drops the reason of a skipped source, shows a count
    // for a source that did not run, or shows `0 found` for a source that timed out.
    let answer = status(
        "done",
        json!([
            found("trust_runtime", 2),
            json!({ "id": "ads", "state": "timed_out", "count": 0 }),
            json!({ "id": "modbus_tcp", "state": "searching", "count": 0 }),
            skipped(
                "ethercat",
                "EtherCAT is searched inside a runtime, on the computer with the bus adapter."
            ),
            skipped(
                "bluetooth",
                "Bluetooth search is not available in this truST version yet."
            ),
        ]),
        json!([]),
    );
    let rows = view_of(&answer, &Scan::ThisHost, &[]).sources;
    assert_eq!(
        rows,
        [
            SourceRow {
                id: "trust_runtime".to_owned(),
                label: "truST runtimes".to_owned(),
                word: "2 found".to_owned(),
                count: Some(2),
                reason: None,
                tone: Tone::Ok,
            },
            SourceRow {
                id: "ads".to_owned(),
                label: "ADS and TwinCAT".to_owned(),
                word: "timed out".to_owned(),
                count: Some(0),
                reason: None,
                tone: Tone::Warning,
            },
            SourceRow {
                id: "modbus_tcp".to_owned(),
                label: "Modbus TCP".to_owned(),
                word: "searching".to_owned(),
                count: Some(0),
                reason: None,
                tone: Tone::Neutral,
            },
            SourceRow {
                id: "ethercat".to_owned(),
                label: "EtherCAT".to_owned(),
                word: "skipped".to_owned(),
                count: None,
                reason: Some(
                    "EtherCAT is searched inside a runtime, on the computer with the bus adapter."
                        .to_owned()
                ),
                tone: Tone::Muted,
            },
            SourceRow {
                id: "bluetooth".to_owned(),
                label: "Bluetooth LE".to_owned(),
                word: "skipped".to_owned(),
                count: None,
                reason: Some(
                    "Bluetooth search is not available in this truST version yet.".to_owned()
                ),
                tone: Tone::Muted,
            },
        ],
        "each row carries its word, and a skipped source its own reason"
    );
}

#[test]
fn hardware_searches_disabled_with_reason_until_origin_ready() {
    // Specification 25:82-88: hardware scans stay visible but disabled, with a reason, until the
    // selected runtime is attached or has connected, running or online health; a stopped runtime
    // disables every scan from it. This computer runs EtherCAT and GPIO never; USB, serial and
    // Bluetooth run from it. A wrong implementation enables a hardware scan of a stopped runtime,
    // leaves a disabled scan without its reason, or disables USB on a ready runtime.
    let answer = status("done", json!([]), json!([]));
    let hardware = ["ethercat", "gpio", "serial", "usb", "bluetooth"];
    let choices = |scan: &Scan, runtimes: &[RuntimeOrigin]| {
        view_of(&answer, scan, runtimes)
            .only
            .into_iter()
            .filter(|choice| hardware.contains(&choice.source.as_str()))
            .map(|choice| (choice.source, choice.enabled, choice.reason))
            .collect::<Vec<_>>()
    };

    let this_host = choices(&Scan::ThisHost, &[]);
    assert_eq!(
        this_host,
        [
            (
                "ethercat".to_owned(),
                false,
                Some("Choose a running runtime for EtherCAT or GPIO scans.".to_owned())
            ),
            (
                "gpio".to_owned(),
                false,
                Some("Choose a running runtime for EtherCAT or GPIO scans.".to_owned())
            ),
            ("serial".to_owned(), true, None),
            ("usb".to_owned(), true, None),
            ("bluetooth".to_owned(), false, Some(BLUETOOTH.to_owned())),
        ],
        "this computer cannot run EtherCAT or GPIO, and runs serial and USB"
    );

    let stopped = vec![origin("runtime:cell", "Cell", false)];
    let scan = Scan::Runtime("runtime:cell".to_owned());
    assert_eq!(
        choices(&scan, &stopped),
        [
            (
                "ethercat".to_owned(),
                false,
                Some("Start or connect a runtime before scanning EtherCAT or GPIO.".to_owned())
            ),
            (
                "gpio".to_owned(),
                false,
                Some("Start or connect a runtime before scanning EtherCAT or GPIO.".to_owned())
            ),
            (
                "serial".to_owned(),
                false,
                Some("Start or connect Cell before scanning from it.".to_owned())
            ),
            (
                "usb".to_owned(),
                false,
                Some("Start or connect Cell before scanning from it.".to_owned())
            ),
            ("bluetooth".to_owned(), false, Some(BLUETOOTH.to_owned())),
        ],
        "a stopped runtime disables every scan from it, each with its reason"
    );

    let ready = ready_cell();
    assert_eq!(
        choices(&Scan::Runtime("runtime:cell".to_owned()), &ready),
        [
            ("ethercat".to_owned(), true, None),
            ("gpio".to_owned(), true, None),
            ("serial".to_owned(), true, None),
            ("usb".to_owned(), true, None),
            ("bluetooth".to_owned(), false, Some(BLUETOOTH.to_owned())),
        ],
        "once the runtime is ready, every hardware scan that this version runs is enabled"
    );
}

#[test]
fn empty_result_suggests_checks() {
    // Specification 25:75-77: an empty result suggests the checks of device power, network
    // reachability, port and firewall restrictions, and the supplied address or subnet. The text
    // adds what the hardware scans need. A wrong implementation shows the checks for a search that
    // still runs, or leaves one of the four checks out.
    let done = status("done", json!([found("modbus_tcp", 0)]), json!([]));
    let text = view_of(&done, &Scan::ThisHost, &ready_cell()).empty_text;
    assert_eq!(
        text.as_deref(),
        Some(
            "Nothing found. Check that the device is powered on, on the same network, and not blocked by a port or firewall. Verify the address or subnet for known-address scans. For EtherCAT or GPIO, choose a running runtime in Scan from."
        ),
        "the four checks, then the hint for the hardware scans"
    );
    let no_runtime = view_of(&done, &Scan::ThisHost, &[]).empty_text;
    assert!(
        no_runtime.is_some_and(
            |text| text.ends_with("For EtherCAT or GPIO, start or connect a runtime first.")
        ),
        "without a runtime that can scan, the hint says to start one"
    );
    let running = status("running", json!([found("modbus_tcp", 0)]), json!([]));
    assert_eq!(
        view_of(&running, &Scan::ThisHost, &ready_cell()).empty_text,
        None,
        "a search that still runs shows no empty result yet"
    );
}

#[test]
fn runtime_card_shows_one_host_port_without_tcp_prefix() {
    // Specification 25:69-70: a runtime card shows one actionable host and port and omits a
    // visible `tcp://` prefix. A wrong implementation shows the endpoint with its prefix, or shows
    // both the host and the endpoint as two addresses.
    let answer = status(
        "done",
        json!([found("trust_runtime", 1)]),
        json!([device(
            "dev:ip:10.0.0.6",
            &["10.0.0.6", "10.0.1.6"],
            &[runtime_find("tcp://10.0.0.6:5680")]
        )]),
    );
    let view = view_of(&answer, &Scan::ThisHost, &ready_cell());
    let card = &view.cards[0];
    assert_eq!(card.addresses, ["10.0.0.6:5680"]);
    assert_eq!(
        card.title, "Line runtime",
        "the runtime's name is its title"
    );
}

#[test]
fn labels_never_backend_identifiers() {
    // Specification 25:71-72: other cards show human-readable protocol, discovery-source and
    // confidence labels, not backend identifiers. Every source row, scan choice, origin, card
    // protocol and card title holds words for people. A wrong implementation shows a token such as
    // `tcp_connect` or `modbus_tcp` as a label.
    let every_source: Vec<Value> = SOURCES.iter().map(|id| found(id, 1)).collect();
    let answer = status(
        "done",
        Value::Array(every_source),
        json!(FIND_SOURCES
            .iter()
            .enumerate()
            .map(|(index, source)| device(
                &format!("dev:ip:10.0.2.{index}"),
                &[],
                &[json!({
                    "protocol": "modbus_tcp",
                    "source": source,
                    "confidence": "likely",
                    "simulated": false,
                    "identity": { "kind": "modbus" },
                    "params": {},
                    "warnings": []
                })]
            ))
            .collect::<Vec<_>>()),
    );
    let view = view_of(&answer, &Scan::ThisHost, &ready_cell());
    let mut labels: Vec<String> = Vec::new();
    labels.extend(view.sources.iter().map(|row| row.label.clone()));
    labels.extend(view.only.iter().map(|choice| choice.label.clone()));
    labels.extend(view.origins.iter().map(|choice| choice.label.clone()));
    for card in &view.cards {
        labels.push(card.title.clone());
        labels.extend(card.protocols.iter().map(|protocol| protocol.name.clone()));
        labels.extend(
            card.protocols
                .iter()
                .map(|protocol| protocol.source.clone()),
        );
        labels.extend(
            card.protocols
                .iter()
                .map(|protocol| protocol.confidence.clone()),
        );
    }
    for label in &labels {
        assert!(!label.contains('_'), "{label} is a backend identifier");
    }
    for token in FIND_SOURCES {
        assert!(
            labels.iter().all(|label| label.as_str() != *token),
            "the discovery source {token} is shown as its own token"
        );
    }
    assert_eq!(view.cards.len(), FIND_SOURCES.len(), "one card per device");
    assert_eq!(
        view.cards[0].protocols[0].source, "Network discovery",
        "mDNS reads in words"
    );
}

#[test]
fn forbidden_discover_copy_absent() {
    // Specification 25:78-81: the Discover copy never uses `Field devices`, `origin's local
    // subnet`, `connect-only`, `Targeted (needs a host/subnet)`, `Runtime-only` or `Discovery
    // needs a runtime that serves it`. The view is checked over the states it can produce: an empty
    // search, a running one, a stopped runtime, a runtime that no longer shows, a runtime that
    // cannot search, and every kind of card action. A wrong implementation reintroduces one of
    // the six phrases in any of them.
    let forbidden = [
        "Field devices",
        "origin's local subnet",
        "connect-only",
        "Targeted (needs a host/subnet)",
        "Runtime-only",
        "Discovery needs a runtime that serves it",
    ];
    let runtime = device(
        "dev:ip:10.0.0.6",
        &["10.0.0.6"],
        &[runtime_find("tcp://10.0.0.6:5680")],
    );
    let no_endpoint = json!({
        "id": "dev:mesh:ring-7",
        "addresses": [],
        "last_seen_ms": NOW_MS,
        "finds": [{ "protocol": "trust_runtime", "source": "mesh", "confidence": "unavailable",
                    "simulated": false, "identity": {"kind": "trust_runtime", "name": "ring-7"},
                    "params": {}, "warnings": ["Runtime found through the mesh without a control endpoint."] }]
    });
    let usb = device(
        "dev:usb:4711",
        &[],
        &[json!({
            "protocol": "usb", "source": "usb_descriptor", "confidence": "confirmed", "simulated": false,
            "identity": {"kind": "usb", "vid": 1155, "pid": 22336, "product": "ST-LINK/V2-1"},
            "params": {}, "warnings": []
        })],
    );
    let all = json!([
        found("trust_runtime", 1),
        found("ads", 0),
        skipped(
            "ethercat",
            "EtherCAT is searched inside a runtime, on the computer with the bus adapter."
        )
    ]);
    let ads = device(
        "dev:netid:192.168.1.20.1.1",
        &["192.168.1.20"],
        &[ads_find("192.168.1.20.1.1")],
    );
    let scenarios = [
        view_of(&status("done", json!([]), json!([])), &Scan::ThisHost, &[]),
        view_of(
            &status("running", all.clone(), json!([])),
            &Scan::ThisHost,
            &ready_cell(),
        ),
        view_of(
            &status(
                "done",
                all.clone(),
                json!([runtime.clone(), no_endpoint, usb]),
            ),
            &Scan::ThisHost,
            &[],
        ),
        view_of(
            &status("done", all.clone(), json!([runtime, ads.clone()])),
            &Scan::Runtime("runtime:gone".to_owned()),
            &[origin("runtime:cell", "Cell", false)],
        ),
        view_of(
            &status("done", json!([]), json!([])),
            &Scan::Runtime("runtime:cell".to_owned()),
            &ready_cell(),
        ),
        view_of(&status("done", all, json!([ads])), &Scan::ThisHost, &[]),
    ];
    for view in &scenarios {
        let text = serde_json::to_string(view).expect("a view serializes");
        for phrase in forbidden {
            assert!(
                !text.contains(phrase),
                "the view shows the forbidden copy {phrase}"
            );
        }
    }
}

#[test]
fn origins_offer_this_computer_then_each_runtime() {
    // Specification 25 §2: `Scan from` chooses this computer or a runtime, and a stopped runtime is
    // still offered (its scans are disabled instead). A wrong implementation leaves a stopped
    // runtime out, or marks a different origin as the selected one.
    let runtimes = [
        origin("runtime:cell", "Cell", true),
        origin("runtime:lab", "Lab", false),
    ];
    let answer = status("done", json!([]), json!([]));
    let origins = view_of(
        &answer,
        &Scan::Runtime("runtime:cell".to_owned()),
        &runtimes,
    )
    .origins;
    let shown: Vec<(&str, &str, bool)> = origins
        .iter()
        .map(|choice| (choice.id.as_str(), choice.label.as_str(), choice.selected))
        .collect();
    assert_eq!(
        shown,
        [
            ("this_host", "This computer", false),
            ("runtime:cell", "Cell", true),
            ("runtime:lab", "Lab", false),
        ]
    );
    assert!(origins
        .iter()
        .all(|choice| choice.enabled && choice.reason.is_none()));
}
