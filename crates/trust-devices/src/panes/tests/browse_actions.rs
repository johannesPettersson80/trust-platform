//! Native tests of the browse views' actions, search and typed port (design §4.3 B8 and D5b; specification
//! 25 §5.4 and §11.2). Every expected value is written out, taken from the specification or from the
//! panel the behaviour came from, named in each test.

use serde_json::{json, Map, Value};

use crate::browse::port::parse_port;
use crate::browse::Browsed;
use crate::panes::browse::{self, Browse};
use crate::request::BrowseRequest;
use crate::texts::browse as texts;
use crate::view::types::{BrowseKind, BrowseView, ItemId, Op};

/// The runtime that owns every test's connection.
fn owner() -> ItemId {
    ItemId("runtime:line".to_owned())
}

/// The `devicesBrowse` body of a browse of the connection `endpoint:line`, named `Line`.
fn request(kind: &str, port: Option<u16>) -> BrowseRequest {
    let mut body = json!({
        "session": "dc-tests",
        "seq": 3,
        "source": { "connection": "endpoint:line", "connection_name": "Line" },
        "kind": kind,
        "trust_server_certificate": false,
    });
    if let Some(port) = port {
        body["port"] = json!(port);
    }
    serde_json::from_value(body).expect("a devicesBrowse body decodes")
}

/// The `devicesBrowse` body of a browse of a device a search found, at its own target.
fn found_request(port: u16) -> BrowseRequest {
    serde_json::from_value(json!({
        "session": "dc-tests",
        "seq": 3,
        "source": {
            "found": "card:ads-1",
            "protocol": "ads",
            "target": { "ip": "192.168.10.5", "ams_net_id": "5.23.91.12.1.1" },
        },
        "kind": "symbols",
        "port": port,
    }))
    .expect("a found device's browse decodes")
}

/// A configuration as its map.
fn params(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("a connection's params are an object: {other}"),
    }
}

/// The browse view of `kind` over `protocol`, projected from the origin's wire `answer` (`None` while
/// the browse runs).
fn view(
    kind: BrowseKind,
    protocol: &str,
    params: &Map<String, Value>,
    request: &BrowseRequest,
    answer: Option<&Value>,
) -> BrowseView {
    let owner = owner();
    let browsed = answer.map(Browsed::of);
    browse::view(
        Browse {
            kind,
            protocol,
            name: "Line",
            params,
            runtime: Some(&owner),
            port: request.port.unwrap_or(0),
            request,
            cached: &[],
        },
        browsed.as_ref(),
    )
}

#[test]
fn add_posts_browse_add_with_the_panes_own_request() {
    // Design D5b R1 and specification 25 §5.4: `Add` carries `browse.add` with the pane's own request
    // in its payload; the webview posts it with the ticked keys and the write toggle, so Rust can read
    // the selection from the same request. The old panel's label counted the ticks, `Add nodes (1)`
    // (`W/BrowseTagsPanel.tsx:347`). A wrong implementation drops the request, posts another action,
    // or changes the label or the count.
    let answer = json!({
        "schema_version": 1, "protocol": "opcua_client", "kind": "nodes",
        "tree": [{
            "id": "n:Line.Speed", "name": "Speed", "path": "Line.Speed",
            "node_id": "ns=2;s=Line.Speed", "type": "Double", "writable": true,
        }],
    });
    let view = view(
        BrowseKind::Nodes,
        "opcua_client",
        &params(json!({ "points": [] })),
        &request("nodes", None),
        Some(&answer),
    );

    let add = view.add.expect("an OPC UA pane has an Add button");
    assert_eq!(add.label, "Add nodes");
    assert_eq!(add.counted_label, "Add nodes ({count})");
    assert!(add.enabled);
    assert_eq!(add.action.label, "Add nodes");
    let Op::Rust { action, payload } = &add.action.op else {
        panic!("Add is a Rust action, got {:?}", add.action.op);
    };
    assert_eq!(action, "browse.add");
    assert_eq!(payload["pane"], json!("browse:opcua_client"));
    let pane_request: BrowseRequest = serde_json::from_value(payload["request"].clone())
        .expect("the payload's request decodes into the pane's own request");
    assert_eq!(pane_request.source.connection(), Some("endpoint:line"));
    assert_eq!(pane_request.source.connection_name(), Some("Line"));
    assert_eq!(pane_request.kind, "nodes");
    assert_eq!(
        pane_request.port, None,
        "an OPC UA browse reads no ADS port"
    );
}

