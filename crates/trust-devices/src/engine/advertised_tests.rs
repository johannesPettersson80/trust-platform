//! Native tests of the protocols that a runtime advertises beyond the ten sources (specification 25 §2): the panel
//! runs them through the origin runtime's `comm.discover`, a search of only such protocols runs at the panel alone,
//! and a search that also names the runtime's own sources sends the runtime only those. Each expected value is a
//! literal from specification 25 §2 or 11 ("One search"), or the fake runtime's script; none is read back from the
//! code under test.

use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::tests::{answered, answering_card, online, search_of, start_at, SLACK};
use super::super::{
    DevicesEngine, SearchCall, SearchOrigin, SearchReply, SearchVerb, TypedTargets,
};
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::test_project::{runtime_toml, TempFolder};
use crate::view::types::DiscoverView;

/// The `comm.schema` of a runtime that advertises `fieldx` with `discover`, beyond the ten sources, as `Field X`.
fn advertising_fieldx() -> Value {
    json!({ "protocols": [
        { "id": "fieldx", "label": "Field X", "actions": ["discover"] },
    ]})
}

/// The `comm.discover` answer of `fieldx` with one unit at 127.0.0.1.
fn one_fieldx_unit() -> Value {
    json!({
        "schema_version": 1,
        "protocol": "fieldx",
        "origin": "runtime",
        "candidates": [{
            "id": "unit-1",
            "label": "Field X unit 1",
            "source": "fieldx_probe",
            "confidence": "likely",
            "params": { "host": "127.0.0.1" },
        }],
    })
}

/// A project of the runtime at `endpoint`, in a folder of its own: its item ID, as the panel's gather names it.
fn runtime_card(
    engine: &DevicesEngine,
    endpoint: &str,
    folder: &TempFolder,
    seq: &mut u64,
) -> String {
    folder.write(
        "cell/runtime.toml",
        &runtime_toml("cell", endpoint, Some("token-1")),
    );
    let project = folder.join("cell");
    answering_card(engine, &folder.path, &project, &online(endpoint), seq)
}

/// The status of the search `search_id` at the runtime `card`, asked until its state is `done`.
fn done_status(engine: &DevicesEngine, card: &str, search_id: &str) -> SearchReply {
    let until = Instant::now() + SLACK;
    loop {
        let mut poll = search_of("dc-test", SearchVerb::Status);
        poll.origin = SearchOrigin::Runtime(card.to_owned());
        poll.search_id = Some(search_id.to_owned());
        let reply = engine.search(&poll);
        if reply.status.get("state") == Some(&json!("done")) {
            return reply;
        }
        assert!(Instant::now() < until, "the search of {search_id} ends");
        thread::sleep(Duration::from_millis(5));
    }
}

/// The Discover view of a status reply.
fn view_of(reply: &SearchReply) -> &DiscoverView {
    reply
        .discover
        .as_deref()
        .expect("a status carries the Discover view")
}

/// The `comm.discover` requests that the fake runtime received for `fieldx`, as the runtime saw them.
fn fieldx_requests(runtime: &FakeRuntime) -> Vec<Value> {
    runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "comm.discover")
        .collect()
}

/// A start with the Known address `host` and no subnet.
fn start_for_host(card: &str, params: Value, host: &str) -> SearchCall {
    let mut start = start_at(card, params);
    start.targets = Some(TypedTargets {
        hosts: host.to_owned(),
        cidr: String::new(),
    });
    start
}

