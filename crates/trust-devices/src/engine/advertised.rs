//! The protocols that a runtime advertises beyond the ten sources (specification 25 §2). A search that ticks one
//! runs it through the origin runtime's `comm.discover` for that protocol, one request for each target, as the old
//! pane did, and shows its finds in the same search as the runtime's own sources. The runtime's search names only
//! the ten sources (specification 11, "One search"), so the panel runs these scans itself.

use std::collections::BTreeSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use trust_runtime::search::{SearchState, DEFAULT_DEADLINE_MS, MAX_DEADLINE_MS};
use trust_runtime_client::{ControlClient, ControlEndpoint, ControlError, RESPONSE_LINE_LIMIT};

use super::search::{runtime_endpoint, SearchCall, SearchReply, SearchVerb, TypedTargets};
use super::{lock, now_millis, DevicesEngine, OwnedSearch, CONTROL_TIMEOUT};
use crate::gather::Gathered;
use crate::request::Token;
use crate::session::KnownRuntime;
use crate::view::discover_context::{self, Discover};
use crate::view::discover_names::runtime_failure_text;

/// One protocol that the origin runtime advertises beyond the ten sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Protocol {
    /// Its protocol ID, as the runtime's `comm.schema` lists it.
    pub(crate) id: String,
    /// The label that the runtime advertises, which the panel shows.
    pub(crate) label: String,
}

/// What a scan runs: the origin runtime's control endpoint and token, the protocols, the `scope` of each request
/// (one for each target, or one with no target) and the search's deadline.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    /// The runtime's control endpoint.
    pub(crate) endpoint: String,
    /// The token that the extension holds for the endpoint, when it holds one.
    pub(crate) token: Option<Token>,
    /// The protocols to scan, in the order the search names them.
    pub(crate) protocols: Vec<Protocol>,
    /// The `scope` of each `comm.discover` request of each protocol.
    pub(crate) scopes: Vec<Value>,
    /// How long the scan may run; it starts no request after this.
    pub(crate) deadline: Duration,
}

/// Where the scan of one protocol stands.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// Not started; the search ended first.
    Waiting,
    /// Asking the runtime.
    Searching,
    /// Asked for every target and done.
    Found,
    /// Refused, or the runtime could not be asked; the reason the panel shows.
    Skipped(String),
}

/// One protocol's source in the search: its progress and what it found.
#[derive(Debug, Clone)]
struct Slot {
    advertised: Protocol,
    step: Step,
    count: u32,
    warnings: Vec<String>,
}

/// What the scan threads write and the status reads.
#[derive(Debug)]
struct Progress {
    slots: Vec<Slot>,
    devices: Vec<Value>,
    /// The protocol and the candidate ID of each find kept, so that a candidate found again from another target
    /// adds nothing.
    seen: BTreeSet<(String, String)>,
    /// The change counter: one for each find that the scan added (`cursor` of a status).
    cursor: u64,
    /// When the search was cancelled, if it was while it ran.
    cancelled_at: Option<Instant>,
    /// When the scan ended, if it ended before its deadline.
    ended_at: Option<Instant>,
}

/// The advertised protocols of one search, scanned on a thread of their own.
#[derive(Debug)]
pub(crate) struct AdvertisedSearch {
    started: Instant,
    deadline: Duration,
    progress: Mutex<Progress>,
}

/// Why one `comm.discover` request failed: the runtime refused it, or it could not be asked.
enum Failure {
    Refused(String),
    Unreachable(String),
}

impl Failure {
    /// The reason the source shows: the runtime's own text, or the failure text of the discovery named `label`.
    fn reason(self, label: &str) -> String {
        match self {
            Self::Refused(text) => text,
            Self::Unreachable(message) => runtime_failure_text(label, &message),
        }
    }
}

