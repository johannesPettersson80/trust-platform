//! The browse panes' projection (design §4.3 B8; specification 25 §5.4 and §11.2): from the typed
//! answer of the origin that owns the connection, the named connection and what it already has,
//! to the contract's [`BrowseView`](crate::view::types::BrowseView).
//!
//! This module is pure: it reads no socket, file or clock. The rules it holds are the ported
//! TypeScript behaviours, named per function:
//!
//! - [`capability`]: which protocol offers a browse, and of what kind (`W/browseActions.ts:18-35`).
//! - the tree projection: the raw protocol identity of a leaf, its access label, and whether the
//!   connection already has it (`W/opcuaClientModel.ts:81-106`; specification 25 §11.2).
//! - the `Add` button and the write toggle: the ladder of reasons it cannot be chosen
//!   (`W/BrowseTagsPanel.tsx:95-153`).
//! - the error box: one human title, one recovery (`W/opcuaClientModel.ts:157-203`).
//! - the ADS tag browser's ports: `ads`, one card per ADS port.

mod ads;

use serde_json::{json, Map, Value};

use crate::actions::plans::global_name;
use crate::browse::{Browsed, BrowsedError, BrowsedNode};
use crate::request::BrowseRequest;
use crate::texts::browse as texts;
use crate::view::types::{
    Action, AddButton, BrowseKind, BrowseView, Emphasis, ErrorBox, ItemId, Op, Pane, Toggle,
    TreeNode, UiOp,
};

/// What one browse pane is projected from: the named connection and the request that opened it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Browse<'a> {
    /// What the pane browses.
    pub(crate) kind: BrowseKind,
    /// The connection's protocol.
    pub(crate) protocol: &'a str,
    /// The connection's own name, drawn as the pane's subtitle.
    pub(crate) name: &'a str,
    /// The connection's configuration, which tells what it already has.
    pub(crate) params: &'a Map<String, Value>,
    /// The runtime that owns the connection, for a recovery that reopens its form.
    pub(crate) runtime: Option<&'a ItemId>,
    /// The ADS port this browse read, for the card it belongs to (design §3.3 J55).
    pub(crate) port: u16,
    /// The request that opened the pane.
    pub(crate) request: &'a BrowseRequest,
}

/// What a protocol offers as a browse (`browseAction`, `W/browseActions.ts:18-35`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Capability {
    /// What the pane shows.
    pub(crate) kind: BrowseKind,
    /// The `comm.browse_symbols` kind the origin is asked for.
    pub(crate) wire_kind: &'static str,
    /// Whether it browses the project's own globals rather than a remote origin.
    pub(crate) local: bool,
}

/// The browse that `protocol` offers, or `None` when it offers none.
#[must_use]
pub(crate) fn capability(protocol: &str) -> Option<Capability> {
    match protocol {
        "opcua_client" => Some(Capability {
            kind: BrowseKind::Nodes,
            wire_kind: "nodes",
            local: false,
        }),
        "ethercat" => Some(Capability {
            kind: BrowseKind::Channels,
            wire_kind: "channels",
            local: false,
        }),
        "ads" => Some(Capability {
            kind: BrowseKind::AdsTags,
            wire_kind: "symbols",
            local: false,
        }),
        "opcua" | "ads_server" => Some(Capability {
            kind: BrowseKind::Expose,
            wire_kind: "symbols",
            local: true,
        }),
        _ => None,
    }
}

/// The pane kind of a browse that names no browsable connection: what the request asked for.
#[must_use]
pub(crate) fn kind_of_request(kind: &str) -> BrowseKind {
    match kind.trim().to_ascii_lowercase().as_str() {
        "nodes" => BrowseKind::Nodes,
        "channels" => BrowseKind::Channels,
        _ => BrowseKind::Expose,
    }
}

/// The pane's identity, by protocol. The ADS route action's payload names it, so that its result
/// returns to the pane that started it (design §4.3 B7 and B8; specification 25 §5.4).
#[must_use]
pub(super) fn pane_id(protocol: &str) -> String {
    format!("browse:{protocol}")
}

