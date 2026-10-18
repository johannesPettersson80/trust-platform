//! The panel engine that the five `trust-lsp.devices*` commands answer through (design §1.5): one
//! panel per open session, the gather, action and search work that runs on threads of its own, and
//! the results of slow actions that a later view delivers. Every request answers from what its
//! panel holds, so a language-server request, of which tower-lsp serves four at a time, is never
//! held while a target is asked (design §1.5).
//!
//! - `DevicesEngine::view` projects the gather the panel already has at once, starts the next
//!   gather in the background, and answers the first view of a panel with the loading shell.
//! - `DevicesEngine::act_with` performs the action a view offered on a thread of its own, on a copy
//!   of the panel's session. An action that is not done within `ANSWER_WAIT` answers `pending`, and
//!   its result is reported exactly once, in the `operations_done` of a later view.
//! - `DevicesEngine::browse` answers a loading browse view first and the tree in a later poll.
//! - `DevicesEngine::search` runs the one search of specification 11 on this computer, or in the
//!   runtime the session showed.
//! - `DevicesEngine::close` ends a session and the searches it started; a session that stays idle
//!   for [`IDLE_EXPIRY`](crate::session::IDLE_EXPIRY) is closed the same way (design §1.4.1).
//!
//! The engine adds no rule of its own to the panel: the view is [`project`](crate::view::project),
//! the actions are [`act_with`](crate::actions::act_with), the browse pane is
//! [`open`](crate::browse::open) and the search is
//! [`SearchRegistry`](trust_runtime::search::SearchRegistry). This module is the transport of the
//! process: what is asked, when, on which thread, and what each reply holds (design §1.6).

mod handoff;
mod search;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::json;
use trust_runtime::search::SearchRegistry;

use self::handoff::Handoff;
use crate::actions::{self, Fleet, SystemFleet};
use crate::browse;
use crate::gather::{gather, Budget, Cache, Gathered, ProjectInputs};
use crate::request::{ActRequest, BrowseRequest, GatherMode, Token, ViewRequest};
use crate::session::{Session, Sessions, IDLE_EXPIRY};
use crate::targets::TargetKey;
use crate::view;
use crate::view::types::{ActReply, BrowseView, DevicesView, OperationDone, Refresh};

pub use search::{SearchCall, SearchOrigin, SearchReply, SearchVerb};

/// The version of a devices command's reply (design §1.4.1 and §2.1).
pub const SCHEMA_VERSION: u32 = 1;
/// The soonest the panel asks again after an I/O change of the simulator (design §1.5).
pub const MIN_REFRESH_MS: u64 = 250;
/// How soon the panel asks again while it waits for its first gather or for the gather it has out.
pub const WAITING_REFRESH_MS: u64 = 200;
/// How soon the panel asks again while an action is pending (design §1.5).
pub const PENDING_REFRESH_MS: u64 = 500;
/// How long a devices request waits for its own background answer before it answers `pending`
/// (design §1.5: every devices command answers within about 50 ms).
const ANSWER_WAIT: Duration = Duration::from_millis(25);
/// How long one control request of a forwarded search or cancel may wait (design §1.5).
const CONTROL_TIMEOUT: Duration = Duration::from_secs(2);

/// The panel engine: the open panels, each behind its own lock, and the one search of this process.
#[derive(Debug, Default)]
pub struct DevicesEngine {
    /// The open panels by session, with when each was last used.
    panels: Mutex<HashMap<String, Handle>>,
    /// The searches this process runs, as `trust-runtime discover` runs them (design §1.4.1).
    searches: SearchRegistry,
    /// How long a gather waits, and how old an answer may be before a target is asked again
    /// (design §1.5).
    budget: Budget,
}

/// One open panel and when it was last used.
#[derive(Debug)]
struct Handle {
    /// The panel, behind its own lock: a view of one panel never waits for a slow action of another
    /// (design §1.5).
    panel: Arc<Mutex<Panel>>,
    /// When the panel was last used; one idle for longer than [`IDLE_EXPIRY`] is closed.
    last_used: Instant,
}

/// A search that a session started, which closing the session ends (design §1.4.1).
#[derive(Debug, Clone)]
enum OwnedSearch {
    /// A search of this computer, in the registry of this process.
    Here(String),
    /// A search that a runtime runs, which is cancelled over the runtime's control endpoint.
    Runtime {
        /// The runtime's control endpoint.
        endpoint: String,
        /// The token of that endpoint, when the extension holds one.
        token: Option<Token>,
        /// The search's ID at the runtime.
        search_id: String,
    },
}