impl AdvertisedSearch {
    /// Starts the scan of `request` on a thread of its own, and answers the search's progress as it runs.
    #[must_use]
    pub(crate) fn start(request: Request) -> Arc<Self> {
        let slots = request
            .protocols
            .iter()
            .map(|advertised| Slot {
                advertised: advertised.clone(),
                step: Step::Waiting,
                count: 0,
                warnings: Vec::new(),
            })
            .collect();
        let search = Arc::new(Self {
            started: Instant::now(),
            deadline: request.deadline,
            progress: Mutex::new(Progress {
                slots,
                devices: Vec::new(),
                seen: BTreeSet::new(),
                cursor: 0,
                cancelled_at: None,
                ended_at: None,
            }),
        });
        let worker = Arc::clone(&search);
        let spawned = thread::Builder::new()
            .name("devices-advertised".to_owned())
            .spawn(move || worker.run(&request));
        if spawned.is_err() {
            search.end_unstarted("The panel could not start the scan of this protocol.");
        }
        search
    }

    /// Asks the runtime for each protocol and each target, in order, and records what it answers. A protocol is
    /// `found` only once every one of its requests was answered; one that the deadline or a cancel stops keeps its
    /// step, which the status shows as `timed out` or `cancelled`.
    fn run(&self, request: &Request) {
        for (index, advertised) in request.protocols.iter().enumerate() {
            let mut complete = true;
            for scope in &request.scopes {
                if !self.running(Instant::now()) {
                    complete = false;
                    break;
                }
                self.set_step(index, Step::Searching);
                let answered = ask(
                    &request.endpoint,
                    request.token.as_ref(),
                    &advertised.id,
                    scope,
                );
                // An answer or a failure that arrives after the deadline or a cancel is dropped, as the runtime's
                // own search drops its late results (specification 11, "Search").
                if !self.running(Instant::now()) {
                    complete = false;
                    break;
                }
                match answered {
                    Ok(answer) => self.add(index, &answer, now_millis()),
                    Err(failure) => {
                        self.set_step(index, Step::Skipped(failure.reason(&advertised.label)));
                        complete = false;
                        break;
                    }
                }
            }
            if complete {
                self.set_step(index, Step::Found);
            }
        }
        self.end();
    }

    /// Whether the scan still runs at `now`: it is neither cancelled nor ended, and its deadline has not passed.
    fn running(&self, now: Instant) -> bool {
        let progress = self.lock();
        progress.ended_at.is_none()
            && progress.cancelled_at.is_none()
            && now < self.started + self.deadline
    }

    /// Records the step of the protocol at `index`.
    fn set_step(&self, index: usize, step: Step) {
        self.lock().slots[index].step = step;
    }

    /// Keeps the candidates of one answer, and its warnings. A candidate kept before, from another target, adds
    /// nothing; a new one joins the device that shares its address, or is a device of its own (`join`).
    fn add(&self, index: usize, answer: &Value, now_ms: u64) {
        let mut guard = self.lock();
        let progress = &mut *guard;
        let protocol = progress.slots[index].advertised.id.clone();
        for candidate in answer
            .get("candidates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(key) = candidate.get("id").and_then(Value::as_str) else {
                progress.slots[index]
                    .warnings
                    .push("A result without an ID was left out.".to_owned());
                continue;
            };
            if !progress.seen.insert((protocol.clone(), key.to_owned())) {
                continue;
            }
            join(
                &mut progress.devices,
                device_of(&protocol, key, candidate, now_ms),
            );
            progress.cursor += 1;
            progress.slots[index].count += 1;
        }
        let warnings = answer
            .get("warnings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned);
        progress.slots[index].warnings.extend(warnings);
    }

    /// Ends the scan once every protocol is done, unless it was cancelled first.
    fn end(&self) {
        self.lock().ended_at.get_or_insert_with(Instant::now);
    }

    /// Ends a scan that never started: every protocol is skipped with `reason`.
    fn end_unstarted(&self, reason: &str) {
        let mut progress = self.lock();
        for slot in &mut progress.slots {
            slot.step = Step::Skipped(reason.to_owned());
        }
        progress.ended_at = Some(Instant::now());
    }