#[test]
fn an_advertised_protocol_is_scanned_through_comm_discover() {
    // Specification 25 §2: a protocol that a runtime advertises beyond the ten sources is scanned through the origin
    // runtime's `comm.discover` for that protocol, one request for the Known address, and its find is a card under
    // the advertised label. A search of only such a protocol runs at the panel and sends nothing to
    // `comm.search.start`, which names only the ten sources (specification 11, "One search"). A wrong
    // implementation sends the name to the runtime's search, never asks `comm.discover`, drops the find, or titles
    // the card by its ID.
    let runtime = FakeRuntime::start(
        Script::running()
            .answer("comm.schema", Reply::Result(advertising_fieldx()))
            .answer("comm.discover", Reply::Result(one_fieldx_unit())),
    );
    let folder = TempFolder::new("advertised-scan");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let card = runtime_card(&engine, &runtime.endpoint, &folder, &mut seq);

    let started = answered(
        &engine,
        &start_for_host(&card, json!({ "only": ["fieldx"] }), "127.0.0.1"),
    );
    assert!(
        started.error.is_none(),
        "the search starts: {:?}",
        started.error
    );
    let search_id = started.search_id.expect("the search has an ID");
    assert_eq!(
        search_id, "a-1",
        "the first search that the panel runs alone"
    );

    let reply = done_status(&engine, &card, &search_id);
    let view = view_of(&reply);
    let row = view
        .sources
        .first()
        .expect("the advertised protocol has its source row");
    assert_eq!(
        (row.id.as_str(), row.label.as_str(), row.count),
        ("fieldx", "Field X", Some(1)),
        "the row names the protocol as the runtime advertises it, with its one find"
    );
    let card_view = view.cards.first().expect("the find is a card");
    assert_eq!(
        card_view.title, "Field X unit 1",
        "the card is titled by the find's name"
    );
    assert_eq!(
        card_view.protocols.first().map(|line| line.name.as_str()),
        Some("Field X")
    );

    assert_eq!(
        runtime.count("comm.search.start"),
        0,
        "the runtime runs no search of this protocol"
    );
    let requests = fieldx_requests(&runtime);
    assert_eq!(
        requests.len(),
        1,
        "one comm.discover request, for the Known address"
    );
    assert_eq!(
        requests[0]["params"],
        json!({
            "protocol": "fieldx",
            "origin": "runtime",
            "scope": { "host": "127.0.0.1" },
        }),
        "the request names the protocol, the origin runtime and the Known address"
    );
}

#[test]
fn an_advertised_protocol_asks_once_for_each_known_address_and_for_the_subnet() {
    // Specification 25 §2: a scan of an advertised protocol asks once for each Known address and once for the
    // subnet. The expected scopes are the literal `host` and `cidr` of the typed targets. A wrong implementation
    // asks once for both, or drops the subnet.
    let runtime = FakeRuntime::start(
        Script::running()
            .answer("comm.schema", Reply::Result(advertising_fieldx()))
            .answer("comm.discover", Reply::Result(one_fieldx_unit())),
    );
    let folder = TempFolder::new("advertised-targets");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let card = runtime_card(&engine, &runtime.endpoint, &folder, &mut seq);

    let mut start = start_at(&card, json!({ "only": ["fieldx"] }));
    start.targets = Some(TypedTargets {
        hosts: "127.0.0.1".to_owned(),
        cidr: "127.0.0.0/30".to_owned(),
    });
    let started = answered(&engine, &start);
    let search_id = started.search_id.expect("the search has an ID");
    done_status(&engine, &card, &search_id);

    let scopes: Vec<Value> = fieldx_requests(&runtime)
        .iter()
        .map(|request| request["params"]["scope"].clone())
        .collect();
    assert_eq!(
        scopes,
        vec![
            json!({ "host": "127.0.0.1" }),
            json!({ "cidr": "127.0.0.0/30" })
        ],
        "one request for the Known address, then one for the subnet"
    );
}

#[test]
fn an_advertised_scan_that_passes_its_deadline_reports_timed_out() {
    // Specification 11 ("One search") and 25 §2: a source still running at the deadline reports `timed out`, with the
    // finds it has. The runtime never answers `comm.discover`, and the search's deadline is 1 ms, so the scan ends
    // with no find. A wrong implementation reports the row as found, or keeps it searching after the deadline.
    let runtime = FakeRuntime::start(
        Script::running()
            .answer("comm.schema", Reply::Result(advertising_fieldx()))
            .answer("comm.discover", Reply::Silent),
    );
    let folder = TempFolder::new("advertised-deadline");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let card = runtime_card(&engine, &runtime.endpoint, &folder, &mut seq);

    let started = answered(
        &engine,
        &start_for_host(
            &card,
            json!({ "only": ["fieldx"], "deadline_ms": 1 }),
            "127.0.0.1",
        ),
    );
    let search_id = started.search_id.expect("the search has an ID");
    let reply = done_status(&engine, &card, &search_id);
    let row = view_of(&reply)
        .sources
        .first()
        .expect("the source row")
        .clone();
    assert_eq!(
        (row.word.as_str(), row.count),
        ("timed out", Some(0)),
        "a scan that passes its deadline reports timed out, with the finds it has (none here)"
    );
    assert!(view_of(&reply).cards.is_empty(), "no find is a card");
}