/// The state of one panel: its session and what it gathered (design §1.5).
#[derive(Debug)]
struct Panel {
    /// The panel's identity.
    id: String,
    /// The panel's session: its newest `seq`, its operation results and the runtime facts its
    /// actions resolve from.
    sessions: Sessions,
    /// The gather a background thread has finished, with the cache that gathered it, until a view
    /// takes both. A gather holds that cache for its whole run and never this panel, so every
    /// request still answers while it runs (design §1.5).
    incoming: Arc<Mutex<Option<Finished>>>,
    /// Whether a gather is out.
    gathering: bool,
    /// The gather the last view was projected from, until a finished one replaces it.
    gathered: Option<Gathered>,
    /// When `gathered` ended; an `auto` gather asks the targets again only after its period.
    gathered_at: Option<Instant>,
    /// The item the last view showed as selected, whose runtime's recent events a gather reads
    /// (design §1.5).
    selected: Option<String>,
    /// How many results of slow actions no view has delivered yet (design §1.5).
    pending: usize,
    /// Whether a delivered result asked for a gather of the new state that no gather has started yet
    /// (design §1.4.1).
    refresh_due: bool,
    /// The searches this session started, which closing the session ends (design §1.4.1).
    searches: Vec<OwnedSearch>,
}

/// A finished gather, the cache that gathered it, and when it ended.
#[derive(Debug)]
struct Finished {
    gathered: Gathered,
    cache: Cache,
    at: Instant,
}

impl DevicesEngine {
    /// The engine without an open panel.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One view (design §1.4.1): the whole panel from what the panel holds, the results of the slow
    /// actions since the last view, and when to ask again. A gather that is missing or older than
    /// its period is started in the background; the first view of a panel is the loading shell
    /// (design §1.5).
    #[must_use]
    pub fn view(&self, request: ViewRequest) -> ViewReply {
        if request.session.trim().is_empty() {
            return ViewReply::refused(ReplyError::new(
                "invalid_session",
                "The request named no session.",
            ));
        }
        let now = Instant::now();
        let panel = self.panel(&request.session, now);
        let mut panel = lock(&panel);
        panel.view(request, &self.budget, now, now_millis())
    }

    /// Performs one action that a view offered (design §1.4.1), through the production
    /// [`SystemFleet`].
    #[must_use]
    pub fn act(&self, request: &ActRequest) -> ActReply {
        self.act_with(request, Arc::new(SystemFleet))
    }

    /// [`Self::act`] with the managed-runtime seam `fleet`: a test passes a fake, so no process is
    /// started. The action runs on a thread of its own, on a copy of its panel's session, so the
    /// panel is not locked while the action works (design §1.5). An action that is not done within
    /// `ANSWER_WAIT` answers `pending`; its result arrives in a later view.
    #[must_use]
    pub fn act_with(&self, request: &ActRequest, fleet: Arc<dyn Fleet + Send + Sync>) -> ActReply {
        let now = Instant::now();
        let panel = self.panel(&request.session, now);
        let (operation, copy) = {
            let mut shared = lock(&panel);
            let session = shared.session(now);
            let operation = session.begin_operation();
            let copy = session.action_copy(operation.clone());
            (operation, copy)
        };
        // The result stays outstanding until a view delivers it, or until this request answers it
        // directly.
        lock(&panel).pending += 1;
        let handoff: Arc<Handoff<ActReply>> = Handoff::open();
        let job = Box::new(ActionJob {
            panel: Arc::clone(&panel),
            handoff: Arc::clone(&handoff),
            request: request.clone(),
            fleet,
            copy,
            operation: operation.clone(),
        });
        let spawned = thread::Builder::new()
            .name("devices-act".to_owned())
            .spawn(move || ActionJob::run(job));
        if spawned.is_err() {
            let mut shared = lock(&panel);
            shared.pending = shared.pending.saturating_sub(1);
            return actions::error(
                &actions::pane(request),
                "The panel could not start the action.",
            );
        }
        let reply = handoff.wait(ANSWER_WAIT).unwrap_or_else(|| {
            actions::pending(
                operation,
                actions::pane(request),
                running_text(&request.op.action).to_owned(),
            )
        });
        if reply.pending.is_none() {
            // A direct answer is the whole result, and no view delivers it.
            let mut shared = lock(&panel);
            shared.pending = shared.pending.saturating_sub(1);
        }
        reply
    }

