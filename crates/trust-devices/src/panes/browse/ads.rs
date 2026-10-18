//! The ADS tag browser's projection (design §4.3 B8 and D5b, and §3.3 J40, J46, J54, J55, J57, J58
//! and J59; specification 25 §5.4 and its lines 878-883): one card per ADS port with its tags, its
//! state and its own error, the port suggestions and their help, the route help of a port whose
//! route is missing with its `Create route`, the `added` flags of the tags the project's ADS
//! configuration already has, and the action that ticking each tag posts.
//!
//! Every rule here is a ported TypeScript behaviour, named per function:
//!
//! - `ports`: the port view model and the `Added` reconciliation
//!   (`W/AdsMultiPortTagBrowser.tsx:73-145, 386-512`).
//! - `classify`: a typed ADS error to its human text (`W/browseErrorModel.ts:13-55`).
//! - `route_help`: the route help of a port whose route is missing (`W/AdsMultiPortTagBrowser.tsx:
//!   521-551`).
//! - `added`: a tag the project's ADS configuration already has
//!   (`W/useDiscoverPane.ts:132-146`; `NC/adsTagBatch.ts:69-87`).
//! - `tick_action`: the `ads.tags_set` that ticking a tag posts (`W/AdsMultiPortTagBrowser.tsx:
//!   163-182`, `onAddTags` and `onRemoveTag`).

use serde_json::{json, Map, Value};

use super::{access_of, browse_body, pane_id, Browse};
use crate::browse::{target_port, Browsed, BrowsedError, BrowsedNode};
use crate::panes::forms::RouteForm;
use crate::texts::browse as texts;
use crate::texts::plans as plan_texts;
use crate::view::types::{
    Action, AdsPortCard, Emphasis, ErrorBox, FieldOption, HostEffect, Op, Pane, RouteHelp,
    TreeNode, UiOp,
};

