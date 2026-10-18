//! The projection tests (design §4.3 B8 and §3.3 J44, J47, J50 and J53; specification 25 §5.4 and
//! §11.2): the raw protocol identity of a leaf, the structured failures and their one recovery,
//! the `Add` button's reasons, and the trust-certificate flag that only `Add` saves. Their
//! inputs and helpers are in the parent, which every one of these suites shares.

use std::time::Instant;

use serde_json::json;

use super::{
    add_request, answer, browse_body, browse_request, connection_protocol, endpoint, failed_answer,
    inputs, leaf, opcua_params, open_live, report, requests_of, script, seen, wait_for_browse,
};
use crate::actions::act;
use crate::browse::open;
use crate::fake_runtime::FakeRuntime;
use crate::targets::TargetOrigin;
use crate::view::tests::fixtures as fx;
use crate::view::types::{AddButton, BrowseView, Op, Pane, TreeNode, UiOp};

/// The Add button of a pane that has one.
fn add_button(view: &BrowseView) -> &AddButton {
    view.add.as_ref().expect("a pane with an Add button")
}

#[test]
fn opcua_leaves_keep_their_raw_node_ids() {
    // Specification 25 §11.2: a leaf's raw NodeId is its protocol identity; leaves with equal
    // names or paths but different raw NodeIds stay distinct and preserve browse order, and a leaf
    // without a raw NodeId cannot be saved. A wrong implementation keys leaves by the sanitized id
    // or the path, so equal ones merge, or makes a leaf without a NodeId selectable.
    let view = open_live(
        "opcua_client",
        &opcua_params("opc.tcp://line:4840"),
        answer(
            "opcua_client",
            "nodes",
            json!([
                leaf(
                    "opcua:node:one",
                    Some("ns=2;i=1"),
                    "Temperature",
                    "Objects.Temperature",
                    "double",
                    Some(true)
                ),
                leaf(
                    "opcua:node:two",
                    Some("ns=2;i=2"),
                    "Temperature",
                    "Objects.Temperature",
                    "double",
                    Some(true)
                ),
                leaf(
                    "opcua:node:three",
                    None,
                    "Anonymous",
                    "Objects.Anonymous",
                    "double",
                    Some(false)
                ),
            ]),
        ),
    );

    assert_eq!(view.tree[0].key, "ns=2;i=1");
    assert_eq!(view.tree[1].key, "ns=2;i=2");
    assert_eq!(view.tree[0].path, view.tree[1].path);
    assert_ne!(view.tree[0].key, view.tree[1].key);
    assert_eq!(view.tree[0].access.as_deref(), Some("read/write"));
    assert!(view.tree[0].selectable);
    assert_eq!(view.tree[2].key, "opcua:node:three");
    assert!(
        !view.tree[2].selectable,
        "a leaf without a NodeId cannot be saved"
    );
}

#[test]
fn every_protocol_browse_failure_is_structured() {
    // Design §4.3 B8, I31: every protocol's browse failure is a structured error, never an empty
    // successful tree and never a raw status token. A wrong implementation answers an empty tree
    // on a failure, or copies the code into the human text.
    let cases = [
        ("opcua_client", opcua_params("opc.tcp://line:4840")),
        ("ethercat", json!({})),
        (
            "opcua",
            json!({ "listen": "0.0.0.0", "expose": [], "enabled": true }),
        ),
    ];

    for (protocol, params) in cases {
        let view = open_live(protocol, &params, failed_answer("endpoint_unreachable"));
        let error = view
            .error
            .unwrap_or_else(|| panic!("{protocol} answered a structured error"));
        assert_eq!(view.tree, Vec::<TreeNode>::new(), "{protocol} has no tree");
        assert!(!error.title.is_empty(), "{protocol} has a human title");
        assert_ne!(
            error.title, "endpoint_unreachable",
            "{protocol} shows no code"
        );
        assert!(!error.detail.is_empty(), "{protocol} has a human detail");
    }
}