    /// Opens or refreshes the browse pane a request names (design §1.4.1): the loading browse view
    /// first, its tree in a later poll (design §1.5).
    #[must_use]
    pub fn browse(&self, request: &BrowseRequest) -> BrowseView {
        let now = Instant::now();
        let panel = self.panel(&request.session, now);
        let mut shared = lock(&panel);
        browse::open(shared.session(now), request)
    }

    /// Ends a session and the searches it started (design §1.4.1).
    #[must_use]
    pub fn close(&self, session: &str) -> CloseReply {
        let handle = lock(&self.panels).remove(session);
        let closed = handle.is_some();
        if let Some(handle) = handle {
            let searches = std::mem::take(&mut lock(&handle.panel).searches);
            self.end_searches(&searches);
        }
        CloseReply {
            session: session.to_owned(),
            closed,
        }
    }

    /// The panel of `session`, opened when it is new. Every panel that stayed idle for longer than
    /// [`IDLE_EXPIRY`] is closed first, with its searches (design §1.4.1).
    fn panel(&self, session: &str, now: Instant) -> Arc<Mutex<Panel>> {
        let mut panels = lock(&self.panels);
        let expired: Vec<String> = panels
            .iter()
            .filter(|(id, handle)| {
                *id != session && now.saturating_duration_since(handle.last_used) > IDLE_EXPIRY
            })
            .map(|(id, _)| id.clone())
            .collect();
        let ended: Vec<Vec<OwnedSearch>> = expired
            .iter()
            .filter_map(|id| panels.remove(id))
            .map(|handle| {
                let mut shared = lock(&handle.panel);
                std::mem::take(&mut shared.searches)
            })
            .collect();
        for searches in ended {
            self.end_searches(&searches);
        }
        let handle = panels
            .entry(session.to_owned())
            .or_insert_with(|| new_handle(session, now));
        handle.last_used = now;
        Arc::clone(&handle.panel)
    }
}

/// One action on its own thread (design §1.5): it works on a copy of its panel's session and hands
/// its answer to the request that started it.
struct ActionJob {
    /// The panel that started the action, locked only briefly to keep what the action queued.
    panel: Arc<Mutex<Panel>>,
    /// The hand-over to the request that waits for the answer.
    handoff: Arc<Handoff<ActReply>>,
    /// The action, as the view offered it.
    request: ActRequest,
    /// The managed-runtime seam of the action.
    fleet: Arc<dyn Fleet + Send + Sync>,
    /// The session copy that the action works on.
    copy: Session,
    /// The action's operation identity, as its pending reply names it.
    operation: String,
}

impl ActionJob {
    /// Performs the action and hands its answer on. An answer that comes after its request has left
    /// is delivered in a later view, exactly once, unless the action queued its own result, as a
    /// managed Start or Stop does (design §1.4.1).
    fn run(job: Box<Self>) {
        let Self {
            panel,
            handoff,
            request,
            fleet,
            mut copy,
            operation,
        } = *job;
        let reply = actions::act_with(&mut copy, &request, fleet.as_ref());
        lock(&panel).absorb(&mut copy);
        let Err(late) = handoff.finish(reply) else {
            return;
        };
        // An action that answers `pending` has queued its own result already.
        if late.pending.is_none() {
            lock(&panel).deliver(OperationDone {
                operation,
                notice: late.notice,
                effects: late.effects,
                close_pane: late.close_pane,
                refresh: late.refresh,
                form: late.form,
            });
        }
    }
}

