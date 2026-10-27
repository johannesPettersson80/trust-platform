//! Native tests of the Discover pane's searches (specification 25 §2): the idle view before the first
//! search, the bodies of Search and Cancel, the text of a failed search, the Bluetooth reason and an
//! unknown confidence. Each expected value is a literal taken from specification 25 §2, from the old
//! pane's words, or from the search catalog; none is read back from the code under test.

use serde_json::{json, Map, Value};

use crate::engine::{DevicesEngine, SearchCall, SearchOrigin, SearchVerb, TypedTargets};
use crate::request::HostFacts;
use crate::view::discover::{self, RuntimeOrigin, Scan};
use crate::view::discover_context::Context;
use crate::view::discover_names;
use crate::view::types::Op;

/// The time of the views, in milliseconds since the Unix epoch.
const NOW_MS: u64 = 1_760_000_000_000;

/// The help above the scan choices (`DiscoverPane.tsx` line 257).
const CHOOSE_WHAT_TO_FIND: &str =
    "Choose what to find, then scan. Only the selected device types are searched.";
/// The reason of the Bluetooth scan in this version of truST (specification 11, "Sources").
const BLUETOOTH: &str = "Bluetooth search is not available in this truST version yet.";
/// The text of an origin that the panel no longer reaches (`discoveryController.ts` line 126).
const NO_LONGER_REACHABLE: &str =
    "The selected runtime is no longer reachable. Start or reconnect it, then scan again.";
/// The text of a runtime that rejected the token (`discoveryErrors.ts` line 8).
const REJECTED: &str = "The selected runtime rejected authentication for truST runtime discovery. Check its auth token. Reconnect or start the selected runtime, then scan again.";
/// The text of a runtime that did not answer in time (`discoveryErrors.ts` line 11).
const TIMED_OUT: &str = "truST runtime discovery timed out on the selected runtime. Reconnect or start the selected runtime, then scan again.";
/// The text of any other failed request to a runtime (`discoveryErrors.ts` line 13).
const COULD_NOT: &str = "The selected runtime could not complete truST runtime discovery. Reconnect or start the selected runtime, then scan again.";
/// The line that asks to update truST (specification 25 §2).
const UPDATE_TRUST: &str = "This search reports a source or protocol that this version of truST does not know. Update truST to see its results.";

/// The Discover view of `status` with nothing remembered beside it: the default context.
fn project(
    status: &Map<String, Value>,
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    now_ms: u64,
) -> Result<crate::view::types::DiscoverView, serde_json::Error> {
    discover::project_in(status, scan, runtimes, &Context::default(), now_ms)
}

/// A status of the search `s-1` in the state given, with the sources and devices of the test.
fn status(state: &str, sources: &Value, devices: &Value) -> Map<String, Value> {
    json!({ "search_id": "s-1", "state": state, "sources": sources, "devices": devices })
        .as_object()
        .cloned()
        .expect("a status is an object")
}

/// A `devicesSearch` request of the panel `p-1`, with no parameters and no typed targets.
fn call(verb: SearchVerb, origin: SearchOrigin, search_id: Option<&str>) -> SearchCall {
    SearchCall {
        session: "p-1".to_owned(),
        verb,
        origin,
        search_id: search_id.map(str::to_owned),
        since: None,
        params: Value::Null,
        targets: None,
        host: HostFacts::default(),
        change: None,
    }
}

/// A runtime that the panel offers as the origin of a search, ready to scan or stopped.
fn runtime(id: &str, name: &str, ready: bool) -> RuntimeOrigin {
    RuntimeOrigin {
        id: id.to_owned(),
        name: name.to_owned(),
        ready,
    }
}

#[test]
fn the_pane_offers_a_search_before_the_first_search() {
    // Specification 25 §2: a status that names no search answers the idle view, which offers Search
    // with ADS and TwinCAT ticked on the first run. A wrong implementation answers an error, leaves
    // Search disabled, or ticks nothing, so the pane has nothing to search from.
    let reply =
        DevicesEngine::new().search(&call(SearchVerb::Status, SearchOrigin::ThisHost, None));
    assert!(reply.error.is_none(), "the idle view is not an error");
    let view = reply.discover.expect("the idle view");
    assert_eq!(view.search_id, None, "no search has run");
    assert!(!view.running, "no search runs before the first search");
    assert!(
        view.sources.is_empty() && view.cards.is_empty(),
        "nothing has been found yet"
    );
    assert_eq!(
        view.empty_text, None,
        "the empty result is drawn only after a search"
    );
    assert_eq!(view.choose_text.as_deref(), Some(CHOOSE_WHAT_TO_FIND));
    let search = &view.actions[0];
    assert_eq!(
        (search.id.as_str(), search.label.as_str(), search.enabled),
        ("search", "Search", true),
        "Search is offered"
    );
    assert_eq!(
        search.op,
        Op::Search {
            request: json!({
                "verb": "start",
                "origin": "this_host",
                "params": { "only": ["ads"] },
            }),
        },
        "Search starts from this computer with ADS, the first-run tick"
    );
}