#[test]
fn opcua_error_boxes_with_recovery_actions() {
    // Specification 25 §5.4 and §11.2, and `classifyOpcuaBrowseError` (`W/opcuaClientModel.ts:157
    // -203`): every OPC UA failure has a human title and detail and at most one recovery;
    // authentication offers `Edit credentials`, which reopens the connection's form with the
    // failed target. A wrong implementation offers the wrong recovery, none where the panel needs
    // one, or a raw code.
    let cert = open_live(
        "opcua_client",
        &opcua_params("opc.tcp://line:4840"),
        failed_answer("cert_untrusted"),
    )
    .error
    .expect("a structured error");
    assert_eq!(cert.title, "Server certificate not trusted");
    assert_eq!(
        cert.detail,
        "Confirm this is the expected server, then choose Trust certificate to browse it."
    );
    let trust = cert.recovery.expect("a recovery");
    assert_eq!(trust.label, "Trust certificate");
    assert!(matches!(
        trust.op,
        Op::Ui {
            ui: UiOp::Open {
                pane: Pane::Browse { .. }
            }
        }
    ));

    let auth = open_live(
        "opcua_client",
        &opcua_params("opc.tcp://line:4840"),
        failed_answer("auth_required"),
    )
    .error
    .expect("a structured error");
    assert_eq!(auth.title, "Authentication required");
    assert_eq!(
        auth.detail,
        "Choose username authentication or update the saved OPC UA credentials, then browse again."
    );
    let credentials = auth.recovery.expect("a recovery");
    assert_eq!(credentials.label, "Edit credentials");
    match credentials.op {
        Op::Rust { action, payload } => {
            assert_eq!(action, "form.open");
            assert_eq!(payload["protocol"], json!("opcua_client"));
            assert_eq!(
                payload["prefill"]["endpoint_url"],
                json!("opc.tcp://line:4840"),
                "the failed target is filled in"
            );
        }
        other => panic!("a Rust form.open, got {other:?}"),
    }

    for code in [
        "unsupported_security_profile",
        "endpoint_unreachable",
        "browse_denied",
    ] {
        let error = open_live(
            "opcua_client",
            &opcua_params("opc.tcp://line:4840"),
            failed_answer(code),
        )
        .error
        .expect("a structured error");
        assert!(error.recovery.is_none(), "{code} offers no recovery");
        assert!(!error.title.is_empty(), "{code} has a human title");
    }
}

#[test]
fn add_button_reasons_ladder() {
    // Design §3.3 J53 and `BrowseTagsPanel.tsx:95-153`: `Add` is disabled with the first reason of
    // today's ladder: while the browse runs, after a failure, and when nothing is available. The
    // tick boxes stay in the webview (design §3.4), so the last rung is `no_selection_reason`. A
    // wrong implementation enables `Add` while the tree is empty or keeps a stale reason.
    let runtime = FakeRuntime::start(script(answer(
        "opcua_client",
        "nodes",
        json!([leaf(
            "opcua:node:a",
            Some("ns=2;i=1"),
            "Temperature",
            "Objects.Temperature",
            "double",
            Some(false)
        )]),
    )));
    let topology = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "connected",
            vec![endpoint(
                "line",
                "opcua_client",
                None,
                "line1",
                &opcua_params("opc.tcp://line:4840"),
            )],
        ),
    );
    let (mut panel, view) = seen(
        inputs(None),
        vec![fx::answered(
            TargetOrigin::Project,
            &runtime.endpoint,
            None,
            fx::status("running", "admin"),
            topology,
        )],
    );
    let connection = connection_protocol(&view, "line", "opcua_client")
        .id
        .as_str()
        .to_owned();
    let body = browse_body(&connection, "line1", &runtime.endpoint, 1, false);
    let request = browse_request(&body);

    let loading = open(panel.session("dc-test", Instant::now()), &request);
    assert!(!add_button(&loading).enabled);
    assert_eq!(
        add_button(&loading).reason.as_deref(),
        Some("Wait for browse results before adding tags.")
    );

    wait_for_browse(&mut panel);
    let ready = open(panel.session("dc-test", Instant::now()), &request);
    assert!(
        add_button(&ready).enabled,
        "a loaded tree can be added from"
    );
    assert_eq!(add_button(&ready).reason, None);
    assert_eq!(
        add_button(&ready).no_selection_reason,
        "Select at least one symbol to add."
    );

    let empty = open_live(
        "opcua_client",
        &opcua_params("opc.tcp://line:4840"),
        answer("opcua_client", "nodes", json!([])),
    );
    assert!(!add_button(&empty).enabled);
    assert_eq!(
        add_button(&empty).reason.as_deref(),
        Some("No symbols are available to add.")
    );

    let failed = open_live(
        "opcua_client",
        &opcua_params("opc.tcp://line:4840"),
        failed_answer("auth_required"),
    );
    assert!(!add_button(&failed).enabled);
    assert_eq!(
        add_button(&failed).reason.as_deref(),
        Some("Resolve the browse error before adding tags.")
    );
}