/// The pane's title, by kind.
#[must_use]
fn title(kind: BrowseKind) -> &'static str {
    match kind {
        BrowseKind::Nodes => texts::NODES_TITLE,
        BrowseKind::Channels => texts::CHANNELS_TITLE,
        BrowseKind::Expose => texts::EXPOSE_TITLE,
        BrowseKind::AdsTags => texts::ADS_TITLE,
    }
}

/// The `Add` button's label, by kind.
#[must_use]
fn add_label(kind: BrowseKind) -> &'static str {
    match kind {
        BrowseKind::Nodes => texts::NODES_ADD,
        BrowseKind::Channels => texts::CHANNELS_ADD,
        BrowseKind::Expose => texts::EXPOSE_ADD,
        BrowseKind::AdsTags => texts::ADS_ADD,
    }
}

/// The whole pane for `data` and the origin's `answer`: `None` while the browse still runs, the
/// origin's error, or its tree (design §1.5, §4.3 B8).
#[must_use]
pub(crate) fn view(data: Browse<'_>, answer: Option<&Result<Browsed, String>>) -> BrowseView {
    let tags = data.kind == BrowseKind::AdsTags;
    // An ADS failure — a typed wire error or an unreached origin — belongs to the port that
    // reported it, never to the whole pane.
    let error = if tags { None } else { error_box(&data, answer) };
    let tree = match answer {
        Some(Ok(browsed)) if browsed.error.is_none() && !tags => project_tree(&browsed.tree, &data),
        _ => Vec::new(),
    };
    let reason = ladder(answer);
    let add = AddButton {
        label: add_label(data.kind).to_owned(),
        enabled: reason.is_none(),
        reason: reason.clone(),
        no_selection_reason: texts::NO_SELECTION.to_owned(),
    };
    let write_toggle = Toggle {
        label: texts::ALLOW_WRITES.to_owned(),
        enabled: add.enabled,
        reason,
    };
    let (ports, port_suggestions, port_help) = if tags {
        (
            ads::ports(&data, answer),
            ads::suggestions(),
            texts::ADS_PORT_HELP.to_owned(),
        )
    } else {
        (Vec::new(), Vec::new(), String::new())
    };
    BrowseView {
        id: pane_id(data.protocol),
        title: title(data.kind).to_owned(),
        subtitle: data.name.to_owned(),
        kind: data.kind,
        loading: answer.is_none(),
        tree,
        ports,
        port_suggestions,
        port_help,
        error,
        add,
        write_toggle,
        notice: None,
    }
}

/// The pane of a browse that never reached the origin: the failure is the whole answer.
#[must_use]
pub(crate) fn failure(request: &BrowseRequest, kind: BrowseKind, error: ErrorBox) -> BrowseView {
    let reason = texts::RESOLVE_ERROR.to_owned();
    BrowseView {
        id: format!("browse:{}", request.source.connection().unwrap_or_default()),
        title: title(kind).to_owned(),
        subtitle: request
            .source
            .connection_name()
            .unwrap_or_default()
            .to_owned(),
        kind,
        loading: false,
        tree: Vec::new(),
        ports: Vec::new(),
        port_suggestions: Vec::new(),
        port_help: String::new(),
        error: Some(error),
        add: AddButton {
            label: add_label(kind).to_owned(),
            enabled: false,
            reason: Some(reason.clone()),
            no_selection_reason: texts::NO_SELECTION.to_owned(),
        },
        write_toggle: Toggle {
            label: texts::ALLOW_WRITES.to_owned(),
            enabled: false,
            reason: Some(reason),
        },
        notice: None,
    }
}

/// Why `Add` cannot be chosen, in today's ladder order (`W/BrowseTagsPanel.tsx:95-153`), or `None`
/// when it can. The tick boxes are the webview's own state, so the last rung is
/// [`AddButton.no_selection_reason`](crate::view::types::AddButton): the webview disables `Add`
/// with that reason while nothing is ticked.
fn ladder(answer: Option<&Result<Browsed, String>>) -> Option<String> {
    match answer {
        Some(Err(_)) => Some(texts::RESOLVE_ERROR.to_owned()),
        Some(Ok(browsed)) if browsed.error.is_some() => Some(texts::RESOLVE_ERROR.to_owned()),
        Some(Ok(browsed)) if browsed.tree.is_empty() => Some(texts::NO_SYMBOLS.to_owned()),
        Some(Ok(_)) => None,
        None => Some(texts::WAIT_FOR_RESULTS.to_owned()),
    }
}