/// What one port's card is drawn from: the browse this pane ran, the session's browse of that port,
/// the device's own evidence, or nothing yet.
#[derive(Clone, Copy)]
enum Answer<'a> {
    /// This port's browse still runs.
    Loading,
    /// This port's browse did not reach the origin, with its text.
    Failed(&'a str),
    /// The origin answered this port.
    Browsed(&'a Browsed),
    /// This port was not browsed here; nothing carries it yet.
    NotBrowsed,
}

/// The suggested ADS ports and their names (specification 25:880-881).
#[must_use]
pub(super) fn suggestions() -> Vec<FieldOption> {
    texts::ADS_PORT_SUGGESTIONS
        .iter()
        .map(|(value, label)| FieldOption {
            value: (*value).to_owned(),
            label: (*label).to_owned(),
        })
        .collect()
}

/// One card per ADS port, in ascending port order: the ports the target reports or finds evidence
/// for, the ports this session has browsed, plus the port this browse read, each with its tags, its
/// state text and its own error or route help (design §3.3 J55 and J57; `adsPortViews`,
/// `W/AdsMultiPortTagBrowser.tsx:386-443`).
#[must_use]
pub(super) fn ports(
    data: &Browse<'_>,
    answer: Option<&Result<Browsed, String>>,
) -> Vec<AdsPortCard> {
    let browse_evidence = evidence(data.params);
    let mut ports = responding_ports(data.params);
    ports.extend(browse_evidence.iter().map(|(port, _)| *port));
    ports.extend(data.cached.iter().map(|(port, _)| *port));
    ports.extend(target_port(data.params));
    ports.push(data.port);
    ports.sort_unstable();
    ports.dedup();
    ports
        .iter()
        .map(|port| card(data, answer, &browse_evidence, *port))
        .collect()
}

/// One port's card (design §3.3 J55; `W/AdsMultiPortTagBrowser.tsx:275-318`). The port this pane
/// read answers from `answer`; another port answers from the session's browse of it, when there is
/// one, and else from the device's evidence.
fn card(
    data: &Browse<'_>,
    answer: Option<&Result<Browsed, String>>,
    evidence: &[(u16, Browsed)],
    port: u16,
) -> AdsPortCard {
    let state = if port == data.port {
        match answer {
            None => Answer::Loading,
            Some(Err(message)) => Answer::Failed(message.as_str()),
            Some(Ok(browsed)) => Answer::Browsed(browsed),
        }
    } else if let Some((_, cached)) = data.cached.iter().find(|(candidate, _)| *candidate == port) {
        match cached {
            None => Answer::Loading,
            Some(Err(message)) => Answer::Failed(message.as_str()),
            Some(Ok(browsed)) => Answer::Browsed(browsed),
        }
    } else {
        evidence
            .iter()
            .find(|(candidate, _)| *candidate == port)
            .map_or(Answer::NotBrowsed, |(_, browsed)| Answer::Browsed(browsed))
    };
    let tags = match state {
        Answer::Browsed(browsed) => browse_tree(&browsed.tree, port, data),
        Answer::Loading | Answer::Failed(_) | Answer::NotBrowsed => Vec::new(),
    };
    let route = match state {
        Answer::Browsed(browsed) => browsed
            .route
            .as_ref()
            .filter(|route| route.missing)
            .map(|route| route_help(port, route.plan.as_ref(), data)),
        Answer::Loading | Answer::Failed(_) | Answer::NotBrowsed => None,
    };
    let empty_text = match state {
        Answer::Loading => Some(texts::LOADING_TAGS),
        Answer::NotBrowsed => Some(texts::NOT_LOADED_TAGS),
        Answer::Failed(_) => None,
        Answer::Browsed(browsed) => {
            if route.is_some() || browsed.error.is_some() || !tags.is_empty() {
                None
            } else {
                Some(texts::NO_TAGS_ON_PORT)
            }
        }
    };
    AdsPortCard {
        port,
        title: texts::ads_port_title(port),
        state_text: match state {
            Answer::Loading => texts::LOADING.to_owned(),
            Answer::Failed(_) | Answer::Browsed(_) | Answer::NotBrowsed => {
                texts::tag_count(leaf_count(&tags))
            }
        },
        empty_text: empty_text.map(str::to_owned),
        tags,
        route,
        error: match state {
            Answer::Failed(message) => Some(ErrorBox {
                title: texts::browse_failed(data.protocol),
                detail: message.to_owned(),
                recovery: None,
            }),
            Answer::Browsed(browsed) => browsed.error.as_ref().map(classify),
            Answer::Loading | Answer::NotBrowsed => None,
        },
        browse: match state {
            Answer::NotBrowsed => Some(browse_action(data, port)),
            Answer::Loading | Answer::Failed(_) | Answer::Browsed(_) => None,
        },
    }
}

/// The button that browses one port whose tags are not loaded here yet (`W/AdsMultiPortTagBrowser
/// .tsx:304-311`).
fn browse_action(data: &Browse<'_>, port: u16) -> Action {
    Action {
        id: format!("browse_{port}"),
        label: texts::browse_port(port),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Secondary,
        confirm: None,
        op: Op::Ui {
            ui: UiOp::Open {
                pane: Pane::Browse {
                    request: browse_body(data, data.request.trust_server_certificate, Some(port)),
                },
            },
        },
    }
}

/// A typed ADS error's human box (`classifyBrowseError`, `W/browseErrorModel.ts:13-55`): a known
/// code carries its own title and detail; any other carries the origin's message, never its code.
fn classify(error: &BrowsedError) -> ErrorBox {
    let (title, detail) = match error.code.as_str() {
        "ads_port_unavailable" => (
            texts::ADS_PORT_UNAVAILABLE_TITLE,
            texts::ADS_PORT_UNAVAILABLE_DETAIL.to_owned(),
        ),
        "symbol_upload_unsupported" => (
            texts::SYMBOL_UPLOAD_UNSUPPORTED_TITLE,
            texts::SYMBOL_UPLOAD_UNSUPPORTED_DETAIL.to_owned(),
        ),
        "empty_symbol_table" => (
            texts::NO_COMPATIBLE_SYMBOLS_TITLE,
            texts::NO_COMPATIBLE_SYMBOLS_DETAIL.to_owned(),
        ),
        _ => (
            texts::ADS_BROWSE_FAILED_TITLE,
            if error.message.is_empty() {
                texts::ADS_BROWSE_FAILED_DETAIL.to_owned()
            } else {
                error.message.clone()
            },
        ),
    };
    ErrorBox {
        title: title.to_owned(),
        detail,
        recovery: None,
    }
}

/// The help of a port whose route is missing: its warning, the administrator explanation, the
/// generated route files and `Create route` (`W/AdsMultiPortTagBrowser.tsx:521-551`;
/// `W/BrowseTagsPanel.tsx:154-158`; specification 25 §5.4).
fn route_help(port: u16, plan: Option<&Value>, data: &Browse<'_>) -> RouteHelp {
    let artifacts = artifacts(plan);
    let help = if artifacts.is_empty() {
        texts::ROUTE_NEEDS_ADMIN_CREATE
    } else {
        texts::ROUTE_NEEDS_ADMIN_SCRIPT
    };
    RouteHelp {
        text: texts::route_required(port),
        help: vec![help.to_owned()],
        artifacts,
        create_route: create_route(plan, data),
    }
}

/// One action per generated route file of the plan, which copies its content
/// (`W/AdsMultiPortTagBrowser.tsx:538-548`).
fn artifacts(plan: Option<&Value>) -> Vec<Action> {
    let Some(artifacts) = plan
        .and_then(|plan| plan.get("artifacts"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    artifacts
        .iter()
        .filter_map(|artifact| {
            let artifact = artifact.as_object()?;
            let content = artifact.get("content").and_then(Value::as_str)?;
            let label = artifact
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("route file");
            let kind = artifact
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("file");
            Some(Action {
                id: format!("copy_{kind}"),
                label: texts::copy_artifact(label),
                enabled: true,
                reason: None,
                emphasis: Emphasis::Secondary,
                confirm: None,
                op: Op::Host {
                    effect: HostEffect::Copy {
                        text: content.to_owned(),
                    },
                },
            })
        })
        .collect()
}

/// The `Create route` action: it opens the login form that asks once for the `TwinCAT`
/// administrator login and posts `ads.route_add` with exactly the payload the guarded operation of
/// specification 11 reads (`W/AdsMultiPortTagBrowser.tsx:535`; design §4.3 B7, N26).
fn create_route(plan: Option<&Value>, data: &Browse<'_>) -> Action {
    let disabled = |reason: &str| Action {
        id: "create_route".to_owned(),
        label: texts::CREATE_ROUTE.to_owned(),
        enabled: false,
        reason: Some(reason.to_owned()),
        emphasis: Emphasis::Quiet,
        confirm: None,
        op: Op::Rust {
            action: "form.open".to_owned(),
            payload: Value::Null,
        },
    };
    let Some(runtime) = data.runtime else {
        return disabled(texts::CREATE_ROUTE_NEEDS_RUNTIME);
    };
    let route_name = plan
        .filter(|plan| plan.is_object())
        .and_then(|plan| plan.get("route_name"))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty());
    let target = plan
        .and_then(|plan| plan.get("target"))
        .filter(|value| value.is_object());
    let local = plan
        .and_then(|plan| plan.get("local"))
        .filter(|value| value.is_object());
    let (Some(route_name), Some(target), Some(local)) = (route_name, target, local) else {
        return disabled(plan_texts::ROUTE_NEEDS_TARGET);
    };
    let route = RouteForm {
        pane: pane_id(data.protocol),
        runtime: runtime.0.clone(),
        route_name: route_name.to_owned(),
        target: target.clone(),
        local: local.clone(),
    };
    Action {
        id: "create_route".to_owned(),
        label: texts::CREATE_ROUTE.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Primary,
        confirm: None,
        op: Op::Rust {
            action: "form.open".to_owned(),
            payload: json!({ "form": route.form_id(), "route": route }),
        },
    }
}

/// The device's own browse evidence per port, from a find's `ads_port_browse_results`
/// (specification 11, "Communication discovery scope and symbol browsing").
fn evidence(params: &Map<String, Value>) -> Vec<(u16, Browsed)> {
    params
        .get("ads_port_browse_results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some((
                item.get("port").and_then(port_of)?,
                Browsed::of_evidence(item)?,
            ))
        })
        .collect()
}