#[test]
fn trust_certificate_is_saved_only_by_add() {
    // Design §3.3 J50 and specification 25 §11.2: the trust-server-certificate flag is used to
    // browse and is saved only by `Add`. A wrong implementation writes the flag while opening the
    // browse, or drops it from the saved connection.
    let runtime = FakeRuntime::start(script(answer(
        "opcua_client",
        "nodes",
        json!([leaf(
            "opcua:node:a",
            Some("ns=2;i=1"),
            "Temperature",
            "Temperature",
            "double",
            Some(false)
        )]),
    )));
    let topology = report(
        "host:line",
        "line-pc",
        &fx::runtime(
            "line",
            "line",
            "connected",
            vec![endpoint(
                "line",
                "opcua_client",
                None,
                "line1",
                &opcua_params("opc.tcp://line:4840"),
            )],
        ),
    );
    let (mut panel, view) = seen(
        inputs(None),
        vec![fx::answered(
            TargetOrigin::Project,
            &runtime.endpoint,
            None,
            fx::status("running", "admin"),
            topology,
        )],
    );
    let connection = connection_protocol(&view, "line", "opcua_client")
        .id
        .as_str()
        .to_owned();

    let trusted = browse_body(&connection, "line1", &runtime.endpoint, 1, true);
    let _ = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&trusted),
    );
    wait_for_browse(&mut panel);
    assert_eq!(
        runtime.count("comm.apply"),
        0,
        "opening a browse saves nothing"
    );
    let sent = requests_of(&runtime, "comm.browse_symbols");
    assert_eq!(
        sent[0]["params"]["target"]["trust_server_certificate"],
        json!(true),
        "the flag reaches the browse"
    );

    let reply = act(
        panel.session("dc-test", Instant::now()),
        add_request(&trusted, "browse:opcua_client", &["ns=2;i=1"], false),
    );
    assert!(reply.ok);
    let applied = requests_of(&runtime, "comm.apply");
    assert_eq!(applied.len(), 1, "Add writes one connection");
    assert_eq!(
        applied[0]["params"]["params"]["connections"][0]["trust_server_certificate"],
        json!(true),
        "Add saves the trusted flag"
    );

    let untrusted = browse_body(&connection, "line1", &runtime.endpoint, 2, false);
    let _ = open(
        panel.session("dc-test", Instant::now()),
        &browse_request(&untrusted),
    );
    wait_for_browse(&mut panel);
    let reply = act(
        panel.session("dc-test", Instant::now()),
        add_request(&untrusted, "browse:opcua_client", &["ns=2;i=1"], false),
    );
    assert!(reply.ok);
    let applied = requests_of(&runtime, "comm.apply");
    assert_eq!(applied.len(), 2, "a second Add writes a second connection");
    assert_eq!(
        applied[1]["params"]["params"]["connections"][0]["trust_server_certificate"],
        json!(false),
        "Add saves the flag it was given"
    );
}
