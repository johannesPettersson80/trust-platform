//! The connections of every runtime (specification 25 §5.2, §5.4, §5.5, §5.8 and §5.13). Each
//! is a pill on its runtime with the protocol, its own name, a status dot, the role word of
//! today's tiles and the `DRAFT` and `Disabled` marks, and an inspector titled with its name that
//! keeps today's summaries and adds the connector and live rows.
//!
//! - `fields`: the summary of the settings from the catalog of the connection's runtime
//!   (`forms::Catalog`), with each protocol's allowlist and product labels; a secret only says
//!   whether it is set.
//! - `servers`: where a server listens, its clients, what proves it and what it exposes.
//! - `ads_client`: the ADS client's devices, ports, state and settings.
//! - `live`: the connector rows, the live values, the last time seen and the response time.
//!
//! `Test` needs the connection's runtime to answer; otherwise it stays visible but disabled with
//! its reason. An OPC UA client of several connections offers one `Test` for each, naming it.

mod ads_client;
mod fields;
mod live;
mod servers;

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::devices;
use super::health;
use super::lifecycle::Owner;
use super::list;
use super::runtimes;
use super::types::{
    Action, Computer, Connection, Emphasis, HoverCard, Inspector, ItemId, Look, Op, Pill, Row,
    Section, Tone, UiOp,
};
use crate::gather::Gathered;
use crate::inputs::Facts;
use crate::panes::forms::{self, Catalog, CatalogProtocol, FormRequest};
use crate::request::IoValue;
use crate::texts::connections as texts;
use crate::texts::graph;
use crate::topology::{self, Endpoint, Topology};
use live::Live;

pub(crate) use live::{last_seen, response_time};

/// The protocols of local I/O: no wire leaves them.
const LOCAL_IO: [&str; 3] = ["gpio", "simulated", "loopback"];

/// The protocols whose values are the simulator's process image.
const PROCESS_IMAGE: [&str; 2] = ["simulated", "loopback"];

/// What the connections are drawn from besides the topology.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Context<'a> {
    /// What the view reads beside the topology.
    pub(crate) facts: &'a Facts,
    /// The gather, whose catalogs describe the protocols.
    pub(crate) gathered: &'a Gathered,
    /// The simulator's current I/O values.
    pub(crate) io_values: &'a [IoValue],
    /// The time of the view, in milliseconds since the Unix epoch.
    pub(crate) now_ms: u64,
}

impl<'a> Context<'a> {
    /// The context of one view.
    pub(crate) fn new(
        facts: &'a Facts,
        gathered: &'a Gathered,
        io_values: &'a [IoValue],
        now_ms: u64,
    ) -> Self {
        Self {
            facts,
            gathered,
            io_values,
            now_ms,
        }
    }
}

/// The runtime that a connection belongs to.
struct Owning {
    /// The runtime's identity.
    id: ItemId,
    /// The name its card shows.
    name: String,
    /// Whether it answered `status` this round.
    answers: bool,
    /// Whether its program runs.
    runs: bool,
    /// Whether the simulator owns it, so that the simulator's I/O values are its own.
    simulator: bool,
}

/// Fills every runtime card of `computers` with the connections of its runtime.
pub(crate) fn fill(computers: &mut [Computer], topology: &Topology, context: &Context<'_>) {
    let runtimes: BTreeMap<&str, &topology::Runtime> = topology
        .runtimes()
        .map(|runtime| (runtime.id.as_str(), runtime))
        .collect();
    for card in computers
        .iter_mut()
        .flat_map(|computer| computer.runtimes.iter_mut())
    {
        let Some(runtime) = runtimes.get(card.id.as_str()) else {
            continue;
        };
        let owning = Owning {
            id: card.id.clone(),
            name: card.name.clone(),
            answers: runtimes::answers(runtime, context.facts),
            runs: runtimes::runs(runtime, context.facts),
            simulator: runtimes::owner(runtime, context.facts) == Owner::Simulator,
        };
        let catalog = Catalog::for_runtime(context.gathered, runtime.control_endpoint.as_deref());
        card.connections = runtime
            .endpoints
            .iter()
            .map(|endpoint| connection(endpoint, &owning, catalog.as_ref(), context))
            .collect();
    }
}

