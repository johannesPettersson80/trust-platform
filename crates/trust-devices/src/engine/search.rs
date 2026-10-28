//! The one search of `trust-lsp.devicesSearch` (design §1.4.1; specification 11, "One search"): on
//! this computer, through the search registry of this process, or inside a runtime the session
//! showed, through that runtime's control endpoint. A runtime's answer is awaited for at most
//! [`ANSWER_WAIT`](super::ANSWER_WAIT). The forwarded request itself runs on a thread of its own.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use trust_runtime::search::project_choices::{self, Memory};
use trust_runtime::search::{
    SearchError, SearchHost, SearchRequest, SearchState, SearchStatus, SourceId,
};
use trust_runtime_client::{ControlClient, ControlEndpoint, ControlError, RESPONSE_LINE_LIMIT};

use super::advertised::{
    self, deadline, names_except, origin_discover, scopes, AdvertisedSearch, Protocol,
};
use super::handoff::Handoff;
use super::{lock, DevicesEngine, OwnedSearch, Panel, ReplyError, ANSWER_WAIT, CONTROL_TIMEOUT};
use crate::gather::Gathered;
use crate::inputs;
use crate::request::{HostFacts, Token};
use crate::session::{KnownRuntime, Session};
use crate::texts::discover as texts;
use crate::view::discover::{self, RuntimeOrigin, Scan};
use crate::view::discover_compare;
use crate::view::discover_context::{Context, OriginFacts};
use crate::view::discover_names;
use crate::view::types::DiscoverView;

/// One `trust-lsp.devicesSearch` request (design §1.4.1): one command with the verb `start`,
/// `status` or `cancel`, mirroring `comm.search.*`.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchCall {
    /// The panel's session.
    pub session: String,
    /// Which of the three verbs runs.
    pub verb: SearchVerb,
    /// Where the search runs; `this_host` when the request names none.
    #[serde(default)]
    pub origin: SearchOrigin,
    /// The search that `status` and `cancel` name.
    #[serde(default)]
    pub search_id: Option<String>,
    /// The `cursor` of an earlier answer: only what changed after it is listed.
    #[serde(default)]
    pub since: Option<u64>,
    /// The `comm.search.start` parameters of the `start` verb, whose unknown members the search
    /// itself refuses (specification 11, "Requests").
    #[serde(default)]
    pub params: Value,
    /// The Known address and the subnet that the Discover pane's inputs hold (specification 25 §2): they
    /// become the `targets` of the `start` verb, and a blank input is no target.
    #[serde(default)]
    pub targets: Option<TypedTargets>,
    /// The facts that only VS Code knows: the token of the runtime a forwarded search reaches.
    #[serde(default)]
    pub host: HostFacts,
    /// The change that the Discover pane records before its view is answered (specification 25 §2): a
    /// tick, an Ignore, `Show ignored` or `Compare with project`. Absent for a plain search.
    #[serde(default)]
    pub change: Option<DiscoverChange>,
}

/// One change that the Discover pane records before its view is answered (specification 25 §2, decision
/// O14): a tick of `Only search`, an `Ignore` or `Stop ignoring` on a find, `Show ignored`, or the lists of
/// `Compare with project`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiscoverChange {
    /// A tick of `Only search`: whether the project remembers the source as ticked.
    Select {
        /// The source's ID.
        source: String,
        /// Whether it is ticked now.
        checked: bool,
    },
    /// `Ignore` or `Stop ignoring` on a find: whether the project ignores the device now.
    Ignore {
        /// The card's ID.
        device: String,
        /// Whether the device is ignored now.
        ignored: bool,
    },
    /// `Show ignored`: whether the finds that the project ignores are shown now.
    ShowIgnored {
        /// Whether they are shown.
        shown: bool,
    },
    /// `Compare with project` or `Hide comparison`: whether the lists of the comparison are shown now.
    Compare {
        /// Whether they are shown.
        shown: bool,
    },
}

