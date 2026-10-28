//! Native tests of the Discover pane's follow-ups and advertised finds (specification 25 §2): the tag browser that
//! a successful `Add to canvas` opens on a `TwinCAT` card, and a find of a protocol that a runtime advertises beyond
//! the ten sources. Each expected value is a literal worked out from specification 25 §2 or from the old pane's
//! `useDiscoverPane.ts`; none is read back from the code under test.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::view::discover::{project_in, RuntimeOrigin, Scan};
use crate::view::discover_context::{self, Context, OriginFacts};
use crate::view::types::{DiscoverView, Op, Pane, UiOp};

/// The time of the views, in milliseconds since the Unix epoch.
const NOW_MS: u64 = 1_760_000_000_000;

/// A search status of `sources` and `devices`, for the search `s-1`.
fn status(sources: Value, devices: Value) -> Map<String, Value> {
    let mut body = Map::new();
    body.insert("search_id".to_owned(), json!("s-1"));
    body.insert("state".to_owned(), json!("done"));
    body.insert("sources".to_owned(), sources);
    body.insert("devices".to_owned(), devices);
    body
}

/// The runtime `runtime:cell`, ready to scan.
fn cell() -> Vec<RuntimeOrigin> {
    vec![RuntimeOrigin {
        id: "runtime:cell".to_owned(),
        name: "Cell".to_owned(),
        ready: true,
    }]
}

/// The Discover view of `answer`, scanned from the runtime `runtime:cell` with `ctx`.
fn view_of(answer: &Map<String, Value>, ctx: &Context) -> DiscoverView {
    project_in(
        answer,
        &Scan::Runtime("runtime:cell".to_owned()),
        &cell(),
        ctx,
        NOW_MS,
    )
    .expect("the status reads as a search status")
}

#[test]
fn add_to_canvas_on_a_twincat_card_opens_its_tag_browser() {
    // Specification 25 §2 (old `useDiscoverPane.ts` lines 98-109): a successful `Add to canvas` is followed by the
    // tag browser of the found device. The view carries it as the action's follow-up: the op is `followed`, its
    // `first` is the add to the owning runtime, and its `then` is the browse that `Manage tags` opens, of the
    // device's own target and with no port. A wrong implementation drops the follow-up, makes the browse the first
    // step, or browses another target.
    let params = json!({
        "ams_net_id": "5.6.7.8.1.1",
        "host": "10.0.0.5",
        "name": "PLC-A",
        "responding_ads_ports": [852, 851],
    });
    let device = json!({
        "id": "dev:ads:5.6.7.8.1.1",
        "addresses": ["10.0.0.5"],
        "ams_net_ids": ["5.6.7.8.1.1"],
        "last_seen_ms": NOW_MS - 3000,
        "finds": [{
            "protocol": "ads",
            "source": "ads_identify",
            "confidence": "confirmed",
            "simulated": false,
            "identity": { "kind": "ads", "ams_net_id": "5.6.7.8.1.1", "name": "PLC-A" },
            "params": params,
            "warnings": [],
        }],
    });
    let answer = status(
        json!([{ "id": "ads", "state": "found", "count": 1 }]),
        json!([device]),
    );
    let view = view_of(&answer, &Context::default());
    let card = view.cards.first().expect("the TwinCAT device is a card");
    let add = card
        .actions
        .iter()
        .find(|action| action.id == "add_to_canvas")
        .expect("the card offers Add to canvas");
    assert!(add.enabled, "the owning runtime takes the connection");
    let Op::Followed { first, then } = &add.op else {
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
                    "responding_ads_ports": [852, 851],
                },
            }),
        },
        "the first step adds one connection per responding port to the owning runtime"
    );
    assert_eq!(
        **then,
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
                                "responding_ads_ports": [852, 851],
                            },
                        },
                        "kind": "symbols",
                        "trust_server_certificate": false,
                    }),
                },
            },
        },
        "the follow-up browses the found device's own target, with no port"
    );
}

#[test]
fn an_advertised_find_is_a_card_and_a_row_under_its_advertised_label() {
    // Specification 25 §2: a protocol that the origin runtime advertises beyond the ten sources has its source row and
    // its finds' cards under the label the runtime advertises (`Field X`), and its choice is ticked because the
    // search ran it. A wrong implementation leaves the find out as an unknown protocol, names it by its ID, or
    // raises the update line for a protocol this build does not know.
    let schema = json!({ "protocols": [
        { "id": "fieldx", "label": "Field X", "actions": ["discover"] },
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
    let device = json!({
        "id": "adv:fieldx:unit-1",
        "addresses": ["127.0.0.1"],
        "last_seen_ms": NOW_MS - 1000,
        "finds": [{
            "protocol": "fieldx",
            "source": "fieldx_probe",
            "confidence": "likely",
            "simulated": false,
            "identity": { "name": "Field X unit 1" },
            "params": { "host": "127.0.0.1" },
            "warnings": [],
            "last_seen_ms": NOW_MS - 1000,
        }],
    });
    let answer = status(
        json!([{ "id": "fieldx", "state": "found", "count": 1 }]),
        json!([device]),
    );
    let view = view_of(&answer, &ctx);
    let row = view
        .sources
        .first()
        .expect("the advertised protocol has a row");
    assert_eq!((row.id.as_str(), row.label.as_str()), ("fieldx", "Field X"));
    assert_eq!(row.count, Some(1));
    let card = view.cards.first().expect("the find is a card");
    assert_eq!(card.title, "Field X unit 1");
    assert_eq!(
        card.protocols.first().map(|line| line.name.as_str()),
        Some("Field X"),
        "the card's protocol line names it as advertised"
    );
    assert_eq!(
        view.update_text, None,
        "an advertised protocol is known, so no update line"
    );
    let choice = view
        .only
        .iter()
        .find(|choice| choice.source == "fieldx")
        .expect("the advertised protocol is a choice");
    assert_eq!(
        (choice.enabled, choice.checked, choice.reason.as_deref()),
        (true, true, None),
        "the search ran it, so its choice is ticked and enabled"
    );
}