#[test]
fn ticking_an_ads_tag_posts_ticked_when_unadded_and_unticked_when_added() {
    // Design D5b R2, design §3.3 J56 and specification 25 §5.4: ticking a tag the project does not
    // have posts `ads.tags_set` with its path as `ticked`; unticking a tag the project has posts the
    // same action with `unticked`. The payload names the card's runtime, connection and port. A wrong
    // implementation sends one key for both, names another port, or posts the tag without its
    // runtime, which `ads.tags_set` refuses.
    let params = params(json!({
        "name": "line1", "host": "192.168.10.5", "target_net_id": "5.23.91.12.1.1",
        "ams_port": 851, "points": [],
        "imported_ads_symbols": [{ "port": 851, "paths": ["MAIN.Counter"] }],
    }));
    let answer = json!({
        "schema_version": 1, "protocol": "ads", "kind": "symbols",
        "tree": [
            { "id": "ads:symbol:MAIN.Counter", "name": "Counter", "path": "MAIN.Counter", "type": "INT", "writable": true },
            { "id": "ads:symbol:MAIN.Speed", "name": "Speed", "path": "MAIN.Speed", "type": "REAL", "writable": false },
        ],
    });
    let view = view(
        BrowseKind::AdsTags,
        "ads",
        &params,
        &request("symbols", Some(851)),
        Some(&answer),
    );

    assert!(view.add.is_none(), "an ADS pane has no Add button");
    let tags = &view.ports[0].tags;
    let counter = &tags[0];
    assert_eq!(counter.name, "Counter");
    assert!(counter.added, "the project has Counter");
    assert!(counter.selectable);
    assert_eq!(
        counter.select_label.as_deref(),
        Some("Select MAIN.Counter"),
        "the tick box is named by the path, in Rust's words"
    );
    let speed = &tags[1];
    assert!(!speed.added);
    assert!(speed.selectable);

    let counter_action = counter
        .action
        .as_ref()
        .expect("a tag carries its tick action");
    let Op::Rust { action, payload } = &counter_action.op else {
        panic!("a tick is a Rust action");
    };
    assert_eq!(action, "ads.tags_set");
    assert_eq!(
        payload,
        &json!({
            "pane": "browse:ads", "runtime": "runtime:line", "connection": "endpoint:line",
            "port": 851, "unticked": ["MAIN.Counter"],
        }),
        "unticking an added tag names it under `unticked`"
    );

    let speed_action = speed
        .action
        .as_ref()
        .expect("a tag carries its tick action");
    let Op::Rust { payload, .. } = &speed_action.op else {
        panic!("a tick is a Rust action");
    };
    assert_eq!(
        payload,
        &json!({
            "pane": "browse:ads", "runtime": "runtime:line", "connection": "endpoint:line",
            "port": 851, "ticked": ["MAIN.Speed"],
        }),
        "ticking an unadded tag names it under `ticked`"
    );
}

#[test]
fn a_found_device_tag_has_no_connection_to_import_into_so_it_cannot_be_ticked() {
    // Specification 25 §5.4: an import writes to a connection the project has. A tag of a device that
    // a search found has none yet, so it carries no tick action and no tick box. A wrong
    // implementation posts `ads.tags_set` with no connection, which the action refuses.
    let found = found_request(851);
    let answer = json!({
        "schema_version": 1, "protocol": "ads", "kind": "symbols",
        "tree": [{ "id": "ads:symbol:MAIN.Speed", "name": "Speed", "path": "MAIN.Speed", "type": "REAL", "writable": false }],
    });
    let view = view(
        BrowseKind::AdsTags,
        "ads",
        &params(json!({ "points": [] })),
        &found,
        Some(&answer),
    );

    let speed = &view.ports[0].tags[0];
    assert!(!speed.selectable, "the tag cannot be ticked");
    assert_eq!(speed.select_label, None);
    assert!(speed.action.is_none(), "the tag posts nothing");
}