/// The Known address and the subnet as the user typed them (specification 25 §2). Each is text, so the
/// search reads them as it reads a request: a blank text is no target.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TypedTargets {
    /// The Known address: one host name or IP address with an optional port, or blank.
    #[serde(default)]
    pub hosts: String,
    /// The subnet: one IPv4 subnet of at most `/24`, or blank.
    #[serde(default)]
    pub cidr: String,
}

impl TypedTargets {
    /// The `targets` of `comm.search.start` (specification 11, "Requests"): the Known address as one
    /// entry, and the subnet, with `null` for a blank input.
    #[must_use]
    fn targets(&self) -> Value {
        let hosts: Vec<&str> = match self.hosts.trim() {
            "" => Vec::new(),
            host => vec![host],
        };
        let cidr = match self.cidr.trim() {
            "" => None,
            cidr => Some(cidr),
        };
        json!({ "hosts": hosts, "cidr": cidr })
    }
}

impl SearchCall {
    /// The `comm.search.start` parameters, or none when the request sends an empty body (which is
    /// a search with every default, as `comm.search.start` reads it). The pane's typed targets replace
    /// the parameters' `targets`, and the project's configured ADS ports join them.
    #[must_use]
    fn params(&self) -> Option<Value> {
        self.with_ads_ports(self.typed_params())
    }

    /// The request's parameters with its typed targets (see [`Self::params`]).
    fn typed_params(&self) -> Option<Value> {
        let params = match &self.params {
            Value::Null => None,
            params => Some(params.clone()),
        };
        match (&self.targets, params) {
            (Some(typed), None) => Some(json!({ "targets": typed.targets() })),
            (Some(typed), Some(Value::Object(mut params))) => {
                params.insert("targets".to_owned(), typed.targets());
                Some(Value::Object(params))
            }
            // Without typed targets, or with parameters that are not an object (which the search
            // refuses, as it refuses every non-object parameter), the parameters pass unchanged.
            (None | Some(_), params) => params,
        }
    }

    /// The parameters with the project's configured ADS ports, when the request names none
    /// (`trust.ads.discoveryPorts`, specification 25 §2).
    fn with_ads_ports(&self, params: Option<Value>) -> Option<Value> {
        let ports = &self.host.settings.ads_discovery_ports;
        if ports.is_empty() {
            return params;
        }
        let mut object = match params {
            None => Map::new(),
            Some(Value::Object(object)) => object,
            // Parameters that are not an object pass unchanged; the search refuses them.
            Some(other) => return Some(other),
        };
        object
            .entry("ads_ports".to_owned())
            .or_insert_with(|| json!(ports));
        Some(Value::Object(object))
    }

    /// Whether a `status` request names no search, which the pane's idle view answers (specification
    /// 25 §2).
    fn names_no_search(&self) -> bool {
        self.search_id
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
    }

    /// The origin the request names, as the Discover view names it.
    fn scan(&self) -> Scan {
        match &self.origin {
            SearchOrigin::ThisHost => Scan::ThisHost,
            SearchOrigin::Runtime(item) => Scan::Runtime(item.clone()),
        }
    }

    /// The search that a `status` or `cancel` verb names.
    fn named_search(&self) -> Result<String, SearchReply> {
        self.search_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| SearchReply::failed("invalid_search", "The request named no search."))
    }

    /// The request that a runtime receives for this verb (specification 11, "One search"): its
    /// control method and its parameters.
    fn forwarded(&self) -> Result<(&'static str, Value), SearchReply> {
        match self.verb {
            SearchVerb::Start => {
                let mut params = match self.params() {
                    None => Map::new(),
                    Some(Value::Object(params)) => params,
                    Some(_) => {
                        return Err(SearchReply::failed(
                            "invalid_search",
                            "The search's parameters must be an object.",
                        ))
                    }
                };
                // The runtime runs a search of its own, as `trust-runtime discover --endpoint`
                // asks it to (specification 11, "One search").
                params.insert("origin".to_owned(), json!("runtime"));
                Ok(("comm.search.start", Value::Object(params)))
            }
            SearchVerb::Status => {
                let search_id = self.named_search()?;
                let mut params = Map::new();
                params.insert("search_id".to_owned(), json!(search_id));
                if let Some(since) = self.since {
                    params.insert("since".to_owned(), json!(since));
                }
                Ok(("comm.search.status", Value::Object(params)))
            }
            SearchVerb::Cancel => {
                let search_id = self.named_search()?;
                Ok(("comm.search.cancel", json!({ "search_id": search_id })))
            }
        }
    }
}