#[test]
fn a_stopped_runtime_origin_offers_no_search_until_it_starts() {
    // Specification 25 §2: every scan from a stopped runtime is disabled with its reason, so no source
    // is ticked and Search is disabled with the reason that names the scan choice. A wrong
    // implementation ticks ADS on a stopped runtime, which then searches nothing.
    let view = discover::idle(
        &Scan::Runtime("rt-1".to_owned()),
        &[runtime("rt-1", "Line A", false)],
        &Context::default(),
    );
    let ads = view
        .only
        .iter()
        .find(|choice| choice.source == "ads")
        .expect("the ADS choice");
    assert!(!ads.enabled, "ADS cannot run from a stopped runtime");
    assert_eq!(
        ads.reason.as_deref(),
        Some("Start or connect Line A before scanning from it.")
    );
    assert!(!view.actions[0].enabled, "Search is disabled");
    assert_eq!(
        view.actions[0].reason.as_deref(),
        Some("Select at least one available scan type.")
    );
}

#[test]
fn search_and_cancel_post_the_search_command_with_the_chosen_scope() {
    // Specification 25 §2 (the request bodies): Search starts at the chosen origin with the ticked
    // sources, and Cancel names the running search and its origin. Each body is read back as the
    // `trust-lsp.devicesSearch` request of its panel, which proves the wire shape. A wrong
    // implementation sends the origin as a bare item ID, which the engine refuses, or leaves the
    // ticked sources out, which the engine then ignores.
    let scan = Scan::Runtime("rt-1".to_owned());
    let line_a = [runtime("rt-1", "Line A", true)];
    let running = project(
        &status(
            "running",
            &json!([{ "id": "ads", "state": "searching", "count": 0 }]),
            &json!([]),
        ),
        &scan,
        &line_a,
        NOW_MS,
    )
    .expect("a running status");
    let Op::Search { request: cancel } = &running.actions[1].op else {
        panic!("Cancel is a search operation");
    };
    assert_eq!(
        cancel,
        &json!({ "verb": "cancel", "origin": { "runtime": "rt-1" }, "search_id": "s-1" }),
        "Cancel names the running search and the runtime it runs in"
    );

    let finished = project(
        &status(
            "done",
            &json!([{ "id": "ads", "state": "found", "count": 1 }]),
            &json!([]),
        ),
        &scan,
        &line_a,
        NOW_MS,
    )
    .expect("a finished status");
    let Op::Search { request: start } = &finished.actions[0].op else {
        panic!("Search is a search operation");
    };
    assert_eq!(
        start,
        &json!({
            "verb": "start",
            "origin": { "runtime": "rt-1" },
            "params": { "only": ["ads"] },
        }),
        "Search starts in the runtime with the ticked source ADS"
    );

    let mut cancel_body = cancel.clone();
    cancel_body["session"] = json!("p-1");
    let cancel_call: SearchCall =
        serde_json::from_value(cancel_body).expect("the engine reads the cancel body");
    assert_eq!(
        (
            cancel_call.verb,
            cancel_call.origin,
            cancel_call.search_id.as_deref()
        ),
        (
            SearchVerb::Cancel,
            SearchOrigin::Runtime("rt-1".to_owned()),
            Some("s-1")
        ),
        "the engine reads the cancel as the cancel of s-1 at the runtime"
    );
    let mut start_body = start.clone();
    start_body["session"] = json!("p-1");
    let start_call: SearchCall =
        serde_json::from_value(start_body).expect("the engine reads the start body");
    assert_eq!(
        (start_call.verb, start_call.params),
        (SearchVerb::Start, json!({ "only": ["ads"] })),
        "the engine reads the ticked source as the start's parameter"
    );
}

#[test]
fn a_failed_search_shows_the_old_panes_error_text() {
    // Specification 25 §2: an origin that the panel no longer shows fails with `unknown_runtime`. The
    // reply keeps its error, and the view carries the old pane's text (`discoveryController.ts` line
    // 126), word for word. A wrong implementation answers the bare error, or a text of its own.
    let reply = DevicesEngine::new().search(&call(
        SearchVerb::Start,
        SearchOrigin::Runtime("rt-gone".to_owned()),
        None,
    ));
    let error = reply.error.expect("the search fails");
    assert_eq!(error.code, "unknown_runtime");
    let view = reply.discover.expect("the failed search carries its view");
    assert_eq!(view.error_text.as_deref(), Some(NO_LONGER_REACHABLE));
}

#[test]
fn a_failed_cancel_keeps_only_its_error_reply() {
    // Specification 25 §2: the view of a failed search is the search's own; a cancel has none, so it
    // keeps its error reply alone. A wrong implementation draws a Discover view on the cancel.
    let reply = DevicesEngine::new().search(&call(
        SearchVerb::Cancel,
        SearchOrigin::Runtime("rt-gone".to_owned()),
        Some("s-1"),
    ));
    let error = reply.error.expect("the cancel fails");
    assert_eq!(error.code, "unknown_runtime");
    assert!(reply.discover.is_none(), "a failed cancel draws no view");
}