#[test]
fn a_refused_advertised_scan_shows_the_runtime_reason_on_its_row() {
    // Specification 25 §2: a request that the runtime refuses leaves the row `skipped` with the runtime's own error
    // text, so the choice gives its reason. The fake runtime's refusal `unknown_protocol` is the error text
    // `unknown protocol`. A wrong implementation draws the row as found, or drops its reason.
    let runtime = FakeRuntime::start(
        Script::running()
            .answer("comm.schema", Reply::Result(advertising_fieldx()))
            .answer("comm.discover", Reply::Refuse("unknown_protocol")),
    );
    let folder = TempFolder::new("advertised-refused");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let card = runtime_card(&engine, &runtime.endpoint, &folder, &mut seq);

    let started = answered(
        &engine,
        &start_for_host(&card, json!({ "only": ["fieldx"] }), "127.0.0.1"),
    );
    let search_id = started.search_id.expect("the search has an ID");
    let reply = done_status(&engine, &card, &search_id);
    let row = view_of(&reply)
        .sources
        .first()
        .expect("the source row")
        .clone();
    assert_eq!(row.word, "skipped", "a refused scan is skipped");
    assert_eq!(row.reason.as_deref(), Some("unknown protocol"));
    assert_eq!(row.count, None, "a skipped source reports no count");
}

#[test]
fn a_search_of_runtime_sources_and_an_advertised_protocol_runs_both_in_one_search() {
    // Specification 11 ("One search"): the runtime runs the sources it names and nothing else, so `comm.search.start`
    // carries only `modbus_tcp`; the advertised `fieldx` runs at the panel through `comm.discover`, and both rows
    // belong to the one search that the panel shows, in the order the search names them. A wrong implementation
    // forwards `fieldx` to the runtime (which refuses it), drops the runtime's row, or shows the search as done
    // while the advertised scan still runs.
    let runtime = FakeRuntime::start(
        Script::running()
            .answer("comm.schema", Reply::Result(advertising_fieldx()))
            .answer("comm.discover", Reply::Result(one_fieldx_unit()))
            .answer(
                "comm.search.start",
                Reply::Result(json!({ "search_id": "s-7" })),
            )
            .answer(
                "comm.search.status",
                Reply::Result(json!({
                    "search_id": "s-7",
                    "origin": "runtime",
                    "state": "done",
                    "sources": [{ "id": "modbus_tcp", "state": "found", "count": 0 }],
                    "devices": [],
                    "warnings": [],
                })),
            ),
    );
    let folder = TempFolder::new("advertised-mixed");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let card = runtime_card(&engine, &runtime.endpoint, &folder, &mut seq);

    let started = answered(
        &engine,
        &start_for_host(
            &card,
            json!({ "only": ["modbus_tcp", "fieldx"] }),
            "127.0.0.1",
        ),
    );
    assert_eq!(
        started.search_id.as_deref(),
        Some("s-7"),
        "the runtime's search is the one search"
    );
    let forwarded: Vec<Value> = runtime
        .requests()
        .into_iter()
        .filter(|request| request["type"] == "comm.search.start")
        .collect();
    assert_eq!(forwarded.len(), 1, "the runtime receives the start once");
    assert_eq!(
        forwarded[0]["params"]["only"],
        json!(["modbus_tcp"]),
        "the runtime is asked for its own source only"
    );

    let reply = done_status(&engine, &card, "s-7");
    let ids: Vec<&str> = view_of(&reply)
        .sources
        .iter()
        .map(|row| row.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["modbus_tcp", "fieldx"],
        "both rows of the one search"
    );
    assert_eq!(
        view_of(&reply).cards.len(),
        1,
        "the advertised find is a card of the search"
    );
}