/// Which of the three search verbs a request runs (design §1.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchVerb {
    /// Starts a search and answers its ID.
    Start,
    /// Answers a search's progress.
    Status,
    /// Ends a search.
    Cancel,
}

/// Where a search runs (design §1.4.1): on this computer, in the language-server process, or
/// inside the runtime the session showed under an item.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchOrigin {
    /// On this computer.
    #[default]
    ThisHost,
    /// Inside the runtime the session showed under this item.
    Runtime(String),
}

/// The answer of `trust-lsp.devicesSearch` (design §1.4.1): the `search_id` a `start` answered, the
/// status of a search, the state of a cancelled one, or why the request could not run.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchReply {
    /// The search's ID, as `comm.search.start` answers it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_id: Option<String>,
    /// The state of the search that `cancel` ended.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<SearchState>,
    /// Why the request could not run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ReplyError>,
    /// The Discover pane's view of the status: its cards, source rows and scan choices (specification
    /// 25 §2); absent for a reply without a status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discover: Option<Box<DiscoverView>>,
    /// The body of a `status` answer, whose every member this reply holds (design §1.4.1).
    #[serde(flatten)]
    pub status: Map<String, Value>,
}

impl SearchReply {
    /// The answer to a `start` verb.
    #[must_use]
    pub(super) fn started(search_id: String) -> Self {
        Self {
            search_id: Some(search_id),
            ..Self::default()
        }
    }

    /// The answer to a `cancel` verb.
    #[must_use]
    pub(super) fn cancelled(search_id: String, state: SearchState) -> Self {
        Self {
            search_id: Some(search_id),
            state: Some(state),
            ..Self::default()
        }
    }

    /// The answer to a `status` verb, from the search's own status.
    #[must_use]
    fn status(status: SearchStatus) -> Self {
        match serde_json::to_value(status) {
            Ok(Value::Object(body)) => Self {
                status: body,
                ..Self::default()
            },
            Ok(other) => Self::failed("search_status_unreadable", other.to_string()),
            Err(error) => Self::failed("search_status_unreadable", error.to_string()),
        }
    }

    /// The answer to a request forwarded to a runtime, from its own body.
    #[must_use]
    fn from_body(kind: &str, body: Value) -> Self {
        match (kind, body) {
            ("comm.search.start", Value::Object(body)) => {
                if let Some(Value::String(search_id)) = body.get("search_id") {
                    Self::started(search_id.clone())
                } else {
                    Self::failed(
                        "runtime_answer_unreadable",
                        "The runtime's answer to comm.search.start has no search_id.",
                    )
                }
            }
            (_, Value::Object(status)) => Self {
                status,
                ..Self::default()
            },
            (_, other) => Self::failed("runtime_answer_unreadable", other.to_string()),
        }
    }

    /// A request that could not run.
    #[must_use]
    pub(super) fn failed(code: &str, message: impl Into<String>) -> Self {
        Self {
            error: Some(ReplyError::new(code, message)),
            ..Self::default()
        }
    }

    /// The status of a runtime whose truST version is older than this search: it cannot search, so
    /// every source is skipped with the update it needs (design §1.4.1; specification 34 TARGET-05).
    #[must_use]
    fn update_needed() -> Self {
        let reason = texts::update_needed(env!("CARGO_PKG_VERSION"));
        let sources: Vec<Value> = SourceId::ALL
            .iter()
            .map(|source| json!({ "id": source, "state": "skipped", "reason": reason }))
            .collect();
        let mut status = Map::new();
        status.insert("state".to_owned(), json!("done"));
        status.insert("sources".to_owned(), Value::Array(sources));
        status.insert("devices".to_owned(), json!([]));
        Self {
            status,
            ..Self::default()
        }
    }
}