#[test]
fn a_runtime_that_rejects_the_token_shows_the_old_auth_text() {
    // Specification 25 §2 with `discoveryErrors.ts` line 8: a failed request whose message names the
    // authentication shows the authentication text. A wrong implementation shows the generic text.
    assert_eq!(
        discover_names::failure_text("runtime_unreachable", "the runtime rejected the auth token")
            .as_deref(),
        Some(REJECTED)
    );
}

#[test]
fn a_runtime_that_times_out_shows_the_old_timeout_text() {
    // Specification 25 §2 with `discoveryErrors.ts` line 11: a failed request whose message names a
    // timeout shows the timeout text. A wrong implementation shows the generic text.
    assert_eq!(
        discover_names::failure_text("runtime_unreachable", "the request timed out").as_deref(),
        Some(TIMED_OUT)
    );
}

#[test]
fn a_runtime_that_cannot_be_reached_shows_the_old_generic_text() {
    // Specification 25 §2 with `discoveryErrors.ts` line 13: any other failed request to a runtime
    // shows the generic text. A wrong implementation shows the authentication or timeout text.
    assert_eq!(
        discover_names::failure_text("runtime_unreachable", "connection refused").as_deref(),
        Some(COULD_NOT)
    );
}

#[test]
fn a_failure_the_old_pane_had_no_text_for_has_none() {
    // Specification 25 §2: a failure with no old text, such as a busy registry, keeps only its error
    // reply. A wrong implementation invents a text for it.
    assert_eq!(
        discover_names::failure_text("search_busy", "eight searches run"),
        None
    );
}

#[test]
fn the_bluetooth_reason_is_the_catalogs() {
    // Specification 25 §2: the Bluetooth reason is the one that trust-runtime's search catalog gives,
    // read from that catalog rather than copied. A wrong implementation keeps its own copy, which can
    // drift from the catalog, or leaves Bluetooth without a reason.
    let view = discover::idle(&Scan::ThisHost, &[], &Context::default());
    let bluetooth = view
        .only
        .iter()
        .find(|choice| choice.source == "bluetooth")
        .expect("the Bluetooth choice");
    assert!(!bluetooth.enabled, "Bluetooth cannot be scanned");
    assert_eq!(
        bluetooth.reason.as_deref(),
        Some(trust_runtime::search::BLUETOOTH_UNAVAILABLE),
        "the reason is the catalog's"
    );
    assert_eq!(
        bluetooth.reason.as_deref(),
        Some(BLUETOOTH),
        "and its words are the catalog's"
    );
}

#[test]
fn an_unknown_confidence_is_left_out() {
    // Specification 25 §2: a find whose confidence this build does not know is left out, as an unknown
    // protocol is, and the view shows the update line. A device whose finds are all left out has no
    // card. A wrong implementation fails the whole view on the value, or keeps the find with a word of
    // its own.
    let answer = status(
        "done",
        &json!([{ "id": "modbus_tcp", "state": "found", "count": 2 }]),
        &json!([
            {
                "id": "dev-a",
                "addresses": ["10.0.0.5:502"],
                "last_seen_ms": NOW_MS,
                "finds": [{
                    "protocol": "modbus_tcp",
                    "source": "tcp_connect",
                    "confidence": "certain",
                    "params": { "host": "10.0.0.5" },
                }],
            },
            {
                "id": "dev-b",
                "addresses": ["10.0.0.6:502"],
                "last_seen_ms": NOW_MS,
                "finds": [{
                    "protocol": "modbus_tcp",
                    "source": "tcp_connect",
                    "confidence": "likely",
                    "params": { "host": "10.0.0.6" },
                }],
            },
        ]),
    );
    let view = project(&answer, &Scan::ThisHost, &[], NOW_MS).expect("the view is built");
    let ids: Vec<&str> = view.cards.iter().map(|card| card.id.as_str()).collect();
    assert_eq!(
        ids,
        ["dev-b"],
        "the device with only an unknown confidence is left out"
    );
    let confidences: Vec<&str> = view.cards[0]
        .protocols
        .iter()
        .map(|protocol| protocol.confidence.as_str())
        .collect();
    assert_eq!(
        confidences,
        ["Likely"],
        "the known confidence keeps its word"
    );
    assert_eq!(view.update_text.as_deref(), Some(UPDATE_TRUST));
}

#[test]
fn a_typed_known_address_that_is_not_a_host_is_refused_before_any_source_runs() {
    // Specification 25 §2 and specification 11 ("Requests"): the Known address is a host name or IP
    // address with an optional port. The typed text reaches the search as `targets.hosts`, which
    // refuses it with `invalid_search` before any source runs. A wrong implementation drops the typed
    // text, and the search starts.
    let mut request = call(SearchVerb::Start, SearchOrigin::ThisHost, None);
    request.targets = Some(TypedTargets {
        hosts: "not a host!".to_owned(),
        cidr: String::new(),
    });
    let reply = DevicesEngine::new().search(&request);
    let error = reply.error.expect("the search refuses the typed address");
    assert_eq!(error.code, "invalid_search");
}