/// The error box of the origin's answer, or `None` while it runs or answered a tree.
fn error_box(data: &Browse<'_>, answer: Option<&Result<Browsed, String>>) -> Option<ErrorBox> {
    match answer {
        Some(Ok(browsed)) => browsed.error.as_ref().map(|error| classify(data, error)),
        Some(Err(message)) => Some(ErrorBox {
            title: texts::browse_failed(data.protocol),
            detail: message.clone(),
            recovery: None,
        }),
        None => None,
    }
}

/// One human error box for a typed protocol error and its one recovery action
/// (`classifyOpcuaBrowseError`, `W/opcuaClientModel.ts:157-203`).
fn classify(data: &Browse<'_>, error: &BrowsedError) -> ErrorBox {
    match error.code.as_str() {
        "cert_untrusted" => ErrorBox {
            title: texts::CERT_UNTRUSTED_TITLE.to_owned(),
            detail: texts::CERT_UNTRUSTED_DETAIL.to_owned(),
            recovery: Some(trust_recovery(data)),
        },
        "auth_required" => ErrorBox {
            title: texts::AUTH_REQUIRED_TITLE.to_owned(),
            detail: texts::AUTH_REQUIRED_DETAIL.to_owned(),
            recovery: credentials_recovery(data),
        },
        "unsupported_security_profile" => ErrorBox {
            title: texts::UNSUPPORTED_SECURITY_TITLE.to_owned(),
            detail: texts::UNSUPPORTED_SECURITY_DETAIL.to_owned(),
            recovery: None,
        },
        "endpoint_unreachable" => ErrorBox {
            title: texts::UNREACHABLE_TITLE.to_owned(),
            detail: texts::UNREACHABLE_DETAIL.to_owned(),
            recovery: None,
        },
        "browse_denied" => ErrorBox {
            title: texts::BROWSE_DENIED_TITLE.to_owned(),
            detail: texts::BROWSE_DENIED_DETAIL.to_owned(),
            recovery: None,
        },
        _ => ErrorBox {
            title: texts::browse_failed(data.protocol),
            detail: if error.message.is_empty() {
                texts::browse_failed(data.protocol)
            } else {
                error.message.clone()
            },
            recovery: None,
        },
    }
}

/// The recovery that browses the server again with its certificate trusted (`useBrowseSession`'s
/// `trustCertificate`, `W/useBrowseSession.ts:186-201`): it reopens the pane with the same target
/// and the trust flag set (design §3.3 J50).
fn trust_recovery(data: &Browse<'_>) -> Action {
    Action {
        id: "trust_certificate".to_owned(),
        label: texts::TRUST_CERTIFICATE.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Primary,
        confirm: None,
        op: Op::Ui {
            ui: UiOp::Open {
                pane: Pane::Browse {
                    request: browse_body(data, true, data.request.port),
                },
            },
        },
    }
}

/// The recovery that reopens the connection's form with the failed target filled in
/// (`onEditBrowseCredentials`, `W/NetworkCanvasApp.tsx:519-533`; specification 25 §5.4 and
/// §5.13:709-711). `None` when the pane knows no runtime to open the form on.
fn credentials_recovery(data: &Browse<'_>) -> Option<Action> {
    let runtime = data.runtime?;
    let mut prefill = data.params.clone();
    prefill.insert(
        "trust_server_certificate".to_owned(),
        Value::Bool(data.request.trust_server_certificate),
    );
    Some(Action {
        id: "edit_credentials".to_owned(),
        label: texts::EDIT_CREDENTIALS.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Primary,
        confirm: None,
        op: Op::Rust {
            action: "form.open".to_owned(),
            payload: json!({
                "protocol": data.protocol,
                "runtime": runtime.0,
                "prefill": Value::Object(prefill),
            }),
        },
    })
}