/// The answer of a search request that the registry refused.
fn search_error(error: &SearchError) -> SearchReply {
    SearchReply::failed(error.code().unwrap_or("invalid_search"), error.to_string())
}

/// The control endpoint of the runtime that the session showed under `item`.
pub(super) fn runtime_endpoint(session: &Session, item: &str) -> Result<String, SearchReply> {
    let runtime = session.runtime(item).ok_or_else(|| {
        SearchReply::failed(
            "unknown_runtime",
            format!("The panel showed no runtime '{item}'."),
        )
    })?;
    runtime
        .control_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            SearchReply::failed(
                "runtime_without_endpoint",
                format!("The runtime '{item}' has no control endpoint to search on."),
            )
        })
}

/// One request that a search forwards to a runtime's control endpoint (design §1.4.1).
#[derive(Debug, Clone)]
struct Forward {
    /// The runtime's control endpoint.
    endpoint: String,
    /// The token that the extension holds for the endpoint, when it holds one.
    token: Option<Token>,
    /// The control method, such as `comm.search.start`.
    kind: &'static str,
    /// The parameters of the method.
    params: Value,
}

impl Forward {
    /// Whether the request starts a search.
    fn is_start(&self) -> bool {
        self.kind == "comm.search.start"
    }

    /// Whether the request reads a search's status or starts one, which an older runtime answers
    /// as unsupported.
    fn is_search_read(&self) -> bool {
        matches!(self.kind, "comm.search.start" | "comm.search.status")
    }

    /// Asks the runtime. Every wait is bounded by `CONTROL_TIMEOUT`, so a silent runtime ends the
    /// request on its own thread, never in the panel's session.
    fn ask(&self) -> SearchReply {
        let Some(endpoint) = ControlEndpoint::parse(&self.endpoint) else {
            return SearchReply::failed(
                "runtime_without_endpoint",
                format!("'{}' is not a control endpoint.", self.endpoint),
            );
        };
        let mut client = match ControlClient::connect(
            &endpoint,
            self.token.as_ref().map(Token::secret),
            CONTROL_TIMEOUT,
        ) {
            Ok(client) => client.with_response_limit(RESPONSE_LINE_LIMIT),
            Err(error) => return SearchReply::failed("runtime_unreachable", error.to_string()),
        };
        match client.request(self.kind, Some(self.params.clone())) {
            Ok(body) => SearchReply::from_body(self.kind, body),
            // A runtime older than this search does not know its requests (specification 34
            // TARGET-05).
            Err(ControlError::Refused { error, .. })
                if self.is_search_read() && error.starts_with("unsupported request") =>
            {
                SearchReply::update_needed()
            }
            Err(error) => SearchReply::failed("runtime_unreachable", error.to_string()),
        }
    }
}

impl DevicesEngine {
    /// One search (design §1.4.1): on this computer in the language-server process, or inside the
    /// runtime the session showed under the request's origin. A reply that carries a status also
    /// carries the Discover view of it, of the whole search even for a poll with `since`; a failed
    /// search that the old pane had a text for carries the view of its failure (specification 25 §2).
    /// A status that names no search answers the idle view.
    #[must_use]
    pub fn search(&self, call: &SearchCall) -> SearchReply {
        if call.verb == SearchVerb::Start {
            // The lists belong to the search they were asked for: a new search starts without them.
            lock(&self.panel(&call.session, Instant::now()))
                .choices
                .compare = false;
        }
        if let Err(text) = self.record_change(call) {
            return self.with_memory_failure(call, text);
        }
        if call.verb == SearchVerb::Status && call.names_no_search() {
            return self.idle(call);
        }
        let reply = match &call.origin {
            SearchOrigin::ThisHost => self.search_here(call),
            SearchOrigin::Runtime(item) => self.search_at_runtime(call, item),
        };
        if call.verb == SearchVerb::Status && call.since.is_some() {
            return self.with_whole_discover(call, reply);
        }
        self.with_discover(call, reply)
    }