/// The name a connection is shown with: `Simulated I/O` and `Loopback I/O` for local I/O, whose
/// role the band shows (specification 25 §5.7), else its own name or its protocol.
pub(crate) fn display_name(endpoint: &Endpoint) -> String {
    let protocol = endpoint.protocol.trim().to_ascii_lowercase();
    if PROCESS_IMAGE.contains(&protocol.as_str()) {
        return texts::protocol_name(&protocol);
    }
    let name = endpoint.name.trim();
    if name.is_empty() {
        protocol.replace('_', " ")
    } else {
        name.to_owned()
    }
}

/// The role word of a connection's band (specification 25 §5.13): `DRAFT` for a draft of the add
/// flow, else the word of its protocol and role, and the role itself in capitals for a protocol
/// without one.
pub(crate) fn role_word(protocol: &str, role: &str) -> String {
    if role == "draft" {
        return texts::DRAFT.to_owned();
    }
    let word = match protocol {
        "ethercat" => texts::ROLE_MASTER,
        "opcua" => {
            if role == "client" {
                texts::ROLE_CLIENT
            } else {
                texts::ROLE_SERVER
            }
        }
        "ads" => {
            if role == "server" {
                texts::ROLE_SERVER
            } else {
                texts::ROLE_CLIENT
            }
        }
        "modbus_tcp" | "modbus" => texts::ROLE_CLIENT,
        "ads_server" | "web" => texts::ROLE_SERVER,
        "mqtt" => texts::ROLE_PUB_SUB,
        "mesh" => texts::ROLE_PEER,
        "discovery" => texts::ROLE_ADVERTISE,
        "realtime" | "realtime_t0" => texts::ROLE_SAME_HOST,
        "runtime_cloud" | "federation" => texts::ROLE_POLICY,
        "gpio" | "simulated" | "loopback" => texts::ROLE_IO,
        _ => return role.to_uppercase(),
    };
    word.to_owned()
}

/// The role word as the inspector's kind writes it, such as `Client` or `Pub/Sub`.
fn role_caption(word: &str) -> String {
    match word {
        texts::ROLE_PUB_SUB => texts::CAPTION_PUB_SUB.to_owned(),
        texts::ROLE_IO => texts::ROLE_IO.to_owned(),
        texts::ROLE_SAME_HOST => texts::CAPTION_SAME_HOST.to_owned(),
        _ => {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                format!("{first}{}", characters.as_str().to_lowercase())
            })
        }
    }
}

/// The `State` row of a connection: the product word of its health and, when it says more, its
/// detail.
fn state_text(health: &str, detail: &str) -> String {
    runtimes::state_row(
        &graph::health_word(health),
        health,
        &runtimes::clean_detail(detail),
    )
}

/// A row without a tone, a copy button or the whole width.
pub(crate) fn row(label: &str, value: impl Into<String>) -> Row {
    Row {
        label: label.to_owned(),
        value: value.into(),
        tone: None,
        wide: false,
        copy: None,
    }
}

/// What one connection shows.
struct Shown {
    /// Its identity.
    id: ItemId,
    /// Its name.
    name: String,
    /// Its protocol's product name.
    protocol_name: String,
    /// Its role word.
    role_word: String,
    /// The product word of its health.
    state_word: String,
    /// Its dot's tone.
    dot: Tone,
    /// Whether its dot pulses: only while it is connected.
    live: bool,
}