/// The `devicesBrowse` body that reopens the pane with the trust flag `trust` and reads `port`.
/// The webview attaches its own host facts, as it does for every request, so they are not carried
/// here.
fn browse_body(data: &Browse<'_>, trust: bool, port: Option<u16>) -> Value {
    let mut body = json!({
        "session": data.request.session,
        "seq": data.request.seq.saturating_add(1),
        "source": source_body(data),
        "kind": data.request.kind,
        "trust_server_certificate": trust,
    });
    if let Some(port) = port {
        body["port"] = json!(port);
    }
    body
}

/// The `source` a reopened browse names: its connection and name, or the found device's card, its
/// protocol and its own target (design §4.3 B8, line 261).
fn source_body(data: &Browse<'_>) -> Value {
    match (
        data.request.source.connection(),
        data.request.source.found(),
    ) {
        (Some(connection), _) => json!({ "connection": connection, "connection_name": data.name }),
        (None, Some(found)) => {
            let mut source = json!({ "found": found, "protocol": data.protocol });
            if let Some(target) = data.request.source.target() {
                source["target"] = Value::Object(target.clone());
            }
            source
        }
        (None, None) => json!({}),
    }
}

/// The projected tree of `nodes`, in browse order.
fn project_tree(nodes: &[BrowsedNode], data: &Browse<'_>) -> Vec<TreeNode> {
    nodes.iter().map(|node| project_node(node, data)).collect()
}

/// One projected node: its selection key keeps the raw protocol identity, so leaves with equal
/// names or paths stay distinct (specification 25 §11.2; `W/opcuaClientModel.ts:81-106`).
fn project_node(node: &BrowsedNode, data: &Browse<'_>) -> TreeNode {
    let leaf = node.children.is_empty();
    TreeNode {
        key: node.key().to_owned(),
        name: node.name.clone(),
        path: node.path.clone(),
        type_label: node.data_type.clone().or_else(|| node.type_label.clone()),
        access: if leaf { access_of(node) } else { None },
        selectable: leaf && (data.kind != BrowseKind::Nodes || node.node_id.is_some()),
        added: if leaf {
            added(node, data.kind, data.params)
        } else {
            false
        },
        children: project_tree(&node.children, data),
    }
}

/// A leaf's access label: `read/write` when the origin reported it writable, else `read-only`; a
/// group, or a leaf without the flag, carries none (specification 25 §5.4).
fn access_of(node: &BrowsedNode) -> Option<String> {
    match node.writable {
        Some(true) => Some("read/write".to_owned()),
        Some(false) => Some("read-only".to_owned()),
        None => None,
    }
}

/// Whether the connection already has this leaf: its OPC UA point, its selected `EtherCAT`
/// channel, or its exposed global (specification 25 §11.2).
fn added(node: &BrowsedNode, kind: BrowseKind, params: &Map<String, Value>) -> bool {
    match kind {
        BrowseKind::Nodes => node
            .node_id
            .as_deref()
            .is_some_and(|node_id| opcua_points(params).iter().any(|point| point == node_id)),
        BrowseKind::Channels => selected_channels(params)
            .iter()
            .any(|channel| channel == &node.path),
        BrowseKind::Expose => expose_names(params)
            .iter()
            .any(|name| name == &global_name(&node.path)),
        BrowseKind::AdsTags => false,
    }
}

/// The `node_id` of every OPC UA point of the connection's configuration.
fn opcua_points(params: &Map<String, Value>) -> Vec<String> {
    params
        .get("points")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|point| point.get("node_id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// The `EtherCAT` channels the connection's configuration selected.
fn selected_channels(params: &Map<String, Value>) -> Vec<String> {
    strings(params.get("selected_channels"))
}

/// The globals the connection's server exposes, without the `global.` prefix.
fn expose_names(params: &Map<String, Value>) -> Vec<String> {
    strings(params.get("expose"))
}

/// The strings of `value`, ignoring everything else.
fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