/// The ports a find reported in `responding_ads_ports` (`respondingAdsPorts`,
/// `NC/adsDiscoveryPorts.ts:38-48`).
#[must_use]
pub(super) fn responding_ports(params: &Map<String, Value>) -> Vec<u16> {
    params
        .get("responding_ads_ports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(port_of)
        .collect()
}

/// The ADS port of `value`: a whole number from 1 through 65,535 (`isAdsPort`,
/// `NC/adsDiscoveryPorts.ts:147-149`).
fn port_of(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port >= 1)
}

/// The projected tags of `nodes`, with the key that tells the ports apart, the `added` flag of each
/// leaf and the action that ticking it posts (design §3.3 J55 and J57; `selectionKey`,
/// `W/AdsMultiPortTagBrowser.tsx:460-462`).
fn browse_tree(nodes: &[BrowsedNode], port: u16, data: &Browse<'_>) -> Vec<TreeNode> {
    let mut tree: Vec<TreeNode> = nodes
        .iter()
        .map(|node| project_node(node, port, data))
        .collect();
    sort_tree(&mut tree);
    tree
}

/// One projected tag: its selection key keeps the port, its access label comes from the origin's
/// writable flag, its `added` flag tells whether the project already has it, and its action is the
/// one ticking it posts.
fn project_node(node: &BrowsedNode, port: u16, data: &Browse<'_>) -> TreeNode {
    let leaf = node.children.is_empty();
    let is_added = leaf && added(&node.path, port, data.params);
    let action = if leaf {
        tick_action(data, port, node, is_added)
    } else {
        None
    };
    TreeNode {
        key: format!("{port}:{}", node.key()),
        name: node.name.clone(),
        path: node.path.clone(),
        type_label: node.data_type.clone().or_else(|| node.type_label.clone()),
        access: if leaf { access_of(node) } else { None },
        selectable: action.is_some(),
        select_label: action.as_ref().map(|_| texts::select_label(&node.path)),
        added: is_added,
        action,
        children: browse_tree(&node.children, port, data),
    }
}