/// The pill of `endpoint` on the runtime `owning`, whose catalog is `catalog`.
fn connection(
    endpoint: &Endpoint,
    owning: &Owning,
    catalog: Option<&Catalog>,
    context: &Context<'_>,
) -> Connection {
    let protocol = endpoint.protocol.trim();
    let role = endpoint.role.as_deref().unwrap_or_default().trim();
    let health = endpoint.health.trim();
    let draft = health::draft_like(protocol, role, health);
    let live_dot = !draft && is_connected(health);
    let shown = Shown {
        id: ItemId(endpoint.id.clone()),
        name: display_name(endpoint),
        protocol_name: texts::protocol_name(protocol),
        role_word: role_word(protocol, role),
        state_word: graph::health_word(health),
        dot: if draft {
            Tone::Muted
        } else {
            health::tone_of(health)
        },
        live: live_dot,
    };
    let schema = catalog.and_then(|catalog| catalog.protocol(protocol));
    let io_values: &[IoValue] =
        if owning.simulator && PROCESS_IMAGE.contains(&protocol) && is_connected(health) {
            context.io_values
        } else {
            &[]
        };
    let live = Live::of(endpoint, io_values, context.now_ms);
    let connector = live::connector_rows(endpoint.connector.as_ref());
    let detail = runtimes::clean_detail(&endpoint.detail);
    let values = live.values_cell();
    Connection {
        list: list::cells(
            "connection",
            [
                (list::KIND, Some(graph::KIND_CONNECTION)),
                (list::NAME, Some(shown.name.as_str())),
                (list::STATE, Some(shown.state_word.as_str())),
                (list::ADDRESS, endpoint.address.as_deref()),
                (list::TYPE, Some(shown.protocol_name.as_str())),
                (list::BELONGS_TO, Some(owning.name.as_str())),
                (list::DETAIL, Some(detail.as_str())),
                (list::VALUES, values.as_deref()),
                (list::LAST_SEEN, live.last_seen.as_deref()),
                (list::RESPONSE_TIME, live.response_time.as_deref()),
            ],
        ),
        hover: HoverCard {
            title: shown.name.clone(),
            rows: hover_rows(&shown, &connector, &detail),
        },
        inspector: inspector(endpoint, owning, schema, &shown, &connector, &live),
        protocol: protocol.to_owned(),
        live: shown.live,
        marks: health::connection_marks(protocol, role, health),
        look: if draft {
            Look::Draft
        } else {
            health::look_of(health)
        },
        wire_port: !LOCAL_IO.contains(&protocol),
        subdevices: endpoint
            .children
            .iter()
            .map(|child| devices::subdevice_id(&endpoint.id, &child.id))
            .collect(),
        dimmed: false,
        hidden: false,
        dot: shown.dot,
        id: shown.id,
        name: shown.name,
        protocol_name: shown.protocol_name,
        role_word: shown.role_word,
    }
}

fn is_connected(health: &str) -> bool {
    health.trim().eq_ignore_ascii_case("connected")
}

/// The facts on hover: the protocol, role and state, the connector rows and the detail.
fn hover_rows(shown: &Shown, connector: &[Row], detail: &str) -> Vec<Row> {
    let mut rows = vec![row(texts::ROW_PROTOCOL, shown.protocol_name.as_str())];
    if !shown.role_word.is_empty() {
        rows.push(row(texts::ROW_ROLE, shown.role_word.as_str()));
    }
    rows.push(row(graph::ROW_STATE, shown.state_word.as_str()));
    rows.extend(connector.iter().cloned());
    if !detail.is_empty() {
        rows.push(row(graph::ROW_DETAIL, detail));
    }
    rows
}

/// The connection's inspector: its actions, the summary of its settings, its connector rows and
/// what its runtime reports live.
fn inspector(
    endpoint: &Endpoint,
    owning: &Owning,
    schema: Option<&CatalogProtocol>,
    shown: &Shown,
    connector: &[Row],
    live: &Live,
) -> Inspector {
    let protocol = endpoint.protocol.trim();
    let ads_client = protocol == "ads" && schema.is_some();
    let kind = if ads_client {
        texts::ADS_CONNECTION.to_owned()
    } else {
        texts::connection_kind(
            &role_caption(&shown.role_word),
            schema.is_some() && endpoint.kind.trim() == "field",
        )
    };
    let form = schema.map(|_| FormRequest {
        protocol: Some(protocol.to_owned()),
        runtime: Some(owning.id.as_str().to_owned()),
        connection: Some(shown.id.as_str().to_owned()),
        ..FormRequest::default()
    });
    let mut sections = match schema {
        Some(_) if ads_client => ads_client::sections(endpoint, owning.runs),
        Some(schema) => vec![Section {
            heading: None,
            rows: summary_rows(endpoint, schema, shown),
        }],
        None => vec![Section {
            heading: None,
            rows: vec![
                row(texts::ROW_PROTOCOL, shown.protocol_name.as_str()),
                row(texts::ROW_ROLE, shown.role_word.as_str()),
                state_row(endpoint, shown),
            ],
        }],
    };
    if !connector.is_empty() {
        sections.push(Section {
            heading: Some(texts::SECTION_CONNECTOR.to_owned()),
            rows: connector.to_vec(),
        });
    }
    let live_rows = live.rows();
    if !live_rows.is_empty() {
        sections.push(Section {
            heading: Some(texts::SECTION_LIVE_VALUES.to_owned()),
            rows: live_rows,
        });
    }
    Inspector {
        breadcrumb: graph::breadcrumb(&kind),
        kind,
        title: shown.name.clone(),
        health: Some(Pill {
            word: shown.state_word.clone(),
            tone: shown.dot,
            live: shown.live,
        }),
        primary: actions(endpoint, owning, schema, &shown.id, form.as_ref()),
        secondary: vec![focus(&shown.id)],
        sections,
        edit: form.as_ref().map(forms::form_id),
    }
}