impl Panel {
    /// A panel that has not gathered yet.
    fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            sessions: Sessions::default(),
            incoming: Arc::new(Mutex::new(None)),
            gathering: false,
            gathered: None,
            gathered_at: None,
            selected: None,
            pending: 0,
            refresh_due: false,
            searches: Vec::new(),
        }
    }

    /// The panel's session, opened when it is new and marked as used at `now`.
    fn session(&mut self, now: Instant) -> &mut Session {
        self.sessions.session(&self.id, now)
    }

    /// Keeps what an action queued on its copy of the session (design §1.5).
    fn absorb(&mut self, copy: &mut Session) {
        self.session(Instant::now()).absorb_action(copy);
    }

    /// Queues the result of an action whose request has left (design §1.5).
    fn deliver(&mut self, done: OperationDone) {
        self.session(Instant::now()).complete_operation(done);
    }

    /// One view of this panel (design §1.5): the gather it already has, the results of the slow
    /// actions, and the gather it starts for the next poll.
    fn view(
        &mut self,
        request: ViewRequest,
        budget: &Budget,
        now: Instant,
        now_ms: u64,
    ) -> ViewReply {
        self.install(now);
        let (newest, done, projected) = {
            let gathered = self.gathered.as_ref();
            let session = self.sessions.session(&self.id, now);
            let newest = session.accept(request.seq);
            let done = session.take_operations_done();
            let projected = gathered.map(|gathered| {
                let projected = view::project(gathered, &request, None, now_ms);
                session.remember(gathered, &projected);
                projected
            });
            (newest, done, projected)
        };
        self.pending = self.pending.saturating_sub(done.len());
        // A delivered result that asks for a refresh (design §1.4.1) makes the next poll come at
        // once, and a gather of the new state starts as soon as none is out.
        let asked = done
            .iter()
            .filter_map(|item| refresh_delay_ms(item.refresh))
            .min();
        self.refresh_due |= done.iter().any(|item| item.refresh == Refresh::Now);
        self.selected.clone_from(&request.view.selected);
        if self.refresh_due && !self.gathering {
            self.refresh_due = false;
            let refresh = ViewRequest {
                gather: GatherMode::Now,
                ..request.clone()
            };
            self.start_gather(&refresh, budget, now);
        } else if newest && !self.gathering {
            let ask = match request.gather {
                // Nothing is asked: the answer comes from what the session holds.
                GatherMode::Cached => false,
                GatherMode::Now => true,
                GatherMode::Auto => self
                    .gathered_at
                    .is_none_or(|at| now.saturating_duration_since(at) >= budget.period),
            };
            if ask {
                self.start_gather(&request, budget, now);
            }
        }
        let normal = if self.gathered.is_none() || self.gathering {
            WAITING_REFRESH_MS
        } else if self.pending > 0 {
            PENDING_REFRESH_MS
        } else {
            millis(budget.period)
        };
        let next_refresh_ms = asked.map_or(normal, |delay| delay.min(normal));
        let view =
            projected.unwrap_or_else(|| view::project(&nothing_gathered(), &request, None, now_ms));
        ViewReply {
            schema_version: SCHEMA_VERSION,
            session: request.session,
            seq: request.seq,
            next_refresh_ms,
            min_refresh_ms: MIN_REFRESH_MS,
            operations_done: done,
            view: Some(view),
            error: None,
        }
    }

    /// Installs the gather a background thread finished, with the cache that gathered it (design
    /// §1.5).
    fn install(&mut self, now: Instant) {
        let Some(finished) = lock(&self.incoming).take() else {
            return;
        };
        let Finished {
            gathered,
            cache,
            at,
        } = finished;
        *self.sessions.session(&self.id, now).cache_mut() = cache;
        self.gathered = Some(gathered);
        self.gathered_at = Some(at);
        self.gathering = false;
    }

    /// Asks every target again on a thread of its own (design §1.5). The thread holds the gather's
    /// cache and never this panel, so every request still answers while it runs.
    fn start_gather(&mut self, request: &ViewRequest, budget: &Budget, now: Instant) {
        let events_for = self.events_target();
        let incoming = Arc::clone(&self.incoming);
        let request = request.clone();
        let budget = *budget;
        let cache = std::mem::take(self.sessions.session(&self.id, now).cache_mut());
        self.gathering = true;
        let spawned = thread::Builder::new()
            .name("devices-gather".to_owned())
            .spawn(move || {
                let mut cache = cache;
                let gathered = gather(&request, events_for.as_ref(), &mut cache, &budget);
                *lock(&incoming) = Some(Finished {
                    gathered,
                    cache,
                    at: Instant::now(),
                });
            });
        if spawned.is_err() {
            // Nothing was gathered, and the cache went with the thread that could not start: the
            // next view asks again and reads the project's files again (design §1.5).
            self.gathering = false;
        }
    }

    /// The target whose recent events a gather reads: the selected runtime's, by the control
    /// endpoint the last view showed for it (design §1.5).
    fn events_target(&mut self) -> Option<TargetKey> {
        let selected = self.selected.as_deref()?;
        let gathered = self.gathered.as_ref()?;
        let endpoint = self
            .sessions
            .session(&self.id, Instant::now())
            .runtime(selected)?
            .control_endpoint
            .clone()?;
        gathered
            .targets
            .iter()
            .find(|report| report.target.endpoint == endpoint)
            .map(|report| report.target.key())
    }
}