/// The action that ticking or unticking an ADS tag posts (specification 25 §5.4; design §4.3 B7):
/// `ads.tags_set` at the card's runtime, connection and port, with the tag's path as `ticked` while
/// the project does not have it, and as `unticked` once it does. A pane that browses a found device
/// has no connection to import into, so its tags carry no action and cannot be ticked.
fn tick_action(data: &Browse<'_>, port: u16, node: &BrowsedNode, is_added: bool) -> Option<Action> {
    let runtime = data.runtime?;
    let connection = data.request.source.connection()?;
    let key = if is_added { "unticked" } else { "ticked" };
    let mut payload = json!({
        "pane": pane_id(data.protocol),
        "runtime": runtime.0,
        "connection": connection,
        "port": port,
    });
    payload[key] = json!([node.path]);
    Some(Action {
        id: format!("{port}:{}", node.key()),
        label: texts::select_label(&node.path),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Quiet,
        confirm: None,
        op: Op::Rust {
            action: "ads.tags_set".to_owned(),
            payload,
        },
    })
}

/// `tree` in lexicographic name order at every level, as ADS browsing orders groups and leaves
/// (specification 11, "Symbol browsing").
fn sort_tree(tree: &mut [TreeNode]) {
    tree.sort_by(|left, right| left.name.cmp(&right.name));
    for node in tree.iter_mut() {
        sort_tree(&mut node.children);
    }
}

/// The number of leaves in `tree`, which a port's state text counts.
fn leaf_count(tree: &[TreeNode]) -> usize {
    tree.iter()
        .map(|node| {
            if node.children.is_empty() {
                1
            } else {
                leaf_count(&node.children)
            }
        })
        .sum()
}

/// Whether the project's ADS configuration already has the tag at `path` on port `port`
/// (`importedTagKeys`, `W/AdsMultiPortTagBrowser.tsx:476-499`; `adsTagSelectionsFromConnections`,
/// `NC/adsTagBatch.ts:69-87`): a `[runtime.ads]` point's symbol or path on the target's port, or a
/// selection the panel merged in for a named port.
fn added(path: &str, port: u16, params: &Map<String, Value>) -> bool {
    imported_paths(params, port)
        .iter()
        .any(|candidate| candidate == path)
        || (target_port(params) == Some(port)
            && point_paths(params)
                .iter()
                .any(|candidate| candidate == path))
}

/// The paths `imported_ads_symbols` already has for port `port`.
fn imported_paths(params: &Map<String, Value>, port: u16) -> Vec<String> {
    let mut paths = Vec::new();
    let selections = params.get("imported_ads_symbols").and_then(Value::as_array);
    for selection in selections.into_iter().flatten() {
        let Some(selection) = selection.as_object() else {
            continue;
        };
        if selection.get("port").and_then(port_of) != Some(port) {
            continue;
        }
        for path in selection
            .get("paths")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(text_of)
        {
            paths.push(path);
        }
    }
    paths
}

/// The symbol or path of each `[runtime.ads]` point of the target's configuration.
fn point_paths(params: &Map<String, Value>) -> Vec<String> {
    let mut paths = Vec::new();
    for point in params
        .get("points")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(point) = point.as_object() else {
            continue;
        };
        for key in ["symbol", "path"] {
            if let Some(path) = point.get(key).and_then(text_of) {
                paths.push(path);
                break;
            }
        }
    }
    paths
}

/// The trimmed, non-empty string of `value`.
fn text_of(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
