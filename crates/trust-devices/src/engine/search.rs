//! The one search of `trust-lsp.devicesSearch` (design §1.4.1; specification 11, "One search"): on
//! this computer, through the search registry of this process, or inside a runtime the session
//! showed, through that runtime's control endpoint. A runtime's answer is awaited for at most
//! [`ANSWER_WAIT`](super::ANSWER_WAIT). The forwarded request itself runs on a thread of its own.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use trust_runtime::search::{SearchError, SearchHost, SearchRequest, SearchState, SearchStatus};
use trust_runtime_client::{ControlClient, ControlEndpoint, RESPONSE_LINE_LIMIT};

use super::handoff::Handoff;
use super::{lock, DevicesEngine, OwnedSearch, Panel, ReplyError, ANSWER_WAIT, CONTROL_TIMEOUT};
use crate::request::{HostFacts, Token};
use crate::session::Session;

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
    /// The facts that only VS Code knows: the token of the runtime a forwarded search reaches.
    #[serde(default)]
    pub host: HostFacts,
}

impl SearchCall {
    /// The `comm.search.start` parameters, or none when the request sends an empty body (which is
    /// a search with every default, as `comm.search.start` reads it).
    #[must_use]
    fn params(&self) -> Option<Value> {
        match &self.params {
            Value::Null => None,
            params => Some(params.clone()),
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
    /// The body of a `status` answer, whose every member this reply holds (design §1.4.1).
    #[serde(flatten)]
    pub status: Map<String, Value>,
}

impl SearchReply {
    /// The answer to a `start` verb.
    #[must_use]
    fn started(search_id: String) -> Self {
        Self {
            search_id: Some(search_id),
            ..Self::default()
        }
    }

    /// The answer to a `cancel` verb.
    #[must_use]
    fn cancelled(search_id: String, state: SearchState) -> Self {
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
    fn failed(code: &str, message: impl Into<String>) -> Self {
        Self {
            error: Some(ReplyError::new(code, message)),
            ..Self::default()
        }
    }
}

/// The answer of a search request that the registry refused.
fn search_error(error: &SearchError) -> SearchReply {
    SearchReply::failed(error.code().unwrap_or("invalid_search"), error.to_string())
}

/// The control endpoint of the runtime that the session showed under `item`.
fn runtime_endpoint(session: &Session, item: &str) -> Result<String, SearchReply> {
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
            Err(error) => SearchReply::failed("runtime_unreachable", error.to_string()),
        }
    }
}

impl DevicesEngine {
    /// One search (design §1.4.1): on this computer in the language-server process, or inside the
    /// runtime the session showed under the request's origin.
    #[must_use]
    pub fn search(&self, call: &SearchCall) -> SearchReply {
        match &call.origin {
            SearchOrigin::ThisHost => self.search_here(call),
            SearchOrigin::Runtime(item) => self.search_at_runtime(call, item),
        }
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

    /// One search inside the runtime that the session showed under `item` (design §1.4.1). The
    /// forwarded request runs on a thread of its own. The answer is awaited for at most
    /// `ANSWER_WAIT`, so a runtime that does not answer in time reports `runtime_pending`, and the
    /// answer it gives later is kept for the session's close.
    fn search_at_runtime(&self, call: &SearchCall, item: &str) -> SearchReply {
        let (panel, forward) = match self.forward_to_runtime(call, item) {
            Ok(forwarded) => forwarded,
            Err(refused) => return refused,
        };
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
                        search_id,
                    });
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
        let panel = self.panel(&call.session, Instant::now());
        let endpoint = {
            let mut shown = lock(&panel);
            runtime_endpoint(shown.session(Instant::now()), item)?
        };
        let token = call.host.token(&endpoint).cloned();
        let (kind, params) = call.forwarded()?;
        Ok((
            panel,
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
    fn remember_search(&self, session: &str, search: OwnedSearch) {
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