    /// The Discover view before any search (specification 25 §2): the idle view of the origin the
    /// request names, built from the panel's topology.
    fn idle(&self, call: &SearchCall) -> SearchReply {
        let runtimes = self.runtime_origins(call);
        let view = match self.discover_context(call) {
            Ok(ctx) => discover::idle(&call.scan(), &runtimes, &ctx),
            Err(text) => discover::failed(&call.scan(), &runtimes, &Context::default(), text),
        };
        SearchReply {
            discover: Some(Box::new(view)),
            ..SearchReply::default()
        }
    }

    /// The reply of a request whose change could not be recorded: the pane shows the text, and its idle view
    /// (specification 25 §2).
    fn with_memory_failure(&self, call: &SearchCall, text: String) -> SearchReply {
        let runtimes = self.runtime_origins(call);
        let view = discover::failed(&call.scan(), &runtimes, &Context::default(), text);
        SearchReply {
            discover: Some(Box::new(view)),
            ..SearchReply::default()
        }
    }

    /// A poll with `since` lists only the devices that changed after its cursor (specification 11,
    /// "One search"), while the pane keeps one card per device of the whole search (specification 25
    /// §2). So the Discover view of such a reply is projected from the whole status, which the search
    /// answers without `since`.
    fn with_whole_discover(&self, call: &SearchCall, reply: SearchReply) -> SearchReply {
        if reply.error.is_some() {
            return self.with_failure(call, reply);
        }
        let whole = self.search(&SearchCall {
            since: None,
            ..call.clone()
        });
        match whole.error {
            None => SearchReply {
                discover: whole.discover,
                ..reply
            },
            Some(_) => whole,
        }
    }

    /// Adds the Discover view to a reply that carries a status (specification 25 §2): the cards,
    /// the source rows and the scan choices that the pane draws from it.
    fn with_discover(&self, call: &SearchCall, reply: SearchReply) -> SearchReply {
        if reply.error.is_some() {
            return self.with_failure(call, reply);
        }
        if reply.status.is_empty() {
            return reply;
        }
        let runtimes = self.runtime_origins(call);
        let ctx = match self.discover_context(call) {
            Ok(ctx) => ctx,
            Err(text) => {
                let view = discover::failed(&call.scan(), &runtimes, &Context::default(), text);
                return SearchReply {
                    discover: Some(Box::new(view)),
                    ..reply
                };
            }
        };
        match discover::project_in(
            &reply.status,
            &call.scan(),
            &runtimes,
            &ctx,
            super::now_millis(),
        ) {
            Ok(view) => SearchReply {
                discover: Some(Box::new(view)),
                ..reply
            },
            Err(error) => SearchReply::failed("search_status_unreadable", error.to_string()),
        }
    }

    /// Adds the view of a search that failed at a runtime, with the text the old pane gave that failure
    /// (specification 25 §2). A failure that the old pane had no text for, and a failed cancel, keep
    /// only their error reply.
    fn with_failure(&self, call: &SearchCall, reply: SearchReply) -> SearchReply {
        let text = reply.error.as_ref().and_then(|error| match call.verb {
            SearchVerb::Start | SearchVerb::Status => {
                discover_names::failure_text(&error.code, &error.message)
            }
            SearchVerb::Cancel => None,
        });
        match text {
            Some(text) => {
                let runtimes = self.runtime_origins(call);
                // A project file that cannot be read is the text the pane shows, not the runtime's failure.
                let view = match self.discover_context(call) {
                    Ok(ctx) => discover::failed(&call.scan(), &runtimes, &ctx, text),
                    Err(memory) => {
                        discover::failed(&call.scan(), &runtimes, &Context::default(), memory)
                    }
                };
                SearchReply {
                    discover: Some(Box::new(view)),
                    ..reply
                }
            }
            None => reply,
        }
    }