/// The `State` row, in the tone of the connection's dot.
fn state_row(endpoint: &Endpoint, shown: &Shown) -> Row {
    Row {
        tone: Some(shown.dot),
        ..row(
            graph::ROW_STATE,
            state_text(&endpoint.health, &endpoint.detail),
        )
    }
}

/// The summary of a connection whose protocol the catalog describes: its name, where a server
/// listens and what proves it, its settings, and its state when it has a detail.
fn summary_rows(endpoint: &Endpoint, schema: &CatalogProtocol, shown: &Shown) -> Vec<Row> {
    let protocol = endpoint.protocol.trim();
    let params = endpoint.params.as_ref();
    let mut rows = vec![row(texts::ROW_NAME, shown.name.as_str())];
    rows.extend(servers::rows(protocol, params, endpoint.live.as_ref()));
    rows.extend(fields::summary_rows(protocol, schema, params));
    if !endpoint.detail.trim().is_empty() {
        rows.push(state_row(endpoint, shown));
    }
    rows
}

fn action(id: &str, label: &str, emphasis: Emphasis, op: Op) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: true,
        reason: None,
        emphasis,
        confirm: None,
        op,
    }
}

/// An action that its runtime must answer for: disabled, neutral and with `reason` while it does
/// not.
fn needs_runtime(id: &str, label: &str, reason: &str, answers: bool, op: Op) -> Action {
    if answers {
        action(id, label, Emphasis::Secondary, op)
    } else {
        Action {
            enabled: false,
            reason: Some(reason.to_owned()),
            ..action(id, label, Emphasis::Quiet, op)
        }
    }
}

fn rust(action: &str, payload: Value) -> Op {
    Op::Rust {
        action: action.to_owned(),
        payload,
    }
}

fn focus(id: &ItemId) -> Action {
    action(
        "focus",
        graph::FOCUS,
        Emphasis::Secondary,
        Op::Ui {
            ui: UiOp::Focus { item: id.clone() },
        },
    )
}

/// The inspector's larger actions (specification 25 §5.13): `Edit settings` while the catalog
/// describes the protocol, `Test` while the connection's runtime or the catalog says it can be
/// tested, and `Diagnose` for an ADS client.
fn actions(
    endpoint: &Endpoint,
    owning: &Owning,
    schema: Option<&CatalogProtocol>,
    id: &ItemId,
    form: Option<&FormRequest>,
) -> Vec<Action> {
    let protocol = endpoint.protocol.trim();
    let payload = |name: Option<&str>| {
        let mut payload = json!({
            "runtime": owning.id.as_str(),
            "connection": id.as_str(),
            "protocol": protocol,
        });
        if let Some(name) = name {
            payload["name"] = json!(name);
        }
        payload
    };
    let mut actions = Vec::new();
    if let Some(form) = form {
        let label = if protocol == "ads" {
            texts::EDIT_ADVANCED_SETTINGS
        } else {
            texts::EDIT_SETTINGS
        };
        actions.push(action(
            "edit",
            label,
            Emphasis::Primary,
            rust("form.open", form.payload()),
        ));
    }
    if endpoint.supports_test || schema.is_some_and(|schema| schema.supports_test) {
        let names = if protocol == "opcua_client" {
            client_connection_names(endpoint.params.as_ref())
        } else {
            Vec::new()
        };
        if names.len() > 1 {
            actions.extend(names.iter().enumerate().map(|(index, name)| {
                needs_runtime(
                    &format!("test:{index}"),
                    &texts::test_named(name),
                    texts::TEST_NEEDS_RUNTIME,
                    owning.answers,
                    rust("connection.test", payload(Some(name))),
                )
            }));
        } else {
            actions.push(needs_runtime(
                "test",
                texts::TEST,
                texts::TEST_NEEDS_RUNTIME,
                owning.answers,
                rust(
                    "connection.test",
                    payload(names.first().map(String::as_str)),
                ),
            ));
        }
    }
    if protocol == "ads" {
        actions.push(needs_runtime(
            "diagnose",
            texts::DIAGNOSE,
            texts::DIAGNOSE_NEEDS_RUNTIME,
            owning.answers,
            rust("connection.diagnose", payload(None)),
        ));
    }
    actions
}

/// The names of the client connections that a connection's settings list.
fn client_connection_names(params: Option<&Value>) -> Vec<String> {
    params
        .and_then(|params| params.get("connections"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|connection| connection.get("name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}
