//! The typed ADS port of a browse pane (design §3.3 J59 and D5b R3; specification 25 §5.4 and its
//! lines 880-883): `browse.port` reads the typed text, answers its message in the pane when the text
//! names no port, and starts the browse of a valid port, whose card joins the pane on its next poll.
//! Every expected value is written out, taken from the panel the rule came from
//! (`webview/adsTargetPort.ts`, `AdsMultiPortTagBrowser.tsx`). The origin is the scripted fake
//! runtime, so nothing reaches the network.

use std::time::Instant;

use serde_json::{json, Value};

use super::{
    browse_body, browse_request, connection_protocol, endpoint, inputs, report, script, seen,
    wait_for_browse,
};
use crate::actions::act;
use crate::browse::open;
use crate::fake_runtime::FakeRuntime;
use crate::request::{ActOp, ActRequest, BrowseRequest, HostFacts};
use crate::session::Sessions;
use crate::targets::TargetOrigin;
use crate::view::tests::fixtures as fx;
use crate::view::types::{BrowseView, Op};

/// A session that has seen one ADS connection of a single-runtime project, the browse request that
/// reads its target port, and the scripted runtime that answers every browse with `answer`.
fn ads_setup(answer: Value) -> (Sessions, BrowseRequest, FakeRuntime) {
    let runtime = FakeRuntime::start(script(answer));
    let params = json!({
        "name": "line1", "host": "192.168.10.5", "target_net_id": "5.23.91.12.1.1",
        "ams_port": 851, "points": [],
    });
    let topology = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "connected",
            vec![endpoint("line", "ads", None, "line1", &params)],
        ),
    );
    let (panel, view) = seen(
        inputs(None),
        vec![fx::answered(
            TargetOrigin::Project,
            &runtime.endpoint,
            None,
            fx::status("running", "admin"),
            topology,
        )],
    );
    let connection = connection_protocol(&view, "line", "ads")
        .id
        .as_str()
        .to_owned();
    let body = browse_body(&connection, "line1", &runtime.endpoint, 1, false);
    (panel, browse_request(&body), runtime)
}

/// The `browse.port` action of the pane `view`, as the webview posts it: its payload, which the
/// view built, and the typed text as its `port` value.
fn typed_port_request(view: &BrowseView, typed: &str) -> ActRequest {
    let action = view
        .port_input
        .as_ref()
        .expect("an ADS pane has a typed port")
        .browse
        .clone();
    let Op::Rust { payload, .. } = action.op else {
        panic!("browse this port is a Rust action");
    };
    let mut values = serde_json::Map::new();
    values.insert("port".to_owned(), json!(typed));
    ActRequest {
        session: "dc-test".to_owned(),
        op: ActOp {
            action: "browse.port".to_owned(),
            payload,
        },
        values,
        project: None,
        host: HostFacts::default(),
    }
}

#[test]
fn a_typed_port_that_names_no_port_answers_its_message_in_the_pane() {
    // Specification 25 §5.4 and its lines 880-883, the rule of `webview/adsTargetPort.ts:9-22`: a
    // text that is not whole digits answers `Enter a whole-number ADS port from 1 to 65535.`, and
    // a port outside 1 through 65,535 answers `ADS port must be between 1 and 65535.`. Both
    // messages stay in the pane that started the action, and nothing is browsed. A wrong
    // implementation browses the text as a port, answers the other message, or names the wrong pane.
    let (mut panel, request, _runtime) = ads_setup(json!({
        "schema_version": 1, "protocol": "ads", "kind": "symbols", "tree": [],
    }));
    let view = open(panel.session("dc-test", Instant::now()), &request);

    let reply = act(
        panel.session("dc-test", Instant::now()),
        typed_port_request(&view, "3x1"),
    );

    assert!(!reply.ok, "a text that names no port is refused");
    let notice = reply.notice.expect("the refusal is a notice");
    assert_eq!(
        notice.pane, "browse:ads",
        "the message stays in the browse pane"
    );
    assert_eq!(
        notice.text,
        "Enter a whole-number ADS port from 1 to 65535."
    );

    let reply = act(
        panel.session("dc-test", Instant::now()),
        typed_port_request(&view, "65536"),
    );
    assert!(!reply.ok);
    assert_eq!(
        reply.notice.map(|notice| notice.text),
        Some("ADS port must be between 1 and 65535.".to_owned()),
        "a port above 65,535 answers the range message"
    );
}

#[test]
fn a_typed_port_starts_its_browse_and_its_card_joins_the_pane() {
    // Design D5b R3 and design §3.3 J55: the typed port 301 is browsed in the background, and the
    // pane (which reads port 851) shows port 301's card with its tags on its next poll, as the old
    // panel's merged port views did (`AdsMultiPortTagBrowser.tsx:73-145`). A wrong implementation
    // drops the card, browses the pane's own port, or answers the action as an error.
    let (mut panel, request, _runtime) = ads_setup(json!({
        "schema_version": 1, "protocol": "ads", "kind": "symbols",
        "tree": [{
            "id": "ads:symbol:MAIN.Speed", "name": "Speed", "path": "MAIN.Speed",
            "type": "REAL", "writable": false,
        }],
    }));
    let _ = open(panel.session("dc-test", Instant::now()), &request);
    wait_for_browse(&mut panel);
    let view = open(panel.session("dc-test", Instant::now()), &request);

    let reply = act(
        panel.session("dc-test", Instant::now()),
        typed_port_request(&view, "301"),
    );
    assert!(reply.ok, "a valid port starts its browse");
    assert!(
        reply.notice.is_none(),
        "a started browse says nothing: its card shows the state"
    );

    wait_for_browse(&mut panel);
    let later = open(panel.session("dc-test", Instant::now()), &request);
    let card = later
        .ports
        .iter()
        .find(|card| card.port == 301)
        .expect("the typed port's card joins the pane");
    assert_eq!(card.state_text, "1 tag");
    assert_eq!(card.tags[0].name, "Speed");
    assert_eq!(card.tags[0].path, "MAIN.Speed");
    assert_eq!(
        later
            .ports
            .iter()
            .map(|card| card.port)
            .collect::<Vec<u16>>(),
        vec![301, 851],
        "the pane's own port and the typed port are the two cards"
    );
}