    /// What the Discover pane knows beside the search (specification 25 §2): the project's remembered choices and
    /// ignored finds, the panel's `Show ignored` tick and its request for the lists of `Compare with project`, the
    /// devices that the connections on the canvas name, the configured ADS ports, and the facts of each origin
    /// runtime, read from the panel's last gather. A project file that cannot be read is an error, with its text.
    fn discover_context(&self, call: &SearchCall) -> Result<Context, String> {
        let panel = self.panel(&call.session, Instant::now());
        let mut shown = lock(&panel);
        let session_current = call.search_id.as_deref().is_none_or(|id| {
            shown
                .searches
                .iter()
                .any(|search| owned_search_id(search) == id)
        });
        let show_ignored = shown.choices.show_ignored;
        let compare = shown.choices.compare;
        let session = shown.session(Instant::now());
        let known: Vec<KnownRuntime> = session.runtimes().to_vec();
        let project = session.project_folder().map(Path::to_path_buf);
        let memory = match &project {
            Some(folder) => project_choices::load(folder)
                .map_err(|reason| texts::devices_file_unreadable(&reason))?,
            None => Memory::default(),
        };
        let project = discover_compare::project_of(&known);
        let gathered = shown.gathered.as_ref();
        let facts = gathered.map(|gathered| inputs::read(gathered, &call.host, None).facts);
        let origins: BTreeMap<String, OriginFacts> = known
            .iter()
            .map(|runtime| {
                let facts = origin_facts(gathered, facts.as_ref(), runtime);
                (runtime.id.as_str().to_owned(), facts)
            })
            .collect();
        Ok(Context {
            memory,
            show_ignored,
            compare,
            session_current,
            project,
            ads_ports: call.host.settings.ads_discovery_ports.clone(),
            origins,
        })
    }

    /// Records the change of the Discover pane that a request carries (specification 25 §2, decision O14): a
    /// tick and an Ignore are saved in the project's `devices.toml`; `Show ignored` and the lists of `Compare
    /// with project` are the panel's own.
    ///
    /// # Errors
    /// A tick or an Ignore needs an open project, and the project's file must be readable and writable; the
    /// text says which.
    fn record_change(&self, call: &SearchCall) -> Result<(), String> {
        let Some(change) = &call.change else {
            return Ok(());
        };
        let panel = self.panel(&call.session, Instant::now());
        match change {
            DiscoverChange::ShowIgnored { shown } => {
                lock(&panel).choices.show_ignored = *shown;
                return Ok(());
            }
            DiscoverChange::Compare { shown } => {
                lock(&panel).choices.compare = *shown;
                return Ok(());
            }
            DiscoverChange::Select { .. } | DiscoverChange::Ignore { .. } => {}
        }
        let folder = lock(&panel)
            .session(Instant::now())
            .project_folder()
            .map(Path::to_path_buf)
            .ok_or_else(|| texts::CHOOSE_PROJECT_TO_REMEMBER.to_owned())?;
        let mut memory = project_choices::load(&folder)
            .map_err(|reason| texts::devices_file_unreadable(&reason))?;
        match change {
            DiscoverChange::Select { source, checked } => {
                let mut ticked = memory
                    .protocols
                    .take()
                    .unwrap_or_else(|| vec![SourceId::Ads.as_str().to_owned()]);
                ticked.retain(|name| name != source);
                if *checked {
                    ticked.push(source.clone());
                }
                memory.protocols = Some(ticked);
            }
            DiscoverChange::Ignore { device, ignored } => {
                if *ignored {
                    memory.ignored.insert(device.clone());
                } else {
                    memory.ignored.remove(device);
                }
            }
            DiscoverChange::ShowIgnored { .. } | DiscoverChange::Compare { .. } => {}
        }
        project_choices::save(&folder, &memory)
            .map_err(|reason| texts::devices_file_unsaved(&reason))
    }

    /// The runtimes the panel offers as origins of the search, each with whether it is ready
    /// (specification 25 §2): the ones the session's last view showed, read against the panel's
    /// last gather.
    fn runtime_origins(&self, call: &SearchCall) -> Vec<RuntimeOrigin> {
        let panel = self.panel(&call.session, Instant::now());
        let mut shown = lock(&panel);
        let known = shown.session(Instant::now()).runtimes().to_vec();
        discover::runtime_origins(&known, shown.gathered.as_ref(), &call.host)
    }