#[test]
fn the_search_box_is_offered_once_the_tree_is_there() {
    // The old panels: `Search symbols` under a loaded tree (`BrowseTagsPanel.tsx:241-245`), and
    // `Search tags on all ADS ports` on the ADS pane (`AdsMultiPortTagBrowser.tsx:255-261`), each with
    // its no-match text. A wrong implementation offers the search over a browse that has no answer,
    // or drops a placeholder or a no-match text.
    let loaded = json!({
        "schema_version": 1, "protocol": "opcua_client", "kind": "nodes",
        "tree": [{ "id": "n:Line", "name": "Line", "path": "Line", "node_id": "ns=2;s=Line", "type": "Double", "writable": true }],
    });
    let nodes = params(json!({ "points": [] }));
    let running = view(
        BrowseKind::Nodes,
        "opcua_client",
        &nodes,
        &request("nodes", None),
        None,
    );
    assert!(
        running.search.is_none(),
        "no search over a browse that has no answer"
    );
    assert_eq!(running.empty_text.as_deref(), Some("Loading symbols…"));

    let answered = view(
        BrowseKind::Nodes,
        "opcua_client",
        &nodes,
        &request("nodes", None),
        Some(&loaded),
    );
    let search = answered.search.expect("a loaded tree can be searched");
    assert_eq!(search.placeholder, "Search symbols");
    assert_eq!(search.no_match, "No matching symbols.");

    let ads = view(
        BrowseKind::AdsTags,
        "ads",
        &params(json!({ "points": [] })),
        &request("symbols", Some(851)),
        None,
    );
    let search = ads.search.expect("the ADS pane is always searchable");
    assert_eq!(search.placeholder, "Search tags on all ADS ports");
    assert_eq!(search.no_match, "No matching tags on this port.");
    assert_eq!(ads.port_summary.as_deref(), Some("1 ADS port"));
}

#[test]
fn the_typed_port_is_read_with_the_rule_of_the_panel_it_replaces() {
    // `webview/adsTargetPort.ts:9-22`: an empty text is the default port 851; whole digits from 1 to
    // 65,535 are the port (surrounding spaces and leading zeros do not count); any other digits are
    // out of range; any other text is not a whole number. A wrong implementation refuses the empty
    // text, accepts `0` or `65536`, or reads `1e3` or `-1` as a port.
    assert_eq!(parse_port(""), Ok(851), "an empty text is the default port");
    assert_eq!(parse_port("   "), Ok(851));
    assert_eq!(parse_port(" 301 "), Ok(301));
    assert_eq!(parse_port("00851"), Ok(851));
    assert_eq!(parse_port("65535"), Ok(65_535));
    assert_eq!(parse_port("0"), Err(texts::ADS_PORT_RANGE));
    assert_eq!(parse_port("65536"), Err(texts::ADS_PORT_RANGE));
    assert_eq!(
        parse_port("99999999999999999999"),
        Err(texts::ADS_PORT_RANGE),
        "a number too large for a port is out of range"
    );
    assert_eq!(parse_port("1e3"), Err(texts::ENTER_WHOLE_PORT));
    assert_eq!(parse_port("-1"), Err(texts::ENTER_WHOLE_PORT));
    assert_eq!(parse_port("85 1"), Err(texts::ENTER_WHOLE_PORT));
}

#[test]
fn the_typed_port_action_posts_its_value_and_the_pane_request() {
    // Design D5b R3: `Browse this port` is a Rust action whose payload names the pane and its request,
    // so Rust can start the browse; the webview posts the typed text as the `port` value. A wrong
    // implementation makes it a `ui` op, which carries no value, or leaves the request out.
    let answer = json!({ "schema_version": 1, "protocol": "ads", "kind": "symbols", "tree": [] });
    let view = view(
        BrowseKind::AdsTags,
        "ads",
        &params(json!({ "ams_port": 851, "points": [] })),
        &request("symbols", Some(851)),
        Some(&answer),
    );

    let input = view.port_input.expect("an ADS pane has the typed port");
    assert_eq!(input.show_label, "Advanced: browse another ADS port");
    assert_eq!(input.hide_label, "Hide advanced");
    assert_eq!(input.label, "ADS port");
    assert_eq!(input.placeholder, "851");
    assert_eq!(input.browse.label, "Browse this port");
    assert!(
        input.browse.enabled,
        "a loaded pane can browse another port"
    );
    let Op::Rust { action, payload } = &input.browse.op else {
        panic!("the typed port's browse is a Rust action");
    };
    assert_eq!(action, "browse.port");
    assert_eq!(payload["pane"], json!("browse:ads"));
    assert_eq!(payload["request"]["port"], json!(851));
}