/// A panel that has not been used yet.
fn new_handle(session: &str, now: Instant) -> Handle {
    Handle {
        panel: Arc::new(Mutex::new(Panel::new(session))),
        last_used: now,
    }
}

/// How soon the panel asks again after a delivered result that asks for a refresh (design §1.4.1):
/// at once for `now`, within the pending cadence for `soon`, and not because of it for `none`.
fn refresh_delay_ms(refresh: Refresh) -> Option<u64> {
    match refresh {
        Refresh::Now => Some(0),
        Refresh::Soon => Some(PENDING_REFRESH_MS),
        Refresh::None => None,
    }
}

/// What an action shows while it runs, when its request has left without the answer (design
/// §1.4.1).
fn running_text(action: &str) -> &'static str {
    match action {
        "runtime.managed_start" => "Starting…",
        "runtime.managed_stop" => "Stopping…",
        _ => "Working…",
    }
}

/// The answer of `trust-lsp.devicesView` (design §1.4.1): the panel, the results of the slow
/// actions since the last view, and when the extension asks again.
#[derive(Debug, Clone, Serialize)]
pub struct ViewReply {
    /// [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The panel's session.
    pub session: String,
    /// The request's number.
    pub seq: u64,
    /// When the extension asks again (design §1.5).
    pub next_refresh_ms: u64,
    /// The soonest the extension asks again, such as after an I/O change (design §1.5).
    pub min_refresh_ms: u64,
    /// The results of the slow actions, each delivered once (specification 25 §5.13).
    pub operations_done: Vec<OperationDone>,
    /// The whole panel; absent only when the request could not be answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<DevicesView>,
    /// Why the panel could not be shown (design §1.4.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ReplyError>,
}

impl ViewReply {
    /// A reply that names no panel: the request itself was refused. The next-refresh values are
    /// the shortest of a live panel's, so a refused reply never makes the extension poll at once.
    #[must_use]
    fn refused(error: ReplyError) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            session: String::new(),
            seq: 0,
            next_refresh_ms: WAITING_REFRESH_MS,
            min_refresh_ms: MIN_REFRESH_MS,
            operations_done: Vec::new(),
            view: None,
            error: Some(error),
        }
    }
}

/// The answer of `trust-lsp.devicesClose` (design §1.4.1).
#[derive(Debug, Clone, Serialize)]
pub struct CloseReply {
    /// The panel's session.
    pub session: String,
    /// Whether that session was open.
    pub closed: bool,
}

/// Why a devices command could not answer (design §1.4.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplyError {
    /// The stable code.
    pub code: String,
    /// The text for the user.
    pub message: String,
}

impl ReplyError {
    /// An error with the stable code `code`.
    #[must_use]
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

/// A gather of nothing: no project and no target has answered, so the first view of a session
/// draws the panel's own frame while the background gather runs (design §1.5). Its catalog is the
/// one the tool knows before a runtime has served one: no protocol at all.
fn nothing_gathered() -> Gathered {
    Gathered {
        project: ProjectInputs {
            projects: Vec::new(),
            chosen: None,
            topology: None,
            managed: Ok(Vec::new()),
            catalog: Ok(json!({ "protocols": [] })),
        },
        targets: Vec::new(),
    }
}

/// The time of a view, in milliseconds since the Unix epoch.
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, millis)
}

/// `duration` in whole milliseconds, saturating.
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Locks `mutex`, recovering a poisoned lock: the threads this module starts write plain values,
/// and a panic of one leaves the others' state whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
impl DevicesEngine {
    /// Whether the gather of `session` is still out, so a test waits for the next poll.
    pub(crate) fn gathering(&self, session: &str) -> bool {
        let panels = lock(&self.panels);
        panels
            .get(session)
            .is_some_and(|handle| lock(&handle.panel).gathering)
    }
}