    /// One search of this computer (design §1.4.1), through the registry of this process.
    fn search_here(&self, call: &SearchCall) -> SearchReply {
        match call.verb {
            SearchVerb::Start => {
                let started = SearchRequest::from_params(call.params())
                    .and_then(|request| self.searches.start(request, SearchHost::ThisHost));
                match started {
                    Ok(search_id) => {
                        self.remember_search(&call.session, OwnedSearch::Here(search_id.clone()));
                        SearchReply::started(search_id)
                    }
                    Err(error) => search_error(&error),
                }
            }
            SearchVerb::Status => match call.named_search() {
                Ok(search_id) => match self.searches.status(&search_id, call.since) {
                    Ok(status) => SearchReply::status(status),
                    Err(error) => search_error(&error),
                },
                Err(refused) => refused,
            },
            SearchVerb::Cancel => match call.named_search() {
                Ok(search_id) => match self.searches.cancel(&search_id) {
                    Ok(state) => SearchReply::cancelled(search_id, state),
                    Err(error) => search_error(&error),
                },
                Err(refused) => refused,
            },
        }
    }

    /// One search inside the runtime that the session showed under `item` (design §1.4.1). A start that names
    /// protocols which the runtime advertises beyond the ten sources runs them through the runtime's
    /// `comm.discover` (specification 25 §2): beside the runtime's own sources in the same search, or alone when
    /// it names no other source.
    fn search_at_runtime(&self, call: &SearchCall, item: &str) -> SearchReply {
        if let Some(reply) = self.own_search_reply(call) {
            return reply;
        }
        let beyond = match call.verb {
            SearchVerb::Start => self.advertised_named(call, item),
            SearchVerb::Status | SearchVerb::Cancel => Vec::new(),
        };
        if beyond.is_empty() {
            let reply = self.forwarded(call, item, Vec::new());
            return self.with_advertised(call, reply);
        }
        let runtime_names = names_except(&call.params, &beyond);
        if runtime_names.is_empty() {
            return self.start_alone(call, item, beyond);
        }
        let mut runtime_call = call.clone();
        runtime_call.params["only"] = json!(runtime_names);
        self.forwarded(&runtime_call, item, beyond)
    }

    /// One search that the runtime runs (design §1.4.1), with the advertised protocols `beyond` that the panel runs
    /// beside it when it starts. The forwarded request runs on a thread of its own. The answer is awaited for at
    /// most `ANSWER_WAIT`, so a runtime that does not answer in time reports `runtime_pending`, and the answer it
    /// gives later is kept for the session's close.
    fn forwarded(&self, call: &SearchCall, item: &str, beyond: Vec<Protocol>) -> SearchReply {
        let (panel, forward) = match self.forward_to_runtime(call, item) {
            Ok(forwarded) => forwarded,
            Err(refused) => return refused,
        };
        let beyond = (!beyond.is_empty()).then(|| {
            (
                beyond,
                scopes(call.targets.as_ref()),
                deadline(&call.params),
            )
        });
        let handoff: Arc<Handoff<SearchReply>> = Handoff::open();
        let job = Arc::clone(&handoff);
        let spawned = thread::Builder::new()
            .name("devices-search".to_owned())
            .spawn(move || {
                let answer = forward.ask();
                // A search that the runtime started is the session's, so closing the session can
                // end it, even when this answer comes after the request has left.
                if let Some(search_id) = answer.search_id.clone().filter(|_| forward.is_start()) {
                    lock(&panel).searches.push(OwnedSearch::Runtime {
                        endpoint: forward.endpoint.clone(),
                        token: forward.token.clone(),
                        search_id: search_id.clone(),
                    });
                    if let Some((protocols, request_scopes, limit)) = beyond {
                        let request = advertised::Request {
                            endpoint: forward.endpoint.clone(),
                            token: forward.token.clone(),
                            protocols,
                            scopes: request_scopes,
                            deadline: limit,
                        };
                        lock(&panel).searches.push(OwnedSearch::Advertised {
                            search_id,
                            scan: AdvertisedSearch::start(request),
                            engine_only: false,
                        });
                    }
                }
                // A late answer has no request left to wait for it, so it is dropped here.
                let _ = job.finish(answer);
            });
        if spawned.is_err() {
            return SearchReply::failed(
                "search_not_started",
                "The panel could not start the search's thread.",
            );
        }
        handoff.wait(ANSWER_WAIT).unwrap_or_else(|| {
            SearchReply::failed(
                "runtime_pending",
                "The runtime has not answered yet; the panel asks again.",
            )
        })
    }