    /// Cancels the search while it runs; answers its state afterwards: cancelled, or done when it had ended.
    #[must_use]
    pub(crate) fn cancel(&self) -> SearchState {
        let now = Instant::now();
        if self.running(now) {
            self.lock().cancelled_at = Some(now);
        }
        self.state(now)
    }

    /// The state at `now`: running, cancelled or done.
    #[must_use]
    pub(crate) fn state(&self, now: Instant) -> SearchState {
        if self.running(now) {
            SearchState::Running
        } else if self.lock().cancelled_at.is_some() {
            SearchState::Cancelled
        } else {
            SearchState::Done
        }
    }

    /// The source rows of the advertised protocols, as `comm.search.status` lists them (`warnings` only where some).
    #[must_use]
    pub(crate) fn sources(&self, now: Instant) -> Vec<Value> {
        let running = self.running(now);
        let progress = self.lock();
        let cancelled = progress.cancelled_at.is_some();
        progress
            .slots
            .iter()
            .map(|slot| {
                let mut row = Map::new();
                row.insert("id".to_owned(), json!(slot.advertised.id));
                match &slot.step {
                    Step::Skipped(reason) => {
                        row.insert("state".to_owned(), json!("skipped"));
                        row.insert("reason".to_owned(), json!(reason));
                    }
                    Step::Found => {
                        row.insert("state".to_owned(), json!("found"));
                        row.insert("count".to_owned(), json!(slot.count));
                    }
                    Step::Waiting | Step::Searching => {
                        let state = match (running, cancelled) {
                            (true, _) => "searching",
                            (false, true) => "cancelled",
                            (false, false) => "timed_out",
                        };
                        row.insert("state".to_owned(), json!(state));
                        row.insert("count".to_owned(), json!(slot.count));
                    }
                }
                if !slot.warnings.is_empty() {
                    row.insert("warnings".to_owned(), json!(slot.warnings));
                }
                Value::Object(row)
            })
            .collect()
    }

    /// The devices that the advertised protocols found so far.
    #[must_use]
    pub(crate) fn devices(&self) -> Vec<Value> {
        self.lock().devices.clone()
    }

    /// The whole status of a search that the panel runs alone (only advertised protocols), as `comm.search.status`
    /// answers it for the runtime's searches.
    #[must_use]
    pub(crate) fn status(&self, search_id: &str, origin: &str, now: Instant) -> Map<String, Value> {
        let elapsed = now
            .saturating_duration_since(self.started)
            .min(self.deadline);
        let cursor = self.lock().cursor;
        let mut status = Map::new();
        status.insert("schema_version".to_owned(), json!(1));
        status.insert("search_id".to_owned(), json!(search_id));
        status.insert("origin".to_owned(), json!(origin));
        status.insert("state".to_owned(), json!(state_word(self.state(now))));
        status.insert("deadline_ms".to_owned(), json!(millis(self.deadline)));
        status.insert("elapsed_ms".to_owned(), json!(millis(elapsed)));
        status.insert("cursor".to_owned(), json!(cursor));
        status.insert("sources".to_owned(), Value::Array(self.sources(now)));
        status.insert("devices".to_owned(), Value::Array(self.devices()));
        status.insert("warnings".to_owned(), json!([]));
        status
    }