#[test]
fn an_advertised_find_at_the_address_of_a_runtime_find_joins_its_card() {
    // Specification 11 ("Devices"): results that share a network address are one device listing every protocol found
    // on it, and specification 25 §2 joins the advertised finds into the search by that rule. The runtime's search
    // found a Modbus server at 127.0.0.1 and `fieldx` finds a unit at the same address, so the pane shows one card
    // with both protocol lines. A wrong implementation shows the advertised find as a second card of the same
    // device, or drops the runtime's find.
    let runtime = FakeRuntime::start(
        Script::running()
            .answer("comm.schema", Reply::Result(advertising_fieldx()))
            .answer("comm.discover", Reply::Result(one_fieldx_unit()))
            .answer(
                "comm.search.start",
                Reply::Result(json!({ "search_id": "s-8" })),
            )
            .answer(
                "comm.search.status",
                Reply::Result(json!({
                    "search_id": "s-8",
                    "origin": "runtime",
                    "state": "done",
                    "sources": [{ "id": "modbus_tcp", "state": "found", "count": 1 }],
                    "devices": [{
                        "id": "dev:ip:127.0.0.1",
                        "addresses": ["127.0.0.1"],
                        "last_seen_ms": 1,
                        "finds": [{
                            "protocol": "modbus_tcp",
                            "source": "tcp_probe",
                            "confidence": "confirmed",
                            "simulated": false,
                            "identity": {},
                            "params": { "host": "127.0.0.1", "port": 502 },
                            "warnings": [],
                            "last_seen_ms": 1,
                        }],
                    }],
                    "warnings": [],
                })),
            ),
    );
    let folder = TempFolder::new("advertised-joined");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    let card = runtime_card(&engine, &runtime.endpoint, &folder, &mut seq);

    let started = answered(
        &engine,
        &start_for_host(
            &card,
            json!({ "only": ["modbus_tcp", "fieldx"] }),
            "127.0.0.1",
        ),
    );
    assert_eq!(started.search_id.as_deref(), Some("s-8"));

    let reply = done_status(&engine, &card, "s-8");
    let view = view_of(&reply);
    assert_eq!(
        view.cards.len(),
        1,
        "the two finds at 127.0.0.1 are one device"
    );
    let joined = &view.cards[0];
    assert_eq!(joined.id, "dev:ip:127.0.0.1");
    assert_eq!(
        joined.protocols.len(),
        2,
        "the card lists the runtime's find and the advertised one"
    );
    assert!(
        joined.protocols.iter().any(|line| line.name == "Field X"),
        "the advertised find keeps its advertised label on the joined card"
    );
}

#[test]
fn a_joined_device_keeps_the_id_of_the_device_found_first() {
    // Specification 11 ("Devices"): when a result links two devices they become one, and the device found first keeps
    // its ID. A device that the runtime formed by its AMS Net ID and that lists 10.0.0.5 is joined by an advertised
    // find at 10.0.0.5: it keeps `dev:netid:5.1.2.3.1.1`, lists both finds, and takes the later `last_seen_ms` (900
    // over 100). A find at another address stays a device of its own. A wrong implementation renames the device
    // after the advertised find, keeps the earlier time, or joins finds that share no address.
    let mut devices = vec![json!({
        "id": "dev:netid:5.1.2.3.1.1",
        "addresses": ["10.0.0.5"],
        "last_seen_ms": 100,
        "finds": [{ "protocol": "ads" }],
    })];
    let at_same = super::device_of(
        "fieldx",
        "unit-1",
        &json!({ "id": "unit-1", "params": { "host": "10.0.0.5" } }),
        900,
    );
    let elsewhere = super::device_of(
        "fieldx",
        "unit-2",
        &json!({ "id": "unit-2", "params": { "host": "10.0.0.6" } }),
        900,
    );
    super::join(&mut devices, at_same);
    super::join(&mut devices, elsewhere);

    assert_eq!(devices.len(), 2, "only the find at 10.0.0.5 joins");
    assert_eq!(devices[0]["id"], "dev:netid:5.1.2.3.1.1");
    assert_eq!(devices[0]["last_seen_ms"], 900);
    let protocols: Vec<&Value> = devices[0]["finds"]
        .as_array()
        .expect("the joined device lists its finds")
        .iter()
        .map(|find| &find["protocol"])
        .collect();
    assert_eq!(protocols, vec![&json!("ads"), &json!("fieldx")]);
    assert_eq!(
        devices[1]["id"], "dev:ip:10.0.0.6",
        "a find with an address is named by it, as the runtime names a device"
    );
}