    /// The runtime that the session showed under `item`, with the request that reaches its control
    /// endpoint. The session is read under its panel's lock only; the lock is released before any
    /// request is sent.
    fn forward_to_runtime(
        &self,
        call: &SearchCall,
        item: &str,
    ) -> Result<(Arc<Mutex<Panel>>, Forward), SearchReply> {
        let (endpoint, token) = self.runtime_reach(call, item)?;
        let (kind, params) = call.forwarded()?;
        Ok((
            self.panel(&call.session, Instant::now()),
            Forward {
                endpoint,
                token,
                kind,
                params,
            },
        ))
    }

    /// Records a search that `session` started, so that closing the session ends it (design
    /// §1.4.1).
    pub(super) fn remember_search(&self, session: &str, search: OwnedSearch) {
        let panel = self.panel(session, Instant::now());
        lock(&panel).searches.push(search);
    }

    /// Ends the searches of a closing session (design §1.4.1). A search of this computer is
    /// cancelled here. A runtime's search is cancelled over its control endpoint on a thread of its
    /// own, so that closing never waits for a runtime; when that cancel cannot be sent, the search
    /// ends at its own deadline.
    pub(super) fn end_searches(&self, searches: &[OwnedSearch]) {
        for search in searches {
            match search {
                OwnedSearch::Here(search_id) => {
                    // A search the registry no longer keeps ended long ago: nothing is left to
                    // cancel.
                    let _ = self.searches.cancel(search_id);
                }
                OwnedSearch::Advertised { scan, .. } => {
                    let _ = scan.cancel();
                }
                OwnedSearch::Runtime {
                    endpoint,
                    token,
                    search_id,
                } => {
                    let cancel = Forward {
                        endpoint: endpoint.clone(),
                        token: token.clone(),
                        kind: "comm.search.cancel",
                        params: json!({ "search_id": search_id }),
                    };
                    let _ = thread::Builder::new()
                        .name("devices-cancel".to_owned())
                        .spawn(move || {
                            let _ = cancel.ask();
                        });
                }
            }
        }
    }
}

/// The search ID of a search that the panel started, in either place it runs.
fn owned_search_id(search: &OwnedSearch) -> &str {
    match search {
        OwnedSearch::Here(search_id)
        | OwnedSearch::Runtime { search_id, .. }
        | OwnedSearch::Advertised { search_id, .. } => search_id.as_str(),
    }
}

/// The facts of one origin runtime (specification 25 §2): its role from its status, the protocols its
/// `comm.schema` lists with `discover` from the last gather, and its `EtherCAT` adapter from its connection.
fn origin_facts(
    gathered: Option<&Gathered>,
    facts: Option<&inputs::Facts>,
    runtime: &KnownRuntime,
) -> OriginFacts {
    let endpoint = runtime
        .control_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty());
    let role = match (endpoint, facts) {
        (Some(endpoint), Some(facts)) => {
            facts.statuses.get(endpoint).and_then(|status| status.role)
        }
        _ => None,
    };
    let discover = origin_discover(gathered, runtime);
    let ethercat_adapter = runtime
        .connections
        .iter()
        .find(|connection| connection.protocol == "ethercat")
        .and_then(|connection| connection.params.get("adapter"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    OriginFacts {
        role,
        ethercat_adapter,
        discover,
    }
}