    /// Adds the advertised protocols to a status of the runtime's own search: their source rows and devices, and
    /// the state of the whole search (running while either part runs, cancelled when either was cancelled).
    pub(crate) fn merge_into(&self, status: &mut Map<String, Value>, now: Instant) {
        let runtime_state = status
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("done")
            .to_owned();
        let own = state_word(self.state(now));
        let state = if runtime_state == "running" || own == "running" {
            "running"
        } else if runtime_state == "cancelled" || own == "cancelled" {
            "cancelled"
        } else {
            "done"
        };
        status.insert("state".to_owned(), json!(state));
        append(status, "sources", self.sources(now));
        let entry = status
            .entry("devices".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(devices) = entry {
            for device in self.devices() {
                join(devices, device);
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, Progress> {
        self.progress.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Asks the runtime for one protocol with one `scope` (specification 11, "comm.discover"): the request runs in the
/// runtime's own origin, with the same token as the panel's other requests to it.
fn ask(
    endpoint: &str,
    token: Option<&Token>,
    protocol: &str,
    scope: &Value,
) -> Result<Value, Failure> {
    let Some(parsed) = ControlEndpoint::parse(endpoint) else {
        return Err(Failure::Unreachable(format!(
            "'{endpoint}' is not a control endpoint."
        )));
    };
    let mut client = ControlClient::connect(&parsed, token.map(Token::secret), CONTROL_TIMEOUT)
        .map_err(|error| Failure::Unreachable(error.to_string()))?
        .with_response_limit(RESPONSE_LINE_LIMIT);
    client
        .request(
            "comm.discover",
            Some(json!({ "protocol": protocol, "origin": "runtime", "scope": scope })),
        )
        .map_err(|error| match error {
            ControlError::Refused { error, .. } => Failure::Refused(error),
            other => Failure::Unreachable(other.to_string()),
        })
}

/// The device that the `comm.discover` candidate `key` makes: its address when its parameters name a host, and one
/// find of the protocol. Its ID follows the runtime's search (specification 11, "Devices"): `dev:ip:{host}` for an
/// address, else `dev:{protocol}:{key}` for a find that forms its own device.
fn device_of(protocol: &str, key: &str, candidate: &Value, now_ms: u64) -> Value {
    let params = candidate
        .get("params")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let host = params
        .get("host")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(str::to_owned);
    let name = candidate
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or(key)
        .to_owned();
    let warnings = candidate
        .get("warnings")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let id = host.as_ref().map_or_else(
        || format!("dev:{protocol}:{key}"),
        |host| format!("dev:ip:{host}"),
    );
    json!({
        "id": id,
        "addresses": host.into_iter().collect::<Vec<String>>(),
        "last_seen_ms": now_ms,
        "finds": [{
            "protocol": protocol,
            "source": candidate.get("source").and_then(Value::as_str).unwrap_or_default(),
            "confidence": candidate.get("confidence").and_then(Value::as_str).unwrap_or_default(),
            "simulated": false,
            "identity": { "name": name },
            "params": params,
            "warnings": warnings,
            "last_seen_ms": now_ms,
        }],
    })
}

/// Adds `device` to `devices` by the rule of the runtime's search (specification 11, "Devices"): when it shares an
/// address with one of them, its finds join that device, which keeps its ID and takes the later `last_seen_ms`;
/// otherwise it is a device of its own.
fn join(devices: &mut Vec<Value>, device: Value) {
    let Some(known) = devices.iter_mut().find(|known| {
        addresses(known).any(|address| addresses(&device).any(|other| other == address))
    }) else {
        devices.push(device);
        return;
    };
    let joining = device
        .get("finds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(finds) = known.get_mut("finds").and_then(Value::as_array_mut) {
        finds.extend(joining);
    }
    let seen = |device: &Value| {
        device
            .get("last_seen_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let latest = seen(known).max(seen(&device));
    known["last_seen_ms"] = json!(latest);
}

/// The network addresses of a device of a status.
fn addresses(device: &Value) -> impl Iterator<Item = &str> {
    device
        .get("addresses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

/// Appends `items` to the array at `key` of `status`, which is created when it is missing.
fn append(status: &mut Map<String, Value>, key: &str, items: Vec<Value>) {
    let entry = status
        .entry(key.to_owned())
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Value::Array(array) = entry {
        array.extend(items);
    }
}

/// The word of a state in a status (`running`, `done` or `cancelled`).
fn state_word(state: SearchState) -> &'static str {
    match state {
        SearchState::Running => "running",
        SearchState::Done => "done",
        SearchState::Cancelled => "cancelled",
    }
}

/// A duration in whole milliseconds.
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

impl DevicesEngine {
    /// The protocols that a start names and that the runtime advertises beyond the ten sources, in the
    /// order the runtime lists them. A name that no runtime advertises is not among them: the runtime refuses
    /// it as it refuses any unknown source.
    pub(super) fn advertised_named(&self, call: &SearchCall, item: &str) -> Vec<Protocol> {
        let named = listed_names(&call.params);
        if named.is_empty() {
            return Vec::new();
        }
        self.advertised_by(&call.session, item)
            .into_iter()
            .filter(|protocol| named.contains(&protocol.id))
            .collect()
    }

    /// The protocols that the origin runtime `item` advertises beyond the ten sources, from the panel's last gather.
    fn advertised_by(&self, session: &str, item: &str) -> Vec<Protocol> {
        let panel = self.panel(session, Instant::now());
        let mut shown = lock(&panel);
        let runtime = shown.session(Instant::now()).runtime(item).cloned();
        let Some(runtime) = runtime else {
            return Vec::new();
        };
        origin_discover(shown.gathered.as_ref(), &runtime)
            .map(|discover| {
                discover
                    .beyond
                    .iter()
                    .map(|advertised| Protocol {
                        id: advertised.id.clone(),
                        label: advertised.label.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The control endpoint and token of the runtime that the session showed under `item`, for a search that the
    /// panel runs itself.
    pub(super) fn runtime_reach(
        &self,
        call: &SearchCall,
        item: &str,
    ) -> Result<(String, Option<Token>), SearchReply> {
        let panel = self.panel(&call.session, Instant::now());
        let endpoint = {
            let mut shown = lock(&panel);
            runtime_endpoint(shown.session(Instant::now()), item)?
        };
        let token = call.host.token(&endpoint).cloned();
        Ok((endpoint, token))
    }

    /// A search of only the protocols that the runtime advertises: the panel runs them, and the runtime runs nothing
    /// (specification 25 §2). The answer's ID is the panel's own.
    pub(super) fn start_alone(
        &self,
        call: &SearchCall,
        item: &str,
        beyond: Vec<Protocol>,
    ) -> SearchReply {
        let (endpoint, token) = match self.runtime_reach(call, item) {
            Ok(reach) => reach,
            Err(refused) => return refused,
        };
        let search_id = format!(
            "a-{}",
            self.advertised_searches.fetch_add(1, Ordering::Relaxed) + 1
        );
        let scan = AdvertisedSearch::start(scan_request(call, endpoint, token, beyond));
        self.remember_search(
            &call.session,
            OwnedSearch::Advertised {
                search_id: search_id.clone(),
                scan,
                engine_only: true,
            },
        );
        SearchReply::started(search_id)
    }

    /// The answer of a status or a cancel of a search that the panel runs alone (specification 25 §2); `None` for
    /// every other search.
    pub(super) fn own_search_reply(&self, call: &SearchCall) -> Option<SearchReply> {
        if call.verb == SearchVerb::Start {
            return None;
        }
        let search_id = call
            .search_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())?;
        let (scan, engine_only) = self.advertised_of(&call.session, search_id)?;
        if !engine_only {
            return None;
        }
        match call.verb {
            SearchVerb::Status => Some(SearchReply {
                status: scan.status(search_id, "runtime", Instant::now()),
                ..SearchReply::default()
            }),
            SearchVerb::Cancel => Some(SearchReply::cancelled(search_id.to_owned(), scan.cancel())),
            SearchVerb::Start => None,
        }
    }

    /// The advertised protocols of the search `search_id` that the session started, and whether the panel runs that
    /// search alone.
    fn advertised_of(
        &self,
        session: &str,
        search_id: &str,
    ) -> Option<(Arc<AdvertisedSearch>, bool)> {
        let panel = self.panel(session, Instant::now());
        let shown = lock(&panel);
        shown.searches.iter().find_map(|search| match search {
            OwnedSearch::Advertised {
                search_id: id,
                scan,
                engine_only,
            } if id == search_id => Some((Arc::clone(scan), *engine_only)),
            _ => None,
        })
    }

    /// Adds the advertised protocols of a search that the runtime runs to its status (specification 25 §2): their
    /// rows and devices, and the state of the whole search. A cancel also ends their scan.
    pub(super) fn with_advertised(&self, call: &SearchCall, mut reply: SearchReply) -> SearchReply {
        let Some(search_id) = call
            .search_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            return reply;
        };
        let Some((scan, _)) = self.advertised_of(&call.session, search_id) else {
            return reply;
        };
        match call.verb {
            SearchVerb::Status if reply.error.is_none() => {
                scan.merge_into(&mut reply.status, Instant::now());
            }
            SearchVerb::Cancel => {
                let _ = scan.cancel();
            }
            SearchVerb::Status | SearchVerb::Start => {}
        }
        reply
    }
}

/// The names that `params` lists in `only`, which are not one of `beyond`, in the order they are listed.
pub(super) fn names_except(params: &Value, beyond: &[Protocol]) -> Vec<String> {
    listed_names(params)
        .into_iter()
        .filter(|name| !beyond.iter().any(|protocol| &protocol.id == name))
        .collect()
}

/// The names that `params` lists in `only` (specification 11, "Requests").
fn listed_names(params: &Value) -> Vec<String> {
    params
        .get("only")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// The `scope` of each `comm.discover` request of an advertised protocol: one for the Known address, one for the
/// subnet, or one with no target when neither is typed (specification 25 §2).
pub(super) fn scopes(targets: Option<&TypedTargets>) -> Vec<Value> {
    let Some(typed) = targets else {
        return vec![json!({})];
    };
    let mut scopes = Vec::new();
    let host = typed.hosts.trim();
    if !host.is_empty() {
        scopes.push(json!({ "host": host }));
    }
    let cidr = typed.cidr.trim();
    if !cidr.is_empty() {
        scopes.push(json!({ "cidr": cidr }));
    }
    if scopes.is_empty() {
        scopes.push(json!({}));
    }
    scopes
}

/// How long a scan of advertised protocols may run: the search's `deadline_ms` as the runtime reads it, from 1 to
/// 60,000 ms, and 10,000 ms when the request names none (specification 11, "Requests").
pub(super) fn deadline(params: &Value) -> Duration {
    let millis = params
        .get("deadline_ms")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_DEADLINE_MS)
        .clamp(1, MAX_DEADLINE_MS);
    Duration::from_millis(millis)
}

/// The request of a scan of `protocols` that runs from `endpoint` for `call`.
fn scan_request(
    call: &SearchCall,
    endpoint: String,
    token: Option<Token>,
    protocols: Vec<Protocol>,
) -> Request {
    Request {
        endpoint,
        token,
        protocols,
        scopes: scopes(call.targets.as_ref()),
        deadline: deadline(&call.params),
    }
}

/// The facts of one origin runtime's `comm.schema`: the protocols it lists with `discover`, from the panel's last
/// gather of it (specification 25 §2). `None` while the schema is unknown.
pub(super) fn origin_discover(
    gathered: Option<&Gathered>,
    runtime: &KnownRuntime,
) -> Option<Discover> {
    let endpoint = runtime
        .control_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())?;
    gathered?
        .targets
        .iter()
        .find(|report| report.target.endpoint.trim() == endpoint)
        .and_then(|report| match &report.live.schema {
            Some(Ok(schema)) => Some(discover_context::discover_from_schema(schema)),
            _ => None,
        })
}

#[cfg(test)]
#[path = "advertised_tests.rs"]
mod tests;
